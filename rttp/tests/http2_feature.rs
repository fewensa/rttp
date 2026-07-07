#![cfg(feature = "http2")]

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::HttpResponse;

const H2_FRAME_DATA: u8 = 0x0;
const H2_FRAME_HEADERS: u8 = 0x1;
const H2_FRAME_SETTINGS: u8 = 0x4;
const H2_FRAME_PING: u8 = 0x6;
const H2_FLAG_END_STREAM: u8 = 0x1;
const H2_FLAG_ACK: u8 = 0x1;
const H2_FLAG_END_HEADERS: u8 = 0x4;
const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const H2_SETTINGS_ENABLE_PUSH: u16 = 0x2;
const H2_SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
const H2_SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;

struct H2Frame {
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: Vec<u8>,
}

fn write_h2_frame(
  stream: &mut impl Write,
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

fn try_write_h2_frame(
  stream: &mut impl Write,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> io::Result<()> {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header)?;
  stream.write_all(payload)
}

fn read_h2_frame(stream: &mut impl Read) -> H2Frame {
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

fn try_read_h2_frame(stream: &mut impl Read) -> io::Result<H2Frame> {
  let mut header = [0; 9];
  stream.read_exact(&mut header)?;
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload)?;
  Ok(H2Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  })
}

fn h2_setting(id: u16, value: u32) -> [u8; 6] {
  let mut setting = [0; 6];
  setting[..2].copy_from_slice(&id.to_be_bytes());
  setting[2..].copy_from_slice(&value.to_be_bytes());
  setting
}

fn h2_literal_indexed_name(name_index: u8, value: &[u8]) -> Vec<u8> {
  assert!(value.len() < 128);
  let mut encoded = vec![name_index, value.len() as u8];
  encoded.extend_from_slice(value);
  encoded
}

fn h2_get_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn write_h2_get_request(stream: &mut TcpStream, authority: &[u8]) -> io::Result<()> {
  try_write_h2_frame(
    stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/settings", authority),
  )
}

fn complete_h2_server_handshake_with_settings(stream: &mut TcpStream, payload: &[u8]) {
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(stream, H2_FRAME_SETTINGS, 0, 0, payload);

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

fn assert_malformed_settings_rejected_before_handler(
  initial_payload: &[u8],
  initial_flags: u8,
  subsequent_settings: Option<(u8, &[u8])>,
) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|_| {
      tx.send(()).expect("send unexpected handler call");
      HttpResponse::ok("unexpected")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set client read timeout");
  stream
    .set_write_timeout(Some(Duration::from_secs(2)))
    .expect("set client write timeout");
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    initial_flags,
    0,
    initial_payload,
  );

  if let Some((flags, payload)) = subsequent_settings {
    let _ = read_h2_frame(&mut stream);
    let _ = read_h2_frame(&mut stream);
    let _ = try_write_h2_frame(&mut stream, H2_FRAME_SETTINGS, flags, 0, payload);
  } else {
    let _ = try_read_h2_frame(&mut stream);
    let _ = try_read_h2_frame(&mut stream);
  }

  let _ = write_h2_get_request(&mut stream, addr.to_string().as_bytes());
  drop(stream);

  let result = handle.join().expect("server thread");
  assert!(
    result.is_err(),
    "malformed SETTINGS should reject connection"
  );
  assert!(rx.try_recv().is_err(), "handler must not be called");
}

fn spawn_h2_peer_sending_ping_before_response() -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");

    let mut preface = [0; 24];
    stream
      .read_exact(&mut preface)
      .expect("read client preface");
    assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", &preface);

    let client_settings = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.flags);
    assert_eq!(0, client_settings.stream_id);

    write_h2_frame(&mut stream, H2_FRAME_SETTINGS, 0, 0, &[]);

    let client_settings_ack = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);

    let request_headers = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(
      H2_FLAG_END_STREAM | H2_FLAG_END_HEADERS,
      request_headers.flags
    );
    assert_eq!(1, request_headers.stream_id);

    write_h2_frame(&mut stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
    write_h2_frame(&mut stream, H2_FRAME_PING, 0, 0, b"rttp-png");

    let ping_ack = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_PING, ping_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, ping_ack.flags);
    assert_eq!(0, ping_ack.stream_id);
    assert_eq!(b"rttp-png", ping_ack.payload.as_slice());

    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &[0x88],
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_DATA,
      H2_FLAG_END_STREAM,
      1,
      b"wrapper pong",
    );
  });

  (addr, handle)
}

fn spawn_h2_peer_with_valid_settings_payload() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");

    let mut preface = [0; 24];
    stream
      .read_exact(&mut preface)
      .expect("read client preface");
    assert_eq!(H2_PREFACE.as_slice(), &preface);

    let client_settings = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.flags);
    assert_eq!(0, client_settings.stream_id);

    let mut settings = Vec::new();
    settings.extend_from_slice(&h2_setting(H2_SETTINGS_ENABLE_PUSH, 0));
    settings.extend_from_slice(&h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 65_535));
    settings.extend_from_slice(&h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 32_768));
    write_h2_frame(&mut stream, H2_FRAME_SETTINGS, 0, 0, &settings);

    let client_settings_ack = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);
    assert!(client_settings_ack.payload.is_empty());

    let request_headers = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(
      H2_FLAG_END_STREAM | H2_FLAG_END_HEADERS,
      request_headers.flags
    );
    assert_eq!(1, request_headers.stream_id);

    write_h2_frame(&mut stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &[0x88],
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_DATA,
      H2_FLAG_END_STREAM,
      1,
      b"valid settings round trip",
    );

    request_headers.payload
  });

  (addr, handle)
}

fn spawn_h2_peer_with_malformed_initial_settings() -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");

    let mut preface = [0; 24];
    stream
      .read_exact(&mut preface)
      .expect("read client preface");
    assert_eq!(H2_PREFACE.as_slice(), &preface);

    let client_settings = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
    write_h2_frame(
      &mut stream,
      H2_FRAME_SETTINGS,
      0,
      0,
      &h2_setting(H2_SETTINGS_ENABLE_PUSH, 2),
    );
  });

  (addr, handle)
}

#[test]
fn prior_knowledge_server_acknowledges_valid_settings_payload_and_serves_request() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("/settings", request.target());
        HttpResponse::ok("settings accepted")
      })
      .expect("serve h2 request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  let mut payload = Vec::new();
  payload.extend_from_slice(&h2_setting(H2_SETTINGS_ENABLE_PUSH, 0));
  payload.extend_from_slice(&h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 65_535));
  payload.extend_from_slice(&h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 16_384));
  payload.extend_from_slice(&h2_setting(0xffff, 99));
  complete_h2_server_handshake_with_settings(&mut stream, &payload);
  write_h2_get_request(&mut stream, addr.to_string().as_bytes()).expect("write h2 request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"settings accepted", response_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_rejects_initial_settings_payload_with_invalid_length() {
  assert_malformed_settings_rejected_before_handler(&[0, 1, 0], 0, None);
}

#[test]
fn prior_knowledge_server_rejects_initial_settings_with_invalid_max_frame_size() {
  assert_malformed_settings_rejected_before_handler(
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 16_383),
    0,
    None,
  );
}

#[test]
fn prior_knowledge_server_rejects_initial_settings_with_invalid_enable_push() {
  assert_malformed_settings_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_PUSH, 2),
    0,
    None,
  );
}

#[test]
fn prior_knowledge_server_rejects_initial_settings_with_invalid_initial_window_size() {
  assert_malformed_settings_rejected_before_handler(
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 2_147_483_648),
    0,
    None,
  );
}

#[test]
fn prior_knowledge_server_rejects_settings_ack_with_payload() {
  assert_malformed_settings_rejected_before_handler(
    &[],
    0,
    Some((H2_FLAG_ACK, &h2_setting(0xffff, 1))),
  );
}

#[test]
fn prior_knowledge_server_rejects_invalid_subsequent_settings_before_request() {
  assert_malformed_settings_rejected_before_handler(
    &[],
    0,
    Some((0, &h2_setting(H2_SETTINGS_ENABLE_PUSH, 2))),
  );
}

#[test]
fn wrapper_http2_prior_knowledge_accepts_valid_peer_settings_payload() {
  let (addr, handle) = spawn_h2_peer_with_valid_settings_payload();

  let response = rttp::Http::client()
    .url(format!("http://{}/valid-settings", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with valid peer SETTINGS");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    "valid settings round trip",
    response.body().string().unwrap()
  );

  let request_header_block = handle.join().expect("valid settings peer thread");
  assert!(request_header_block
    .windows(b"/valid-settings".len())
    .any(|window| window == b"/valid-settings"));
}

#[test]
fn wrapper_http2_prior_knowledge_rejects_malformed_peer_settings() {
  let (addr, handle) = spawn_h2_peer_with_malformed_initial_settings();

  let err = rttp::Http::client()
    .url(format!("http://{}/invalid-settings", addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject malformed peer SETTINGS");

  assert!(err.to_string().contains("SETTINGS_ENABLE_PUSH"));
  handle.join().expect("malformed settings peer thread");
}

#[test]
fn wrapper_http2_feature_exposes_prior_knowledge_client_path() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.version().to_string(), request.body().to_vec()))
          .expect("send request version");
        HttpResponse::ok("wrapper h2")
      })
      .expect("serve h2 request");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/wrapper", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response");

  let (request_version, request_body) = rx.recv().expect("receive request version");
  assert_eq!("HTTP/2", request_version);
  assert!(request_body.is_empty());
  assert_eq!("HTTP/2", response.version());
  assert_eq!("wrapper h2", response.body().string().unwrap());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_client_acks_peer_ping_before_response() {
  let (addr, handle) = spawn_h2_peer_sending_ping_before_response();

  let response = rttp::Http::client()
    .url(format!("http://{}/wrapper-ping", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response after ping");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!("wrapper pong", response.body().string().unwrap());

  handle.join().expect("h2 ping peer thread");
}

#[test]
fn wrapper_http2_feature_exposes_response_trailers_from_prior_knowledge_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("wrapper h2 trailers")
          .header("Trailer", "X-Trace, X-Signature")
          .trailer("X-Trace", "abc")
          .trailer("X-Signature", "signed")
      })
      .expect("serve h2 trailer response");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 trailer response");

  assert_eq!("HTTP/2", response.version());
  assert_eq!("wrapper h2 trailers", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
  assert_eq!(
    Some(&"signed".to_string()),
    response.trailer_value("X-SIGNATURE")
  );
  assert!(response.header_value("Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_post_body_round_trips_between_client_and_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();
  let request_body = b"body over h2 from rttp_client".to_vec();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.header("content-type").map(str::to_string),
          request.body().to_vec(),
        ))
        .expect("send parsed h2 request");
        HttpResponse::new(201, "Created")
          .header("Trailer", "X-Trace, X-Upload-Status")
          .body("stored over h2")
          .trailer("X-Trace", "post-body-parity")
          .trailer("X-Upload-Status", "stored")
      })
      .expect("serve h2 POST request");
  });

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{}/upload", addr))
    .content_type("application/octet-stream")
    .binary(request_body.clone())
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 POST response");

  let (method, target, version, content_type, observed_body) =
    rx.recv().expect("receive parsed h2 request");
  assert_eq!("POST", method);
  assert_eq!("/upload", target);
  assert_eq!("HTTP/2", version);
  assert_eq!(Some("application/octet-stream".to_string()), content_type);
  assert_eq!(request_body, observed_body);

  assert_eq!("HTTP/2", response.version());
  assert_eq!(201, response.code());
  assert_eq!("stored over h2", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some(&"post-body-parity".to_string()),
    response.trailer_value("x-trace")
  );
  assert_eq!(
    Some(&"stored".to_string()),
    response.trailer_value("X-UPLOAD-STATUS")
  );
  assert!(response.header_value("Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_post_request_trailers_reach_server_only_as_trailers() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();
  let request_body = b"body with request trailers".to_vec();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.body().to_vec(),
          request.header("x-trace").map(str::to_string),
          request.header("x-upload-status").map(str::to_string),
          request.trailer("x-trace").map(str::to_string),
          request.trailer("X-UPLOAD-STATUS").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed h2 request trailers");
        HttpResponse::ok("stored request trailers")
      })
      .expect("serve h2 request trailer request");
  });

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{}/upload-with-request-trailers", addr))
    .content_type("application/octet-stream")
    .binary(request_body.clone())
    .trailer(("X-Trace", "request-trailer-trace"))
    .expect("configure x-trace request trailer")
    .trailer(("X-Upload-Status", "stored"))
    .expect("configure x-upload-status request trailer")
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 request trailer response");

  assert_eq!(
    (
      "POST".to_string(),
      "/upload-with-request-trailers".to_string(),
      "HTTP/2".to_string(),
      request_body,
      None,
      None,
      Some("request-trailer-trace".to_string()),
      Some("stored".to_string()),
      vec![
        ("x-trace".to_string(), "request-trailer-trace".to_string()),
        ("x-upload-status".to_string(), "stored".to_string()),
      ]
    ),
    rx.recv().expect("receive parsed h2 request trailers")
  );
  assert_eq!("HTTP/2", response.version());
  assert_eq!("stored request trailers", response.body().string().unwrap());

  handle.join().expect("server thread");
}
