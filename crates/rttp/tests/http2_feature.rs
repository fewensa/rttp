#![cfg(feature = "http2")]

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{Http2ServerPolicy, HttpResponse};
use rttp_client::{Config, HttpClient};

const H2_FRAME_DATA: u8 = 0x0;
const H2_FRAME_HEADERS: u8 = 0x1;
const H2_FRAME_PRIORITY: u8 = 0x2;
const H2_FRAME_RST_STREAM: u8 = 0x3;
const H2_FRAME_SETTINGS: u8 = 0x4;
const H2_FRAME_PUSH_PROMISE: u8 = 0x5;
const H2_FRAME_PING: u8 = 0x6;
const H2_FRAME_GOAWAY: u8 = 0x7;
const H2_FRAME_WINDOW_UPDATE: u8 = 0x8;
const H2_FRAME_CONTINUATION: u8 = 0x9;
const H2_FLAG_END_STREAM: u8 = 0x1;
const H2_FLAG_ACK: u8 = 0x1;
const H2_FLAG_END_HEADERS: u8 = 0x4;
const H2_FLAG_PADDED: u8 = 0x8;
const H2_FLAG_PRIORITY: u8 = 0x20;
const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const H2_SETTINGS_ENABLE_PUSH: u16 = 0x2;
const H2_SETTINGS_HEADER_TABLE_SIZE: u16 = 0x1;
const H2_SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
const H2_SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
const H2_SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
const H2_SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x6;
const H2_SETTINGS_ENABLE_CONNECT_PROTOCOL: u16 = 0x8;
const H2_DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;
const H2_DEFAULT_INITIAL_WINDOW_SIZE: usize = 65_535;
const H2_ERROR_CANCEL: u32 = 0x8;
const H2_ERROR_REFUSED_STREAM: u32 = 0x7;
const H2_UNKNOWN_EXTENSION_FRAME: u8 = 0xf1;

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

fn write_raw_h2_frame(
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
  header[5..9].copy_from_slice(&stream_id.to_be_bytes());
  stream.write_all(&header).expect("write raw h2 frame head");
  stream
    .write_all(payload)
    .expect("write raw h2 frame payload");
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

fn read_h2_end_stream_data_streams(
  stream: &mut impl Read,
  expected_count: usize,
  max_frames: usize,
) -> Vec<u32> {
  let mut completed_streams = Vec::new();
  for _ in 0..max_frames {
    let frame = read_h2_frame(stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      completed_streams.push(frame.stream_id);
      if completed_streams.len() == expected_count {
        return completed_streams;
      }
    }
  }
  panic!(
    "expected {} end-stream DATA frames within {} HTTP/2 frames, got {:?}",
    expected_count, max_frames, completed_streams
  );
}

fn h2_setting(id: u16, value: u32) -> [u8; 6] {
  let mut setting = [0; 6];
  setting[..2].copy_from_slice(&id.to_be_bytes());
  setting[2..].copy_from_slice(&value.to_be_bytes());
  setting
}

fn h2_setting_value(payload: &[u8], id: u16) -> Option<u32> {
  payload.chunks_exact(6).find_map(|setting| {
    if u16::from_be_bytes([setting[0], setting[1]]) == id {
      Some(u32::from_be_bytes([
        setting[2], setting[3], setting[4], setting[5],
      ]))
    } else {
      None
    }
  })
}

fn base64url_encode_unpadded(bytes: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  let mut encoded = String::new();
  for chunk in bytes.chunks(3) {
    let first = chunk[0];
    let second = *chunk.get(1).unwrap_or(&0);
    let third = *chunk.get(2).unwrap_or(&0);
    let value = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;
    encoded.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
    encoded.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
    if chunk.len() >= 2 {
      encoded.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
    }
    if chunk.len() == 3 {
      encoded.push(ALPHABET[(value & 0x3f) as usize] as char);
    }
  }
  encoded
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

fn h2_head_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x86];
  headers.extend(h2_literal_indexed_name(2, b"HEAD"));
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn h2_trace_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x86];
  headers.extend(h2_literal_indexed_name(2, b"TRACE"));
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn h2_connect_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = h2_literal_indexed_name(2, b"CONNECT");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn h2_extended_connect_headers(path: &[u8], authority: &[u8], protocol: &[u8]) -> Vec<u8> {
  let mut headers = h2_literal_indexed_name(2, b"CONNECT");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers.extend(h2_literal_new_name(b":protocol", protocol));
  headers
}

fn h2_get_extended_connect_protocol_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = h2_get_headers(path, authority);
  headers.extend(h2_literal_new_name(b":protocol", b"websocket"));
  headers
}

fn h2_extended_connect_headers_with_regular_header_before_protocol(
  path: &[u8],
  authority: &[u8],
) -> Vec<u8> {
  let mut headers = h2_literal_indexed_name(2, b"CONNECT");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers.extend(h2_literal_new_name(b"x-trace", b"trace-before-protocol"));
  headers.extend(h2_literal_new_name(b":protocol", b"websocket"));
  headers
}

fn h2_extended_connect_headers_with_duplicate_protocol(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = h2_extended_connect_headers(path, authority, b"websocket");
  headers.extend(h2_literal_new_name(b":protocol", b"websocket"));
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

fn skip_hpack_string(block: &[u8], cursor: &mut usize) {
  let len = decode_hpack_integer(block, cursor, 7);
  *cursor += len;
}

fn count_hpack_dynamic_indexed_fields(block: &[u8]) -> usize {
  let mut cursor = 0;
  let mut indexed = 0;
  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      let index = decode_hpack_integer(block, &mut cursor, 7);
      if index > 61 {
        indexed += 1;
      }
      continue;
    }
    if byte & 0x40 == 0x40 {
      let name_index = decode_hpack_integer(block, &mut cursor, 6);
      if name_index == 0 {
        skip_hpack_string(block, &mut cursor);
      }
      skip_hpack_string(block, &mut cursor);
      continue;
    }
    if byte & 0x20 == 0x20 {
      let _ = decode_hpack_integer(block, &mut cursor, 5);
      continue;
    }
    let name_index = decode_hpack_integer(block, &mut cursor, 4);
    if name_index == 0 {
      skip_hpack_string(block, &mut cursor);
    }
    skip_hpack_string(block, &mut cursor);
  }
  indexed
}

fn count_hpack_incrementally_indexed_fields(block: &[u8]) -> usize {
  let mut cursor = 0;
  let mut indexed = 0;
  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      let _ = decode_hpack_integer(block, &mut cursor, 7);
      continue;
    }
    if byte & 0x40 == 0x40 {
      indexed += 1;
      let name_index = decode_hpack_integer(block, &mut cursor, 6);
      if name_index == 0 {
        skip_hpack_string(block, &mut cursor);
      }
      skip_hpack_string(block, &mut cursor);
      continue;
    }
    if byte & 0x20 == 0x20 {
      let _ = decode_hpack_integer(block, &mut cursor, 5);
      continue;
    }
    let name_index = decode_hpack_integer(block, &mut cursor, 4);
    if name_index == 0 {
      skip_hpack_string(block, &mut cursor);
    }
    skip_hpack_string(block, &mut cursor);
  }
  indexed
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

fn complete_h2c_upgrade(stream: &mut TcpStream, authority: &str, settings_payload: &[u8]) {
  let settings = base64url_encode_unpadded(settings_payload);
  let request = format!(
    "GET /upgrade HTTP/1.1\r\n\
     Host: {authority}\r\n\
     Connection: keep-alive, HTTP2-Settings, Upgrade\r\n\
     Upgrade: h2c\r\n\
     HTTP2-Settings: {settings}\r\n\
     \r\n"
  );
  stream
    .write_all(request.as_bytes())
    .expect("write h2c upgrade request");

  let mut response = Vec::new();
  let mut byte = [0; 1];
  while !response.ends_with(b"\r\n\r\n") {
    stream
      .read_exact(&mut byte)
      .expect("read h2c upgrade response");
    response.push(byte[0]);
  }
  let response = String::from_utf8(response).expect("utf8 upgrade response");
  assert!(response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
  assert!(response.contains("\r\nConnection: Upgrade\r\n"));
  assert!(response.contains("\r\nUpgrade: h2c\r\n"));

  stream
    .write_all(H2_PREFACE)
    .expect("write h2c upgraded client preface");

  let settings = read_h2_frame(stream);
  assert_eq!(H2_FRAME_SETTINGS, settings.frame_type);
  assert_eq!(0, settings.flags);
  assert_eq!(0, settings.stream_id);

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

fn assert_malformed_h2c_upgrade_rejected_before_handler(http2_settings: &str) {
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

  let mut stream = TcpStream::connect(addr).expect("connect h2c upgrade server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  let request = format!(
    "GET /upgrade HTTP/1.1\r\n\
     Host: {addr}\r\n\
     Connection: Upgrade, HTTP2-Settings\r\n\
     Upgrade: h2c\r\n\
     HTTP2-Settings: {http2_settings}\r\n\
     \r\n"
  );
  stream
    .write_all(request.as_bytes())
    .expect("write malformed h2c upgrade request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown client write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");
  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );

  let result = handle.join().expect("server thread");
  assert!(
    result.is_ok(),
    "malformed h2c upgrade should be handled as 400"
  );
  assert!(rx.try_recv().is_err(), "handler must not be called");
}

fn assert_h2_request_rejected_before_handler(
  settings_payload: &[u8],
  header_block: Vec<u8>,
  expected_error: &str,
) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind rejected h2 request server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("rejected h2 request addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected rejected h2 request handler call");
      HttpResponse::ok("unexpected rejected h2 request handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect rejected h2 request server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set rejected h2 request read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, settings_payload);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &header_block,
  );
  stream.flush().expect("flush rejected h2 request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("rejected h2 request server thread")
    .expect_err("h2 request must reject before handler");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err.to_string().contains(expected_error),
    "unexpected h2 request rejection error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "rejected h2 request must not dispatch"
  );
}

fn assert_connect_protocol_settings_accepted(subsequent: bool) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("GET", request.method());
        assert_eq!("/settings", request.target());
        HttpResponse::ok("connect settings ignored")
      })
      .expect("serve h2 request after connect settings")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  if subsequent {
    complete_h2_server_handshake_with_settings(&mut stream, &[]);
    write_h2_frame(
      &mut stream,
      H2_FRAME_SETTINGS,
      0,
      0,
      &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    );
    let settings_ack = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, settings_ack.flags);
    assert_eq!(0, settings_ack.stream_id);
  } else {
    complete_h2_server_handshake_with_settings(
      &mut stream,
      &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    );
  }
  write_h2_get_request(&mut stream, addr.to_string().as_bytes()).expect("write h2 request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(
    b"connect settings ignored",
    response_body.payload.as_slice()
  );

  handle.join().expect("server thread");
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

fn spawn_h2c_settings_capture_proxy(
  server_addr: SocketAddr,
) -> (SocketAddr, thread::JoinHandle<Option<u32>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 capture proxy");
  let proxy_addr = listener.local_addr().expect("h2 capture proxy addr");

  let handle = thread::spawn(move || {
    let (mut client_stream, _) = listener.accept().expect("accept h2 client through proxy");
    let mut server_stream = TcpStream::connect(server_addr).expect("connect captured h2 server");
    client_stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set proxy client read timeout");
    server_stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set proxy server read timeout");

    let mut preface = [0; 24];
    client_stream
      .read_exact(&mut preface)
      .expect("read proxied h2 client preface");
    assert_eq!(H2_PREFACE.as_slice(), &preface);
    server_stream
      .write_all(&preface)
      .expect("forward proxied h2 client preface");

    let client_settings = read_h2_frame(&mut client_stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.flags);
    assert_eq!(0, client_settings.stream_id);
    let enable_push = h2_setting_value(&client_settings.payload, H2_SETTINGS_ENABLE_PUSH);
    write_h2_frame(
      &mut server_stream,
      client_settings.frame_type,
      client_settings.flags,
      client_settings.stream_id,
      &client_settings.payload,
    );

    let mut client_reader = client_stream
      .try_clone()
      .expect("clone proxied h2 client reader");
    let mut server_writer = server_stream
      .try_clone()
      .expect("clone proxied h2 server writer");
    let client_to_server = thread::spawn(move || {
      let _ = io::copy(&mut client_reader, &mut server_writer);
    });
    let server_to_client = thread::spawn(move || {
      let _ = io::copy(&mut server_stream, &mut client_stream);
    });

    client_to_server
      .join()
      .expect("proxied h2 client-to-server relay");
    server_to_client
      .join()
      .expect("proxied h2 server-to-client relay");
    enable_push
  });

  (proxy_addr, handle)
}

fn spawn_h2c_ping_matrix_proxy(server_addr: SocketAddr) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 ping matrix proxy");
  let proxy_addr = listener.local_addr().expect("h2 ping matrix proxy addr");

  let handle = thread::spawn(move || {
    let (mut client_stream, _) = listener.accept().expect("accept h2 ping matrix client");
    let mut server_stream = TcpStream::connect(server_addr).expect("connect h2 ping matrix server");
    client_stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set ping matrix client read timeout");
    server_stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set ping matrix server read timeout");

    let mut preface = [0; 24];
    client_stream
      .read_exact(&mut preface)
      .expect("read ping matrix client preface");
    assert_eq!(H2_PREFACE.as_slice(), &preface);
    server_stream
      .write_all(&preface)
      .expect("forward ping matrix client preface");

    let client_settings = read_h2_frame(&mut client_stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.flags);
    assert_eq!(0, client_settings.stream_id);
    write_h2_frame(
      &mut server_stream,
      client_settings.frame_type,
      client_settings.flags,
      client_settings.stream_id,
      &client_settings.payload,
    );

    let server_settings = read_h2_frame(&mut server_stream);
    assert_eq!(H2_FRAME_SETTINGS, server_settings.frame_type);
    assert_eq!(0, server_settings.flags);
    assert_eq!(0, server_settings.stream_id);
    write_h2_frame(
      &mut client_stream,
      server_settings.frame_type,
      server_settings.flags,
      server_settings.stream_id,
      &server_settings.payload,
    );

    write_h2_frame(&mut server_stream, H2_FRAME_PING, 0, 0, b"srv-ping");
    let mut server_ping_ack_seen = false;
    for _ in 0..4 {
      let server_frame = read_h2_frame(&mut server_stream);
      if server_frame.frame_type == H2_FRAME_PING && server_frame.flags == H2_FLAG_ACK {
        assert_eq!(0, server_frame.stream_id);
        assert_eq!(b"srv-ping", server_frame.payload.as_slice());
        server_ping_ack_seen = true;
        break;
      }
      write_h2_frame(
        &mut client_stream,
        server_frame.frame_type,
        server_frame.flags,
        server_frame.stream_id,
        &server_frame.payload,
      );
    }
    assert!(server_ping_ack_seen, "server must echo PING ACK");

    let client_settings_ack = read_h2_frame(&mut client_stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);
    assert!(client_settings_ack.payload.is_empty());
    write_h2_frame(
      &mut server_stream,
      client_settings_ack.frame_type,
      client_settings_ack.flags,
      client_settings_ack.stream_id,
      &client_settings_ack.payload,
    );

    let request_headers = read_h2_frame(&mut client_stream);
    assert_eq!(H2_FRAME_HEADERS, request_headers.frame_type);
    assert_ne!(0, request_headers.stream_id);
    let response_stream_id = request_headers.stream_id;
    let request_ended = request_headers.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM;
    write_h2_frame(
      &mut server_stream,
      request_headers.frame_type,
      request_headers.flags,
      request_headers.stream_id,
      &request_headers.payload,
    );

    if !request_ended {
      loop {
        let request_frame = read_h2_frame(&mut client_stream);
        let is_terminal = request_frame.stream_id == response_stream_id
          && request_frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM;
        write_h2_frame(
          &mut server_stream,
          request_frame.frame_type,
          request_frame.flags,
          request_frame.stream_id,
          &request_frame.payload,
        );
        if is_terminal {
          break;
        }
      }
    }

    write_h2_frame(
      &mut client_stream,
      H2_FRAME_PING,
      H2_FLAG_ACK,
      0,
      b"old-ack!",
    );
    write_h2_frame(&mut client_stream, H2_FRAME_PING, 0, 0, b"clt-ping");
    let client_ping_ack = read_h2_frame(&mut client_stream);
    assert_eq!(H2_FRAME_PING, client_ping_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, client_ping_ack.flags);
    assert_eq!(0, client_ping_ack.stream_id);
    assert_eq!(b"clt-ping", client_ping_ack.payload.as_slice());

    loop {
      let response_frame = read_h2_frame(&mut server_stream);
      let is_terminal = response_frame.stream_id == response_stream_id
        && response_frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM;
      write_h2_frame(
        &mut client_stream,
        response_frame.frame_type,
        response_frame.flags,
        response_frame.stream_id,
        &response_frame.payload,
      );
      if is_terminal {
        break;
      }
    }
  });

  (proxy_addr, handle)
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

fn spawn_goaway_matrix_peer(
  goaway_payload: [u8; 8],
  response_before_goaway: bool,
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);

    if response_before_goaway {
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
        b"completed before shutdown",
      );
    }
    write_h2_frame(&mut stream, H2_FRAME_GOAWAY, 0, 0, &goaway_payload);
    if !response_before_goaway {
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
        b"must not be accepted",
      );
    }
  });

  (addr, handle)
}

fn spawn_rst_stream_matrix_peer(
  stream_id: u32,
  payload: &'static [u8],
  response_body: Option<&'static [u8]>,
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);
    write_h2_frame(&mut stream, H2_FRAME_RST_STREAM, 0, stream_id, payload);
    if let Some(body) = response_body {
      write_h2_frame(
        &mut stream,
        H2_FRAME_HEADERS,
        H2_FLAG_END_HEADERS,
        1,
        &[0x88],
      );
      write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, body);
    }
  });

  (addr, handle)
}

fn spawn_h2_peer_advertising_max_concurrent_streams_zero() -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 settings peer");
  let addr = listener.local_addr().expect("h2 settings peer addr");

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

    write_h2_frame(
      &mut stream,
      H2_FRAME_SETTINGS,
      0,
      0,
      &h2_setting(H2_SETTINGS_MAX_CONCURRENT_STREAMS, 0),
    );

    let client_settings_ack = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(H2_FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);
    assert!(client_settings_ack.payload.is_empty());

    stream
      .set_read_timeout(Some(Duration::from_millis(200)))
      .expect("set short h2 peer read timeout");
    assert!(
      try_read_h2_frame(&mut stream).is_err(),
      "client must not open a request stream after peer advertises zero concurrency"
    );
  });

  (addr, handle)
}

#[test]
fn cross_crate_h2c_goaway_after_completed_stream_keeps_wrapper_response_complete() {
  let (addr, handle) = spawn_goaway_matrix_peer([0, 0, 0, 0, 0, 0, 0, 0], true);

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{}/completed-before-goaway", addr))
    .emit_http2_prior_knowledge()
    .expect("completed stream must survive later GOAWAY");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "completed before shutdown",
    response.body().string().unwrap()
  );
  handle.join().expect("completed GOAWAY peer thread");
}

#[test]
fn cross_crate_h2c_goaway_lower_last_stream_id_rejects_wrapper_response() {
  let (addr, handle) = spawn_goaway_matrix_peer([0, 0, 0, 0, 0, 0, 0, 0], false);

  let error = rttp::Http::client()
    .get()
    .url(format!("http://{}/excluded-by-goaway", addr))
    .emit_http2_prior_knowledge()
    .expect_err("GOAWAY excluding active stream must fail");

  assert!(
    error
      .to_string()
      .contains("HTTP/2 connection received GOAWAY"),
    "unexpected error: {error}"
  );
  handle.join().expect("lower GOAWAY peer thread");
}

#[test]
fn cross_crate_h2c_server_graceful_shutdown_reports_last_completed_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve bounded h2 requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/matrix-one", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/matrix-two", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 matrix requests");

  assert_eq!(
    vec![1, 3],
    read_h2_end_stream_data_streams(&mut stream, 2, 8)
  );
  let shutdown = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_GOAWAY, shutdown.frame_type);
  assert_eq!(0, shutdown.flags);
  assert_eq!(0, shutdown.stream_id);
  assert_eq!(8, shutdown.payload.len());
  assert_eq!(
    3,
    u32::from_be_bytes(shutdown.payload[0..4].try_into().unwrap())
  );
  assert_eq!(
    0,
    u32::from_be_bytes(shutdown.payload[4..8].try_into().unwrap())
  );

  handle.join().expect("server thread");
}

#[test]
fn cross_crate_h2c_server_goaway_rejects_new_streams_and_drains_accepted_streams() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send graceful h2 request target");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve graceful h2 requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/accepted-inflight", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/accepted-ready", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush accepted h2 streams");

  let mut completed_streams = Vec::new();
  let shutdown = loop {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      completed_streams.push(frame.stream_id);
    }
    if frame.frame_type == H2_FRAME_GOAWAY {
      break frame;
    }
  };
  assert_eq!(0, shutdown.flags);
  assert_eq!(0, shutdown.stream_id);
  assert_eq!(8, shutdown.payload.len());
  assert_eq!(
    3,
    u32::from_be_bytes(shutdown.payload[0..4].try_into().unwrap())
  );
  assert_eq!(
    0,
    u32::from_be_bytes(shutdown.payload[4..8].try_into().unwrap())
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    5,
    &h2_get_headers(b"/rejected-after-goaway", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_GOAWAY,
    0,
    0,
    &[0, 0, 0, 3, 0, 0, 0, 0],
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_GOAWAY,
    0,
    0,
    &[0, 0, 0, 3, 0, 0, 0, 0],
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    b"accepted body",
  );
  stream
    .flush()
    .expect("flush rejected and in-flight h2 streams");

  while !(completed_streams.contains(&1) && completed_streams.contains(&3)) {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      completed_streams.push(frame.stream_id);
    }
  }

  assert!(completed_streams.contains(&1));
  assert!(completed_streams.contains(&3));
  handle.join().expect("server thread");
  assert_eq!(
    "/accepted-ready",
    rx.recv().expect("receive ready h2 request target")
  );
  assert_eq!(
    "/accepted-inflight",
    rx.recv().expect("receive in-flight h2 request target")
  );
  assert!(
    rx.try_recv().is_err(),
    "new streams after GOAWAY must not be dispatched"
  );
}

#[test]
fn cross_crate_h2c_prior_knowledge_client_server_goaway_shutdown_matrix() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        tx.send(request.target().to_string())
          .expect("send h2c shutdown matrix target");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve h2c shutdown matrix request");
  });

  let mut client = rttp::Http::client();
  let response = client
    .get()
    .url(format!("http://{}/before-goaway", addr))
    .emit_http2_prior_knowledge()
    .expect("request accepted before server GOAWAY");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!("served /before-goaway", response.body().string().unwrap());
  assert_eq!(
    "/before-goaway",
    rx.recv().expect("receive h2c shutdown matrix target")
  );

  let error = client
    .get()
    .url(format!("http://{}/after-goaway", addr))
    .emit_http2_prior_knowledge()
    .expect_err("closed h2c prior-knowledge client must refuse a replacement request");
  assert!(
    error
      .to_string()
      .to_ascii_lowercase()
      .contains("connection is closed"),
    "unexpected closed-client error: {error}"
  );
  assert!(
    rx.try_recv().is_err(),
    "closed client must not dispatch a replacement request after GOAWAY"
  );

  handle.join().expect("server thread");
}

#[test]
fn cross_crate_h2c_prior_knowledge_client_remains_single_use_after_shutdown() {
  let (addr, handle) = spawn_goaway_matrix_peer([0, 0, 0, 1, 0, 0, 0, 0], true);
  let mut client = rttp::Http::client();

  let response = client
    .get()
    .url(format!("http://{}/single-use", addr))
    .emit_http2_prior_knowledge()
    .expect("initial h2c request");
  assert_eq!(
    "completed before shutdown",
    response.body().string().unwrap()
  );

  let error = client
    .emit_http2_prior_knowledge()
    .expect_err("h2c prior-knowledge calls do not keep a reusable session");
  let error_message = error.to_string();
  assert!(
    error_message
      .to_ascii_lowercase()
      .contains("connection is closed"),
    "unexpected error: {error}"
  );
  handle.join().expect("single-use GOAWAY peer thread");
}

#[test]
fn cross_crate_h2c_rst_stream_on_active_stream_rejects_wrapper_response() {
  let (addr, handle) = spawn_rst_stream_matrix_peer(1, &[0, 0, 0, 8], None);

  let error = rttp::Http::client()
    .get()
    .url(format!("http://{}/reset-active", addr))
    .emit_http2_prior_knowledge()
    .expect_err("active stream reset must fail the wrapper response");

  assert!(
    error.to_string().contains("RST_STREAM error code 8"),
    "unexpected error: {error}"
  );
  handle.join().expect("active RST_STREAM peer thread");
}

#[test]
fn cross_crate_h2c_rst_stream_malformed_boundaries_reject_wrapper_response() {
  let cases: &[(&str, u32, &[u8])] = &[
    ("stream-zero", 0, &[0, 0, 0, 0]),
    ("short-payload", 1, &[0, 0, 0]),
    ("long-payload", 1, &[0, 0, 0, 8, 0]),
  ];

  for (name, stream_id, payload) in cases {
    let (addr, handle) = spawn_rst_stream_matrix_peer(*stream_id, payload, Some(b"ignored"));

    let error = rttp::Http::client()
      .get()
      .url(format!("http://{}/bad-rst-{}", addr, name))
      .emit_http2_prior_knowledge()
      .expect_err("malformed RST_STREAM must fail the wrapper response");

    assert!(
      error
        .to_string()
        .contains("invalid HTTP/2 RST_STREAM frame"),
      "unexpected error for {name}: {error}"
    );
    handle.join().expect("malformed RST_STREAM peer thread");
  }
}

#[test]
fn cross_crate_h2c_max_concurrent_streams_matrix_wrapper_allows_one_active_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 max concurrent server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 max concurrent addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
        ))
        .expect("send single active h2 request");
        HttpResponse::ok("single stream accepted")
      })
      .expect("serve single bounded h2 request")
  });

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{}/max-concurrent/one", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper client may open one stream when peer permits one");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "single stream accepted",
    response.body().string().expect("response body")
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "GET".to_string(),
      "/max-concurrent/one".to_string(),
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive single active request")
  );
  handle.join().expect("single stream h2 server thread");
}

#[test]
fn cross_crate_h2c_max_concurrent_streams_matrix_wrapper_rejects_zero_peer_bound() {
  let (addr, handle) = spawn_h2_peer_advertising_max_concurrent_streams_zero();

  let err = rttp::Http::client()
    .get()
    .url(format!("http://{}/max-concurrent/zero", addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject zero peer concurrency");
  assert!(
    err
      .to_string()
      .contains("SETTINGS_MAX_CONCURRENT_STREAMS forbids opening a request stream"),
    "unexpected zero-concurrency error: {err}"
  );

  handle.join().expect("zero max concurrent peer thread");
}

#[test]
fn cross_crate_h2c_max_concurrent_streams_matrix_server_rejects_over_limit_interleaved_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 max concurrent server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 max concurrent addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send allowed h2 request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve allowed bounded h2 requests")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 max concurrent server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 max concurrent read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/max-concurrent/allowed-one", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &h2_post_headers(b"/max-concurrent/allowed-two", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    5,
    &h2_get_headers(b"/max-concurrent/rejected", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush over-limit h2 headers");

  let reset = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_RST_STREAM, reset.frame_type);
  assert_eq!(0, reset.flags);
  assert_eq!(5, reset.stream_id);
  assert_eq!(
    H2_ERROR_REFUSED_STREAM.to_be_bytes(),
    reset.payload.as_slice()
  );
  assert!(
    rx.try_recv().is_err(),
    "over-limit stream must be rejected before dispatching allowed incomplete streams"
  );

  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"");
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 3, b"");
  stream.flush().expect("flush allowed h2 completions");

  let mut completed_response_streams = read_h2_end_stream_data_streams(&mut stream, 2, 12);
  completed_response_streams.sort_unstable();
  assert_eq!(vec![1, 3], completed_response_streams);

  let mut received = vec![
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive first allowed request"),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive second allowed request"),
  ];
  received.sort();
  assert_eq!(
    vec![
      "/max-concurrent/allowed-one".to_string(),
      "/max-concurrent/allowed-two".to_string(),
    ],
    received
  );
  assert!(
    rx.try_recv().is_err(),
    "over-limit stream must not reach the handler"
  );

  handle.join().expect("max concurrent h2 server thread");
}

#[test]
fn cross_crate_h2c_server_drops_inbound_reset_stream_and_serves_next_wrapper_request() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
        ))
        .expect("send parsed h2 request");
        HttpResponse::ok("survived reset")
      })
      .expect("serve h2 stream after reset")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/reset-inbound", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    0,
    1,
    b"body the wrapper must drop",
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_RST_STREAM,
    0,
    1,
    &H2_ERROR_CANCEL.to_be_bytes(),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    b"late reset data",
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/after-reset", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 reset matrix");

  let response_headers = (0..8)
    .map(|_| read_h2_frame(&mut stream))
    .find(|frame| frame.frame_type == H2_FRAME_HEADERS && frame.stream_id == 3)
    .expect("surviving stream response headers");
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(3, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(3, response_body.stream_id);
  assert_eq!(b"survived reset", response_body.payload.as_slice());

  assert_eq!(
    ("HTTP/2".to_string(), "/after-reset".to_string(), Vec::new(),),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive surviving request")
  );
  assert!(
    rx.try_recv().is_err(),
    "reset stream must not reach handler"
  );

  handle.join().expect("server thread");
}

#[test]
fn cross_crate_h2c_server_rejects_malformed_rst_stream_boundaries_before_handler() {
  let cases: &[(u32, &[u8])] = &[(0, &[0, 0, 0, 0]), (1, &[0, 0, 0]), (1, &[0, 0, 0, 8, 0])];

  for (stream_id, payload) in cases {
    assert_malformed_h2_request_rejected_before_handler(|stream, _| {
      write_h2_frame(stream, H2_FRAME_RST_STREAM, 0, *stream_id, payload);
    });
  }
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
fn h2c_upgrade_server_transitions_to_bounded_http2_loop_on_same_socket() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("GET", request.method());
        assert_eq!("/settings", request.target());
        HttpResponse::ok("h2c upgrade served")
      })
      .expect("serve h2c upgrade request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2c upgrade server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  let mut settings_payload = Vec::new();
  settings_payload.extend_from_slice(&h2_setting(H2_SETTINGS_ENABLE_PUSH, 0));
  settings_payload.extend_from_slice(&h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 16_384));
  complete_h2c_upgrade(&mut stream, &addr.to_string(), &settings_payload);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/settings", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(3, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(3, response_body.stream_id);
  assert_eq!(b"h2c upgrade served", response_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn h2c_upgrade_server_preserves_request_and_response_trailers_after_transition() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
          request.header("x-trace").map(str::to_string),
          request.trailer("x-trace").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed upgraded h2 trailers");
        HttpResponse::ok("upgraded trailers accepted")
          .trailer("X-Response-Trace", "upgrade-response-trailer")
      })
      .expect("serve upgraded h2 trailer request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2c upgrade server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2c_upgrade(&mut stream, &addr.to_string(), &[]);

  let mut headers = h2_post_headers(b"/upgrade-trailers", addr.to_string().as_bytes());
  headers.extend(h2_literal_new_name(b"x-trace", b"initial-header"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 3, b"upgraded body");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_literal_new_name(b"x-trace", b"request-trailer"),
  );

  let response_headers = loop {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type != H2_FRAME_WINDOW_UPDATE {
      break frame;
    }
  };
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(3, response_headers.stream_id);

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(0, response_body.flags & H2_FLAG_END_STREAM);
  assert_eq!(3, response_body.stream_id);
  assert_eq!(
    b"upgraded trailers accepted",
    response_body.payload.as_slice()
  );

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_trailers.flags
  );
  assert_eq!(3, response_trailers.stream_id);

  assert_eq!(
    (
      "HTTP/2".to_string(),
      "POST".to_string(),
      "/upgrade-trailers".to_string(),
      b"upgraded body".to_vec(),
      Some("initial-header".to_string()),
      Some("request-trailer".to_string()),
      vec![("x-trace".to_string(), "request-trailer".to_string())],
    ),
    rx.recv().expect("receive parsed upgraded h2 trailers")
  );
  handle.join().expect("server thread");
}

#[test]
fn h2c_upgrade_rejects_malformed_http2_settings_before_handler_dispatch() {
  assert_malformed_h2c_upgrade_rejected_before_handler("%%%");
  assert_malformed_h2c_upgrade_rejected_before_handler(&base64url_encode_unpadded(&h2_setting(
    H2_SETTINGS_ENABLE_PUSH,
    2,
  )));
}

#[test]
fn cross_crate_http11_h2c_upgrade_client_server_matrix() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind cross-crate h2c upgrade server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("cross-crate h2c addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("GET", request.method());
        assert_eq!("/upgrade-matrix?mode=http11", request.target());
        assert_eq!(Some(addr.to_string().as_str()), request.header("host"));
        HttpResponse::ok("cross-crate h2c upgrade").header("x-cross-crate-path", "http11-upgrade")
      })
      .expect("serve cross-crate h2c upgrade request")
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/upgrade-matrix?mode=http11", addr))
    .emit_http2_upgrade()
    .expect("cross-crate h2c upgrade response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"http11-upgrade".to_string()),
    response.header_value("x-cross-crate-path")
  );
  assert_eq!("cross-crate h2c upgrade", response.body().string().unwrap());

  handle.join().expect("cross-crate h2c upgrade thread");
}

#[test]
fn cross_crate_http11_h2c_upgrade_rejects_invalid_or_missing_settings_before_dispatch() {
  for request in [
    format!(
      "GET /bad-h2c HTTP/1.1\r\n\
       Host: ignored\r\n\
       Connection: Upgrade, HTTP2-Settings\r\n\
       Upgrade: h2c\r\n\
       HTTP2-Settings: {}\r\n\
       \r\n",
      base64url_encode_unpadded(&h2_setting(H2_SETTINGS_ENABLE_PUSH, 2))
    ),
    "GET /missing-h2c HTTP/1.1\r\n\
     Host: ignored\r\n\
     Connection: Upgrade, HTTP2-Settings\r\n\
     Upgrade: h2c\r\n\
     \r\n"
      .to_string(),
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind cross-crate h2c rejection server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c rejection addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server.accept_one(|_| {
        tx.send(()).expect("send unexpected handler call");
        HttpResponse::ok("unexpected")
      })
    });

    let mut stream = TcpStream::connect(addr).expect("connect h2c rejection server");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2c rejection read timeout");
    stream
      .write_all(
        request
          .replace("Host: ignored", &format!("Host: {addr}"))
          .as_bytes(),
      )
      .expect("write h2c rejection request");
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown h2c rejection client write");

    let mut response = String::new();
    stream
      .read_to_string(&mut response)
      .expect("read h2c rejection response");
    assert_eq!(
      "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
      response
    );

    let result = handle.join().expect("h2c rejection server thread");
    assert!(result.is_ok(), "invalid h2c upgrade must map to HTTP 400");
    assert!(rx.try_recv().is_err(), "handler must not be dispatched");
  }
}

#[test]
fn cross_crate_http11_h2c_upgrade_detection_preserves_non_h2c_upgrade_handoff() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind non-h2c upgrade handoff server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("non-h2c upgrade addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("GET", request.method());
        assert_eq!("/chat", request.target());
        assert_eq!(Some("websocket"), request.header("Upgrade"));
        rttp::server::HttpHandoff::upgrade(
          HttpResponse::new(101, "Switching Protocols")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket"),
          |mut stream| {
            stream.write_all(b"server-websocket")?;
            let mut client_bytes = [0u8; 16];
            stream.read_exact(&mut client_bytes)?;
            assert_eq!(b"client-websocket", &client_bytes);
            Ok(())
          },
        )
      })
      .expect("serve non-h2c upgrade handoff")
  });

  let mut upgraded = HttpClient::new()
    .url(format!("http://{}/chat", addr))
    .header(("Connection", "Upgrade"))
    .header(("Upgrade", "websocket"))
    .upgrade()
    .expect("non-h2c upgrade handoff");
  assert_eq!(101, upgraded.response().code());
  assert_eq!(
    Some(&"websocket".to_string()),
    upgraded.response().header_value("Upgrade")
  );

  let mut server_bytes = [0u8; 16];
  upgraded
    .stream_mut()
    .read_exact(&mut server_bytes)
    .expect("read non-h2c upgraded server bytes");
  assert_eq!(b"server-websocket", &server_bytes);
  upgraded
    .stream_mut()
    .write_all(b"client-websocket")
    .expect("write non-h2c upgraded client bytes");

  handle.join().expect("non-h2c upgrade handoff thread");
}

#[test]
fn cross_crate_prior_knowledge_h2c_still_bypasses_http11_upgrade_path() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind prior-knowledge regression server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server
    .local_addr()
    .expect("prior-knowledge regression addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("GET", request.method());
        assert_eq!("/prior-knowledge-regression", request.target());
        assert!(request.header("upgrade").is_none());
        assert!(request.header("http2-settings").is_none());
        HttpResponse::ok("prior knowledge unchanged")
      })
      .expect("serve prior-knowledge regression request")
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/prior-knowledge-regression", addr))
    .emit_http2_prior_knowledge()
    .expect("prior-knowledge h2c regression response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "prior knowledge unchanged",
    response.body().string().unwrap()
  );

  handle.join().expect("prior-knowledge regression thread");
}

#[test]
fn prior_knowledge_server_advertises_conservative_max_frame_size() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        HttpResponse::ok("advertised max frame size")
      })
      .expect("serve h2 request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, 0, 0, &[]);

  let settings = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings.frame_type);
  assert_eq!(0, settings.flags);
  assert_eq!(0, settings.stream_id);
  assert_eq!(
    Some(H2_DEFAULT_MAX_FRAME_SIZE as u32),
    h2_setting_value(&settings.payload, H2_SETTINGS_MAX_FRAME_SIZE)
  );

  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert_eq!(0, settings_ack.stream_id);

  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
  write_h2_get_request(&mut stream, addr.to_string().as_bytes()).expect("write h2 request");
  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(
    b"advertised max frame size",
    response_body.payload.as_slice()
  );

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_policy_advertises_and_enforces_frame_and_metadata_bounds() {
  let policy = Http2ServerPolicy::new()
    .with_max_frame_size(32_768)
    .with_max_header_list_size(256);
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_http2_policy(policy)
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request.target().to_string())
        .expect("send unexpected bounded h2 request");
      HttpResponse::ok("unexpected bounded h2 request")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set client read timeout");
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, 0, 0, &[]);
  let settings = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings.frame_type);
  assert_eq!(
    Some(32_768),
    h2_setting_value(&settings.payload, H2_SETTINGS_MAX_FRAME_SIZE)
  );
  assert_eq!(
    Some(256),
    h2_setting_value(&settings.payload, H2_SETTINGS_MAX_HEADER_LIST_SIZE)
  );
  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);

  write_h2_frame(&mut stream, H2_FRAME_SETTINGS, H2_FLAG_ACK, 0, &[]);
  let mut headers = h2_head_headers(b"/policy", addr.to_string().as_bytes());
  headers.extend(h2_literal_new_name(b"x-policy-limit", &vec![b'x'; 100]));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &headers,
  );
  stream.flush().expect("flush bounded h2 request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("oversized h2 metadata must reject the connection");
  assert_eq!(io::ErrorKind::InvalidData, error.kind());
  assert_eq!("HTTP/2 header list size exceeded", error.to_string());
  assert!(
    rx.try_recv().is_err(),
    "oversized metadata must not dispatch"
  );
}

#[test]
fn prior_knowledge_server_policy_rejects_inbound_frame_exceeding_configured_max_before_handler() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_http2_policy(Http2ServerPolicy::new().with_max_frame_size(32_768))
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request.target().to_string())
        .expect("send unexpected oversized h2 request");
      HttpResponse::ok("unexpected oversized frame")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &vec![0x82; 32_768 + 1],
  );
  stream.flush().expect("flush oversized h2 frame");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("server thread")
    .expect_err("oversized inbound frame must reject the connection");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 frame payload exceeds active max frame size"),
    "unexpected oversized frame error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "oversized frame must not reach the handler"
  );
}

#[test]
fn prior_knowledge_server_uses_legal_peer_max_frame_size_update_for_response_data() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("/updated-response-frame-size", request.target());
        HttpResponse::ok(vec![b'x'; 40_000])
      })
      .expect("serve h2 response using updated frame size")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, 32_768),
  );
  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert_eq!(0, settings_ack.stream_id);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/updated-response-frame-size", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let mut data_lengths = Vec::new();
  loop {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type != H2_FRAME_DATA {
      continue;
    }
    assert!(frame.payload.len() <= 32_768);
    data_lengths.push(frame.payload.len());
    if frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      break;
    }
  }
  assert!(
    data_lengths
      .iter()
      .any(|len| *len > H2_DEFAULT_MAX_FRAME_SIZE),
    "legal SETTINGS_MAX_FRAME_SIZE update should allow larger response DATA frames"
  );
  assert_eq!(40_000, data_lengths.iter().sum::<usize>());

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_splits_response_trailing_headers_to_peer_max_frame_size() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("/large-trailers", request.target());
        let mut response = HttpResponse::ok("body");
        for index in 0..420 {
          response = response.trailer(
            format!("X-Trailer-{index}"),
            format!("value-{index}-{}", "t".repeat(120)),
          );
        }
        response
      })
      .expect("serve h2 response with large trailers")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, H2_DEFAULT_MAX_FRAME_SIZE as u32),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/large-trailers", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(b"body", response_body.payload.as_slice());
  assert_eq!(0, response_body.flags & H2_FLAG_END_STREAM);

  let first_trailer = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, first_trailer.frame_type);
  assert_eq!(1, first_trailer.stream_id);
  assert!(first_trailer.payload.len() <= H2_DEFAULT_MAX_FRAME_SIZE);
  assert_eq!(H2_FLAG_END_STREAM, first_trailer.flags & H2_FLAG_END_STREAM);
  assert_eq!(0, first_trailer.flags & H2_FLAG_END_HEADERS);

  let mut saw_final_continuation = false;
  for _ in 0..8 {
    let frame = read_h2_frame(&mut stream);
    assert_eq!(1, frame.stream_id);
    assert!(frame.payload.len() <= H2_DEFAULT_MAX_FRAME_SIZE);
    if frame.frame_type == H2_FRAME_CONTINUATION
      && frame.flags & H2_FLAG_END_HEADERS == H2_FLAG_END_HEADERS
    {
      saw_final_continuation = true;
      break;
    }
  }
  assert!(
    saw_final_continuation,
    "large response trailers should be split with CONTINUATION frames"
  );

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_ends_head_response_on_headers_without_data_frame() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("HEAD", request.method());
        assert_eq!("/metadata", request.target());
        HttpResponse::ok("metadata body")
      })
      .expect("serve h2 HEAD request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_head_headers(b"/metadata", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 HEAD request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_headers.flags
  );
  assert_eq!(1, response_headers.stream_id);
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set short client read timeout");
  let shutdown = read_h2_frame(&mut stream);
  assert_eq!(
    H2_FRAME_GOAWAY, shutdown.frame_type,
    "HEAD responses must end on HEADERS before graceful shutdown"
  );
  assert_eq!(0, shutdown.flags);
  assert_eq!(0, shutdown.stream_id);
  assert_eq!(8, shutdown.payload.len());
  assert_eq!(
    1,
    u32::from_be_bytes(shutdown.payload[0..4].try_into().unwrap())
  );
  assert_eq!(
    0,
    u32::from_be_bytes(shutdown.payload[4..8].try_into().unwrap())
  );

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_delivers_bodyless_trace_once_with_origin_form_target() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
        ))
        .expect("send parsed h2 TRACE request");
        HttpResponse::ok("trace accepted over h2")
      })
      .expect("serve h2 TRACE request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_trace_headers(b"/trace/socket2?via=h2c", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 TRACE request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"trace accepted over h2", response_body.payload.as_slice());

  assert_eq!(
    (
      "HTTP/2".to_string(),
      "TRACE".to_string(),
      "/trace/socket2?via=h2c".to_string(),
      Vec::new(),
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive parsed h2 TRACE request")
  );
  assert!(rx.try_recv().is_err(), "handler must be called once");

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_options_prior_knowledge_round_trips_against_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
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
        .expect("send parsed h2 request");

        HttpResponse::ok("options accepted over h2")
          .header("Content-Type", "text/plain")
          .header("X-RTTP-H2C", "socket2")
      })
      .expect("serve h2 OPTIONS request")
  });

  let response = HttpClient::new()
    .options()
    .url(format!("http://{}/matrix/options?via=h2c", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 OPTIONS response");

  let (method, target, version) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("receive parsed h2 request");
  assert_eq!("OPTIONS", method);
  assert_eq!("/matrix/options?via=h2c", target);
  assert_eq!("HTTP/2", version);

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"socket2".to_string()),
    response.header_value("x-rttp-h2c")
  );
  assert_eq!(
    "options accepted over h2",
    response.body().string().expect("response body")
  );

  handle.join().expect("server thread");
}

#[test]
fn h2c_priority_unsupported_scheduling_matrix_preserves_wrapper_response_behavior() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 priority peer");
  let addr = listener.local_addr().expect("h2 priority peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);

    write_h2_frame(&mut stream, H2_FRAME_PRIORITY, 0, 1, &[0, 0, 0, 0, 16]);
    let mut headers = vec![0, 0, 0, 0, 32, 0x88];
    headers.extend_from_slice(&h2_literal_new_name(b"x-priority", b"metadata-only"));
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_PRIORITY | H2_FLAG_END_HEADERS,
      1,
      &headers,
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_DATA,
      H2_FLAG_END_STREAM,
      1,
      b"priority metadata ignored",
    );
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/priority/client-boundary", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper client ignores valid priority metadata");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"metadata-only".to_string()),
    response.header_value("X-Priority")
  );
  assert_eq!(
    "priority metadata ignored",
    response.body().string().expect("response body")
  );

  handle.join().expect("h2 priority peer thread");
}

#[test]
fn h2c_priority_unsupported_scheduling_matrix_rejects_malformed_wrapper_peer_priority() {
  for (path, stream_id, payload) in [
    ("zero-stream", 0, vec![0, 0, 0, 0, 16]),
    ("short-payload", 1, vec![0, 0, 0, 0]),
    ("long-payload", 1, vec![0, 0, 0, 0, 16, 0]),
  ] {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 priority peer");
    let addr = listener.local_addr().expect("h2 priority peer addr");

    let handle = thread::spawn(move || {
      let (mut stream, _) = listener.accept().expect("accept h2 client");
      complete_h2_peer_request_handshake(&mut stream);
      write_h2_frame(&mut stream, H2_FRAME_PRIORITY, 0, stream_id, &payload);
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
        b"unexpected",
      );
    });

    let err = rttp::Http::client()
      .url(format!("http://{}/priority/bad-{}", addr, path))
      .emit_http2_prior_knowledge()
      .expect_err("wrapper client must reject malformed PRIORITY frames");
    assert!(
      err.to_string().contains("invalid HTTP/2 PRIORITY frame"),
      "unexpected wrapper client priority error: {err}"
    );

    handle.join().expect("bad h2 priority peer thread");
  }
}

#[test]
fn h2c_priority_unsupported_scheduling_matrix_server_ignores_valid_priority_metadata() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 priority server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 priority server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed priority h2 request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve priority h2 requests")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 priority server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  write_h2_frame(&mut stream, H2_FRAME_PRIORITY, 0, 3, &[0, 0, 0, 0, 1]);
  let mut first_headers = vec![0, 0, 0, 0, 255];
  first_headers.extend_from_slice(&h2_get_headers(
    b"/priority/server-first",
    addr.to_string().as_bytes(),
  ));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_PRIORITY | H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &first_headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_PRIORITY, 0, 1, &[0, 0, 0, 0, 255]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/priority/server-second", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 priority requests");

  assert_eq!(
    vec![1, 3],
    read_h2_end_stream_data_streams(&mut stream, 2, 8),
    "valid priority metadata must not reorder completed server responses"
  );
  assert_eq!(
    "/priority/server-first",
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive first priority request")
  );
  assert_eq!(
    "/priority/server-second",
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive second priority request")
  );

  handle.join().expect("h2 priority server thread");
}

#[test]
fn h2c_priority_unsupported_scheduling_matrix_server_rejects_malformed_priority_frames() {
  for (stream_id, payload) in [
    (0, vec![0, 0, 0, 0, 16]),
    (1, vec![0, 0, 0, 0]),
    (2, vec![0, 0, 0, 0, 16]),
  ] {
    assert_malformed_h2_request_rejected_before_handler(|stream, _| {
      write_h2_frame(stream, H2_FRAME_PRIORITY, 0, stream_id, &payload);
    });
  }
}

#[test]
fn h2c_push_promise_unsupported_matrix_rejects_wrapper_peer_push_before_final_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 push peer");
  let addr = listener.local_addr().expect("h2 push peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);

    write_h2_frame(
      &mut stream,
      H2_FRAME_PUSH_PROMISE,
      H2_FLAG_END_HEADERS,
      1,
      &[0, 0, 0, 2, 0x82],
    );
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
      b"unexpected pushed response",
    );
  });

  let err = rttp::Http::client()
    .get()
    .url(format!("http://{}/push-promise/client-boundary", addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject PUSH_PROMISE before final response");
  assert!(
    err
      .to_string()
      .contains("unsupported HTTP/2 PUSH_PROMISE server push"),
    "unexpected wrapper client PUSH_PROMISE error: {err}"
  );

  handle.join().expect("h2 push peer thread");
}

#[test]
fn h2c_push_promise_unsupported_matrix_server_rejects_client_push_before_handler() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 push server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 push server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request.target().to_string())
        .expect("send unexpected PUSH_PROMISE handler call");
      HttpResponse::ok("unexpected h2 PUSH_PROMISE handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 push server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 push client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_PUSH_PROMISE,
    H2_FLAG_END_HEADERS,
    1,
    &[0, 0, 0, 2, 0x82],
  );
  stream.flush().expect("flush h2 PUSH_PROMISE");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("h2 push server thread")
    .expect_err("client-sent PUSH_PROMISE must reject before handler");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 PUSH_PROMISE frame is unsupported"),
    "unexpected h2c server PUSH_PROMISE rejection error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "client-sent PUSH_PROMISE must not be dispatched as a normal request"
  );
}

#[test]
fn h2c_connect_unsupported_matrix_preserves_http11_handoff_boundary() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 client preflight peer");
  listener
    .set_nonblocking(true)
    .expect("set h2 client preflight peer nonblocking");
  let addr = listener.local_addr().expect("h2 client preflight addr");

  let err = HttpClient::new()
    .method("CONNECT")
    .url(format!("http://{}/client-preflight", addr))
    .emit_http2_prior_knowledge()
    .expect_err("h2c CONNECT must be rejected before opening a socket");
  assert!(err.is_builder());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 prior-knowledge CONNECT or extended CONNECT is unsupported"),
    "unexpected h2c client preflight error: {err}"
  );
  assert!(
    matches!(listener.accept(), Err(ref err) if err.kind() == io::ErrorKind::WouldBlock),
    "h2c CONNECT preflight must not open a server connection"
  );

  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected h2 CONNECT handler call");
      HttpResponse::ok("unexpected h2 CONNECT handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_connect_headers(b"/h2c-tunnel", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 CONNECT request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("h2 server thread")
    .expect_err("h2c CONNECT must reject before handler");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 prior-knowledge CONNECT/proxy tunneling is unsupported"),
    "unexpected h2c server rejection error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "h2c CONNECT must not be dispatched as a normal request"
  );

  let server = rttp::Http::server("127.0.0.1:0").expect("bind HTTP/1.1 CONNECT server");
  let addr = server.local_addr().expect("HTTP/1.1 CONNECT server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("CONNECT", request.method());
        assert_eq!(addr.to_string(), request.target());
        rttp::server::HttpHandoff::connect(
          HttpResponse::new(200, "Connection Established"),
          |mut stream| {
            let mut ping = [0u8; 4];
            stream.read_exact(&mut ping)?;
            assert_eq!(b"ping", &ping);
            stream.write_all(b"pong")?;
            Ok(())
          },
        )
      })
      .expect("serve HTTP/1.1 CONNECT handoff");
  });

  let mut tunnel = HttpClient::new()
    .url(format!("http://{}", addr))
    .connect()
    .expect("HTTP/1.1 CONNECT handoff remains supported");
  assert_eq!(200, tunnel.response().code());
  tunnel
    .stream_mut()
    .write_all(b"ping")
    .expect("write HTTP/1.1 tunnel bytes");
  let mut pong = [0u8; 4];
  tunnel
    .stream_mut()
    .read_exact(&mut pong)
    .expect("read HTTP/1.1 tunnel bytes");
  assert_eq!(b"pong", &pong);

  handle.join().expect("HTTP/1.1 CONNECT server thread");
}

#[test]
fn h2c_extended_connect_protocol_pseudo_header_rejects_before_handler() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 extended connect boundary server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server
    .local_addr()
    .expect("h2 extended connect boundary addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected h2 :protocol handler call");
      HttpResponse::ok("unexpected h2 :protocol handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 extended connect boundary server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 extended connect boundary client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_extended_connect_protocol_headers(
      b"/extended-connect-boundary",
      addr.to_string().as_bytes(),
    ),
  );
  stream.flush().expect("flush h2 :protocol request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("h2 extended connect boundary server thread")
    .expect_err("h2c :protocol must reject before handler");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 extended CONNECT :protocol requires SETTINGS_ENABLE_CONNECT_PROTOCOL"),
    "unexpected h2c :protocol rejection error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "h2c :protocol must not be dispatched as a normal request"
  );
}

#[test]
fn h2c_extended_connect_dispatches_after_initial_connect_protocol_negotiation() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 extended connect server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 extended connect addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("CONNECT", request.method());
        assert_eq!("/ws", request.target());
        assert_eq!(Some(addr.to_string().as_str()), request.header("host"));
        assert_eq!(Some("trace-1"), request.header("x-trace"));
        assert_eq!(Some("websocket"), request.extended_connect_protocol());
        assert!(request.body().is_empty());
        tx.send(()).expect("send extended CONNECT dispatch");
        HttpResponse::ok("extended connect dispatched")
      })
      .expect("serve negotiated h2 extended CONNECT")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 extended connect server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 extended connect read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &{
      let mut headers =
        h2_extended_connect_headers(b"/ws", addr.to_string().as_bytes(), b"websocket");
      headers.extend(h2_literal_new_name(b"x-trace", b"trace-1"));
      headers
    },
  );
  stream
    .flush()
    .expect("flush negotiated h2 extended CONNECT request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(
    b"extended connect dispatched",
    response_body.payload.as_slice()
  );

  handle.join().expect("h2 extended connect server thread");
  rx.recv_timeout(Duration::from_secs(2))
    .expect("receive one extended CONNECT dispatch");
  assert!(
    rx.try_recv().is_err(),
    "extended CONNECT must dispatch exactly once"
  );
}

#[test]
fn h2c_extended_connect_rejects_body_or_trailers_before_handler() {
  for case in ["data", "trailers"] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind bounded h2 extended connect server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("bounded h2 extended connect addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server.accept_one(|request| {
        tx.send((request.method().to_string(), request.body().to_vec()))
          .expect("send unexpected extended CONNECT body dispatch");
        HttpResponse::ok("unexpected extended CONNECT body dispatch")
      })
    });

    let mut stream = TcpStream::connect(addr).expect("connect bounded h2 extended connect server");
    stream
      .set_read_timeout(Some(Duration::from_millis(200)))
      .expect("set bounded h2 extended connect read timeout");
    complete_h2_server_handshake_with_settings(
      &mut stream,
      &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &h2_extended_connect_headers(b"/bounded-ws", addr.to_string().as_bytes(), b"websocket"),
    );
    match case {
      "data" => write_h2_frame(
        &mut stream,
        H2_FRAME_DATA,
        H2_FLAG_END_STREAM,
        1,
        b"outside bounded scope",
      ),
      "trailers" => write_h2_frame(
        &mut stream,
        H2_FRAME_HEADERS,
        H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
        1,
        &h2_literal_new_name(b"x-late", b"outside-bounded-scope"),
      ),
      _ => unreachable!(),
    }
    stream
      .flush()
      .expect("flush bounded h2 extended CONNECT request");
    let _ = try_read_h2_frame(&mut stream);
    drop(stream);

    let err = handle
      .join()
      .expect("bounded h2 extended connect server thread")
      .expect_err("extended CONNECT body boundary must reject before handler");
    assert_eq!(io::ErrorKind::InvalidData, err.kind(), "{case}");
    assert!(
      err
        .to_string()
        .contains("HTTP/2 extended CONNECT request bodies are unsupported"),
      "unexpected bounded extended CONNECT rejection for {case}: {err}"
    );
    assert!(
      rx.try_recv().is_err(),
      "bounded extended CONNECT {case} must not dispatch"
    );
  }
}

#[test]
fn h2c_extended_connect_dispatches_after_subsequent_connect_protocol_negotiation() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 extended connect server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 extended connect addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("CONNECT", request.method());
        assert_eq!("/late-ws", request.target());
        assert_eq!(Some("websocket"), request.extended_connect_protocol());
        HttpResponse::ok("late extended connect dispatched")
      })
      .expect("serve late-negotiated h2 extended CONNECT")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 extended connect server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 extended connect read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
  );
  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_extended_connect_headers(b"/late-ws", addr.to_string().as_bytes(), b"websocket"),
  );
  stream
    .flush()
    .expect("flush late-negotiated h2 extended CONNECT request");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(
    b"late extended connect dispatched",
    response_body.payload.as_slice()
  );

  handle.join().expect("h2 extended connect server thread");
}

#[test]
fn h2c_ordinary_connect_stays_rejected_after_connect_protocol_negotiation() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 ordinary connect server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2 ordinary connect addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected ordinary CONNECT handler call");
      HttpResponse::ok("unexpected ordinary CONNECT")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 ordinary connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 ordinary connect read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_connect_headers(b"/ordinary-connect", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 ordinary CONNECT request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("h2 ordinary connect server thread")
    .expect_err("ordinary h2 CONNECT must reject before handler");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 prior-knowledge CONNECT/proxy tunneling is unsupported"),
    "unexpected ordinary h2 CONNECT rejection error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "ordinary h2 CONNECT must not be dispatched"
  );
}

#[test]
fn cross_crate_h2c_extended_connect_matrix_preserves_http11_handoffs() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind cross-crate h2 extended connect server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server
    .local_addr()
    .expect("cross-crate h2 extended connect addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("CONNECT", request.method());
        assert_eq!("/chat?room=blue", request.target());
        assert_eq!(Some(addr.to_string().as_str()), request.header("host"));
        assert_eq!(Some("websocket"), request.extended_connect_protocol());
        HttpResponse::ok("cross-crate extended connect")
      })
      .expect("serve cross-crate h2 extended CONNECT")
  });

  let response = HttpClient::new()
    .http2_extended_connect("websocket")
    .url(format!("http://{}/chat?room=blue", addr))
    .emit_http2_prior_knowledge()
    .expect("cross-crate h2 extended CONNECT response");
  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "cross-crate extended connect",
    response.body().string().unwrap()
  );
  handle
    .join()
    .expect("cross-crate h2 extended connect server thread");

  let listener = TcpListener::bind("127.0.0.1:0").expect("bind cross-crate body preflight peer");
  listener
    .set_nonblocking(true)
    .expect("set cross-crate body preflight peer nonblocking");
  let addr = listener
    .local_addr()
    .expect("cross-crate body preflight addr");
  let err = HttpClient::new()
    .http2_extended_connect("websocket")
    .url(format!("http://{}/chat", addr))
    .raw("outside bounded extended CONNECT scope")
    .emit_http2_prior_knowledge()
    .expect_err("extended CONNECT request body must be rejected before connecting");
  assert!(err.is_builder());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 extended CONNECT cannot send a request body"),
    "unexpected cross-crate body preflight error: {err}"
  );
  assert!(
    matches!(listener.accept(), Err(ref err) if err.kind() == io::ErrorKind::WouldBlock),
    "cross-crate extended CONNECT body preflight must not open a server connection"
  );

  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind missing-connect-protocol server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("missing setting addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected missing setting handler call");
      HttpResponse::ok("unexpected missing setting handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect missing-connect-protocol server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set missing-connect-protocol read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_extended_connect_headers(
      b"/missing-setting",
      addr.to_string().as_bytes(),
      b"websocket",
    ),
  );
  stream
    .flush()
    .expect("flush missing-connect-protocol request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("missing-connect-protocol server thread")
    .expect_err("missing SETTINGS_ENABLE_CONNECT_PROTOCOL must reject");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 extended CONNECT :protocol requires SETTINGS_ENABLE_CONNECT_PROTOCOL"),
    "unexpected missing-connect-protocol error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "missing connect protocol setting must not dispatch"
  );

  assert_h2_request_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    h2_connect_headers(b"/missing-protocol", b"example.test:443"),
    "HTTP/2 prior-knowledge CONNECT/proxy tunneling is unsupported",
  );

  assert_h2_request_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    h2_extended_connect_headers_with_regular_header_before_protocol(
      b"/invalid-pseudo-order",
      b"example.test:443",
    ),
    "HTTP/2 pseudo-header appeared after a regular header",
  );

  assert_h2_request_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
    h2_extended_connect_headers_with_duplicate_protocol(
      b"/duplicate-protocol",
      b"example.test:443",
    ),
    "duplicate HTTP/2 pseudo-header",
  );

  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind unsupported-protocol-metadata server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("unsupported metadata addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send((request.method().to_string(), request.target().to_string()))
        .expect("send unexpected unsupported protocol metadata handler call");
      HttpResponse::ok("unexpected unsupported protocol metadata handler")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect unsupported-protocol-metadata server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set unsupported-protocol-metadata read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 1),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_extended_connect_protocol_headers(
      b"/unsupported-protocol-metadata",
      addr.to_string().as_bytes(),
    ),
  );
  stream
    .flush()
    .expect("flush unsupported-protocol-metadata request");
  let _ = try_read_h2_frame(&mut stream);
  drop(stream);

  let err = handle
    .join()
    .expect("unsupported-protocol-metadata server thread")
    .expect_err("unsupported :protocol metadata must reject");
  assert_eq!(io::ErrorKind::InvalidData, err.kind());
  assert!(
    err
      .to_string()
      .contains("HTTP/2 extended CONNECT :protocol requires CONNECT"),
    "unexpected unsupported-protocol-metadata error: {err}"
  );
  assert!(
    rx.try_recv().is_err(),
    "unsupported :protocol metadata must not dispatch"
  );

  let server = rttp::Http::server("127.0.0.1:0").expect("bind HTTP/1.1 CONNECT handoff server");
  let addr = server.local_addr().expect("HTTP/1.1 CONNECT handoff addr");
  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("CONNECT", request.method());
        assert_eq!(addr.to_string(), request.target());
        rttp::server::HttpHandoff::connect(
          HttpResponse::new(200, "Connection Established"),
          |mut stream| {
            let mut ping = [0u8; 4];
            stream.read_exact(&mut ping)?;
            assert_eq!(b"ping", &ping);
            stream.write_all(b"pong")?;
            Ok(())
          },
        )
      })
      .expect("serve HTTP/1.1 CONNECT handoff")
  });

  let mut tunnel = HttpClient::new()
    .url(format!("http://{}", addr))
    .connect()
    .expect("HTTP/1.1 CONNECT handoff");
  assert_eq!(200, tunnel.response().code());
  tunnel
    .stream_mut()
    .write_all(b"ping")
    .expect("write HTTP/1.1 CONNECT tunnel bytes");
  let mut pong = [0u8; 4];
  tunnel
    .stream_mut()
    .read_exact(&mut pong)
    .expect("read HTTP/1.1 CONNECT tunnel bytes");
  assert_eq!(b"pong", &pong);
  handle.join().expect("HTTP/1.1 CONNECT handoff thread");

  let server = rttp::Http::server("127.0.0.1:0").expect("bind HTTP/1.1 Upgrade handoff server");
  let addr = server.local_addr().expect("HTTP/1.1 Upgrade handoff addr");
  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("GET", request.method());
        assert_eq!("/chat", request.target());
        assert_eq!(Some("websocket"), request.header("Upgrade"));
        rttp::server::HttpHandoff::upgrade(
          HttpResponse::new(101, "Switching Protocols")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket"),
          |mut stream| {
            stream.write_all(b"server-bytes")?;
            let mut client_bytes = [0u8; 12];
            stream.read_exact(&mut client_bytes)?;
            assert_eq!(b"client-bytes", &client_bytes);
            Ok(())
          },
        )
      })
      .expect("serve HTTP/1.1 Upgrade handoff")
  });

  let mut upgraded = HttpClient::new()
    .url(format!("http://{}/chat", addr))
    .header(("Connection", "Upgrade"))
    .header(("Upgrade", "websocket"))
    .upgrade()
    .expect("HTTP/1.1 Upgrade handoff");
  assert_eq!(101, upgraded.response().code());
  assert_eq!(
    Some(&"websocket".to_string()),
    upgraded.response().header_value("Upgrade")
  );
  let mut server_bytes = [0u8; 12];
  upgraded
    .stream_mut()
    .read_exact(&mut server_bytes)
    .expect("read HTTP/1.1 Upgrade bytes");
  assert_eq!(b"server-bytes", &server_bytes);
  upgraded
    .stream_mut()
    .write_all(b"client-bytes")
    .expect("write HTTP/1.1 Upgrade bytes");
  handle.join().expect("HTTP/1.1 Upgrade handoff thread");
}

#[test]
fn h2c_rejects_invalid_connect_protocol_settings_values_before_handler() {
  assert_malformed_settings_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_CONNECT_PROTOCOL, 2),
    0,
    None,
  );
}

#[test]
fn prior_knowledge_server_serves_two_complete_streams_on_one_socket2_connection() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        assert_eq!("HTTP/2", request.version());
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve two h2 streams")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/first", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/second", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 requests");

  let first_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, first_headers.frame_type);
  assert_eq!(1, first_headers.stream_id);
  let first_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, first_body.flags);
  assert_eq!(1, first_body.stream_id);
  assert_eq!(b"served /first", first_body.payload.as_slice());

  let second_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, second_headers.frame_type);
  assert_eq!(3, second_headers.stream_id);
  let second_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, second_body.flags);
  assert_eq!(3, second_body.stream_id);
  assert_eq!(b"served /second", second_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_ignores_reset_stream_blocked_on_response_flow_control() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_millis(500)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        assert_eq!("HTTP/2", request.version());
        match request.target() {
          "/reset" => HttpResponse::ok("x".repeat(H2_DEFAULT_INITIAL_WINDOW_SIZE + 1024)),
          "/second" => HttpResponse::ok("served second"),
          target => panic!("unexpected request target {target}"),
        }
      })
      .expect("serve h2 streams after reset")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/reset", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush reset h2 request");

  let reset_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, reset_headers.frame_type);
  assert_eq!(1, reset_headers.stream_id);

  let mut reset_body_len = 0;
  while reset_body_len < H2_DEFAULT_INITIAL_WINDOW_SIZE {
    let reset_body = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_DATA, reset_body.frame_type);
    assert_eq!(1, reset_body.stream_id);
    reset_body_len += reset_body.payload.len();
  }
  assert_eq!(H2_DEFAULT_INITIAL_WINDOW_SIZE, reset_body_len);

  write_h2_frame(
    &mut stream,
    H2_FRAME_RST_STREAM,
    0,
    1,
    &H2_ERROR_CANCEL.to_be_bytes(),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    0,
    &(H2_DEFAULT_INITIAL_WINDOW_SIZE as u32).to_be_bytes(),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/second", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush second h2 request");

  let second_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, second_headers.frame_type);
  assert_eq!(3, second_headers.stream_id);
  let second_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, second_body.flags);
  assert_eq!(3, second_body.stream_id);
  assert_eq!(b"served second", second_body.payload.as_slice());

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
fn h2c_connect_protocol_settings_metadata_is_ignored_for_ordinary_requests() {
  assert_connect_protocol_settings_accepted(false);
  assert_connect_protocol_settings_accepted(true);
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
fn cross_crate_h2c_wrapper_client_advertises_enable_push_zero_to_wrapper_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let server_addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let server_handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
        ))
        .expect("send captured wrapper h2 request");
        HttpResponse::ok("push disabled through wrappers")
      })
      .expect("serve captured wrapper h2 request");
  });
  let (proxy_addr, proxy_handle) = spawn_h2c_settings_capture_proxy(server_addr);

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{}/push-disabled", proxy_addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response through settings capture proxy");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "push disabled through wrappers",
    response.body().string().unwrap()
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "GET".to_string(),
      "/push-disabled".to_string()
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("captured wrapper h2 request")
  );
  assert_eq!(
    Some(0),
    proxy_handle.join().expect("settings capture proxy thread")
  );
  server_handle.join().expect("server thread");
}

#[test]
fn cross_crate_h2c_enable_push_settings_matrix_rejects_invalid_push_values() {
  let (addr, handle) = spawn_h2_peer_with_malformed_initial_settings();

  let client_error = rttp::Http::client()
    .get()
    .url(format!("http://{}/invalid-peer-push", addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrapper client must reject invalid peer SETTINGS_ENABLE_PUSH");

  assert!(client_error.to_string().contains("SETTINGS_ENABLE_PUSH"));
  handle.join().expect("invalid peer push settings thread");

  assert_malformed_settings_rejected_before_handler(
    &h2_setting(H2_SETTINGS_ENABLE_PUSH, 2),
    0,
    None,
  );
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
fn rttp_client_delete_prior_knowledge_interoperates_with_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
        ))
        .expect("send captured DELETE request");
        HttpResponse::ok("deleted by rttp server")
      })
      .expect("serve rttp_client h2 DELETE request");
  });

  let response = rttp_client::HttpClient::new()
    .delete()
    .url(format!("http://{}/matrix/delete?hard=true", addr))
    .emit_http2_prior_knowledge()
    .expect("rttp_client DELETE h2 response");

  let (request_version, request_method, request_target, request_body) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("receive captured DELETE request");
  assert_eq!("HTTP/2", request_version);
  assert_eq!("DELETE", request_method);
  assert_eq!("/matrix/delete?hard=true", request_target);
  assert!(request_body.is_empty());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!("deleted by rttp server", response.body().string().unwrap());

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_trace_prior_knowledge_interoperates_with_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
        ))
        .expect("send captured TRACE request");
        HttpResponse::ok("traced by rttp server").header("X-Trace-Handled", "socket2-h2c")
      })
      .expect("serve rttp_client h2 TRACE request");
  });

  let response = HttpClient::new()
    .trace()
    .url(format!("http://{}/matrix/trace?loopback=true", addr))
    .emit_http2_prior_knowledge()
    .expect("rttp_client TRACE h2 response");

  let (request_version, request_method, request_target, request_body) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("receive captured TRACE request");
  assert_eq!("HTTP/2", request_version);
  assert_eq!("TRACE", request_method);
  assert_eq!("/matrix/trace?loopback=true", request_target);
  assert!(request_body.is_empty());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"socket2-h2c".to_string()),
    response.header_value("x-trace-handled")
  );
  assert_eq!("traced by rttp server", response.body().string().unwrap());

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
fn cross_crate_h2c_ping_matrix_preserves_status_body_trailers_and_settings_bounds() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 ping matrix server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let server_addr = server.local_addr().expect("h2 ping matrix server addr");
  let (tx, rx) = mpsc::channel();
  let (proxy_addr, proxy_handle) = spawn_h2c_ping_matrix_proxy(server_addr);

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
        ))
        .expect("send h2 ping matrix request");
        HttpResponse::new(207, "Multi-Status")
          .body("ping-preserved body")
          .header("X-Ping-Matrix", "response")
          .trailer("X-Ping-Trailer", "trailer-ok")
      })
      .expect("serve h2 ping matrix request")
  });

  let config = Config::builder()
    .http2_header_table_size(0)
    .http2_max_frame_size(H2_DEFAULT_MAX_FRAME_SIZE)
    .build();
  let response = HttpClient::new()
    .config(config)
    .get()
    .url(format!("http://{}/ping-matrix?trailers=true", proxy_addr))
    .emit_http2_prior_knowledge()
    .expect("cross-crate h2c PING matrix response");

  assert_eq!(207, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"response".to_string()),
    response.header_value("x-ping-matrix")
  );
  assert_eq!("ping-preserved body", response.body().string().unwrap());
  assert_eq!(
    Some(&"trailer-ok".to_string()),
    response.trailer_value("x-ping-trailer")
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "GET".to_string(),
      "/ping-matrix?trailers=true".to_string(),
      Vec::new(),
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive h2 ping matrix request")
  );

  handle.join().expect("h2 ping matrix server thread");
  proxy_handle.join().expect("h2 ping matrix proxy thread");
}

#[test]
fn cross_crate_h2c_ping_matrix_preserves_extended_connect() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind h2 extended CONNECT ping matrix server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let server_addr = server
    .local_addr()
    .expect("h2 extended CONNECT ping matrix server addr");
  let (tx, rx) = mpsc::channel();
  let (proxy_addr, proxy_handle) = spawn_h2c_ping_matrix_proxy(server_addr);

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
          request.extended_connect_protocol().map(str::to_string),
          request.body().to_vec(),
        ))
        .expect("send h2 extended CONNECT ping matrix request");
        HttpResponse::ok("extended connect survived ping")
      })
      .expect("serve h2 extended CONNECT ping matrix request")
  });

  let response = HttpClient::new()
    .http2_extended_connect("websocket")
    .url(format!("http://{}/ping-matrix-ws?room=blue", proxy_addr))
    .emit_http2_prior_knowledge()
    .expect("cross-crate h2c extended CONNECT PING matrix response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "extended connect survived ping",
    response.body().string().unwrap()
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "CONNECT".to_string(),
      "/ping-matrix-ws?room=blue".to_string(),
      Some("websocket".to_string()),
      Vec::new(),
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("receive h2 extended CONNECT ping matrix request")
  );

  handle
    .join()
    .expect("h2 extended CONNECT ping matrix server thread");
  proxy_handle
    .join()
    .expect("h2 extended CONNECT ping matrix proxy thread");
}

#[test]
fn cross_crate_h2c_ping_matrix_rejects_malformed_ping_before_dispatch() {
  let cases: &[(u8, u32, &[u8])] = &[
    (0, 0, b"short"),
    (0, 1, b"bad-ping"),
    (H2_FLAG_ACK, 0, b"short"),
  ];

  for (flags, stream_id, payload) in cases {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind malformed h2 ping matrix server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("malformed h2 ping matrix server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server.accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send unexpected malformed h2 ping dispatch");
        HttpResponse::ok("unexpected malformed h2 ping dispatch")
      })
    });

    let mut stream = TcpStream::connect(addr).expect("connect malformed h2 ping server");
    stream
      .set_read_timeout(Some(Duration::from_millis(200)))
      .expect("set malformed h2 ping read timeout");
    complete_h2_server_handshake_with_settings(&mut stream, &[]);
    write_h2_frame(&mut stream, H2_FRAME_PING, *flags, *stream_id, payload);
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &h2_get_headers(b"/malformed-ping", addr.to_string().as_bytes()),
    );
    stream.flush().expect("flush malformed h2 ping matrix");
    let _ = try_read_h2_frame(&mut stream);
    drop(stream);

    let err = handle
      .join()
      .expect("malformed h2 ping server thread")
      .expect_err("malformed h2 ping must reject before handler");
    assert_eq!(io::ErrorKind::InvalidData, err.kind());
    assert!(
      err.to_string().contains("invalid HTTP/2 PING frame"),
      "unexpected malformed h2 ping rejection: {err}"
    );
    assert!(
      rx.try_recv().is_err(),
      "malformed PING must not dispatch a request"
    );
  }
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
fn wrapper_http2_prior_knowledge_ignores_unknown_extension_frames_around_successful_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_peer_request_handshake(&mut stream);

    write_h2_frame(
      &mut stream,
      H2_UNKNOWN_EXTENSION_FRAME,
      0,
      0,
      b"connection extension ignored",
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &[
        0x88, 0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
    write_h2_frame(
      &mut stream,
      H2_UNKNOWN_EXTENSION_FRAME,
      0,
      1,
      b"stream extension ignored before data",
    );
    write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"matrix");
    write_raw_h2_frame(
      &mut stream,
      H2_UNKNOWN_EXTENSION_FRAME,
      0,
      0x8000_0001,
      b"reserved stream-id high bit ignored",
    );
    write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b" body");
    write_h2_frame(
      &mut stream,
      H2_UNKNOWN_EXTENSION_FRAME,
      H2_FLAG_END_STREAM,
      1,
      b"extension end stream ignored",
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &h2_literal_new_name(b"x-matrix-trailer", b"kept"),
    );
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/extension-response-matrix", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with ignored extension frames");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
  assert_eq!("matrix body", response.body().string().unwrap());
  assert_eq!(
    Some(&"kept".to_string()),
    response.trailer_value("x-matrix-trailer")
  );

  handle.join().expect("extension response peer thread");
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
    .with_read_timeout(Some(Duration::from_secs(10)))
    .with_write_timeout(Some(Duration::from_secs(10)));
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
fn prior_knowledge_server_reassembles_request_headers_split_by_continuation() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-split-179").map(str::to_string),
        ))
        .expect("send parsed split h2 request header");
        HttpResponse::ok("split request headers")
      })
      .expect("serve split h2 request headers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let mut headers = h2_get_headers(b"/split-request-headers", addr.to_string().as_bytes());
  for index in 0..180 {
    headers.extend(h2_literal_new_name(
      format!("x-split-{index:03}").as_bytes(),
      format!("value-{index:03}-{}", "r".repeat(84)).as_bytes(),
    ));
  }

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_STREAM,
    1,
    &headers[..64],
  );
  let mut continuation_chunks = headers[64..].chunks(4096).peekable();
  while let Some(chunk) = continuation_chunks.next() {
    let flags = if continuation_chunks.peek().is_none() {
      H2_FLAG_END_HEADERS
    } else {
      0
    };
    write_h2_frame(&mut stream, H2_FRAME_CONTINUATION, flags, 1, chunk);
  }
  stream.flush().expect("flush split h2 request headers");

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(b"split request headers", response_body.payload.as_slice());
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "/split-request-headers".to_string(),
      Some(format!("value-179-{}", "r".repeat(84)))
    ),
    rx.recv().expect("receive parsed split h2 request header")
  );

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_splits_large_response_headers_to_peer_max_frame_size() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("/large-response-headers", request.target());
        let mut response = HttpResponse::ok("large response headers");
        for index in 0..420 {
          response = response.header(
            format!("X-Split-{index:03}"),
            format!("value-{index:03}-{}", "s".repeat(84)),
          );
        }
        response
      })
      .expect("serve large split h2 response headers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, H2_DEFAULT_MAX_FRAME_SIZE as u32),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/large-response-headers", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush h2 request");

  let first_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, first_headers.frame_type);
  assert_eq!(1, first_headers.stream_id);
  assert!(first_headers.payload.len() <= H2_DEFAULT_MAX_FRAME_SIZE);
  assert_eq!(0, first_headers.flags & H2_FLAG_END_HEADERS);

  let mut saw_final_continuation = false;
  for _ in 0..8 {
    let frame = read_h2_frame(&mut stream);
    assert_eq!(1, frame.stream_id);
    assert!(frame.payload.len() <= H2_DEFAULT_MAX_FRAME_SIZE);
    if frame.frame_type == H2_FRAME_CONTINUATION
      && frame.flags & H2_FLAG_END_HEADERS == H2_FLAG_END_HEADERS
    {
      saw_final_continuation = true;
      break;
    }
  }
  assert!(
    saw_final_continuation,
    "large response headers should be split with CONTINUATION frames"
  );

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(b"large response headers", response_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn prior_knowledge_server_rejects_continuation_on_stream_zero_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
    write_h2_frame(
      stream,
      H2_FRAME_CONTINUATION,
      H2_FLAG_END_HEADERS,
      0,
      b"orphaned header block",
    );
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &h2_get_headers(
        b"/ignored-after-bad-continuation",
        addr.to_string().as_bytes(),
      ),
    );
  });
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
fn wrapper_http2_prior_knowledge_dynamic_request_fields_reach_socket2_server_decoded() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-repeat").map(str::to_string),
          request.trailer("x-repeat").map(str::to_string),
        ))
        .expect("send parsed dynamic h2 request fields");
        HttpResponse::ok("decoded dynamic request fields")
      })
      .expect("serve dynamic h2 request fields");
  });

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{}/dynamic-request-fields", addr))
    .header(("X-Repeat", "same-value"))
    .trailer(("X-Repeat", "same-value"))
    .expect("configure repeated trailer")
    .raw("dynamic request body")
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response after dynamic request fields");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "decoded dynamic request fields",
    response.body().string().unwrap()
  );
  assert_eq!(
    (
      "HTTP/2".to_string(),
      "/dynamic-request-fields".to_string(),
      Some("same-value".to_string()),
      Some("same-value".to_string())
    ),
    rx.recv().expect("receive parsed dynamic h2 request fields")
  );

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_http2_prior_knowledge_dynamic_request_fields_reach_rttp_server_decoded() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-repeat").map(str::to_string),
          request.header("x-client-only").map(str::to_string),
          request.trailer("x-repeat").map(str::to_string),
          request.trailer("x-client-only").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send direct client dynamic h2 request fields");
        HttpResponse::ok("decoded direct client dynamic request fields")
      })
      .expect("serve direct client dynamic h2 request fields");
  });

  let response = rttp_client::HttpClient::new()
    .post()
    .url(format!("http://{}/direct-dynamic-request-fields", addr))
    .header(("X-Repeat", "same-value"))
    .header(("X-Client-Only", "header-value"))
    .trailer(("X-Repeat", "same-value"))
    .expect("configure repeated direct client trailer")
    .trailer(("X-Client-Only", "trailer-value"))
    .expect("configure direct client-only trailer")
    .raw("direct dynamic request body")
    .emit_http2_prior_knowledge()
    .expect("direct client h2 response after dynamic request fields");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "decoded direct client dynamic request fields",
    response.body().string().unwrap()
  );
  assert_eq!(
    (
      "POST".to_string(),
      "HTTP/2".to_string(),
      "/direct-dynamic-request-fields".to_string(),
      Some("same-value".to_string()),
      Some("header-value".to_string()),
      Some("same-value".to_string()),
      Some("trailer-value".to_string()),
      vec![
        ("x-repeat".to_string(), "same-value".to_string()),
        ("x-client-only".to_string(), "trailer-value".to_string()),
      ],
    ),
    rx.recv()
      .expect("receive direct client dynamic h2 request fields")
  );

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_http2_prior_knowledge_request_trailer_boundary_matrix_round_trips_application_trailers(
) {
  struct TrailerCase {
    name: &'static str,
    path: &'static str,
    trailers: &'static [(&'static str, &'static str)],
  }

  for case in [
    TrailerCase {
      name: "metadata",
      path: "/direct-request-trailers/metadata",
      trailers: &[
        ("X-Trace", "request-trailer-trace"),
        ("X-Upload-Status", "stored"),
      ],
    },
    TrailerCase {
      name: "integrity",
      path: "/direct-request-trailers/integrity",
      trailers: &[
        ("X-Upload-Checksum", "sha256-boundary"),
        ("X-Client-Metric", "42"),
      ],
    },
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.method().to_string(),
            request.version().to_string(),
            request.target().to_string(),
            request.body().to_vec(),
            request.trailers().to_vec(),
          ))
          .expect("send parsed direct h2 request trailer matrix");
          HttpResponse::ok(format!("accepted {}", case.name))
        })
        .expect("serve direct h2 request trailer matrix");
    });

    let mut client = rttp_client::HttpClient::new();
    client
      .post()
      .url(format!("http://{}{}", addr, case.path))
      .raw(format!("request body for {}", case.name));
    for trailer in case.trailers {
      client
        .trailer(*trailer)
        .expect("configure application request trailer");
    }
    let response = client
      .emit_http2_prior_knowledge()
      .expect("direct client h2 request trailer matrix response");

    assert_eq!("HTTP/2", response.version(), "{}", case.name);
    assert_eq!(200, response.code(), "{}", case.name);
    assert_eq!(
      format!("accepted {}", case.name),
      response.body().string().unwrap(),
      "{}",
      case.name
    );

    let expected_trailers = case
      .trailers
      .iter()
      .map(|(name, value)| (name.to_ascii_lowercase(), (*value).to_string()))
      .collect::<Vec<_>>();
    assert_eq!(
      (
        "POST".to_string(),
        "HTTP/2".to_string(),
        case.path.to_string(),
        format!("request body for {}", case.name).into_bytes(),
        expected_trailers,
      ),
      rx.recv()
        .expect("receive parsed direct h2 request trailer matrix"),
      "{}",
      case.name
    );

    handle.join().expect("server thread");
  }
}

#[test]
fn rttp_client_http2_prior_knowledge_header_list_size_matrix_respects_socket2_server_bound() {
  struct HeaderListCase {
    name: &'static str,
    path: &'static str,
    header_value_len: usize,
    trailer_value_len: usize,
    expect_success: bool,
  }

  for case in [
    HeaderListCase {
      name: "under-advertised-bound",
      path: "/bounded-header-list/under",
      header_value_len: 1024,
      trailer_value_len: 1024,
      expect_success: true,
    },
    HeaderListCase {
      name: "over-advertised-bound",
      path: "/bounded-header-list/over",
      header_value_len: 64 * 1024,
      trailer_value_len: 1024,
      expect_success: false,
    },
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      let result = server.accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.version().to_string(),
          request.target().to_string(),
          request.header("x-boundary-header").map(str::to_string),
          request.trailer("x-boundary-trailer").map(str::to_string),
        ))
        .expect("send parsed bounded h2 request metadata");
        HttpResponse::ok(format!("accepted {}", case.name))
      });

      if case.expect_success {
        result.expect("serve bounded h2 request metadata");
      } else {
        let error = result.expect_err("oversized h2 metadata should close before dispatch");
        assert!(
          matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof | io::ErrorKind::TimedOut | io::ErrorKind::ConnectionReset
          ),
          "unexpected server error for {}: {error}",
          case.name
        );
      }
    });

    let header_value = "h".repeat(case.header_value_len);
    let expected_header_value = header_value.clone();
    let trailer_value = "t".repeat(case.trailer_value_len);
    let expected_trailer_value = trailer_value.clone();
    let response = rttp_client::HttpClient::new()
      .post()
      .url(format!("http://{}{}", addr, case.path))
      .header(("X-Boundary-Header".to_string(), header_value))
      .trailer(("X-Boundary-Trailer".to_string(), trailer_value))
      .expect("configure bounded request trailer")
      .raw("bounded metadata body")
      .emit_http2_prior_knowledge();

    if case.expect_success {
      let response = response.expect("bounded h2 metadata response");
      assert_eq!("HTTP/2", response.version(), "{}", case.name);
      assert_eq!(
        format!("accepted {}", case.name),
        response.body().string().unwrap(),
        "{}",
        case.name
      );
      assert_eq!(
        (
          "POST".to_string(),
          "HTTP/2".to_string(),
          case.path.to_string(),
          Some(expected_header_value),
          Some(expected_trailer_value),
        ),
        rx.recv().expect("receive parsed bounded h2 metadata"),
        "{}",
        case.name
      );
    } else {
      let error = response.expect_err("oversized h2 metadata should be rejected");
      assert!(
        error
          .to_string()
          .contains("HTTP/2 peer SETTINGS_MAX_HEADER_LIST_SIZE"),
        "unexpected client error for {}: {error}",
        case.name
      );
      assert!(
        rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "oversized h2 metadata must not dispatch to the socket2 server handler"
      );
    }

    handle.join().expect("server thread");
  }
}

#[test]
fn rttp_client_http2_prior_knowledge_head_interoperates_with_socket2_h2c_server_matrix() {
  struct HeadCase {
    name: &'static str,
    path: &'static str,
    status_code: u16,
    expected_code: u32,
    reason: &'static str,
    response_body: &'static str,
  }

  for case in [
    HeadCase {
      name: "ok-with-suppressed-body",
      path: "/direct-head-ok",
      status_code: 200,
      expected_code: 200,
      reason: "OK",
      response_body: "metadata body suppressed for HEAD",
    },
    HeadCase {
      name: "no-content",
      path: "/direct-head-no-content",
      status_code: 204,
      expected_code: 204,
      reason: "No Content",
      response_body: "ignored no-content body",
    },
    HeadCase {
      name: "not-modified",
      path: "/direct-head-not-modified",
      status_code: 304,
      expected_code: 304,
      reason: "Not Modified",
      response_body: "ignored not-modified body",
    },
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.method().to_string(),
            request.version().to_string(),
            request.target().to_string(),
            request.header("x-head-matrix").map(str::to_string),
            request.body().to_vec(),
          ))
          .expect("send parsed direct h2 HEAD request");

          HttpResponse::new(case.status_code, case.reason)
            .header("X-Head-Matrix", case.name)
            .body(case.response_body)
        })
        .expect("serve direct h2 HEAD request");
    });

    let response = rttp_client::HttpClient::new()
      .head()
      .url(format!("http://{}{}", addr, case.path))
      .header(("X-Head-Matrix", case.name))
      .emit_http2_prior_knowledge()
      .expect("direct client h2 HEAD response from socket2 server");

    assert_eq!("HTTP/2", response.version(), "{}", case.name);
    assert_eq!(case.expected_code, response.code(), "{}", case.name);
    assert_eq!(
      Some(&case.name.to_string()),
      response.header_value("X-Head-Matrix"),
      "{}",
      case.name
    );
    assert_eq!(b"", response.body().binary(), "{}", case.name);
    assert_eq!(
      (
        "HEAD".to_string(),
        "HTTP/2".to_string(),
        case.path.to_string(),
        Some(case.name.to_string()),
        Vec::new(),
      ),
      rx.recv().expect("receive parsed direct h2 HEAD request"),
      "{}",
      case.name
    );

    handle.join().expect("server thread");
  }
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
fn socket2_h2_response_uses_dynamic_entries_for_repeated_smaller_fields() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("dynamic fields")
          .header("X-Dynamic-Response", "repeatable-response-value")
          .header("X-Dynamic-Response", "repeatable-response-value")
          .header("Trailer", "X-Dynamic-Trailer")
          .trailer("X-Dynamic-Trailer", "repeatable-trailer-value")
          .trailer("X-Dynamic-Trailer", "repeatable-trailer-value")
      })
      .expect("serve dynamic h2 response fields");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/dynamic-response-fields", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  assert_eq!(
    1,
    count_hpack_dynamic_indexed_fields(&response_headers.payload)
  );
  assert_eq!(
    1,
    count_hpack_incrementally_indexed_fields(&response_headers.payload)
  );

  let response_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_data.frame_type);
  assert_eq!(0, response_data.flags & H2_FLAG_END_STREAM);

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_trailers.flags
  );
  assert_eq!(
    1,
    count_hpack_dynamic_indexed_fields(&response_trailers.payload)
  );
  assert_eq!(
    1,
    count_hpack_incrementally_indexed_fields(&response_trailers.payload)
  );

  handle.join().expect("server thread");
}

#[test]
fn socket2_h2_response_dynamic_table_evicts_entries_at_default_size() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_value = "v".repeat(4020);

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("evicted dynamic fields")
          .header("X-Evict-Small", "small")
          .header("X-Evict-Large", &large_value)
          .header("X-Evict-Large", &large_value)
          .header("X-Evict-Small", "small")
      })
      .expect("serve h2 response with dynamic table eviction");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/dynamic-eviction", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  assert_eq!(
    1,
    count_hpack_dynamic_indexed_fields(&response_headers.payload)
  );
  assert_eq!(
    3,
    count_hpack_incrementally_indexed_fields(&response_headers.payload)
  );

  handle.join().expect("server thread");
}

#[test]
fn socket2_h2_response_honors_zero_peer_header_table_size() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("zero dynamic table")
          .header("X-Zero-Dynamic", "repeatable-response-value")
          .header("X-Zero-Dynamic", "repeatable-response-value")
          .header("Trailer", "X-Zero-Dynamic-Trailer")
          .trailer("X-Zero-Dynamic-Trailer", "repeatable-trailer-value")
          .trailer("X-Zero-Dynamic-Trailer", "repeatable-trailer-value")
      })
      .expect("serve h2 response with zero peer table size");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_HEADER_TABLE_SIZE, 0),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/zero-response-table", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(
    0,
    count_hpack_dynamic_indexed_fields(&response_headers.payload)
  );
  assert_eq!(
    0,
    count_hpack_incrementally_indexed_fields(&response_headers.payload)
  );

  let response_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_data.frame_type);
  assert_eq!(0, response_data.flags & H2_FLAG_END_STREAM);

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    0,
    count_hpack_dynamic_indexed_fields(&response_trailers.payload)
  );
  assert_eq!(
    0,
    count_hpack_incrementally_indexed_fields(&response_trailers.payload)
  );

  handle.join().expect("server thread");
}

#[test]
fn socket2_h2_response_honors_small_peer_header_table_size_for_headers_and_trailers() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("small dynamic table")
          .header("X-Small-Dynamic-Header", "repeatable-response-value")
          .header("X-Small-Dynamic-Header", "repeatable-response-value")
          .header("Trailer", "X-Small-Dynamic-Trailer")
          .trailer("X-Small-Dynamic-Trailer", "repeatable-trailer-value")
          .trailer("X-Small-Dynamic-Trailer", "repeatable-trailer-value")
      })
      .expect("serve h2 response with small peer table size");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_HEADER_TABLE_SIZE, 48),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/small-response-table", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(
    0,
    count_hpack_dynamic_indexed_fields(&response_headers.payload)
  );
  assert_eq!(
    0,
    count_hpack_incrementally_indexed_fields(&response_headers.payload)
  );

  let response_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_data.frame_type);
  assert_eq!(0, response_data.flags & H2_FLAG_END_STREAM);

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    0,
    count_hpack_dynamic_indexed_fields(&response_trailers.payload)
  );
  assert_eq!(
    0,
    count_hpack_incrementally_indexed_fields(&response_trailers.payload)
  );

  handle.join().expect("server thread");
}

#[test]
fn socket2_h2_response_applies_later_header_table_size_before_blocked_trailers() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("x".repeat(H2_DEFAULT_INITIAL_WINDOW_SIZE + 1))
          .header("X-Blocked-Dynamic", "repeatable-response-value")
          .header("X-Blocked-Dynamic", "repeatable-response-value")
          .header("Trailer", "X-Blocked-Dynamic-Trailer")
          .trailer("X-Blocked-Dynamic-Trailer", "repeatable-trailer-value")
          .trailer("X-Blocked-Dynamic-Trailer", "repeatable-trailer-value")
      })
      .expect("serve h2 response with later peer table size");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/blocked-response-table", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(
    1,
    count_hpack_dynamic_indexed_fields(&response_headers.payload)
  );
  assert_eq!(
    1,
    count_hpack_incrementally_indexed_fields(&response_headers.payload)
  );

  let mut response_body_len = 0;
  while response_body_len < H2_DEFAULT_INITIAL_WINDOW_SIZE {
    let response_body = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_DATA, response_body.frame_type);
    assert_eq!(1, response_body.stream_id);
    response_body_len += response_body.payload.len();
  }
  assert_eq!(H2_DEFAULT_INITIAL_WINDOW_SIZE, response_body_len);

  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_HEADER_TABLE_SIZE, 0),
  );
  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);
  assert!(settings_ack.payload.is_empty());

  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    0,
    &1u32.to_be_bytes(),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    1,
    &1u32.to_be_bytes(),
  );

  let final_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, final_data.frame_type);
  assert_eq!(1, final_data.payload.len());
  assert_eq!(0, final_data.flags & H2_FLAG_END_STREAM);

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_trailers.flags
  );
  assert_eq!(
    0,
    count_hpack_dynamic_indexed_fields(&response_trailers.payload)
  );
  assert_eq!(
    0,
    count_hpack_incrementally_indexed_fields(&response_trailers.payload)
  );

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_decodes_dynamic_response_fields_from_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("decoded dynamic response fields")
          .header("X-Dynamic-Response", "repeatable-response-value")
          .header("X-Dynamic-Response", "repeatable-response-value")
          .header("Trailer", "X-Dynamic-Trailer")
          .trailer("X-Dynamic-Trailer", "repeatable-trailer-value")
          .trailer("X-Dynamic-Trailer", "repeatable-trailer-value")
      })
      .expect("serve dynamic h2 response fields");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/dynamic-response-fields", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response with dynamic response fields");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    "decoded dynamic response fields",
    response.body().string().unwrap()
  );
  assert_eq!(
    vec![
      &"repeatable-response-value".to_string(),
      &"repeatable-response-value".to_string()
    ],
    response.header_values("X-Dynamic-Response")
  );
  assert_eq!(
    vec![
      &"repeatable-trailer-value".to_string(),
      &"repeatable-trailer-value".to_string()
    ],
    response.trailer_values("X-Dynamic-Trailer")
  );
  assert!(response.header_value("Trailer").is_none());
  assert!(response.header_value("X-Dynamic-Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_http2_prior_knowledge_decodes_rttp_server_dynamic_response_eviction() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let large_value = "v".repeat(4020);
  let expected_large_value = large_value.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("decoded direct client dynamic response eviction")
          .header("X-Evict-Small", "small")
          .header("X-Evict-Large", &large_value)
          .header("X-Evict-Large", &large_value)
          .header("X-Evict-Small", "small")
          .header("Trailer", "X-Evict-Trailer")
          .trailer("X-Evict-Trailer", "trailer-repeat")
          .trailer("X-Evict-Trailer", "trailer-repeat")
      })
      .expect("serve direct client dynamic h2 response eviction");
  });

  let response = rttp_client::HttpClient::new()
    .get()
    .url(format!("http://{}/direct-dynamic-response-eviction", addr))
    .emit_http2_prior_knowledge()
    .expect("direct client h2 response with dynamic response eviction");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(
    "decoded direct client dynamic response eviction",
    response.body().string().unwrap()
  );
  assert_eq!(
    vec![&"small".to_string(), &"small".to_string()],
    response.header_values("X-Evict-Small")
  );
  assert_eq!(
    vec![&expected_large_value, &expected_large_value],
    response.header_values("X-Evict-Large")
  );
  assert_eq!(
    vec![&"trailer-repeat".to_string(), &"trailer-repeat".to_string()],
    response.trailer_values("X-Evict-Trailer")
  );
  assert!(response.header_value("Trailer").is_none());
  assert!(response.header_value("X-Evict-Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_http2_prior_knowledge_settings_header_table_size_matrix_with_rttp_server() {
  struct MatrixCase {
    name: &'static str,
    local_header_table_size: usize,
    response_headers: Vec<(&'static str, &'static str)>,
    response_trailers: Vec<(&'static str, &'static str)>,
  }

  let cases = [
    MatrixCase {
      name: "zero",
      local_header_table_size: 0,
      response_headers: vec![
        ("X-Matrix-Zero", "repeatable-response-value"),
        ("X-Matrix-Zero", "repeatable-response-value"),
      ],
      response_trailers: vec![
        ("X-Matrix-Zero-Trailer", "repeatable-trailer-value"),
        ("X-Matrix-Zero-Trailer", "repeatable-trailer-value"),
      ],
    },
    MatrixCase {
      name: "small-eviction",
      local_header_table_size: 64,
      response_headers: vec![
        ("X-Matrix-One", "a"),
        ("X-Matrix-Two", "b"),
        ("X-Matrix-One", "a"),
        ("X-Matrix-Two", "b"),
      ],
      response_trailers: vec![("X-Matrix-Trailer-One", "a"), ("X-Matrix-Trailer-Two", "b")],
    },
  ];

  for case in cases {
    let case_name = case.name;
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("server addr");
    let request_field_name = format!("X-Matrix-Request-{}", case.name);
    let request_trailer_name = request_field_name.clone();
    let handler_request_field_name = request_field_name.clone();
    let handler_request_trailer_name = request_trailer_name.clone();
    let response_headers = case.response_headers.clone();
    let response_trailers = case.response_trailers.clone();
    let response_body = format!("matrix {}", case_name);
    let expected_response_body = response_body.clone();

    let handle = thread::spawn(move || {
      server
        .accept_one(move |request| {
          assert_eq!("HTTP/2", request.version());
          assert_eq!("POST", request.method());
          assert_eq!(
            format!("/settings-header-table-size/{}", case_name),
            request.target()
          );
          assert_eq!(
            Some("repeatable-request-value"),
            request.header(&handler_request_field_name)
          );
          assert_eq!(b"matrix request body", request.body());
          assert_eq!(
            Some("repeatable-request-value"),
            request.trailer(&handler_request_trailer_name)
          );

          let mut response = HttpResponse::ok(&response_body);
          for (name, value) in &response_headers {
            response = response.header(*name, *value);
          }
          for (name, _) in &response_trailers {
            response = response.header("Trailer", *name);
          }
          for (name, value) in &response_trailers {
            response = response.trailer(*name, *value);
          }
          response
        })
        .expect("serve header table size matrix request");
    });

    let config = Config::builder()
      .http2_header_table_size(case.local_header_table_size)
      .build();
    let response = rttp_client::HttpClient::new()
      .post()
      .url(format!(
        "http://{}/settings-header-table-size/{}",
        addr, case_name
      ))
      .config(config)
      .header((
        request_field_name.clone(),
        "repeatable-request-value".to_string(),
      ))
      .trailer((request_trailer_name, "repeatable-request-value".to_string()))
      .expect("configure matrix request trailer")
      .raw("matrix request body")
      .emit_http2_prior_knowledge()
      .expect("matrix h2 response");

    assert_eq!("HTTP/2", response.version());
    assert_eq!(200, response.code());
    assert_eq!(expected_response_body, response.body().string().unwrap());
    for (name, value) in case.response_headers {
      assert!(
        response
          .header_values(name)
          .iter()
          .any(|actual| *actual == value),
        "{name} response header value {value:?} missing for {} table",
        case_name
      );
    }
    for (name, value) in case.response_trailers {
      assert!(
        response
          .trailer_values(name)
          .iter()
          .any(|actual| *actual == value),
        "{name} response trailer value {value:?} missing for {} table",
        case_name
      );
    }
    assert!(response.header_value("Trailer").is_none());
    handle.join().expect("server thread");
  }
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
fn wrapper_http2_prior_knowledge_strips_connection_specific_headers_across_crates() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("connection").map(str::to_string),
          request.header("keep-alive").map(str::to_string),
          request.header("te").map(str::to_string),
          request.header("upgrade").map(str::to_string),
          request.header("x-boundary").map(str::to_string),
        ))
        .expect("send h2 connection header boundary observation");
        HttpResponse::ok("clean h2c response")
          .header("Connection", "close")
          .header("Keep-Alive", "timeout=5")
          .header("TE", "trailers")
          .header("Trailer", "X-Forbidden-Trailer")
          .header("Transfer-Encoding", "chunked")
          .header("Upgrade", "websocket")
          .header("X-Boundary-Response", "present")
      })
      .expect("serve h2 connection header boundary request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/connection-boundary", addr))
    .header(("Connection", "keep-alive"))
    .header(("Keep-Alive", "timeout=5"))
    .header(("TE", "trailers"))
    .header(("X-Boundary", "present"))
    .emit_http2_prior_knowledge()
    .expect("h2 connection header boundary response");

  assert_eq!("clean h2c response", response.body().string().unwrap());
  assert_eq!(
    (
      None,
      None,
      Some("trailers".to_string()),
      None,
      Some("present".to_string())
    ),
    rx.recv()
      .expect("receive h2 connection header boundary observation")
  );
  assert!(response.header_value("connection").is_none());
  assert!(response.header_value("keep-alive").is_none());
  assert!(response.header_value("te").is_none());
  assert!(response.header_value("trailer").is_none());
  assert!(response.header_value("transfer-encoding").is_none());
  assert!(response.header_value("upgrade").is_none());
  assert_eq!(
    Some(&"present".to_string()),
    response.header_value("x-boundary-response")
  );
  assert!(response.trailers().is_empty());

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

#[derive(Clone, Copy, Debug)]
enum CrossCrateTrailerTransport {
  PriorKnowledge,
  Upgrade,
}

impl CrossCrateTrailerTransport {
  fn name(self) -> &'static str {
    match self {
      Self::PriorKnowledge => "prior-knowledge",
      Self::Upgrade => "h2c-upgrade",
    }
  }

  fn request_stream_id(self) -> u32 {
    match self {
      Self::PriorKnowledge => 1,
      Self::Upgrade => 3,
    }
  }

  fn connect(self, addr: SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(addr).expect("connect cross-crate h2c server");
    stream
      .set_read_timeout(Some(Duration::from_millis(200)))
      .expect("set cross-crate h2c read timeout");
    stream
      .set_write_timeout(Some(Duration::from_secs(2)))
      .expect("set cross-crate h2c write timeout");
    match self {
      Self::PriorKnowledge => complete_h2_server_handshake_with_settings(&mut stream, &[]),
      Self::Upgrade => complete_h2c_upgrade(&mut stream, &addr.to_string(), &[]),
    }
    stream
  }
}

#[test]
fn cross_crate_http2_continuation_matrix_round_trips_large_headers_on_each_h2c_path() {
  for transport in [
    CrossCrateTrailerTransport::PriorKnowledge,
    CrossCrateTrailerTransport::Upgrade,
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind cross-crate CONTINUATION matrix server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("cross-crate CONTINUATION matrix addr");
    let request_header = format!("request-{}-{}", transport.name(), "r".repeat(18 * 1024));
    let expected_request_header = request_header.clone();
    let response_header = format!("response-{}-{}", transport.name(), "s".repeat(18 * 1024));
    let expected_response_header = response_header.clone();
    let path = format!("/cross-crate-continuation/{}", transport.name());
    let expected_path = path.clone();
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.version().to_string(),
            request.target().to_string(),
            request.header("x-large-request").map(str::to_string),
          ))
          .expect("send parsed cross-crate CONTINUATION request");

          HttpResponse::ok(format!("large header matrix via {}", transport.name()))
            .header("X-Large-Response", response_header)
        })
        .expect("serve cross-crate CONTINUATION matrix request");
    });

    let mut client = rttp_client::HttpClient::new();
    client
      .get()
      .url(format!("http://{}{}", addr, path))
      .header(("X-Large-Request".to_string(), request_header));
    let response = match transport {
      CrossCrateTrailerTransport::PriorKnowledge => client
        .emit_http2_prior_knowledge()
        .expect("cross-crate prior-knowledge large-header response"),
      CrossCrateTrailerTransport::Upgrade => client
        .emit_http2_upgrade()
        .expect("cross-crate h2c upgrade large-header response"),
    };

    assert_eq!("HTTP/2", response.version(), "{}", transport.name());
    assert_eq!(200, response.code(), "{}", transport.name());
    assert_eq!(
      format!("large header matrix via {}", transport.name()),
      response.body().string().unwrap(),
      "{}",
      transport.name()
    );
    assert_eq!(
      Some(&expected_response_header),
      response.header_value("X-Large-Response"),
      "{}",
      transport.name()
    );
    assert_eq!(
      (
        "HTTP/2".to_string(),
        expected_path,
        Some(expected_request_header),
      ),
      rx.recv()
        .expect("receive parsed cross-crate CONTINUATION request"),
      "{}",
      transport.name()
    );

    handle
      .join()
      .expect("cross-crate CONTINUATION matrix server thread");
  }
}

#[test]
fn cross_crate_http2_continuation_matrix_accepts_max_frame_size_boundaries_on_each_h2c_path() {
  for (transport, max_frame_size) in [
    (CrossCrateTrailerTransport::PriorKnowledge, 16_384),
    (CrossCrateTrailerTransport::Upgrade, 16_384),
    (CrossCrateTrailerTransport::PriorKnowledge, 16_777_215),
    (CrossCrateTrailerTransport::Upgrade, 16_777_215),
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind cross-crate frame-size boundary server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("cross-crate frame-size boundary addr");
    let path = format!(
      "/cross-crate-frame-size/{}/{}",
      transport.name(),
      max_frame_size
    );
    let expected_path = path.clone();
    let large_response_header = format!(
      "frame-size-{}-{}-{}",
      transport.name(),
      max_frame_size,
      "f".repeat(18 * 1024)
    );
    let expected_response_header = large_response_header.clone();
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((request.version().to_string(), request.target().to_string()))
            .expect("send cross-crate frame-size boundary request");
          HttpResponse::ok(format!(
            "frame-size boundary {} via {}",
            max_frame_size,
            transport.name()
          ))
          .header("X-Frame-Size-Boundary", large_response_header)
        })
        .expect("serve cross-crate frame-size boundary request");
    });

    let config = Config::builder()
      .http2_max_frame_size(max_frame_size)
      .build();
    let mut client = rttp_client::HttpClient::new();
    client
      .config(&config)
      .get()
      .url(format!("http://{}{}", addr, path));
    let response = match transport {
      CrossCrateTrailerTransport::PriorKnowledge => client
        .emit_http2_prior_knowledge()
        .expect("cross-crate prior-knowledge frame-size boundary response"),
      CrossCrateTrailerTransport::Upgrade => client
        .emit_http2_upgrade()
        .expect("cross-crate h2c upgrade frame-size boundary response"),
    };

    assert_eq!("HTTP/2", response.version(), "{}", transport.name());
    assert_eq!(
      format!(
        "frame-size boundary {} via {}",
        max_frame_size,
        transport.name()
      ),
      response.body().string().unwrap(),
      "{}",
      transport.name()
    );
    assert_eq!(
      Some(&expected_response_header),
      response.header_value("X-Frame-Size-Boundary"),
      "{} {}",
      transport.name(),
      max_frame_size
    );
    assert_eq!(
      ("HTTP/2".to_string(), expected_path),
      rx.recv()
        .expect("receive cross-crate frame-size boundary request"),
      "{} {}",
      transport.name(),
      max_frame_size
    );

    handle
      .join()
      .expect("cross-crate frame-size boundary server thread");
  }
}

#[test]
fn cross_crate_http2_continuation_matrix_rejects_interleaved_frames_on_each_h2c_path() {
  for transport in [
    CrossCrateTrailerTransport::PriorKnowledge,
    CrossCrateTrailerTransport::Upgrade,
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind interleaved CONTINUATION matrix server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("interleaved CONTINUATION matrix addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      let result = server.accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send unexpected interleaved CONTINUATION dispatch");
        HttpResponse::ok("unexpected interleaved continuation")
      });
      let error = result.expect_err("interleaved CONTINUATION must reject the h2c connection");
      assert!(
        matches!(
          error.kind(),
          io::ErrorKind::InvalidData
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::TimedOut
        ),
        "unexpected {} interleaved CONTINUATION error: {error}",
        transport.name()
      );
    });

    let mut stream = transport.connect(addr);
    let stream_id = transport.request_stream_id();
    write_h2_frame(
      &mut stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_STREAM,
      stream_id,
      &h2_get_headers(
        format!("/interleaved-continuation/{}", transport.name()).as_bytes(),
        addr.to_string().as_bytes(),
      ),
    );
    write_h2_frame(
      &mut stream,
      H2_FRAME_DATA,
      H2_FLAG_END_STREAM,
      stream_id,
      b"illegal while header block is open",
    );
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("shutdown interleaved CONTINUATION writer");

    handle
      .join()
      .expect("interleaved CONTINUATION matrix server thread");
    assert!(
      rx.try_recv().is_err(),
      "{} interleaved CONTINUATION must not dispatch",
      transport.name()
    );
  }
}

#[test]
fn cross_crate_http2_trailers_matrix_round_trips_after_streaming_data_on_each_h2c_path() {
  for transport in [
    CrossCrateTrailerTransport::PriorKnowledge,
    CrossCrateTrailerTransport::Upgrade,
  ] {
    let server = rttp::Http::server("127.0.0.1:0")
      .expect("bind cross-crate trailer matrix server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("cross-crate trailer matrix addr");
    let (tx, rx) = mpsc::channel();
    let request_body = format!("streaming DATA before trailers via {}", transport.name());
    let expected_request_body = request_body.clone().into_bytes();
    let path = format!("/cross-crate-trailers/{}", transport.name());
    let expected_path = path.clone();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.method().to_string(),
            request.version().to_string(),
            request.target().to_string(),
            request.body().to_vec(),
            request.header("x-request-trace").map(str::to_string),
            request.trailer("x-request-trace").map(str::to_string),
            request.trailer("x-upload-checksum").map(str::to_string),
            request.trailers().to_vec(),
          ))
          .expect("send parsed cross-crate h2 trailer matrix request");

          HttpResponse::ok(format!(
            "response DATA before trailers via {}",
            transport.name()
          ))
          .header("Trailer", "X-Response-Trace, X-Response-Checksum")
          .trailer("X-Response-Trace", format!("response-{}", transport.name()))
          .trailer("X-Response-Checksum", "sha256-response")
        })
        .expect("serve cross-crate h2 trailer matrix request");
    });

    let mut client = rttp_client::HttpClient::new();
    client
      .post()
      .url(format!("http://{}{}", addr, path))
      .header((
        "X-Request-Trace".to_string(),
        format!("header-{}", transport.name()),
      ))
      .content_type("application/octet-stream")
      .raw(request_body);
    client
      .trailer((
        "X-Request-Trace".to_string(),
        format!("trailer-{}", transport.name()),
      ))
      .expect("configure request trace trailer");
    client
      .trailer(("X-Upload-Checksum", "sha256-request"))
      .expect("configure request checksum trailer");

    let response = match transport {
      CrossCrateTrailerTransport::PriorKnowledge => client
        .emit_http2_prior_knowledge()
        .expect("cross-crate prior-knowledge h2 trailer response"),
      CrossCrateTrailerTransport::Upgrade => client
        .emit_http2_upgrade()
        .expect("cross-crate h2c upgrade trailer response"),
    };

    assert_eq!("HTTP/2", response.version(), "{}", transport.name());
    assert_eq!(200, response.code(), "{}", transport.name());
    assert_eq!(
      format!("response DATA before trailers via {}", transport.name()),
      response.body().string().unwrap(),
      "{}",
      transport.name()
    );
    assert!(
      response.header_value("Trailer").is_none(),
      "{}",
      transport.name()
    );
    assert_eq!(
      Some(&format!("response-{}", transport.name())),
      response.trailer_value("x-response-trace"),
      "{}",
      transport.name()
    );
    assert_eq!(
      Some(&"sha256-response".to_string()),
      response.trailer_value("X-RESPONSE-CHECKSUM"),
      "{}",
      transport.name()
    );

    assert_eq!(
      (
        "POST".to_string(),
        "HTTP/2".to_string(),
        expected_path,
        expected_request_body,
        Some(format!("header-{}", transport.name())),
        Some(format!("trailer-{}", transport.name())),
        Some("sha256-request".to_string()),
        vec![
          (
            "x-request-trace".to_string(),
            format!("trailer-{}", transport.name())
          ),
          (
            "x-upload-checksum".to_string(),
            "sha256-request".to_string()
          ),
        ],
      ),
      rx.recv()
        .expect("receive parsed cross-crate h2 trailer matrix request"),
      "{}",
      transport.name()
    );

    handle
      .join()
      .expect("cross-crate h2 trailer matrix server thread");
  }
}

#[test]
fn cross_crate_http2_trailers_matrix_rejects_request_trailer_pseudo_headers_before_h2c_dispatch() {
  for transport in [
    CrossCrateTrailerTransport::PriorKnowledge,
    CrossCrateTrailerTransport::Upgrade,
  ] {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind pseudo-trailer guard listener");
    listener
      .set_nonblocking(true)
      .expect("set pseudo-trailer guard listener nonblocking");
    let addr = listener.local_addr().expect("pseudo-trailer guard addr");

    let mut client = rttp_client::HttpClient::new();
    client
      .post()
      .url(format!(
        "http://{}/cross-crate-trailers/{}/pseudo-header",
        addr,
        transport.name()
      ))
      .raw("body that must not be dispatched");
    let error = client
      .trailer(rttp_client::types::Header::new(":path", "/hidden"))
      .expect_err("pseudo-header request trailer must be rejected");

    assert!(
      error.to_string().contains("Invalid request trailer"),
      "unexpected {} pseudo-header trailer error: {error}",
      transport.name()
    );
    assert!(
      matches!(listener.accept(), Err(ref err) if err.kind() == io::ErrorKind::WouldBlock),
      "{} pseudo-header trailer rejection must happen before connecting",
      transport.name()
    );
  }
}

#[test]
fn wrapper_http2_prior_knowledge_uploads_flow_controlled_body_with_trailers_to_socket2_server() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();
  let request_body = (0..H2_DEFAULT_INITIAL_WINDOW_SIZE + 32 * 1024)
    .map(|idx| b'a' + (idx % 26) as u8)
    .collect::<Vec<_>>();
  let expected_body = request_body.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.body().to_vec(),
          request.header("x-flow-control").map(str::to_string),
          request.trailer("x-flow-control").map(str::to_string),
          request.trailer("x-upload-checksum").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed flow-controlled h2 request");
        HttpResponse::ok("flow-controlled upload accepted")
      })
      .expect("serve flow-controlled h2 upload");
  });

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{}/flow-controlled-upload", addr))
    .header(("X-Flow-Control", "body-over-default-window"))
    .binary(request_body)
    .trailer(("X-Flow-Control", "request-trailer"))
    .expect("configure request flow-control trailer")
    .trailer(("X-Upload-Checksum", "window-updated"))
    .expect("configure request checksum trailer")
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 flow-controlled upload response");

  assert_eq!(
    (
      "POST".to_string(),
      "/flow-controlled-upload".to_string(),
      "HTTP/2".to_string(),
      expected_body,
      Some("body-over-default-window".to_string()),
      Some("request-trailer".to_string()),
      Some("window-updated".to_string()),
      vec![
        ("x-flow-control".to_string(), "request-trailer".to_string()),
        (
          "x-upload-checksum".to_string(),
          "window-updated".to_string()
        ),
      ],
    ),
    rx.recv()
      .expect("receive parsed flow-controlled h2 request")
  );
  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(b"flow-controlled upload accepted", response.body().binary());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_receives_flow_controlled_response_body_with_trailers_from_socket2_server(
) {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let response_body = (0..H2_DEFAULT_INITIAL_WINDOW_SIZE + 40 * 1024)
    .map(|idx| b'0' + (idx % 10) as u8)
    .collect::<Vec<_>>();
  let expected_body = response_body.clone();

  let handle = thread::spawn(move || {
    server
      .accept_one(move |request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!("/flow-controlled-download", request.target());
        HttpResponse::ok(response_body)
          .header("Trailer", "X-Flow-Control, X-Response-Checksum")
          .trailer("X-Flow-Control", "response-trailer")
          .trailer("X-Response-Checksum", "window-updated")
      })
      .expect("serve flow-controlled h2 download");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/flow-controlled-download", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 flow-controlled download response");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(200, response.code());
  assert_eq!(expected_body.len(), response.body().binary().len());
  assert_eq!(&expected_body[..64], &response.body().binary()[..64]);
  assert_eq!(
    &expected_body[expected_body.len() - 64..],
    &response.body().binary()[response.body().binary().len() - 64..]
  );
  assert!(
    expected_body.as_slice() == response.body().binary(),
    "flow-controlled response body bytes differ"
  );
  assert_eq!(
    Some(&"response-trailer".to_string()),
    response.trailer_value("x-flow-control")
  );
  assert_eq!(
    Some(&"window-updated".to_string()),
    response.trailer_value("X-RESPONSE-CHECKSUM")
  );
  assert!(response.header_value("Trailer").is_none());
  assert!(response.header_value("X-Flow-Control").is_none());

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
fn http2_feature_socket2_ignores_unknown_extension_frames_around_successful_request_response() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.header("x-matrix").map(str::to_string),
          request.body().to_vec(),
          request.trailer("x-matrix-trailer").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed extension-frame matrix request");

        HttpResponse::ok("extension matrix response")
          .header("X-Matrix-Response", "kept")
          .header("Trailer", "X-Matrix-Response-Trailer")
          .trailer("X-Matrix-Response-Trailer", "kept")
      })
      .expect("serve extension-frame matrix h2 request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  write_h2_frame(
    &mut stream,
    H2_UNKNOWN_EXTENSION_FRAME,
    0,
    0,
    b"ignored before request",
  );

  let mut headers = h2_post_headers(b"/extension-request-matrix", addr.to_string().as_bytes());
  headers.extend_from_slice(&h2_literal_new_name(b"x-matrix", b"kept"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );
  write_h2_frame(
    &mut stream,
    H2_UNKNOWN_EXTENSION_FRAME,
    0,
    1,
    b"ignored after headers",
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"matrix ");
  write_raw_h2_frame(
    &mut stream,
    H2_UNKNOWN_EXTENSION_FRAME,
    0,
    0x8000_0001,
    b"reserved-bit extension ignored",
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"upload");
  write_h2_frame(
    &mut stream,
    H2_UNKNOWN_EXTENSION_FRAME,
    H2_FLAG_END_STREAM,
    1,
    b"extension end stream ignored",
  );

  let trailers = h2_literal_new_name(b"x-matrix-trailer", b"kept");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &trailers,
  );

  let response_headers = loop {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type != H2_FRAME_WINDOW_UPDATE {
      break frame;
    }
  };
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(
    b"extension matrix response",
    response_body.payload.as_slice()
  );
  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_trailers.flags
  );
  assert_eq!(1, response_trailers.stream_id);

  assert_eq!(
    (
      "POST".to_string(),
      "/extension-request-matrix".to_string(),
      "HTTP/2".to_string(),
      Some("kept".to_string()),
      b"matrix upload".to_vec(),
      Some("kept".to_string()),
      vec![("x-matrix-trailer".to_string(), "kept".to_string())],
    ),
    rx.recv()
      .expect("receive parsed extension-frame matrix request")
  );

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_interleaved_request_data_and_trailers_stay_per_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send((
          request.target().to_string(),
          request.body().to_vec(),
          request.trailers().to_vec(),
        ))
        .expect("send parsed interleaved h2 request");
        HttpResponse::ok(format!("accepted {}", request.target()))
      })
      .expect("serve interleaved h2 requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let first_headers = h2_post_headers(b"/upload-one", addr.to_string().as_bytes());
  let second_headers = h2_post_headers(b"/upload-two", addr.to_string().as_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &first_headers,
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &second_headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"one-");
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 3, b"two-");
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 3, b"body");

  let first_trailers = h2_literal_new_name(b"x-stream-check", b"first");
  let second_trailers = h2_literal_new_name(b"x-stream-check", b"second");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &first_trailers,
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &second_trailers,
  );

  let mut completed_response_streams = read_h2_end_stream_data_streams(&mut stream, 2, 16);
  completed_response_streams.sort_unstable();
  assert_eq!(vec![1, 3], completed_response_streams);

  let mut received = vec![
    rx.recv().expect("receive first interleaved h2 request"),
    rx.recv().expect("receive second interleaved h2 request"),
  ];
  received.sort_by(|left, right| left.0.cmp(&right.0));
  assert_eq!(
    vec![
      (
        "/upload-one".to_string(),
        b"one-body".to_vec(),
        vec![("x-stream-check".to_string(), "first".to_string())],
      ),
      (
        "/upload-two".to_string(),
        b"two-body".to_vec(),
        vec![("x-stream-check".to_string(), "second".to_string())],
      ),
    ],
    received
  );

  handle.join().expect("server thread");
}

#[test]
fn http2_feature_socket2_bounded_h2c_multiplexing_stops_at_request_limit() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed bounded h2 request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve bounded h2 request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/ready", addr.to_string().as_bytes()),
  );
  stream.flush().expect("flush bounded h2 requests");

  assert_eq!(vec![1], read_h2_end_stream_data_streams(&mut stream, 1, 8));
  let shutdown = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_GOAWAY, shutdown.frame_type);
  assert_eq!(0, shutdown.flags);
  assert_eq!(0, shutdown.stream_id);
  assert_eq!(8, shutdown.payload.len());
  assert_eq!(
    1,
    u32::from_be_bytes(shutdown.payload[0..4].try_into().unwrap())
  );
  assert_eq!(
    0,
    u32::from_be_bytes(shutdown.payload[4..8].try_into().unwrap())
  );
  assert_eq!(
    "/ready",
    rx.recv().expect("receive bounded h2 request target")
  );

  handle.join().expect("server thread");
  assert!(
    rx.try_recv().is_err(),
    "no additional stream must be dispatched after request limit"
  );
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
fn http2_feature_socket2_rejects_connection_specific_request_headers_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
    let mut headers = h2_get_headers(b"/bad-connection-header", addr.to_string().as_bytes());
    headers.extend(h2_literal_new_name(b"connection", b"keep-alive"));
    headers.extend(h2_literal_new_name(b"keep-alive", b"timeout=5"));
    headers.extend(h2_literal_new_name(b"te", b"gzip"));
    headers.extend(h2_literal_new_name(b"transfer-encoding", b"chunked"));
    headers.extend(h2_literal_new_name(b"upgrade", b"websocket"));
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &headers,
    );
  });
}

#[test]
fn http2_feature_socket2_accepts_te_trailers_request_header() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.accept_one(|request| {
      tx.send(request.header("te").map(str::to_string))
        .expect("send observed te header");
      HttpResponse::ok("accepted te trailers")
    })
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set client read timeout");
  stream
    .set_write_timeout(Some(Duration::from_secs(2)))
    .expect("set client write timeout");
  complete_h2_server_handshake_with_settings(&mut stream, &[]);

  let mut headers = h2_get_headers(b"/te-trailers", addr.to_string().as_bytes());
  headers.extend(h2_literal_new_name(b"te", b"trailers"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &headers,
  );

  let response_body = (0..8)
    .map(|_| read_h2_frame(&mut stream))
    .find(|frame| {
      frame.frame_type == H2_FRAME_DATA
        && frame.stream_id == 1
        && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM
    })
    .expect("h2 response body");
  assert_eq!(b"accepted te trailers", response_body.payload.as_slice());
  assert_eq!(
    Some("trailers".to_string()),
    rx.recv().expect("receive observed te header")
  );
  handle
    .join()
    .expect("server thread")
    .expect("serve h2 request");
}

#[test]
fn http2_feature_socket2_rejects_non_trailers_te_request_header_before_handler() {
  assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
    let mut headers = h2_get_headers(b"/bad-te-header", addr.to_string().as_bytes());
    headers.extend(h2_literal_new_name(b"te", b"gzip"));
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
      1,
      &headers,
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

#[test]
fn http2_feature_socket2_rejects_hostile_request_trailer_boundary_matrix_before_handler() {
  struct HostileTrailerCase {
    name: &'static str,
    trailer_block: Vec<u8>,
  }

  for case in [
    HostileTrailerCase {
      name: "pseudo-path",
      trailer_block: h2_literal_indexed_name(4, b"/not-a-trailer"),
    },
    HostileTrailerCase {
      name: "routing-host",
      trailer_block: h2_literal_new_name(b"host", b"example.invalid"),
    },
    HostileTrailerCase {
      name: "routing-authorization",
      trailer_block: h2_literal_new_name(b"authorization", b"Bearer secret"),
    },
    HostileTrailerCase {
      name: "framing-content-length",
      trailer_block: h2_literal_new_name(b"content-length", b"4"),
    },
    HostileTrailerCase {
      name: "framing-transfer-encoding",
      trailer_block: h2_literal_new_name(b"transfer-encoding", b"chunked"),
    },
    HostileTrailerCase {
      name: "framing-te",
      trailer_block: h2_literal_new_name(b"te", b"trailers"),
    },
    HostileTrailerCase {
      name: "framing-trailer",
      trailer_block: h2_literal_new_name(b"trailer", b"x-late"),
    },
  ] {
    assert_malformed_h2_request_rejected_before_handler(|stream, addr| {
      let path = format!("/bad-request-trailer-{}", case.name);
      let headers = h2_post_headers(path.as_bytes(), addr.to_string().as_bytes());
      write_h2_frame(stream, H2_FRAME_HEADERS, H2_FLAG_END_HEADERS, 1, &headers);
      write_h2_frame(stream, H2_FRAME_DATA, 0, 1, b"body");
      write_h2_frame(
        stream,
        H2_FRAME_HEADERS,
        H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
        1,
        &case.trailer_block,
      );
    });
  }
}
