use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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
fn server_rejects_malformed_request_line_before_reading_declared_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.accept_one(|request| {
      tx.send(request).expect("send parsed request");
      HttpResponse::ok("unexpected")
    });
    assert!(result.is_ok(), "serve one request: {result:?}");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(b"GET /too many parts HTTP/1.1\r\nContent-Length: 1000000\r\n\r\n")
    .expect("write request head");

  let mut response = String::new();
  let read_result = stream.read_to_string(&mut response);
  if read_result.is_err() {
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown write");
    let _ = stream.read_to_string(&mut response);
  }

  handle.join().expect("server thread");
  assert!(read_result.is_ok(), "server waited for the declared body");
  assert!(rx.try_recv().is_err());
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
fn server_decodes_chunked_request_body() {
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
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "4\r\nWiki\r\n",
        "5\r\npedia\r\n",
        "0\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!("POST", request.method());
  assert_eq!("/upload", request.target());
  assert_eq!(b"Wikipedia", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_ignores_chunk_extensions_and_trailers() {
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
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "7;foo=bar\r\nchunked\r\n",
        "6\r\n body!\r\n",
        "0\r\n",
        "X-Trace: abc\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!(b"chunked body!", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_malformed_chunk_size() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "not-hex\r\nhello\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_truncated_chunked_request_body() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhel"
    )
    .as_bytes(),
  );

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_huge_truncated_chunked_request_body() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "4000000000000000\r\nhel"
    )
    .as_bytes(),
  );

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_invalid_chunk_terminator() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhelloXX",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_unsupported_transfer_encoding() {
  let (response, handler_called) =
    send_raw_request(b"POST /upload HTTP/1.1\r\nTransfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n");

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_transfer_encoding_with_content_length() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Content-Length: 0\r\n",
      "\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );

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
fn serve_requests_counts_malformed_connection_toward_limit() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (handler_tx, handler_rx) = mpsc::channel();
  let (done_tx, done_rx) = mpsc::channel();

  thread::spawn(move || {
    let result = server.serve_requests(1, |request| {
      handler_tx.send(request).expect("send parsed request");
      HttpResponse::ok("unexpected")
    });
    done_tx.send(result).expect("send server result");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"GET /too many parts HTTP/1.1\r\n\r\n")
    .expect("write malformed request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert!(handler_rx.try_recv().is_err());
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
  let result = done_rx
    .recv_timeout(Duration::from_millis(250))
    .expect("serve_requests returned after rejected connection");
  assert!(result.is_ok(), "serve_requests failed: {result:?}");
}

#[test]
fn serve_requests_counts_empty_connection_toward_limit() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (handler_tx, handler_rx) = mpsc::channel();
  let (done_tx, done_rx) = mpsc::channel();

  thread::spawn(move || {
    let result = server.serve_requests(1, |request| {
      handler_tx.send(request).expect("send parsed request");
      HttpResponse::ok("unexpected")
    });
    done_tx.send(result).expect("send server result");
  });

  let stream = TcpStream::connect(addr).expect("connect server");
  stream
    .shutdown(std::net::Shutdown::Both)
    .expect("close connection");

  assert!(handler_rx.try_recv().is_err());
  let result = done_rx
    .recv_timeout(Duration::from_millis(250))
    .expect("serve_requests returned after empty connection");
  assert!(result.is_ok(), "serve_requests failed: {result:?}");
}

#[test]
fn server_accepts_multiple_sequential_connections_on_one_listener() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve sequential requests");
  });

  let first = send_request(addr, b"GET /first HTTP/1.1\r\nHost: localhost\r\n\r\n");
  let second = send_request(addr, b"GET /second HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nserved /first",
    first
  );
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    second
  );
  assert_eq!("/first", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_serves_multiple_requests_on_one_kept_alive_connection() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n",
        "GET /second HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nserved /first",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/first", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_http11_connection_alive_by_default() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "\r\n",
        "GET /second HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nserved /first",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/first", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_closes_http10_connection_by_default() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /first HTTP/1.0\r\n",
        "Host: localhost\r\n",
        "\r\n",
        "GET /ignored HTTP/1.0\r\n",
        "Host: localhost\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nserved /first",
    response
  );
  assert_eq!("/first", rx.recv().expect("receive first target"));

  let second = send_request(addr, b"GET /second HTTP/1.0\r\nHost: localhost\r\n\r\n");
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    second
  );
  assert_eq!("/second", rx.recv().expect("receive second target"));
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_http10_connection_alive_when_requested() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /first HTTP/1.0\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n",
        "GET /second HTTP/1.0\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: keep-alive\r\n\r\nserved /first",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/first", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_stops_one_connection_after_connection_close_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /final HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: close\r\n",
        "\r\n",
        "GET /ignored HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined requests");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nserved /final",
    response
  );
  assert_eq!("/final", rx.recv().expect("receive final target"));

  let second = send_request(addr, b"GET /next HTTP/1.1\r\nHost: localhost\r\n\r\n");
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nserved /next",
    second
  );
  assert_eq!("/next", rx.recv().expect("receive next target"));
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
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

fn send_request(addr: std::net::SocketAddr, request: &[u8]) -> String {
  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream.write_all(request).expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");
  response
}
