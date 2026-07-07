use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{HttpRequest, HttpResponse};
use rttp_http11_test_fixtures as fixtures;

#[test]
fn model_parser_accepts_shared_fixed_length_request_fixture() {
  let fixture = fixtures::request::fixed_length_post();

  let request = HttpRequest::parse(fixture.raw).expect("fixed-length request should parse");

  assert_eq!(fixture.method, request.method());
  assert_eq!(fixture.path, request.path());
  assert_eq!(fixture.query, request.query());
  assert_eq!(fixture.version, request.version());
  assert_eq!(Some(fixture.host), request.header("host"));
  assert_eq!(fixture.body, request.body());
}

#[test]
fn model_parser_rejects_shared_host_and_target_validation_fixtures() {
  for fixture in fixtures::request::invalid_host_and_target_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn model_parser_rejects_shared_framing_ambiguity_fixtures() {
  for fixture in fixtures::request::framing_ambiguity_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn model_parser_rejects_shared_obsolete_line_folding_fixtures() {
  for fixture in fixtures::request::obsolete_line_folding_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn live_socket2_server_accepts_shared_chunk_extensions_and_trailers_fixture() {
  let fixture = fixtures::request::chunked_with_extensions_and_trailers();
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let trailers = fixture
          .trailers
          .iter()
          .map(|(name, _)| (name.to_string(), request.trailer(name).map(str::to_string)))
          .collect::<Vec<_>>();
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
          trailers,
        ))
        .expect("send parsed request");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(fixture.raw)
    .expect("write chunked request");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let (method, target, body, trailers) = rx.recv().expect("parsed request");
  assert_eq!(fixture.method, method);
  assert_eq!(fixture.target, target);
  assert_eq!(fixture.body, body.as_slice());
  for ((name, value), (observed_name, observed_value)) in fixture.trailers.iter().zip(trailers) {
    assert_eq!(*name, observed_name);
    assert_eq!(Some((*value).to_string()), observed_value);
  }
  assert!(response.starts_with("HTTP/1.1 200 OK"));

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_preserves_connection_lifetime_boundaries() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send((request.target().to_string(), request.body().to_vec()))
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(fixtures::request::keep_alive_pipeline())
    .expect("write pipelined requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read responses");

  assert!(response.contains("served /matrix/first"));
  assert!(response.contains("served /matrix/second"));
  assert_eq!(
    ("/matrix/first".to_string(), b"alpha".to_vec()),
    rx.recv().expect("first request")
  );
  assert_eq!(
    ("/matrix/second".to_string(), b"bravo!".to_vec()),
    rx.recv().expect("second request")
  );

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_sends_continue_before_reading_shared_body_fixture() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.target().to_string(), request.body().to_vec()))
          .expect("send parsed request");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(fixture.head)
    .expect("write expect-continue head");

  let mut interim = vec![0; fixtures::response::CONTINUE.len()];
  stream
    .read_exact(&mut interim)
    .expect("read interim response");
  assert_eq!(fixtures::response::CONTINUE, interim.as_slice());

  stream
    .write_all(fixture.body)
    .expect("write expect-continue body");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read final response");

  assert!(response.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    (fixture.target.to_string(), fixture.body.to_vec()),
    rx.recv().expect("parsed request")
  );

  handle.join().expect("server thread");
}
