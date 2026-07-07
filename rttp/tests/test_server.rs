use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{HttpResponse, Request};
use rttp_client::HttpClient;

const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const H2_FRAME_DATA: u8 = 0x0;
const H2_FRAME_HEADERS: u8 = 0x1;
const H2_FRAME_RST_STREAM: u8 = 0x3;
const H2_FRAME_SETTINGS: u8 = 0x4;
const H2_FRAME_PING: u8 = 0x6;
const H2_FRAME_GOAWAY: u8 = 0x7;
const H2_FRAME_WINDOW_UPDATE: u8 = 0x8;
const H2_FRAME_CONTINUATION: u8 = 0x9;
const H2_FLAG_END_STREAM: u8 = 0x1;
const H2_FLAG_ACK: u8 = 0x1;
const H2_FLAG_END_HEADERS: u8 = 0x4;

struct H2Frame {
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: Vec<u8>,
}

fn write_h2_frame(
  stream: &mut TcpStream,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header).expect("write h2 frame head");
  stream.write_all(payload).expect("write h2 frame payload");
}

fn read_h2_frame(stream: &mut TcpStream) -> H2Frame {
  let mut header = [0; 9];
  stream.read_exact(&mut header).expect("read h2 frame head");
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream
    .read_exact(&mut payload)
    .expect("read h2 frame payload");
  H2Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  }
}

fn read_h2_frame_skipping_window_updates(stream: &mut TcpStream) -> H2Frame {
  loop {
    let frame = read_h2_frame(stream);
    if frame.frame_type != H2_FRAME_WINDOW_UPDATE {
      return frame;
    }
  }
}

fn h2_literal_indexed_name(name_index: u8, value: &[u8]) -> Vec<u8> {
  assert!(value.len() < 128);
  let mut encoded = vec![name_index, value.len() as u8];
  encoded.extend_from_slice(value);
  encoded
}

fn h2_literal_new_name(name: &[u8], value: &[u8]) -> Vec<u8> {
  assert!(name.len() < 128);
  assert!(value.len() < 128);
  let mut encoded = vec![0, name.len() as u8];
  encoded.extend_from_slice(name);
  encoded.push(value.len() as u8);
  encoded.extend_from_slice(value);
  encoded
}

fn h2_get_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn h2_post_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x83, 0x86];
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn complete_h2_server_handshake(stream: &mut TcpStream) {
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(stream, H2_FRAME_SETTINGS, 0, 0, &[]);

  let settings = read_h2_frame(stream);
  assert_eq!(H2_FRAME_SETTINGS, settings.frame_type);
  assert_eq!(0, settings.flags);
  assert_eq!(0, settings.stream_id);

  let settings_ack = read_h2_frame(stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert_eq!(0, settings_ack.stream_id);

  write_h2_frame(stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
}

fn assert_invalid_h2_headers_without_handler(header_block: &[u8]) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    header_block,
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 headers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

fn assert_invalid_h2_request_trailers_without_handler(trailer_block: &[u8]) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/invalid-trailers", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    trailer_block,
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 trailers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

fn assert_invalid_h2_frame_without_handler(
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(&mut stream, frame_type, flags, stream_id, payload);
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 frame should reject connection");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

fn send_raw_request_capture(raw: &[u8]) -> (String, Option<Request>) {
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
  (response, rx.try_recv().ok())
}

fn send_raw_request(raw: &[u8]) -> (String, bool) {
  let (response, request) = send_raw_request_capture(raw);

  (response, request.is_some())
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
fn server_accepts_absolute_form_get_request_as_origin_target() {
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
    .write_all(b"GET http://example.com/a/b?x=1 HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let request: Request = rx.recv().expect("receive parsed request");
  assert_eq!("GET", request.method());
  assert_eq!("/a/b?x=1", request.target());
  assert_eq!("HTTP/1.1", request.version());
  assert_eq!(Some("localhost"), request.header("host"));

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_accepts_http2_prior_knowledge_get_and_writes_single_stream_response() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request).expect("send parsed h2 request");
        HttpResponse::ok("hello over h2").header("X-Server-Mode", "prior-knowledge")
      })
      .expect("serve one h2 request");
  });

  let response = HttpClient::new()
    .url(format!("http://{}/h2?prior=true", addr))
    .emit_http2_prior_knowledge()
    .expect("single h2 response");

  let request = rx.recv().expect("receive parsed h2 request");
  assert_eq!("GET", request.method());
  assert_eq!("/h2?prior=true", request.target());
  assert_eq!("HTTP/2", request.version());
  assert_eq!(Some(addr.to_string().as_str()), request.header("host"));

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    Some("prior-knowledge"),
    response
      .header_value("x-server-mode")
      .map(|value| value.as_str())
  );
  assert_eq!("hello over h2", response.body().string().unwrap());

  handle.join().expect("server thread");
}

#[test]
fn serve_requests_accepts_next_connection_after_http2_client_closes_cleanly() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send served target");
        HttpResponse::ok(format!("response for {}", request.target()))
      })
      .expect("serve h2 then h1 requests");
  });

  {
    let mut stream = TcpStream::connect(addr).expect("connect h2 server");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 read timeout");
    complete_h2_server_handshake(&mut stream);

    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &h2_get_headers(b"/h2-once", addr.to_string().as_bytes()),
    );

    let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
    assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
    assert_eq!(1, response_headers.stream_id);

    let response_body = read_h2_frame_skipping_window_updates(&mut stream);
    assert_eq!(H2_FRAME_DATA, response_body.frame_type);
    assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
    assert_eq!(1, response_body.stream_id);
    assert_eq!(b"response for /h2-once", response_body.payload.as_slice());
  }

  let response = send_request(addr, b"GET /after-h2 HTTP/1.1\r\nHost: localhost\r\n\r\n");

  assert_eq!("/h2-once", rx.recv().expect("h2 target"));
  assert_eq!("/after-h2", rx.recv().expect("h1 target"));
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 22\r\nConnection: close\r\n\r\nresponse for /after-h2",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_accepts_http2_prior_knowledge_headers_and_data_before_calling_handler() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("POST", request.method());
        assert_eq!("/upload", request.target());
        assert_eq!("HTTP/2", request.version());
        assert_eq!(Some("text/plain"), request.header("content-type"));
        assert_eq!(b"body over h2", request.body());
        HttpResponse::new(201, "Created").body("stored")
      })
      .expect("serve one h2 data request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, 0, 0, &[]);

  let settings = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings.frame_type);
  assert_eq!(0, settings.flags);
  assert_eq!(0, settings.stream_id);

  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert_eq!(0, settings_ack.stream_id);

  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);

  let mut headers = vec![0x83, 0x86];
  headers.extend(h2_literal_indexed_name(4, b"/upload"));
  headers.extend(h2_literal_indexed_name(1, addr.to_string().as_bytes()));
  headers.extend([0, b"content-type".len() as u8]);
  headers.extend(b"content-type");
  headers.extend([b"text/plain".len() as u8]);
  headers.extend(b"text/plain");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    b"body over h2",
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"stored", response_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn server_decodes_http2_prior_knowledge_request_trailers_after_data() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.body().to_vec(),
          request.header("x-trace").map(str::to_string),
          request.trailer("x-trace").map(str::to_string),
          request.trailer("X-UPLOAD-STATUS").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed h2 trailer request");
        HttpResponse::ok("stored")
      })
      .expect("serve one h2 trailer request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/upload-with-trailers", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body before trailers");

  let mut trailers = h2_literal_new_name(b"x-trace", b"from-trailer");
  trailers.extend(h2_literal_new_name(b"x-upload-status", b"stored"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &trailers,
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"stored", response_body.payload.as_slice());

  assert_eq!(
    (
      b"body before trailers".to_vec(),
      None,
      Some("from-trailer".to_string()),
      Some("stored".to_string()),
      vec![
        ("x-trace".to_string(), "from-trailer".to_string()),
        ("x-upload-status".to_string(), "stored".to_string()),
      ]
    ),
    rx.recv().expect("receive parsed h2 trailer request")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_decodes_http2_prior_knowledge_request_trailers_with_continuation() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.trailers().to_vec())
          .expect("send parsed h2 continuation trailers");
        HttpResponse::ok("stored")
      })
      .expect("serve one h2 continuation trailer request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(
      b"/upload-with-continued-trailers",
      addr.to_string().as_bytes(),
    ),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_STREAM,
    1,
    &h2_literal_new_name(b"x-first", b"one"),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_CONTINUATION,
    H2_FLAG_END_HEADERS,
    1,
    &h2_literal_new_name(b"x-second", b"two"),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"stored", response_body.payload.as_slice());

  assert_eq!(
    vec![
      ("x-first".to_string(), "one".to_string()),
      ("x-second".to_string(), "two".to_string()),
    ],
    rx.recv().expect("receive parsed h2 continuation trailers")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_request_trailer_pseudo_header() {
  assert_invalid_h2_request_trailers_without_handler(&h2_literal_indexed_name(2, b"GET"));
}

#[test]
fn server_rejects_http2_prior_knowledge_forbidden_request_trailer_name() {
  assert_invalid_h2_request_trailers_without_handler(&h2_literal_new_name(b"content-length", b"4"));
}

#[test]
fn server_rejects_http2_prior_knowledge_request_trailers_without_end_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/non-terminal-trailers", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_literal_new_name(b"x-trace", b"not-terminal"),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"after");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("non-terminal h2 trailers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_request_trailer_value_control_bytes() {
  assert_invalid_h2_request_trailers_without_handler(&h2_literal_new_name(
    b"x-trace",
    b"safe\r\nx-evil: true",
  ));
}

#[test]
fn server_acknowledges_http2_prior_knowledge_ping_around_request_frames() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send h2 target");
        HttpResponse::ok("pong path")
      })
      .expect("serve h2 ping request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(&mut stream, H2_FRAME_PING, 0, 0, b"12345678");
  let ping_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_PING, ping_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, ping_ack.flags);
  assert_eq!(0, ping_ack.stream_id);
  assert_eq!(b"12345678", ping_ack.payload.as_slice());

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/ping-before-response", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_PING, H2_FLAG_ACK, 0, b"ignored!");
  write_h2_frame(&mut stream, H2_FRAME_PING, 0, 0, b"abcdefgh");

  let second_ping_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_PING, second_ping_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, second_ping_ack.flags);
  assert_eq!(0, second_ping_ack.stream_id);
  assert_eq!(b"abcdefgh", second_ping_ack.payload.as_slice());

  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"");

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"pong path", response_body.payload.as_slice());

  assert_eq!(
    "/ping-before-response",
    rx.recv().expect("receive h2 target")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_ping_with_non_zero_stream_id() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_PING, 0, 1, b"12345678");
}

#[test]
fn server_rejects_http2_prior_knowledge_ping_with_invalid_payload_length() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_PING, 0, 0, b"too short");
}

#[test]
fn server_rejects_http2_prior_knowledge_ping_ack_with_invalid_payload_length() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_PING, H2_FLAG_ACK, 0, b"short");
}

#[test]
fn server_accepts_http2_prior_knowledge_continuation_headers_before_data() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.header("x-split").map(str::to_string),
          request.body().to_vec(),
        ))
        .expect("send parsed h2 continuation request");
        HttpResponse::new(201, "Created").body("stored")
      })
      .expect("serve one h2 continuation request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);

  let first_headers = vec![0x83, 0x86];
  let mut continued_headers = h2_literal_indexed_name(4, b"/continued-upload");
  continued_headers.extend(h2_literal_indexed_name(1, addr.to_string().as_bytes()));
  continued_headers.extend(h2_literal_new_name(b"x-split", b"continued"));
  write_h2_frame(&mut stream, H2_FRAME_HEADERS, 0, 1, &first_headers);
  write_h2_frame(
    &mut stream,
    H2_FRAME_CONTINUATION,
    H2_FLAG_END_HEADERS,
    1,
    &continued_headers,
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    b"body after continuation",
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"stored", response_body.payload.as_slice());

  assert_eq!(
    (
      "POST".to_string(),
      "/continued-upload".to_string(),
      "HTTP/2".to_string(),
      Some("continued".to_string()),
      b"body after continuation".to_vec()
    ),
    rx.recv().expect("receive parsed h2 continuation request")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_interleaved_frame_before_end_headers() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(&mut stream, H2_FRAME_HEADERS, 0, 1, &[0x83, 0x86]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/interleaved", addr.to_string().as_bytes()),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("interleaved h2 frame should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_continuation_without_open_header_block() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(
    &mut stream,
    H2_FRAME_CONTINUATION,
    H2_FLAG_END_HEADERS,
    1,
    &[0x82],
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("orphan CONTINUATION should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_continuation_on_wrong_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(&mut stream, H2_FRAME_HEADERS, 0, 1, &[0x83, 0x86]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_CONTINUATION,
    H2_FLAG_END_HEADERS,
    3,
    &h2_get_headers(b"/wrong-continuation", addr.to_string().as_bytes()),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("wrong-stream CONTINUATION should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_eof_before_end_headers() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(&mut stream, H2_FRAME_HEADERS, 0, 1, &[0x83, 0x86]);
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("EOF before END_HEADERS should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(
    error.to_string().contains("incomplete HTTP/2 header block"),
    "unexpected error: {error}"
  );
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_data_before_headers_without_handler() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request).expect("send unexpected h2 request");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    b"body before headers",
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("DATA before HEADERS should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_request_missing_scheme() {
  let mut headers = vec![0x82];
  headers.extend(h2_literal_indexed_name(4, b"/missing-scheme"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_accepts_http2_prior_knowledge_options_asterisk_without_authority() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.header("host").map(str::to_string),
        ))
        .expect("send h2 options request");
        HttpResponse::ok("options")
      })
      .expect("serve h2 options request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  let mut headers = h2_literal_indexed_name(2, b"OPTIONS");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, b"*"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &headers,
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"options", response_body.payload.as_slice());

  let request = rx.recv().expect("receive h2 options request");
  assert_eq!(("OPTIONS".to_string(), "*".to_string(), None), request);
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_request_duplicate_pseudo_header() {
  let mut headers = vec![0x82, 0x82, 0x86];
  headers.extend(h2_literal_indexed_name(4, b"/duplicate-method"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_pseudo_header_after_regular_header() {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_new_name(b"x-before-pseudo", b"present"));
  headers.extend(h2_literal_indexed_name(4, b"/late-path"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_handles_multiple_interleaved_http2_streams_on_one_connection() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send h2 target");
        HttpResponse::ok(format!("response for {}", request.target()))
      })
      .expect("serve multiplexed h2 requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  let first_headers = h2_post_headers(b"/first", addr.to_string().as_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &first_headers,
  );

  let second_headers = h2_get_headers(b"/second", addr.to_string().as_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &second_headers,
  );

  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"one ");
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"body");

  let mut response_streams = Vec::new();
  let mut response_bodies = Vec::new();
  while response_bodies.len() < 2 {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      response_streams.push(frame.stream_id);
      response_bodies.push(String::from_utf8(frame.payload).expect("h2 body utf8"));
    }
  }

  assert_eq!(
    vec!["/second", "/first"],
    vec![
      rx.recv().expect("first h2 target"),
      rx.recv().expect("second h2 target"),
    ]
  );
  assert!(response_streams.contains(&1));
  assert!(response_streams.contains(&3));
  assert!(response_bodies.contains(&"response for /first".to_string()));
  assert!(response_bodies.contains(&"response for /second".to_string()));

  handle.join().expect("server thread");
}

#[test]
fn server_ignores_reset_stream_and_serves_surviving_http2_stream() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        tx.send(request.target().to_string())
          .expect("send h2 target");
        HttpResponse::ok("survivor")
      })
      .expect("serve h2 reset/goaway sequence");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/reset-me", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"partial");
  write_h2_frame(&mut stream, H2_FRAME_RST_STREAM, 0, 1, &0u32.to_be_bytes());
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"after-reset");

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/survivor", addr.to_string().as_bytes()),
  );

  let mut saw_survivor = false;
  while !saw_survivor {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.stream_id == 3 {
      assert_eq!(b"survivor", frame.payload.as_slice());
      saw_survivor = true;
    }
  }

  assert_eq!("/survivor", rx.recv().expect("survivor h2 target"));

  handle.join().expect("server thread");
}

#[test]
fn server_stops_http2_connection_on_goaway_without_calling_handler() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |_| panic!("GOAWAY must not call handler"))
      .expect("serve h2 goaway");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_GOAWAY,
    0,
    0,
    &[0, 0, 0, 0, 0, 0, 0, 0],
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_handler_reads_large_chunked_request_body_incrementally() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|request, mut body| {
        assert_eq!("POST", request.method());
        assert_eq!("/upload", request.target());
        let mut total = 0usize;
        let mut buffer = [0u8; 8192];
        loop {
          let read = body.read(&mut buffer).expect("read streaming body");
          if read == 0 {
            break;
          }
          total += read;
        }
        tx.send(total).expect("send streamed byte count");
        HttpResponse::ok(total.to_string())
      })
      .expect("serve streaming request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n")
    .expect("write request head");
  for _ in 0..128 {
    stream.write_all(b"1000\r\n").expect("write chunk size");
    stream.write_all(&vec![b'x'; 4096]).expect("write chunk");
    stream.write_all(b"\r\n").expect("write chunk terminator");
  }
  stream
    .write_all(b"0\r\nX-Done: yes\r\n\r\n")
    .expect("write terminating chunk");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(128 * 4096, rx.recv().expect("receive streamed byte count"));
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n524288",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_handler_can_reject_before_reading_request_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|request, _body| {
        assert_eq!("/reject", request.target());
        HttpResponse::new(413, "Payload Too Large").body("rejected")
      })
      .expect("serve streaming rejection");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"POST /reject HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1048576\r\n\r\nprefix")
    .expect("write partial request");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 8\r\nConnection: close\r\n\r\nrejected",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_handler_is_not_invoked_for_transfer_encoding_with_content_length() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|_request, _body| {
        tx.send(()).expect("send unexpected handler invocation");
        HttpResponse::ok("unexpected")
      })
      .expect("serve streaming request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
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

  assert!(rx.try_recv().is_err());
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_handler_can_reject_chunked_request_before_body_arrives() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|_request, _body| {
        tx.send(()).expect("record chunked handler call");
        HttpResponse::new(413, "Payload Too Large").body("rejected")
      })
      .expect("serve chunked streaming rejection");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"POST /reject HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n")
    .expect("write chunked request head");

  if rx.recv_timeout(Duration::from_secs(2)).is_err() {
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown write");
    handle.join().expect("server thread");
    panic!("chunked streaming handler was not invoked after headers");
  }

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 8\r\nConnection: close\r\n\r\nrejected",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_body_reader_rejects_malformed_chunk_size_on_read() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|_request, mut body| {
        let mut buffer = [0u8; 16];
        let error = body
          .read(&mut buffer)
          .expect_err("malformed chunk size should fail on read");
        assert_eq!(ErrorKind::InvalidData, error.kind());
        assert_eq!("invalid chunk size", error.to_string());
        HttpResponse::new(400, "Bad Request").body("Bad Request")
      })
      .expect("serve malformed streaming request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "not-hex\r\n",
        "hello\r\n",
        "0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write malformed chunked request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn streaming_body_reader_rejects_malformed_chunked_trailer_before_exposing_trailers() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_streaming(|_request, mut body| {
        let mut buffer = Vec::new();
        let error = body
          .read_to_end(&mut buffer)
          .expect_err("malformed chunk trailer should fail on read");
        assert_eq!(ErrorKind::InvalidData, error.kind());
        assert_eq!("invalid request trailer", error.to_string());
        assert!(body.trailers().is_empty());
        assert_eq!(b"hello", buffer.as_slice());
        HttpResponse::new(400, "Bad Request").body("Bad Request")
      })
      .expect("serve malformed streaming request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "POST /upload HTTP/1.1\r\n",
        "Host: localhost\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "5\r\n",
        "hello\r\n",
        "0\r\n",
        "X-Trace abc\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write malformed chunked request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
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
fn forced_close_overrides_conflicting_response_connection_keep_alive() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |_request| {
        HttpResponse::ok("terminal").header("Connection", "keep-alive")
      })
      .expect("serve request");
  });

  let response = send_request(
    addr,
    b"GET /terminal HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
  );

  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nterminal",
    response
  );
  assert!(!response.contains("\r\nConnection: keep-alive\r\n"));

  handle.join().expect("server thread");
}

#[test]
fn keep_alive_response_connection_header_survives_when_connection_remains_open() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        if request.target() == "/first" {
          HttpResponse::ok("first").header("Connection", "keep-alive")
        } else {
          HttpResponse::ok("second")
        }
      })
      .expect("serve requests");
  });

  let response = send_request(
    addr,
    concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Connection: keep-alive\r\n",
      "\r\n",
      "GET /second HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Connection: keep-alive\r\n",
      "\r\n",
    )
    .as_bytes(),
  );

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Connection: keep-alive\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "first",
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 6\r\n",
      "Connection: close\r\n",
      "\r\n",
      "second",
    ),
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
fn rttp_client_streaming_chunked_request_trailers_round_trip_over_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.body().to_vec(),
          request.trailer("x-trace").map(str::to_string),
          request.trailer("X-UPLOAD-CHECKSUM").map(str::to_string),
        ))
        .expect("send parsed request");

        HttpResponse::ok("stored by socket2")
          .header("Transfer-Encoding", "chunked")
          .header("Trailer", "X-Response-Trace, X-Response-Status")
          .trailer("X-Response-Trace", "response-trace-7")
          .trailer("X-Response-Status", "ok")
      })
      .expect("serve one request");
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/upload", addr))
    .header(("Trailer", "X-Trace, X-Upload-Checksum"))
    .trailer("X-Trace: request-trace-42")
    .expect("request trace trailer should be accepted")
    .trailer("X-Upload-Checksum: sha256:abc123")
    .expect("request checksum trailer should be accepted")
    .emit_streaming_chunked("hello ".as_bytes().chain("trailers".as_bytes()))
    .expect("chunked trailer upload");

  let (body, trace, checksum) = rx.recv().expect("parsed request");
  assert_eq!(b"hello trailers", body.as_slice());
  assert_eq!(Some("request-trace-42".to_string()), trace);
  assert_eq!(Some("sha256:abc123".to_string()), checksum);

  assert_eq!("stored by socket2", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some(&"response-trace-7".to_string()),
    response.trailer_value("x-response-trace")
  );
  assert_eq!(
    Some(&"ok".to_string()),
    response.trailer_value("X-RESPONSE-STATUS")
  );

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_streaming_chunked_post_without_trailers_stays_optional() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.body().to_vec(), request.trailers().to_vec()))
          .expect("send parsed request");

        HttpResponse::ok("stored without trailers")
          .header("Transfer-Encoding", "chunked")
          .trailer("X-Response-Mode", "optional")
      })
      .expect("serve one request");
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/upload", addr))
    .emit_streaming_chunked("plain chunked body".as_bytes())
    .expect("chunked upload without trailers");

  let (body, trailers) = rx.recv().expect("parsed request");
  assert_eq!(b"plain chunked body", body.as_slice());
  assert!(trailers.is_empty());

  assert_eq!("stored without trailers", response.body().string().unwrap());
  assert_eq!(
    Some(&"optional".to_string()),
    response.trailer_value("x-response-mode")
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
fn server_returns_expectation_failed_for_unsupported_expectation_without_calling_handler() {
  let (response, handler_called) = send_raw_request(
    concat!(
      "POST /submit HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Expect: magic\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello"
    )
    .as_bytes(),
  );

  assert!(!handler_called);
  assert_eq!(
    "HTTP/1.1 417 Expectation Failed\r\nContent-Length: 18\r\nConnection: close\r\n\r\nExpectation Failed",
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

  let expected = b"HTTP/1.1 417 Expectation Failed\r\nContent-Length: 18\r\nConnection: close\r\n\r\nExpectation Failed";
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
fn server_returns_bad_request_for_unsupported_and_malformed_http_versions() {
  for raw in [
    b"GET / HTTP/0.9\r\nHost: localhost\r\n\r\n".as_slice(),
    b"GET / HTTP/2.0\r\nHost: localhost\r\n\r\n",
    b"GET / HTP/1.1\r\nHost: localhost\r\n\r\n",
  ] {
    assert_bad_request_without_handler(raw);
  }
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
fn server_returns_bad_request_for_http_11_request_with_empty_host_before_handler() {
  assert_bad_request_without_handler(b"GET / HTTP/1.1\r\nHost: \r\n\r\n");
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
  let (response, request) = send_raw_request_capture(
    b"GET http://example.test/path?query=1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
  );

  let request = request.expect("handler receives absolute-form request");
  assert_eq!("GET", request.method());
  assert_eq!("/path?query=1", request.target());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_options_asterisk_request_target() {
  let (response, request) =
    send_raw_request_capture(b"OPTIONS * HTTP/1.1\r\nHost: localhost\r\n\r\n");

  let request = request.expect("handler receives OPTIONS asterisk-form request");
  assert_eq!("OPTIONS", request.method());
  assert_eq!("*", request.target());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nunexpected",
    response
  );
}

#[test]
fn server_accepts_connect_authority_request_target() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = format!("{} {}", request.method(), request.target());
        tx.send(observed.clone()).expect("send observed request");
        HttpResponse::ok(observed)
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n")
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "CONNECT example.com:443",
    rx.recv().expect("observed request")
  );
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 23\r\nConnection: close\r\n\r\nCONNECT example.com:443",
    response
  );

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_get_asterisk_request_target_before_handler() {
  assert_bad_request_without_handler(b"GET * HTTP/1.1\r\nHost: localhost\r\n\r\n");
}

#[test]
fn server_rejects_request_target_forms_for_wrong_methods() {
  for raw in [
    b"GET example.test:443 HTTP/1.1\r\nHost: example.test\r\n\r\n".as_slice(),
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
    b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Test: one\r\n two\r\n\r\n",
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
fn server_returns_bad_request_for_signed_content_length() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Content-Length: +5\r\n",
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
fn server_accepts_chunk_extension_without_exposing_extension_metadata() {
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
        "4;foo=bar\r\n",
        "body\r\n",
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
  assert_eq!(b"body", request.body());
  assert_eq!(None, request.header("foo"));
  assert_eq!(None, request.trailer("foo"));
  assert!(request.trailers().is_empty());
  assert_eq!(
    "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\naccepted",
    response
  );

  handle.join().expect("server thread");
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
        "x-trace: duplicate\r\n",
        "MiXeD-Trailer: Case Preserved\r\n",
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
  assert_eq!(Some("chunked"), request.header("Transfer-Encoding"));
  assert_eq!(None, request.header("X-Trace"));
  assert_eq!(Some("abc"), request.trailer("x-trace"));
  assert_eq!(Some("Case Preserved"), request.trailer("mixed-trailer"));
  assert_eq!(Some("signed"), request.trailer("X-SIGNATURE"));
  assert_eq!(
    &[
      ("X-Trace".to_string(), "abc".to_string()),
      ("x-trace".to_string(), "duplicate".to_string()),
      ("MiXeD-Trailer".to_string(), "Case Preserved".to_string()),
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
fn server_preserves_chunked_request_trailers_after_chunk_extension() {
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
        "4;foo=bar\r\nbody\r\n",
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
  assert_eq!(b"body", request.body());
  assert_eq!(None, request.header("foo"));
  assert_eq!(None, request.trailer("foo"));
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
fn server_returns_bad_request_for_payload_processing_chunked_request_trailer() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\n",
      "OK\r\n",
      "0\r\n",
      "Content-Type: text/plain\r\n",
      "\r\n"
    )
    .as_bytes(),
  );
}

#[test]
fn server_returns_bad_request_for_pseudo_header_chunked_request_trailer() {
  assert_bad_request_without_handler(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\n",
      "OK\r\n",
      "0\r\n",
      ":method: POST\r\n",
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
      "/first",
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
