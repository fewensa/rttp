use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use rttp::server::{HttpResponse, Request};

fn send_raw_request(raw: &[u8]) -> (String, bool) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request).expect("send parsed request");
        HttpResponse::ok("unexpected")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream.write_all(raw).expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  handle.join().expect("server thread");
  (response, rx.try_recv().is_ok())
}

#[test]
fn server_accepts_get_request_and_writes_response() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request).expect("send parsed request");
        HttpResponse::ok("hello")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"GET /hello?debug=true HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!("GET", request.method());
  assert_eq!("/hello?debug=true", request.target());
  assert_eq!("HTTP/1.1", request.version());
  assert_eq!(Some("localhost"), request.header("host"));

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_request_body_stops_at_declared_content_length() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request).expect("send parsed request");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      b"POST /submit HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhelloGET /next HTTP/1.1\r\nHost: localhost\r\n\r\n",
    )
    .expect("write pipelined request bytes");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!("POST", request.method());
  assert_eq!("/submit", request.target());
  assert_eq!(b"hello", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_malformed_request_line() {
  let (response, handler_called) = send_raw_request(b"GET /too many parts HTTP/1.1\r\n\r\n");

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_invalid_header_syntax() {
  let (response, handler_called) = send_raw_request(b"GET / HTTP/1.1\r\nHost localhost\r\n\r\n");

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_unsupported_transfer_encoding() {
  let (response, handler_called) =
    send_raw_request(b"POST /upload HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n");

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_short_request_body() {
  let (response, handler_called) =
    send_raw_request(b"POST /upload HTTP/1.1\r\nContent-Length: 5\r\n\r\nhel");

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn response_write_to_omits_content_length_and_body_for_204() {
  let response = HttpResponse::new(204, "No Content").body("ignored");
  let mut serialized = Vec::new();

  response
    .write_to(&mut serialized)
    .expect("serialize response");

  assert_eq!(
    b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n",
    serialized.as_slice()
  );
}

#[test]
fn response_write_to_omits_content_length_and_body_for_1xx() {
  let response = HttpResponse::new(101, "Switching Protocols").body("ignored");
  let mut serialized = Vec::new();

  response
    .write_to(&mut serialized)
    .expect("serialize response");

  assert_eq!(
    b"HTTP/1.1 101 Switching Protocols\r\nConnection: close\r\n\r\n",
    serialized.as_slice()
  );
}
