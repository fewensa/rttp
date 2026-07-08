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
const H2_FRAME_WINDOW_UPDATE: u8 = 0x8;
const H2_FLAG_END_STREAM: u8 = 0x1;
const H2_FLAG_ACK: u8 = 0x1;
const H2_FLAG_END_HEADERS: u8 = 0x4;
const H2_FLAG_PADDED: u8 = 0x8;
const H2_FLAG_PRIORITY: u8 = 0x20;
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

fn find_request_path(block: &[u8]) -> Option<Vec<u8>> {
  let mut cursor = 0;
  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      decode_hpack_integer(block, &mut cursor, 7);
      continue;
    }

    let name_index = decode_hpack_integer(block, &mut cursor, 4);
    if name_index == 0 {
      let _ = decode_hpack_string(block, &mut cursor);
    }
    let value = decode_hpack_string(block, &mut cursor);
    if name_index == 4 {
      return Some(value);
    }
  }
  None
}

fn decode_hpack_integer(block: &[u8], cursor: &mut usize, prefix_bits: u8) -> usize {
  let max_prefix = (1usize << prefix_bits) - 1;
  let mut value = (block[*cursor] as usize) & max_prefix;
  *cursor += 1;
  if value < max_prefix {
    return value;
  }

  let mut shift = 0;
  loop {
    let byte = block[*cursor];
    *cursor += 1;
    value += ((byte & 0x7f) as usize) << shift;
    if byte & 0x80 == 0 {
      return value;
    }
    shift += 7;
  }
}

fn decode_hpack_string(block: &[u8], cursor: &mut usize) -> Vec<u8> {
  let huffman = block[*cursor] & 0x80 == 0x80;
  let len = decode_hpack_integer(block, cursor, 7);
  let end = *cursor + len;
  let encoded = &block[*cursor..end];
  *cursor = end;
  if huffman {
    decode_path_huffman_string(encoded)
  } else {
    encoded.to_vec()
  }
}

fn decode_path_huffman_string(encoded: &[u8]) -> Vec<u8> {
  let mut value = Vec::new();
  let mut code = 0u32;
  let mut code_len = 0u8;

  for byte in encoded {
    for bit_offset in (0..8).rev() {
      code = (code << 1) | (((byte >> bit_offset) & 1) as u32);
      code_len += 1;

      if let Some(symbol) = path_huffman_symbol(code, code_len) {
        value.push(symbol);
        code = 0;
        code_len = 0;
      }
    }
  }

  if code_len > 0 {
    assert_eq!(
      (1u32 << code_len) - 1,
      code,
      "invalid HPACK Huffman padding in request path"
    );
    assert!(code_len <= 7, "overlong HPACK Huffman padding");
  }

  value
}

fn path_huffman_symbol(code: u32, code_len: u8) -> Option<u8> {
  match (code, code_len) {
    (0x18, 6) => Some(b'/'),
    (0x16, 6) => Some(b'-'),
    (0x3, 5) => Some(b'a'),
    (0x24, 6) => Some(b'd'),
    (0x5, 5) => Some(b'e'),
    (0x26, 6) => Some(b'g'),
    (0x6, 5) => Some(b'i'),
    (0x28, 6) => Some(b'l'),
    (0x2a, 6) => Some(b'n'),
    (0x8, 5) => Some(b's'),
    (0x9, 5) => Some(b't'),
    (0x77, 7) => Some(b'v'),
    _ => None,
  }
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

fn complete_h2_peer_request_handshake(stream: &mut TcpStream) -> H2Frame {
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 peer read timeout");

  let mut preface = [0; 24];
  stream
    .read_exact(&mut preface)
    .expect("read client preface");
  assert_eq!(H2_PREFACE.as_slice(), &preface);

  let client_settings = read_h2_frame(stream);
  assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
  assert_eq!(0, client_settings.flags);
  assert_eq!(0, client_settings.stream_id);

  write_h2_frame(stream, H2_FRAME_SETTINGS, 0, 0, &[]);

  let client_settings_ack = read_h2_frame(stream);
  assert_eq!(H2_FRAME_SETTINGS, client_settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, client_settings_ack.flags);
  assert_eq!(0, client_settings_ack.stream_id);
  assert!(client_settings_ack.payload.is_empty());

  let request_headers = read_h2_frame(stream);
  assert_eq!(H2_FRAME_HEADERS, request_headers.frame_type);
  assert_eq!(
    H2_FLAG_END_STREAM | H2_FLAG_END_HEADERS,
    request_headers.flags
  );
  assert_eq!(1, request_headers.stream_id);

  write_h2_frame(stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
  request_headers
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
  assert_eq!(
    b"/valid-settings",
    find_request_path(&request_header_block)
      .expect("request path")
      .as_slice()
  );
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
fn wrapper_http2_prior_knowledge_accepts_padded_peer_response_frames_without_padding_bytes() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);

    let mut header_payload = vec![3, 0x88];
    header_payload.extend_from_slice(&h2_literal_new_name(b"x-padded", b"headers"));
    header_payload.extend_from_slice(&[0, 0, 0]);
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PADDED | H2_FLAG_END_HEADERS,
      1,
      &header_payload,
    );

    write_h2_frame(
      &mut stream,
      H2_FRAME_DATA,
      H2_FLAG_PADDED,
      1,
      &[4, b'b', b'o', b'd', b'y', 0, 0, 0, 0],
    );

    let mut trailer_payload = vec![2];
    trailer_payload.extend_from_slice(&h2_literal_new_name(b"x-trace", b"padded-trailer"));
    trailer_payload.extend_from_slice(&[0, 0]);
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PADDED | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &trailer_payload,
    );
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/padded-response", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with padded peer frames");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"headers".to_string()),
    response.header_value("X-Padded")
  );
  assert_eq!("body", response.body().string().unwrap());
  assert_eq!(
    Some(&"padded-trailer".to_string()),
    response.trailer_value("X-Trace")
  );

  handle.join().expect("padded response peer thread");
}

#[test]
fn wrapper_http2_prior_knowledge_rejects_malformed_padded_peer_response_frames() {
  let data_listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let data_addr = data_listener.local_addr().expect("h2 peer addr");
  let data_handle = thread::spawn(move || {
    let (mut stream, _) = data_listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);
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
      H2_FLAG_PADDED | H2_FLAG_END_STREAM,
      1,
      &[10, b'x'],
    );
  });

  let data_error = rttp::Http::client()
    .url(format!("http://{}/bad-data-padding", data_addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject malformed padded DATA");
  assert!(
    data_error.to_string().contains("padding"),
    "unexpected error: {data_error}"
  );
  data_handle.join().expect("bad data padding peer thread");

  let headers_listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let headers_addr = headers_listener.local_addr().expect("h2 peer addr");
  let headers_handle = thread::spawn(move || {
    let (mut stream, _) = headers_listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PADDED | H2_FLAG_END_HEADERS,
      1,
      &[10, 0x88],
    );
  });

  let headers_error = rttp::Http::client()
    .url(format!("http://{}/bad-headers-padding", headers_addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject malformed padded HEADERS");
  assert!(
    headers_error.to_string().contains("padding"),
    "unexpected error: {headers_error}"
  );
  headers_handle
    .join()
    .expect("bad headers padding peer thread");
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
fn wrapper_http2_prior_knowledge_large_request_header_reaches_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_header_value = "r".repeat(16 * 1024 + 512);
  let expected_header_value = large_header_value.clone();
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-large-header").map(str::to_string),
        ))
        .expect("send parsed large h2 request header");
        HttpResponse::ok("large request header")
      })
      .expect("serve large h2 request header");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/large-request-header", addr))
    .header(("X-Large-Header".to_string(), large_header_value))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response after large request header");

  assert_eq!("HTTP/2", response.version());
  assert_eq!("large request header", response.body().string().unwrap());
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "/large-request-header".to_string(),
      Some(expected_header_value)
    ),
    rx.recv().expect("receive parsed large h2 request header")
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_huffman_request_headers_reach_socket2_server_decoded() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let huffman_header_value = "a".repeat(64);
  let expected_header_value = huffman_header_value.clone();
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-hpack-huffman").map(str::to_string),
        ))
        .expect("send parsed Huffman h2 request header");
        HttpResponse::ok("decoded request huffman")
      })
      .expect("serve Huffman h2 request header");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/huffman-request-header", addr))
    .header(("X-HPACK-Huffman".to_string(), huffman_header_value))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response after Huffman request header");

  assert_eq!("HTTP/2", response.version());
  assert_eq!("decoded request huffman", response.body().string().unwrap());
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "/huffman-request-header".to_string(),
      Some(expected_header_value)
    ),
    rx.recv().expect("receive parsed Huffman h2 request header")
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_large_huffman_request_header_reaches_socket2_server_decoded() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_huffman_value = "a".repeat(28 * 1024);
  let expected_header_value = large_huffman_value.clone();
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-large-hpack-huffman").map(str::to_string),
        ))
        .expect("send parsed large Huffman h2 request header");
        HttpResponse::ok("decoded large request huffman")
      })
      .expect("serve large Huffman h2 request header");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/large-huffman-request-header", addr))
    .header(("X-Large-HPACK-Huffman".to_string(), large_huffman_value))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response after large Huffman request header");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "decoded large request huffman",
    response.body().string().unwrap()
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "/large-huffman-request-header".to_string(),
      Some(expected_header_value)
    ),
    rx.recv()
      .expect("receive parsed large Huffman h2 request header")
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_reads_large_response_header_from_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_header_value = "h".repeat(16 * 1024 + 512);
  let expected_header_value = large_header_value.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("large response header").header("X-Large-Response", large_header_value)
      })
      .expect("serve large h2 response header");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/large-response-header", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with large response header");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!("large response header", response.body().string().unwrap());
  assert_eq!(
    Some(&expected_header_value),
    response.header_value("X-Large-Response")
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_decodes_huffman_response_headers_and_trailers_from_socket2_server()
{
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let header_value = "a".repeat(64);
  let trailer_value = "e".repeat(64);
  let expected_header_value = header_value.clone();
  let expected_trailer_value = trailer_value.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("decoded response huffman")
          .header("X-HPACK-Huffman", header_value)
          .header("Trailer", "X-HPACK-Trailer")
          .trailer("X-HPACK-Trailer", trailer_value)
      })
      .expect("serve Huffman h2 response headers and trailers");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/huffman-response-fields", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with Huffman response fields");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    "decoded response huffman",
    response.body().string().unwrap()
  );
  assert_eq!(
    Some(&expected_header_value),
    response.header_value("X-HPACK-Huffman")
  );
  assert_eq!(
    Some(&expected_trailer_value),
    response.trailer_value("X-HPACK-Trailer")
  );
  assert!(response.header_value("Trailer").is_none());
  assert!(response.header_value("X-HPACK-Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_decodes_large_huffman_response_header_split_by_continuation() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_header_value = "a".repeat(28 * 1024);
  let expected_header_value = large_header_value.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("decoded large response huffman")
          .header("X-Large-HPACK-Huffman", large_header_value)
      })
      .expect("serve large Huffman h2 response header");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/large-huffman-response-header", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with large Huffman response header");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    "decoded large response huffman",
    response.body().string().unwrap()
  );
  assert_eq!(
    Some(&expected_header_value),
    response.header_value("X-Large-HPACK-Huffman")
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_reads_large_response_trailer_from_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_trailer_value = "t".repeat(16 * 1024 + 512);
  let expected_trailer_value = large_trailer_value.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("large response trailer")
          .header("Trailer", "X-Large-Trailer")
          .trailer("X-Large-Trailer", large_trailer_value)
      })
      .expect("serve large h2 response trailer");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/large-response-trailer", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with large response trailer");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!("large response trailer", response.body().string().unwrap());
  assert_eq!(
    Some(&expected_trailer_value),
    response.trailer_value("X-Large-Trailer")
  );
  assert!(response.header_value("Trailer").is_none());
  assert!(response.header_value("X-Large-Trailer").is_none());

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

#[test]
fn http2_feature_socket2_applies_later_max_frame_size_settings_to_response_frames() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| HttpResponse::ok("x".repeat(20_000)))
      .expect("serve h2 response after updated settings");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 65_535),
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 16_384),
  );
  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert_eq!(0, settings_ack.stream_id);
  assert!(settings_ack.payload.is_empty());

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/updated-max-frame-size", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let first_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_data.frame_type);
  assert_eq!(1, first_data.stream_id);
  assert_eq!(16_384, first_data.payload.len());
  assert_eq!(0, first_data.flags & H2_FLAG_END_STREAM);

  let second_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_data.frame_type);
  assert_eq!(1, second_data.stream_id);
  assert_eq!(3_616, second_data.payload.len());
  assert_eq!(H2_FLAG_END_STREAM, second_data.flags & H2_FLAG_END_STREAM);

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_padded_request_headers_reach_server_without_padding() {
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
        ))
        .expect("send parsed padded h2 request headers");
        HttpResponse::ok("padded headers")
      })
      .expect("serve padded h2 request headers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let headers = h2_get_headers(b"/padded-headers", addr.to_string().as_bytes());
  let mut payload = Vec::new();
  payload.push(3);
  payload.extend_from_slice(&headers);
  payload.extend_from_slice(&[0, 0, 0]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_PADDED | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &payload,
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);

  assert_eq!(
    (
      "GET".to_string(),
      "/padded-headers".to_string(),
      "HTTP/2".to_string()
    ),
    rx.recv().expect("receive parsed padded h2 request headers")
  );

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_padded_request_data_reaches_server_without_padding() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.body().to_vec())
          .expect("send parsed padded h2 request data");
        HttpResponse::ok("padded data")
      })
      .expect("serve padded h2 request data");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let headers = h2_post_headers(b"/padded-data", addr.to_string().as_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );

  let mut data = Vec::new();
  data.push(2);
  data.extend_from_slice(b"body without padding");
  data.extend_from_slice(&[0, 0]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_PADDED | H2_FLAG_END_STREAM,
    1,
    &data,
  );

  let window_update = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_WINDOW_UPDATE, window_update.frame_type);
  let stream_window_update = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_WINDOW_UPDATE, stream_window_update.frame_type);
  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);

  assert_eq!(
    b"body without padding".to_vec(),
    rx.recv().expect("receive parsed padded h2 request data")
  );

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_padded_request_trailers_reach_server_without_padding() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.body().to_vec(),
          request.trailer("x-trace").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed padded h2 request trailers");
        HttpResponse::ok("padded trailers")
      })
      .expect("serve padded h2 request trailers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let headers = h2_post_headers(b"/padded-trailers", addr.to_string().as_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"trailer body");

  let trailers = h2_literal_new_name(b"x-trace", b"padded-trailer");
  let mut trailer_payload = Vec::new();
  trailer_payload.push(4);
  trailer_payload.extend_from_slice(&trailers);
  trailer_payload.extend_from_slice(&[0, 0, 0, 0]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_PADDED | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &trailer_payload,
  );

  let window_update = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_WINDOW_UPDATE, window_update.frame_type);
  let stream_window_update = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_WINDOW_UPDATE, stream_window_update.frame_type);
  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);

  assert_eq!(
    (
      b"trailer body".to_vec(),
      Some("padded-trailer".to_string()),
      vec![("x-trace".to_string(), "padded-trailer".to_string())]
    ),
    rx.recv()
      .expect("receive parsed padded h2 request trailers")
  );

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_request_headers_with_priority_reach_server_without_priority_metadata() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send parsed priority h2 request headers");
        HttpResponse::ok("priority headers")
      })
      .expect("serve priority h2 request headers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let mut payload = vec![0, 0, 0, 0, 16];
  payload.extend_from_slice(&h2_get_headers(
    b"/priority-headers",
    addr.to_string().as_bytes(),
  ));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_PRIORITY | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &payload,
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);

  assert_eq!(
    "/priority-headers",
    rx.recv()
      .expect("receive parsed priority h2 request headers")
  );

  handle.join().expect("server thread");
}

fn assert_malformed_h2_request_rejected_before_handler(
  write_request: impl FnOnce(&mut TcpStream, SocketAddr),
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
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  write_request(&mut stream, addr);
  drop(stream);

  let result = handle.join().expect("server thread");
  assert!(
    result.is_err(),
    "malformed request should reject connection"
  );
  assert!(rx.try_recv().is_err(), "handler must not be called");
}

#[test]
fn http2_feature_socket2_rejects_malformed_padded_headers_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
    let headers = h2_get_headers(b"/bad-padded-headers", addr.to_string().as_bytes());
    let mut payload = Vec::new();
    payload.push(10);
    payload.extend_from_slice(&headers);
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PADDED | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &payload,
    );
  });
}

#[test]
fn http2_feature_socket2_rejects_short_priority_headers_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, _| {
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PRIORITY | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &[0, 0, 0, 0],
    );
  });
}

#[test]
fn http2_feature_socket2_rejects_malformed_padded_data_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
    let headers = h2_post_headers(b"/bad-padded-data", addr.to_string().as_bytes());
    write_h2_frame(stream, H2_FRAME_HEADERS, H2_FLAG_END_HEADERS, 1, &headers);
    write_h2_frame(
      stream,
      H2_FRAME_DATA,
      H2_FLAG_PADDED | H2_FLAG_END_STREAM,
      1,
      &[10, b'x'],
    );
  });
}
