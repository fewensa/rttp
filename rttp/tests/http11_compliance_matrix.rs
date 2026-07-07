use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{HttpRequest, HttpResponse};
use rttp_http11_test_fixtures as fixtures;

#[derive(Debug)]
struct ParsedResponse<'a> {
  head: &'a str,
  body: &'a str,
}

fn parse_content_length_response(input: &str) -> (ParsedResponse<'_>, &str) {
  let (head, after_head) = input.split_once("\r\n\r\n").expect("response head");
  let content_length = head
    .lines()
    .find_map(|line| line.strip_prefix("Content-Length: "))
    .expect("content length")
    .parse::<usize>()
    .expect("content length value");
  let (body, remaining) = after_head.split_at(content_length);

  (ParsedResponse { head, body }, remaining)
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
  head
    .lines()
    .filter_map(|line| line.split_once(':'))
    .find(|(observed_name, _)| observed_name.eq_ignore_ascii_case(name))
    .map(|(_, value)| value.trim())
}

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
fn live_socket2_server_accepts_shared_origin_and_absolute_form_fixtures() {
  for fixture in fixtures::request::valid_origin_and_absolute_form_cases() {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((request.method().to_string(), request.target().to_string()))
            .expect("send parsed request");
          HttpResponse::ok(format!("served {}", request.target()))
        })
        .expect("serve one request");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream.write_all(fixture.raw).expect(fixture.name);
    stream.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert_eq!(
      (fixture.method.to_string(), fixture.target.to_string()),
      rx.recv().expect("parsed request"),
      "{}",
      fixture.name
    );
    assert!(
      response.starts_with("HTTP/1.1 200 OK"),
      "{} returned {response:?}",
      fixture.name
    );

    handle.join().expect("server thread");
  }
}

#[test]
fn live_socket2_server_rejects_shared_invalid_host_and_target_fixtures_before_handler() {
  for fixture in fixtures::request::invalid_host_and_target_cases() {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send(request.target().to_string())
            .expect("send unexpected request");
          HttpResponse::ok("unexpected")
        })
        .expect("serve invalid request");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream.write_all(fixture.raw).expect(fixture.name);
    stream.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert!(
      rx.try_recv().is_err(),
      "{} should not dispatch to the handler",
      fixture.name
    );
    assert!(
      response.starts_with("HTTP/1.1 400 Bad Request"),
      "{} returned {response:?}",
      fixture.name
    );

    handle.join().expect("server thread");
  }
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

  let (first_response, remaining) = parse_content_length_response(&response);
  let (second_response, remaining) = parse_content_length_response(remaining);

  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(None, header_value(first_response.head, "Connection"));
  assert_eq!("served /matrix/first", first_response.body);
  assert!(second_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(second_response.head, "Connection")
  );
  assert_eq!("served /matrix/second", second_response.body);
  assert_eq!("", remaining);
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
fn live_socket2_server_stops_pipelined_connection_after_request_close() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/close-first HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Connection: close\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/ignored HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined close request");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let (first_response, remaining) = parse_content_length_response(&response);
  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/close-first", first_response.body);
  assert_eq!("", remaining);
  assert_eq!("/matrix/close-first", rx.recv().expect("first request"));
  assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

  let mut next_stream = TcpStream::connect(addr).expect("connect next request");
  next_stream
    .write_all(
      concat!(
        "GET /matrix/next-connection HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write next request");
  next_stream
    .shutdown(Shutdown::Write)
    .expect("shutdown next write");

  let mut next_response = String::new();
  next_stream
    .read_to_string(&mut next_response)
    .expect("read next response");

  let (next_response, next_remaining) = parse_content_length_response(&next_response);
  assert!(next_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!("served /matrix/next-connection", next_response.body);
  assert_eq!("", next_remaining);
  assert_eq!(
    "/matrix/next-connection",
    rx.recv().expect("next connection request")
  );
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_closes_http10_without_keep_alive_before_next_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/http10-terminal HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/ignored HTTP/1.0\r\n",
        "Connection: keep-alive\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined HTTP/1.0 requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let mut next_stream = TcpStream::connect(addr).expect("connect next request");
  next_stream
    .write_all(
      concat!(
        "GET /matrix/http10-next-connection HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write next HTTP/1.0 request");
  next_stream
    .shutdown(Shutdown::Write)
    .expect("shutdown next write");

  let mut next_response = String::new();
  next_stream
    .read_to_string(&mut next_response)
    .expect("read next response");

  let (first_response, remaining) = parse_content_length_response(&response);
  let (next_response, next_remaining) = parse_content_length_response(&next_response);
  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-terminal", first_response.body);
  assert_eq!("", remaining);
  assert!(next_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!("served /matrix/http10-next-connection", next_response.body);
  assert_eq!("", next_remaining);
  assert_eq!("/matrix/http10-terminal", rx.recv().expect("first request"));
  assert_eq!(
    "/matrix/http10-next-connection",
    rx.recv().expect("next connection request")
  );
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_keeps_http10_alive_when_explicitly_requested() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/http10-first HTTP/1.0\r\n",
        "Connection: keep-alive\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/http10-final HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined HTTP/1.0 requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read responses");

  let (first_response, remaining) = parse_content_length_response(&response);
  let (second_response, remaining) = parse_content_length_response(remaining);

  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("keep-alive"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-first", first_response.body);
  assert!(second_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(second_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-final", second_response.body);
  assert_eq!("", remaining);
  assert_eq!("/matrix/http10-first", rx.recv().expect("first request"));
  assert_eq!("/matrix/http10-final", rx.recv().expect("second request"));

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

#[test]
fn live_socket2_server_rejects_unsupported_expectation_without_reading_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send unexpected request");
        HttpResponse::ok("unexpected")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /matrix/unsupported-expect HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Expect: tea-time\r\n",
        "Content-Length: 12\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write unsupported expectation head");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read expectation failure");

  assert!(response.starts_with("HTTP/1.1 417 Expectation Failed"));
  assert!(
    rx.try_recv().is_err(),
    "unsupported expectation reached the request handler"
  );

  handle.join().expect("server thread");
}
