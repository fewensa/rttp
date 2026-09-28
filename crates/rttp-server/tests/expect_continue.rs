use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_server::server::{HttpResponse, HttpServer, Request};

fn read_response(stream: &mut TcpStream) -> Vec<u8> {
  let mut response = Vec::new();
  stream.read_to_end(&mut response).expect("read response");
  response
}

fn start_one<F>(handler: F) -> (TcpStream, thread::JoinHandle<std::io::Result<()>>)
where
  F: FnOnce(Request) -> HttpResponse + Send + 'static,
{
  let server = HttpServer::bind(("127.0.0.1", 0)).expect("server should bind");
  let client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  let handle = thread::spawn(move || server.accept_one(handler));
  (client, handle)
}

fn request_head(path: &str, framing: &str, expect: &str) -> String {
  format!("POST {path} HTTP/1.1\r\nHost: example.test\r\nExpect: {expect}\r\n{framing}\r\n\r\n")
}

#[test]
fn fixed_body_is_gated_until_continue_and_metadata_is_preserved() {
  let (tx, rx) = mpsc::channel();
  let (mut client, handle) = start_one(move |request| {
    tx.send((
      request.body().to_vec(),
      request.expectations().expect("Expect should parse"),
    ))
    .expect("send request metadata");
    HttpResponse::ok("accepted")
  });
  client
    .set_read_timeout(Some(Duration::from_secs(1)))
    .expect("set client timeout");
  client
    .write_all(request_head("/fixed", "Content-Length: 5", "100-continue").as_bytes())
    .expect("write request head");

  let mut interim = [0; 25];
  client
    .read_exact(&mut interim)
    .expect("read continue response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);

  client.write_all(b"hello").expect("write request body");
  client
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");
  assert_eq!(
    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    read_response(&mut client).as_slice()
  );
  let (body, expectations) = rx.recv().expect("receive request");
  assert_eq!(b"hello", body.as_slice());
  assert!(expectations
    .expect("Expect metadata should be present")
    .expects_continue());
  handle.join().expect("join server").expect("serve request");
}

#[test]
fn chunked_body_is_gated_until_continue() {
  let (mut client, handle) = start_one(|request| {
    assert_eq!(b"Wikipedia", request.body());
    HttpResponse::ok("accepted")
  });
  client
    .set_read_timeout(Some(Duration::from_secs(1)))
    .expect("set client timeout");
  client
    .write_all(request_head("/chunked", "Transfer-Encoding: chunked", "100-continue").as_bytes())
    .expect("write request head");
  let mut interim = [0; 25];
  client
    .read_exact(&mut interim)
    .expect("read continue response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);
  client
    .write_all(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n")
    .expect("write chunked body");
  client
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");
  assert!(String::from_utf8(read_response(&mut client))
    .expect("response should be text")
    .starts_with("HTTP/1.1 200 OK"));
  handle.join().expect("join server").expect("serve request");
}

#[test]
fn early_body_arrival_still_gets_exactly_one_continue() {
  let (mut client, handle) = start_one(|request| {
    assert_eq!(b"hello", request.body());
    HttpResponse::ok("accepted")
  });
  client
    .write_all(
      format!(
        "{}hello",
        request_head("/early", "Content-Length: 5", "100-continue")
      )
      .as_bytes(),
    )
    .expect("write head and early body");
  client
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");
  let response = String::from_utf8(read_response(&mut client)).expect("response should be text");
  assert_eq!(1, response.matches("100 Continue").count());
  assert!(response.contains("HTTP/1.1 200 OK"));
  handle.join().expect("join server").expect("serve request");
}

#[test]
fn unsupported_or_malformed_expectation_is_rejected_without_continue() {
  for (expect, status) in [
    ("magic", "417 Expectation Failed"),
    ("100-continue, magic", "417 Expectation Failed"),
    ("100-continue,", "400 Bad Request"),
  ] {
    let (mut client, handle) = start_one(|_| panic!("handler must not run"));
    client
      .write_all(request_head("/reject", "Content-Length: 5", expect).as_bytes())
      .expect("write rejected request head");
    client
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown write");
    let response = String::from_utf8(read_response(&mut client)).expect("response should be text");
    assert!(response.starts_with(&format!("HTTP/1.1 {status}")));
    assert!(!response.contains("100 Continue"));
    handle.join().expect("join server").expect("serve request");
  }
}

#[test]
fn oversized_body_is_rejected_without_continue() {
  let server = HttpServer::bind(("127.0.0.1", 0))
    .expect("server should bind")
    .with_max_request_body_bytes(4);
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  let handle = thread::spawn(move || server.accept_one(|_| panic!("handler must not run")));
  client
    .write_all(request_head("/large", "Content-Length: 5", "100-continue").as_bytes())
    .expect("write oversized request head");
  client
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");
  let response = String::from_utf8(read_response(&mut client)).expect("response should be text");
  assert!(response.starts_with("HTTP/1.1 413 Payload Too Large"));
  assert!(!response.contains("100 Continue"));
  handle.join().expect("join server").expect("serve request");
}

#[test]
fn body_timeout_after_continue_is_reported_without_handler_dispatch() {
  let server = HttpServer::bind(("127.0.0.1", 0))
    .expect("server should bind")
    .with_read_timeout(Some(Duration::from_millis(100)));
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  let handle = thread::spawn(move || server.accept_one(|_| panic!("handler must not run")));
  client
    .set_read_timeout(Some(Duration::from_secs(1)))
    .expect("set client timeout");
  client
    .write_all(request_head("/timeout", "Content-Length: 5", "100-continue").as_bytes())
    .expect("write request head");
  let mut interim = [0; 25];
  client
    .read_exact(&mut interim)
    .expect("read continue response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);
  let error = handle
    .join()
    .expect("join server")
    .expect_err("body read should time out");
  assert_eq!(std::io::ErrorKind::TimedOut, error.kind());
}

#[test]
fn client_cancellation_after_continue_is_reported_without_handler_dispatch() {
  let server = HttpServer::bind(("127.0.0.1", 0)).expect("server should bind");
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  let handle = thread::spawn(move || server.accept_one(|_| panic!("handler must not run")));
  client
    .write_all(request_head("/cancel", "Content-Length: 5", "100-continue").as_bytes())
    .expect("write request head");
  let mut interim = [0; 25];
  client
    .read_exact(&mut interim)
    .expect("read continue response");
  drop(client);
  let error = handle
    .join()
    .expect("join server")
    .expect_err("cancellation should fail body read");
  assert!(matches!(
    error.kind(),
    std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::BrokenPipe
  ));
}

#[test]
fn keep_alive_reuses_connection_after_continue_body() {
  let server = HttpServer::bind(("127.0.0.1", 0)).expect("server should bind");
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  client
    .set_read_timeout(Some(Duration::from_secs(1)))
    .expect("set client timeout");
  let handle =
    thread::spawn(move || server.serve_requests(2, |request| HttpResponse::ok(request.target())));
  client
    .write_all(
      format!(
        "{}helloPOST /second HTTP/1.1\r\nHost: example.test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        request_head("/first", "Content-Length: 5\r\nConnection: keep-alive", "100-continue")
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  let mut interim = [0; 25];
  client
    .read_exact(&mut interim)
    .expect("read continue response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);
  let mut response = Vec::new();
  client
    .read_to_end(&mut response)
    .expect("read final responses");
  let response = String::from_utf8(response).expect("responses should be text");
  assert_eq!(2, response.matches("HTTP/1.1 200 OK").count());
  assert!(response.contains("/first"));
  assert!(response.contains("/second"));
  handle.join().expect("join server").expect("serve requests");
}
