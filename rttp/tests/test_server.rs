use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
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

fn assert_bad_request_without_handler(raw: &[u8]) {
  let (response, handler_called) = send_raw_request(raw);

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

fn reserve_local_addr() -> (TcpListener, SocketAddr) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local addr");
  let addr = listener.local_addr().expect("reserved addr");
  (listener, addr)
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
fn server_bind_falls_back_to_later_candidate_when_first_addr_is_occupied() {
  let (_occupied_listener, occupied_addr) = reserve_local_addr();
  let (available_listener, available_addr) = reserve_local_addr();
  drop(available_listener);

  let candidates = [occupied_addr, available_addr];
  let server = rttp::Http::server(candidates.as_slice()).expect("bind later candidate");

  assert_eq!(available_addr, server.local_addr().expect("server addr"));

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("/fallback", request.target());
        HttpResponse::ok("fallback")
      })
      .expect("serve one request");
  });

  let response = send_request(
    available_addr,
    b"GET /fallback HTTP/1.1\r\nHost: localhost\r\n\r\n",
  );

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nfallback",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_bind_returns_io_error_when_all_candidates_fail() {
  let (_first_listener, first_addr) = reserve_local_addr();
  let (_second_listener, second_addr) = reserve_local_addr();
  let candidates = [first_addr, second_addr];

  let err = match rttp::Http::server(candidates.as_slice()) {
    Ok(_) => panic!("all candidates should fail"),
    Err(err) => err,
  };

  assert_eq!(std::io::ErrorKind::AddrInUse, err.kind());
}

#[test]
fn server_accepts_get_request_with_default_timeout_configuration() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_request| HttpResponse::ok("default"))
      .expect("serve one request");
  });

  let response = send_request(addr, b"GET /default HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ndefault",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn configured_read_timeout_bounds_idle_accepted_connection() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_millis(100)));
  let addr = server.local_addr().expect("server addr");
  let (result_tx, result_rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.accept_one(|_request| HttpResponse::ok("unexpected"));
    result_tx.send(result).expect("send server result");
  });

  let _stream = TcpStream::connect(addr).expect("connect server");
  let result = result_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server returned after read timeout");
  let err = result.expect_err("idle accepted connection should time out");
  assert_eq!(std::io::ErrorKind::TimedOut, err.kind());

  handle.join().expect("server thread");
}

#[test]
fn configured_read_timeout_bounds_idle_keep_alive_connection_after_response() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_millis(100)));
  let addr = server.local_addr().expect("server addr");
  let (handler_tx, handler_rx) = mpsc::channel();
  let (result_tx, result_rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.serve_requests(2, |request| {
      let target = request.target().to_string();
      handler_tx.send(target.clone()).expect("send parsed target");
      HttpResponse::ok(format!("served {target}"))
    });
    result_tx.send(result).expect("send server result");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"GET /first HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n")
    .expect("write first request");

  let expected_response = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nserved /first";
  let mut response = vec![0; expected_response.len()];
  stream
    .read_exact(&mut response)
    .expect("read first response");
  assert_eq!(expected_response, response.as_slice());

  let result = result_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server returned after keep-alive read timeout");
  let err = result.expect_err("idle keep-alive connection should time out");
  assert_eq!(std::io::ErrorKind::TimedOut, err.kind());
  assert_eq!("/first", handler_rx.recv().expect("receive first target"));
  assert!(handler_rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn configured_write_timeout_preserves_normal_response_behavior() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_write_timeout(Some(Duration::from_secs(1)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_request| HttpResponse::ok("write bounded"))
      .expect("serve one request");
  });

  let response = send_request(addr, b"GET /write HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 13\r\nConnection: close\r\n\r\nwrite bounded",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_writes_chunked_response_framing() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_request| HttpResponse::ok("hello").header("Transfer-Encoding", "chunked"))
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Connection: close\r\n",
      "\r\n",
      "5\r\n",
      "hello\r\n",
      "0\r\n",
      "\r\n"
    ),
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_writes_chunked_response_trailers_on_live_connection() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_request| {
        HttpResponse::ok("hello")
          .header("Transfer-Encoding", "chunked")
          .trailer("X-Trace", "abc")
          .trailer("X-Signature", "signed")
      })
      .expect("serve one request");
  });

  let response = send_request(addr, b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Connection: close\r\n",
      "\r\n",
      "5\r\n",
      "hello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "X-Signature: signed\r\n",
      "\r\n"
    ),
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
fn server_sends_continue_before_reading_expected_content_length_body() {
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
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /submit HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Expect: 100-continue\r\n",
        "Content-Length: 5\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write request head");

  let mut interim = [0u8; 25];
  stream
    .read_exact(&mut interim)
    .expect("read interim response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);

  stream.write_all(b"hello").expect("write request body");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!(b"hello", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_continue_request_body_aligned_before_follow_up_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let observed = (request.target().to_string(), request.body().to_vec());
        tx.send(observed).expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "Expect: 100-continue\r\n",
        "Content-Length: 5\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write first request head");

  let expected_interim = b"HTTP/1.1 100 Continue\r\n\r\n";
  let mut interim = vec![0u8; expected_interim.len()];
  stream
    .read_exact(&mut interim)
    .expect("read interim response");
  assert_eq!(expected_interim, interim.as_slice());

  stream
    .write_all(
      concat!(
        "hello",
        "POST /second HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "Content-Length: 6\r\n",
        "\r\n",
        "second"
      )
      .as_bytes(),
    )
    .expect("write first body and follow-up request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let expected_first = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nserved /first";
  let mut first = vec![0u8; expected_first.len()];
  stream.read_exact(&mut first).expect("read first response");
  assert_eq!(expected_first, first.as_slice());

  let expected_second =
    b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second";
  let mut second = vec![0u8; expected_second.len()];
  stream
    .read_exact(&mut second)
    .expect("read second response");
  assert_eq!(expected_second, second.as_slice());

  let mut trailing = [0u8; 1];
  let bytes_read = stream.read(&mut trailing).expect("read trailing bytes");
  assert_eq!(0, bytes_read);

  assert_eq!(
    ("/first".to_string(), b"hello".to_vec()),
    rx.recv().expect("receive first request")
  );
  assert_eq!(
    ("/second".to_string(), b"second".to_vec()),
    rx.recv().expect("receive second request")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_sends_continue_before_reading_expected_chunked_body() {
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
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Expect: 100-continue\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write request head");

  let mut interim = [0u8; 25];
  stream
    .read_exact(&mut interim)
    .expect("read interim response");
  assert_eq!(b"HTTP/1.1 100 Continue\r\n\r\n", &interim);

  stream
    .write_all(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n")
    .expect("write chunked body");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!(b"Wikipedia", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_does_not_send_continue_for_expect_without_request_body() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "GET /empty HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Expect: 100-continue\r\n",
      "\r\n"
    )
    .as_bytes(),
  );

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_does_not_send_continue_for_expect_with_zero_content_length() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /empty HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Expect: 100-continue\r\n",
      "Content-Length: 0\r\n",
      "\r\n"
    )
    .as_bytes(),
  );

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_returns_bad_request_for_unsupported_expectation_without_calling_handler() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /submit HTTP/1.1\r\n",
      "Expect: magic\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello"
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
fn server_rejects_unsupported_expectation_before_body_is_sent() {
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
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /submit HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Expect: magic\r\n",
        "Content-Length: 5\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write request head");

  let expected =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request";
  let mut response = vec![0; expected.len()];
  stream
    .read_exact(&mut response)
    .expect("read final response");

  assert_eq!(expected, response.as_slice());
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_expect_with_conflicting_body_framing_without_continue() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Expect: 100-continue\r\n",
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
fn server_returns_bad_request_for_expect_with_unsupported_transfer_encoding_without_continue() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Expect: 100-continue\r\n",
      "Transfer-Encoding: gzip, chunked\r\n",
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
fn server_accept_one_sends_head_headers_without_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.method().to_string())
          .expect("send parsed method");
        HttpResponse::ok("head body")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"HEAD /resource HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!("HEAD", rx.recv().expect("receive parsed method"));
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_malformed_request_line() {
  assert_bad_request_without_handler(b"GET /too many parts HTTP/1.1\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_invalid_http_version() {
  assert_bad_request_without_handler(b"GET / HTTP/2.0\r\nHost: localhost\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_invalid_method_token() {
  assert_bad_request_without_handler(b"GE(T / HTTP/1.1\r\nHost: localhost\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_http_11_request_without_host_before_handler() {
  assert_bad_request_without_handler(b"GET / HTTP/1.1\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_http_11_request_with_multiple_host_headers_before_handler() {
  assert_bad_request_without_handler(
    b"GET / HTTP/1.1\r\nHost: localhost\r\nhOSt: other.localhost\r\n\r\n",
  );
}

#[test]
fn server_accepts_http_10_request_without_host() {
  let (response, handler_called) = send_raw_request(b"GET /legacy HTTP/1.0\r\n\r\n");

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_absolute_form_request_target() {
  let (response, handler_called) =
    send_raw_request(b"GET http://example.test/path?query=1 HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_options_asterisk_request_target() {
  let (response, handler_called) =
    send_raw_request(b"OPTIONS * HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_connect_authority_request_target() {
  let (response, handler_called) =
    send_raw_request(b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test:443\r\n\r\n");

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_rejects_request_target_forms_for_wrong_methods() {
  for raw in [
    b"GET * HTTP/1.1\r\nHost: localhost\r\n\r\n".as_slice(),
    b"GET example.test:443 HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"CONNECT /tunnel HTTP/1.1\r\nHost: example.test\r\n\r\n",
  ] {
    assert_bad_request_without_handler(raw);
  }
}

#[test]
fn server_returns_bad_request_for_invalid_absolute_form_target() {
  assert_bad_request_without_handler(b"GET http:///path HTTP/1.1\r\nHost: localhost\r\n\r\n");
  assert_bad_request_without_handler(b"GET http://:80/path HTTP/1.1\r\nHost: localhost\r\n\r\n");
  assert_bad_request_without_handler(
    b"GET http://example.test:port/path HTTP/1.1\r\nHost: localhost\r\n\r\n",
  );
  assert_bad_request_without_handler(
    b"GET http://example.test/path#frag HTTP/1.1\r\nHost: localhost\r\n\r\n",
  );
}

#[test]
fn server_returns_bad_request_for_header_name_with_whitespace() {
  assert_bad_request_without_handler(b"GET / HTTP/1.1\r\nBad Name: value\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_obsolete_folded_header() {
  assert_bad_request_without_handler(
    b"GET / HTTP/1.1\r\nHost: localhost\r\n folded: value\r\n\r\n",
  );
}

#[test]
fn server_accepts_mixed_case_header_names() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("x-custom-header").map(str::to_string))
          .expect("send parsed header");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Custom-Header: Mixed\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    Some("Mixed".to_string()),
    rx.recv().expect("receive header")
  );
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
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
  assert_bad_request_without_handler(b"GET / HTTP/1.1\r\nHost localhost\r\n\r\n");
}

#[test]
fn server_returns_bad_request_for_oversized_request_head() {
  let header_value = "x".repeat(70 * 1024);
  let raw = format!("GET / HTTP/1.1\r\nX-Large: {header_value}\r\n\r\n");
  let (response, handler_called) = send_raw_request(raw.as_bytes());

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_oversized_content_length_body() {
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
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(b"POST /upload HTTP/1.1\r\nContent-Length: 1048577\r\n\r\n")
    .expect("write request head");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  handle.join().expect("server thread");
  assert!(rx.try_recv().is_err());
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
fn server_accepts_small_content_length_request_body() {
  let (response, handler_called) =
    send_raw_request(b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\nbody");

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_duplicate_matching_content_length_request_body() {
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
        "Content-Length: 5\r\n",
        "Content-Length: 5\r\n",
        "\r\n",
        "hello"
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
  assert_eq!(b"hello", request.body());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_accepts_matching_comma_separated_content_length_values() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Content-Length: 5, 5\r\n",
      "\r\n",
      "hello"
    )
    .as_bytes(),
  );

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_returns_bad_request_for_conflicting_duplicate_content_length() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 6\r\n",
      "\r\n",
      "hello!"
    )
    .as_bytes(),
  );
}

#[test]
fn server_returns_bad_request_for_conflicting_comma_separated_content_length_values() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Content-Length: 5, 6\r\n",
      "\r\n",
      "hello!"
    )
    .as_bytes(),
  );
}

#[test]
fn server_returns_bad_request_for_malformed_content_length() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Content-Length: five\r\n",
      "\r\n",
      "hello"
    )
    .as_bytes(),
  );
}

#[test]
fn server_accepts_small_chunked_request_body() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "4\r\nbody\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );

  assert!(handler_called);
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_returns_bad_request_for_oversized_chunked_request_body() {
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
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "100001\r\n"
      )
      .as_bytes(),
    )
    .expect("write oversized chunk header");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  handle.join().expect("server thread");
  assert!(rx.try_recv().is_err());
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_preserves_chunked_request_trailers() {
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
        "X-Signature: signed\r\n",
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
  assert_eq!(Some("abc"), request.trailer("x-trace"));
  assert_eq!(Some("signed"), request.trailer("X-SIGNATURE"));
  assert_eq!(
    &[
      ("X-Trace".to_string(), "abc".to_string()),
      ("X-Signature".to_string(), "signed".to_string())
    ],
    request.trailers()
  );
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_accepts_quoted_chunk_extensions() {
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
        "7;foo=\"bar;baz\";answer=42\r\nchunked\r\n",
        "6;empty;quoted=\"\\\\\\\"\"\r\n body!\r\n",
        "0;done=\"yes\"\r\n",
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
  assert_eq!(Some("abc"), request.trailer("x-trace"));
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_forbidden_chunked_request_trailer() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\n",
      "OK\r\n",
      "0\r\n",
      "Content-Length: 2\r\n",
      "\r\n"
    )
    .as_bytes(),
  );
}

#[test]
fn server_exposes_empty_trailers_for_chunked_request_without_trailers() {
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
        "4\r\nbody\r\n",
        "0\r\n",
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
  assert_eq!(b"body", request.body());
  assert!(request.trailers().is_empty());
  assert_eq!(None, request.trailer("x-trace"));
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_malformed_chunk_extension() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "7;bad name=value\r\nchunked\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );

  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "7;foo=\"unterminated\r\nchunked\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
  );
}

#[test]
fn server_returns_bad_request_for_malformed_chunked_request_trailer() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace abc\r\n",
      "\r\n"
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
fn server_returns_bad_request_for_oversized_chunked_request_trailer() {
  let trailer_value = "x".repeat(1024 * 1024);
  let raw = format!(
    "POST /upload HTTP/1.1\r\n\
     Host: localhost\r\n\
     Transfer-Encoding: chunked\r\n\
     \r\n\
     0\r\n\
     X-Trace: {trailer_value}\r\n\
     \r\n"
  );
  let (response, handler_called) = send_raw_request(raw.as_bytes());

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );
}

#[test]
fn server_returns_bad_request_for_malformed_chunk_size() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
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
      "Host: localhost\r\n",
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
fn configured_read_timeout_is_preserved_for_stalled_chunked_request_body() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_millis(100)));
  let addr = server.local_addr().expect("server addr");
  let (result_tx, result_rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.accept_one(|_request| HttpResponse::ok("unexpected"));
    result_tx.send(result).expect("send server result");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "5\r\nhel"
      )
      .as_bytes(),
    )
    .expect("write partial chunked request body");

  let result = result_rx
    .recv_timeout(Duration::from_secs(1))
    .expect("server returned after chunked request body timeout");
  let err = result.expect_err("stalled chunked request body should time out");
  assert_eq!(std::io::ErrorKind::TimedOut, err.kind());

  handle.join().expect("server thread");
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
      "Host: localhost\r\n",
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
fn server_closes_keep_alive_after_malformed_chunked_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (handler_tx, handler_rx) = mpsc::channel();
  let (done_tx, done_rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.serve_requests(2, |request| {
      handler_tx
        .send(request.target().to_string())
        .expect("send parsed target");
      HttpResponse::ok(format!("served {}", request.target()))
    });
    done_tx.send(result).expect("send server result");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /broken HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "5\r\nhello",
        "XX",
        "GET /leaked HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write malformed pipelined request");
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

  let second_stream = TcpStream::connect(addr).expect("connect second client");
  second_stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown second write");

  let result = done_rx
    .recv_timeout(Duration::from_millis(250))
    .expect("serve_requests returned after second connection");
  assert!(result.is_ok(), "serve_requests failed: {result:?}");

  handle.join().expect("server thread");
}

#[test]
fn server_returns_bad_request_for_unsupported_transfer_encoding() {
  let (response, handler_called) =
    send_raw_request(b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n");

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
      "Host: localhost\r\n",
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
    send_raw_request(b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhel");

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
fn server_rejects_invalid_second_request_target_on_kept_alive_connection() {
  for (invalid_request, expected_first_target) in [
    (
      concat!(
        "GET /first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n",
        "CONNECT /tunnel HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      ),
      "/first",
    ),
    (
      concat!(
        "GET http://example.test/first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n",
        "GET * HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
        "\r\n"
      ),
      "http://example.test/first",
    ),
  ] {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (handler_tx, handler_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      let result = server.serve_requests(2, |request| {
        let target = request.target().to_string();
        handler_tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}"))
      });
      done_tx.send(result).expect("send server result");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream
      .write_all(invalid_request.as_bytes())
      .expect("write pipelined requests");
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert_eq!(
      format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\nserved {}HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
        7 + expected_first_target.len(),
        expected_first_target
      ),
      response
    );
    assert_eq!(
      expected_first_target,
      handler_rx.recv().expect("receive first target")
    );
    assert!(handler_rx.try_recv().is_err());
    let result = done_rx
      .recv_timeout(Duration::from_millis(250))
      .expect("serve_requests returned after rejected second request");
    assert!(result.is_ok(), "serve_requests failed: {result:?}");

    handle.join().expect("server thread");
  }
}

#[test]
fn server_rejects_invalid_second_host_without_corrupting_kept_alive_connection() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (handler_tx, handler_rx) = mpsc::channel();
  let (done_tx, done_rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    let result = server.serve_requests(2, |request| {
      let target = request.target().to_string();
      handler_tx.send(target.clone()).expect("send parsed target");
      HttpResponse::ok(format!("served {target}"))
    });
    done_tx.send(result).expect("send server result");
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
        "Host: localhost/path\r\n",
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
      "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    ),
    response
  );
  assert_eq!("/first", handler_rx.recv().expect("receive first target"));
  assert!(handler_rx.try_recv().is_err());
  let result = done_rx
    .recv_timeout(Duration::from_millis(250))
    .expect("serve_requests returned after rejected second request");
  assert!(result.is_ok(), "serve_requests failed: {result:?}");

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_head_connection_framed_for_following_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let method = request.method().to_string();
        let target = request.target().to_string();
        tx.send((method.clone(), target.clone()))
          .expect("send parsed request");
        HttpResponse::ok(format!("{method} {target} body"))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "HEAD /first HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
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
      "HTTP/1.1 200 OK\r\nContent-Length: 16\r\n\r\n",
      "HTTP/1.1 200 OK\r\nContent-Length: 16\r\nConnection: close\r\n\r\nGET /second body",
    ),
    response
  );
  assert_eq!(
    ("HEAD".to_string(), "/first".to_string()),
    rx.recv().expect("receive first request")
  );
  assert_eq!(
    ("GET".to_string(), "/second".to_string()),
    rx.recv().expect("receive second request")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_204_connection_framed_for_following_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        if target == "/empty" {
          HttpResponse::new(204, "No Content").body("ignored body")
        } else {
          HttpResponse::ok(format!("served {target}"))
        }
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /empty HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
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
      "HTTP/1.1 204 No Content\r\n\r\n",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/empty", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_304_connection_framed_for_following_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        if target == "/cached" {
          HttpResponse::new(304, "Not Modified").body("ignored body")
        } else {
          HttpResponse::ok(format!("served {target}"))
        }
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /cached HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
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
      "HTTP/1.1 304 Not Modified\r\n\r\n",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/cached", rx.recv().expect("receive first target"));
  assert_eq!("/second", rx.recv().expect("receive second target"));

  handle.join().expect("server thread");
}

#[test]
fn server_keeps_chunked_response_framed_for_following_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        if target == "/chunked" {
          HttpResponse::ok("chunk body").header("Transfer-Encoding", "chunked")
        } else {
          HttpResponse::ok(format!("served {target}"))
        }
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /chunked HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Connection: keep-alive\r\n",
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
      "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
      "a\r\nchunk body\r\n0\r\n\r\n",
      "HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\nserved /second",
    ),
    response
  );
  assert_eq!("/chunked", rx.recv().expect("receive first target"));
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
fn server_completes_chunked_response_before_connection_close_request_stops_pipeline() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        let target = request.target().to_string();
        tx.send(target.clone()).expect("send parsed target");
        HttpResponse::ok(format!("served {target}")).header("Transfer-Encoding", "chunked")
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
    concat!(
      "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
      "d\r\nserved /final\r\n0\r\n\r\n",
    ),
    response
  );
  assert_eq!("/final", rx.recv().expect("receive final target"));
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn server_overrides_keep_alive_response_header_when_it_will_close() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |_request| {
        HttpResponse::ok("terminal").header("Connection", "keep-alive")
      })
      .expect("serve request");
  });

  let response = send_request(addr, b"GET /terminal HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nterminal",
    response
  );

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
fn response_write_to_omits_content_length_and_body_for_304() {
  let response = HttpResponse::new(304, "Not Modified").body("ignored");
  let mut serialized = Vec::new();

  response
    .write_to(&mut serialized)
    .expect("serialize response");

  assert_eq!(
    b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n",
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

#[test]
fn response_write_to_omits_transfer_encoding_and_chunk_framing_for_no_body_statuses() {
  for (status_code, reason) in [
    (101, "Switching Protocols"),
    (204, "No Content"),
    (304, "Not Modified"),
  ] {
    let response = HttpResponse::new(status_code, reason)
      .header("Transfer-Encoding", "chunked")
      .body("ignored");
    let mut serialized = Vec::new();

    response
      .write_to(&mut serialized)
      .expect("serialize response");

    assert_eq!(
      format!("HTTP/1.1 {status_code} {reason}\r\nConnection: close\r\n\r\n").as_bytes(),
      serialized.as_slice()
    );
  }
}

#[test]
fn server_omits_transfer_encoding_and_chunk_framing_for_no_body_statuses() {
  for (status_code, reason) in [
    (101, "Switching Protocols"),
    (204, "No Content"),
    (304, "Not Modified"),
  ] {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let response_reason = reason.to_string();

    let handle = thread::spawn(move || {
      server
        .accept_one(|_request| {
          HttpResponse::new(status_code, response_reason)
            .header("Transfer-Encoding", "chunked")
            .body("ignored")
        })
        .expect("serve one request");
    });

    let response = send_request(addr, b"GET /empty HTTP/1.1\r\nHost: localhost\r\n\r\n");

    assert_eq!(
      format!("HTTP/1.1 {status_code} {reason}\r\nConnection: close\r\n\r\n"),
      response
    );

    handle.join().expect("server thread");
  }
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
