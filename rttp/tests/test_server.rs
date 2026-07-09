use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use rttp::server::{
  HttpConditionalMetadata, HttpConditionalRequestOutcome, HttpEntityTag, HttpResponse, Request,
};
use rttp_client::{Config, HttpClient};

const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
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
const H2_SETTINGS_ENABLE_PUSH: u16 = 0x2;
const H2_SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
const H2_SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
const H2_SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
const H2_SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x6;
const H2_DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;
const H2_MAX_FRAME_SIZE_LIMIT: usize = 16_777_215;
const H2_SERVER_MAX_HEADER_LIST_SIZE: u32 = 64 * 1024;
const H2_ERROR_REFUSED_STREAM: u32 = 0x7;

#[derive(Debug)]
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
  write_raw_h2_frame(stream, frame_type, flags, stream_id & 0x7fff_ffff, payload);
}

fn try_write_h2_frame(
  stream: &mut TcpStream,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> io::Result<()> {
  try_write_raw_h2_frame(stream, frame_type, flags, stream_id & 0x7fff_ffff, payload)
}

fn write_h2_header_block(stream: &mut TcpStream, stream_id: u32, end_stream: bool, block: &[u8]) {
  if block.len() <= H2_DEFAULT_MAX_FRAME_SIZE {
    let flags = H2_FLAG_END_HEADERS | if end_stream { H2_FLAG_END_STREAM } else { 0 };
    write_h2_frame(stream, H2_FRAME_HEADERS, flags, stream_id, block);
    return;
  }

  let mut chunks = block.chunks(H2_DEFAULT_MAX_FRAME_SIZE);
  write_h2_frame(
    stream,
    H2_FRAME_HEADERS,
    if end_stream { H2_FLAG_END_STREAM } else { 0 },
    stream_id,
    chunks.next().expect("first h2 header chunk"),
  );

  while let Some(chunk) = chunks.next() {
    let flags = if chunks.len() == 0 {
      H2_FLAG_END_HEADERS
    } else {
      0
    };
    write_h2_frame(stream, H2_FRAME_CONTINUATION, flags, stream_id, chunk);
  }
}

fn write_raw_h2_frame(
  stream: &mut TcpStream,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) {
  try_write_raw_h2_frame(stream, frame_type, flags, stream_id, payload).expect("write h2 frame");
}

fn try_write_raw_h2_frame(
  stream: &mut TcpStream,
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
  header[5..9].copy_from_slice(&stream_id.to_be_bytes());
  stream.write_all(&header)?;
  stream.write_all(payload)
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

fn try_read_h2_frame(stream: &mut TcpStream) -> io::Result<H2Frame> {
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

fn read_h2_frame_skipping_window_updates(stream: &mut TcpStream) -> H2Frame {
  loop {
    let frame = read_h2_frame(stream);
    if frame.frame_type != H2_FRAME_WINDOW_UPDATE {
      return frame;
    }
  }
}

fn h2_window_update_increment(frame: &H2Frame) -> u32 {
  assert_eq!(H2_FRAME_WINDOW_UPDATE, frame.frame_type);
  assert_eq!(4, frame.payload.len());
  u32::from_be_bytes([
    frame.payload[0] & 0x7f,
    frame.payload[1],
    frame.payload[2],
    frame.payload[3],
  ])
}

fn h2_window_update(increment: u32) -> [u8; 4] {
  increment.to_be_bytes()
}

fn read_h2_window_updates_until(
  stream: &mut TcpStream,
  expected_connection_increment: u32,
  expected_stream_increment: u32,
  stream_id: u32,
) {
  let mut connection_increment = 0;
  let mut stream_increment = 0;
  for _ in 0..16 {
    let update = read_h2_frame(stream);
    assert_eq!(H2_FRAME_WINDOW_UPDATE, update.frame_type);
    match update.stream_id {
      0 => connection_increment += h2_window_update_increment(&update),
      id if id == stream_id => stream_increment += h2_window_update_increment(&update),
      id => panic!("unexpected WINDOW_UPDATE stream id {id}"),
    }
    if connection_increment == expected_connection_increment
      && stream_increment == expected_stream_increment
    {
      return;
    }
  }
  panic!(
    "missing WINDOW_UPDATE totals: connection={connection_increment}, stream={stream_increment}"
  );
}

fn h2_goaway_last_stream_id(frame: &H2Frame) -> u32 {
  assert_eq!(H2_FRAME_GOAWAY, frame.frame_type);
  assert_eq!(0, frame.flags);
  assert_eq!(0, frame.stream_id);
  assert_eq!(8, frame.payload.len());
  assert_eq!(
    0,
    u32::from_be_bytes(frame.payload[4..8].try_into().unwrap())
  );
  u32::from_be_bytes(frame.payload[0..4].try_into().unwrap()) & 0x7fff_ffff
}

fn h2_setting_value(frame: &H2Frame, setting_id: u16) -> Option<u32> {
  assert_eq!(H2_FRAME_SETTINGS, frame.frame_type);
  assert_eq!(0, frame.flags);
  assert_eq!(0, frame.stream_id);
  assert_eq!(0, frame.payload.len() % 6);

  frame.payload.chunks_exact(6).find_map(|setting| {
    let id = u16::from_be_bytes(setting[..2].try_into().unwrap());
    let value = u32::from_be_bytes(setting[2..].try_into().unwrap());
    (id == setting_id).then_some(value)
  })
}

fn h2_literal_indexed_name(name_index: u8, value: &[u8]) -> Vec<u8> {
  assert!(value.len() < 128);
  let mut encoded = vec![name_index, value.len() as u8];
  encoded.extend_from_slice(value);
  encoded
}

fn h2_literal_indexed_name_sized(name_index: u8, value: &[u8]) -> Vec<u8> {
  let mut encoded = h2_encode_integer(name_index as usize, 4, 0);
  encoded.extend(h2_encode_integer(value.len(), 7, 0));
  encoded.extend_from_slice(value);
  encoded
}

fn h2_encode_integer(value: usize, prefix_bits: u8, first_byte_prefix: u8) -> Vec<u8> {
  let max_prefix = (1usize << prefix_bits) - 1;
  if value < max_prefix {
    return vec![first_byte_prefix | value as u8];
  }

  let mut encoded = vec![first_byte_prefix | max_prefix as u8];
  let mut remaining = value - max_prefix;
  while remaining >= 128 {
    encoded.push((remaining % 128) as u8 + 128);
    remaining /= 128;
  }
  encoded.push(remaining as u8);
  encoded
}

fn h2_indexed_header(index: usize) -> Vec<u8> {
  h2_encode_integer(index, 7, 0x80)
}

fn h2_table_size_update(size: usize) -> Vec<u8> {
  h2_encode_integer(size, 5, 0x20)
}

fn h2_literal_new_name_incremental(name: &[u8], value: &[u8]) -> Vec<u8> {
  assert!(name.len() < 128);
  assert!(value.len() < 128);
  let mut encoded = vec![0x40, name.len() as u8];
  encoded.extend_from_slice(name);
  encoded.push(value.len() as u8);
  encoded.extend_from_slice(value);
  encoded
}

fn h2_literal_indexed_name_huffman(name_index: u8, encoded_value: &[u8]) -> Vec<u8> {
  assert!(encoded_value.len() < 128);
  let mut encoded = vec![name_index, 0x80 | encoded_value.len() as u8];
  encoded.extend_from_slice(encoded_value);
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

fn h2_literal_new_name_huffman(encoded_name: &[u8], encoded_value: &[u8]) -> Vec<u8> {
  assert!(encoded_name.len() < 128);
  assert!(encoded_value.len() < 128);
  let mut encoded = vec![0, 0x80 | encoded_name.len() as u8];
  encoded.extend_from_slice(encoded_name);
  encoded.push(0x80 | encoded_value.len() as u8);
  encoded.extend_from_slice(encoded_value);
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

fn h2_delete_headers(path: &[u8], authority: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x86];
  headers.extend(h2_literal_indexed_name(2, b"DELETE"));
  headers.extend(h2_literal_indexed_name(4, path));
  headers.extend(h2_literal_indexed_name(1, authority));
  headers
}

fn h2_get_headers_with_huffman_path(encoded_path: &[u8]) -> Vec<u8> {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_indexed_name_huffman(4, encoded_path));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));
  headers
}

#[derive(Debug)]
struct CapturedHpackLiteral {
  name: Option<String>,
  name_huffman: bool,
  value: Option<String>,
  value_huffman: bool,
}

fn decode_captured_hpack_integer(block: &[u8], cursor: &mut usize, prefix_bits: u8) -> usize {
  assert!(*cursor < block.len(), "truncated HPACK integer");
  let max_prefix = (1usize << prefix_bits) - 1;
  let mut value = (block[*cursor] as usize) & max_prefix;
  *cursor += 1;
  if value < max_prefix {
    return value;
  }

  let mut shift = 0;
  loop {
    assert!(*cursor < block.len(), "truncated HPACK integer");
    let byte = block[*cursor];
    *cursor += 1;
    value += ((byte & 0x7f) as usize) << shift;
    if byte & 0x80 == 0 {
      return value;
    }
    shift += 7;
  }
}

fn decode_captured_hpack_string(block: &[u8], cursor: &mut usize) -> (Option<String>, bool) {
  assert!(*cursor < block.len(), "truncated HPACK string");
  let huffman = block[*cursor] & 0x80 == 0x80;
  let len = decode_captured_hpack_integer(block, cursor, 7);
  let end = *cursor + len;
  assert!(end <= block.len(), "truncated HPACK string");
  let value = if huffman {
    None
  } else {
    Some(String::from_utf8(block[*cursor..end].to_vec()).expect("raw HPACK string is UTF-8"))
  };
  *cursor = end;
  (value, huffman)
}

fn captured_hpack_literals(block: &[u8]) -> Vec<CapturedHpackLiteral> {
  let mut literals = Vec::new();
  let mut cursor = 0;
  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      decode_captured_hpack_integer(block, &mut cursor, 7);
      continue;
    }

    let prefix_bits = match byte {
      b if b & 0x40 == 0x40 => 6,
      b if b & 0xf0 == 0x00 => 4,
      b if b & 0xf0 == 0x10 => 4,
      _ => panic!("unsupported HPACK field representation: {byte:#x}"),
    };
    let name_index = decode_captured_hpack_integer(block, &mut cursor, prefix_bits);
    let (name, name_huffman) = if name_index == 0 {
      decode_captured_hpack_string(block, &mut cursor)
    } else {
      (None, false)
    };
    let (value, value_huffman) = decode_captured_hpack_string(block, &mut cursor);
    literals.push(CapturedHpackLiteral {
      name,
      name_huffman,
      value,
      value_huffman,
    });
  }
  literals
}

fn captured_hpack_literal<'a>(
  literals: &'a [CapturedHpackLiteral],
  name: &str,
) -> &'a CapturedHpackLiteral {
  literals
    .iter()
    .find(|literal| literal.name.as_deref() == Some(name))
    .unwrap_or_else(|| panic!("missing HPACK literal {name}"))
}

const H2_HUFFMAN_PATH: &[u8] = &[0x62, 0x7b, 0x65, 0x96, 0x91, 0xd4, 0xb5, 0x63, 0x4c, 0xff];
const H2_HUFFMAN_LOCALHOST: &[u8] = &[0xa0, 0xe4, 0x1d, 0x13, 0x9d, 0x09];
const H2_HUFFMAN_X_HUFFMAN: &[u8] = &[0xf2, 0xb4, 0xf6, 0xcb, 0x2d, 0x23, 0xab];
const H2_HUFFMAN_DECODED_HEADER: &[u8] =
  &[0x90, 0xa4, 0x3c, 0x85, 0x91, 0x69, 0xca, 0x39, 0x0b, 0x67];
const H2_HUFFMAN_X_HUFF_TRAILER: &[u8] = &[
  0xf2, 0xb4, 0xf6, 0xcb, 0x2a, 0xc9, 0xb0, 0x66, 0xa0, 0xb6, 0x7f,
];
const H2_HUFFMAN_DECODED_TRAILER: &[u8] = &[
  0x90, 0xa4, 0x3c, 0x85, 0x91, 0x64, 0xd8, 0x33, 0x50, 0x5b, 0x3f,
];
const H2_HUFFMAN_NON_UTF8: &[u8] = &[0xff, 0xff, 0xfb, 0xbf];

fn h2_setting(id: u16, value: u32) -> [u8; 6] {
  let mut setting = [0; 6];
  setting[..2].copy_from_slice(&id.to_be_bytes());
  setting[2..].copy_from_slice(&value.to_be_bytes());
  setting
}

fn complete_h2_server_handshake(stream: &mut TcpStream) {
  complete_h2_server_handshake_with_settings(stream, &[]);
}

fn complete_h2_server_handshake_with_settings(stream: &mut TcpStream, payload: &[u8]) {
  let _ = read_h2_server_settings_during_handshake(stream, payload);
}

fn read_h2_server_settings_during_handshake(stream: &mut TcpStream, payload: &[u8]) -> H2Frame {
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
  settings
}

fn assert_h2_header_block_split(
  stream: &mut TcpStream,
  max_frame_size: usize,
  first_flags: u8,
  final_flags: u8,
) {
  let first = read_h2_frame(stream);
  assert_eq!(H2_FRAME_HEADERS, first.frame_type);
  assert_eq!(first_flags, first.flags);
  assert_eq!(1, first.stream_id);
  assert!(first.payload.len() <= max_frame_size);
  assert!(
    first.flags & H2_FLAG_END_HEADERS == 0,
    "first HEADERS frame must not end an oversized header block"
  );

  let mut continuation_count = 0;
  loop {
    let frame = read_h2_frame(stream);
    assert_eq!(H2_FRAME_CONTINUATION, frame.frame_type);
    assert_eq!(1, frame.stream_id);
    assert!(frame.payload.len() <= max_frame_size);
    continuation_count += 1;

    if frame.flags & H2_FLAG_END_HEADERS == H2_FLAG_END_HEADERS {
      assert_eq!(final_flags, frame.flags);
      break;
    }

    assert_eq!(0, frame.flags);
  }
  assert!(continuation_count > 0);
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
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 trailers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

fn assert_invalid_h2_request_trailer_sequence_without_handler(
  write_sequence: impl FnOnce(&mut TcpStream, SocketAddr),
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
  write_sequence(&mut stream, addr);
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 trailer sequence should reject request");
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
fn server_handler_can_parse_request_cache_control_directives() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let cache_control = request
          .cache_control()
          .expect("valid cache-control should parse")
          .expect("cache-control header should be present");
        tx.send((
          cache_control.no_cache(),
          cache_control.max_age(),
          cache_control.only_if_cached(),
          cache_control.extensions()[0]
            .value()
            .map(ToString::to_string),
        ))
        .expect("send cache-control state");
        HttpResponse::ok("cached")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      b"GET /cached HTTP/1.1\r\nHost: localhost\r\nCache-Control: no-cache, max-age=5, only-if-cached, ext=\"a,b\"\r\n\r\n",
    )
    .expect("write request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    (true, Some(5), true, Some("a,b".to_string())),
    rx.recv().expect("receive cache-control state")
  );
  assert!(response.starts_with("HTTP/1.1 200 OK"));

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
fn rttp_client_h2c_max_frame_size_interoperates_with_socket2_server_matrix() {
  let min_response_body = vec![b's'; H2_DEFAULT_MAX_FRAME_SIZE * 2 + 7];
  let min_request_body = vec![b'c'; H2_DEFAULT_MAX_FRAME_SIZE * 2 + 11];
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();
  let min_response_body_for_server = min_response_body.clone();
  let min_request_body_for_client = min_request_body.clone();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, move |request| match request.target() {
        "/min-split" => {
          tx.send((
            request.target().to_string(),
            request.body().len(),
            request.body().first().copied(),
            request.body().last().copied(),
          ))
          .expect("send min split request details");
          HttpResponse::ok(min_response_body_for_server.clone())
        }
        "/max-single" => HttpResponse::ok(vec![b'm'; H2_DEFAULT_MAX_FRAME_SIZE + 19]),
        target => panic!("unexpected h2c target {target}"),
      })
      .expect("serve h2c max-frame-size matrix");
  });

  let min_response = HttpClient::new()
    .post()
    .url(format!("http://{}/min-split", addr))
    .binary(min_request_body_for_client)
    .emit_http2_prior_knowledge()
    .expect("min legal max-frame-size h2c response");
  assert_eq!("HTTP/2", min_response.version());
  assert_eq!(min_response_body, min_response.body().binary());
  assert_eq!(
    (
      "/min-split".to_string(),
      min_request_body.len(),
      Some(b'c'),
      Some(b'c')
    ),
    rx.recv().expect("receive min split request")
  );

  let max_frame_config = Config::builder()
    .http2_max_frame_size(H2_MAX_FRAME_SIZE_LIMIT)
    .build();
  let max_response = HttpClient::new()
    .get()
    .url(format!("http://{}/max-single", addr))
    .config(max_frame_config)
    .emit_http2_prior_knowledge()
    .expect("max legal max-frame-size h2c response");
  assert_eq!("HTTP/2", max_response.version());
  assert_eq!(
    vec![b'm'; H2_DEFAULT_MAX_FRAME_SIZE + 19],
    max_response.body().binary()
  );

  handle.join().expect("server thread");
}

#[test]
fn server_accepts_http2_prior_knowledge_delete_with_end_stream_once() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.method().to_string(), request.target().to_string()))
          .expect("send parsed h2 DELETE request");
        HttpResponse::ok("deleted").header("X-Delete-Handled", "once")
      })
      .expect("serve one h2 DELETE request");
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
    &h2_delete_headers(b"/resource", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);
  assert_eq!(
    0x88, response_headers.payload[0],
    "200 response must use the HTTP/2 static status entry"
  );

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"deleted", response_body.payload.as_slice());

  let request = rx.recv().expect("receive parsed h2 DELETE request");
  assert_eq!(("DELETE".to_string(), "/resource".to_string()), request);
  assert!(rx.try_recv().is_err(), "handler must run exactly once");

  handle.join().expect("server thread");
}

#[test]
fn server_sends_http2_prior_knowledge_goaway_after_bounded_request_limit() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(1, |request| {
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve bounded h2 request");
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
    &h2_get_headers(b"/bounded", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"served /bounded", response_body.payload.as_slice());

  let goaway = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(1, h2_goaway_last_stream_id(&goaway));

  handle.join().expect("server thread");
}

#[test]
fn server_sends_http2_prior_knowledge_goaway_with_last_processed_stream_id() {
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
  complete_h2_server_handshake(&mut stream);
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

  let first_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, first_headers.frame_type);
  assert_eq!(1, first_headers.stream_id);
  let first_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, first_body.flags);
  assert_eq!(1, first_body.stream_id);
  assert_eq!(b"served /first", first_body.payload.as_slice());

  let second_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, second_headers.frame_type);
  assert_eq!(3, second_headers.stream_id);
  let second_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, second_body.flags);
  assert_eq!(3, second_body.stream_id);
  assert_eq!(b"served /second", second_body.payload.as_slice());

  let goaway = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(3, h2_goaway_last_stream_id(&goaway));

  handle.join().expect("server thread");
}

#[test]
fn server_huffman_encodes_http2_response_header_and_trailer_literals_only_when_smaller() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("hpack body")
          .header("X", "aaaaaaaaaaaaaaaa")
          .header("Y", "x")
          .header("Trailer", "Z, W")
          .trailer("Z", "aaaaaaaaaaaaaaaa")
          .trailer("W", "x")
      })
      .expect("serve huffman h2 response");
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
    &h2_get_headers(b"/huffman-response", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);
  assert_eq!(
    0x88, response_headers.payload[0],
    "status must stay static-indexed"
  );

  let header_literals = captured_hpack_literals(&response_headers.payload);
  let compressed_header = captured_hpack_literal(&header_literals, "x");
  assert!(!compressed_header.name_huffman);
  assert!(compressed_header.value_huffman);
  assert_eq!(None, compressed_header.value);
  let raw_header = captured_hpack_literal(&header_literals, "y");
  assert!(!raw_header.name_huffman);
  assert!(!raw_header.value_huffman);
  assert_eq!(Some("x"), raw_header.value.as_deref());

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(0, response_body.flags);
  assert_eq!(b"hpack body", response_body.payload.as_slice());

  let response_trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_trailers.frame_type);
  assert_eq!(
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    response_trailers.flags
  );
  assert_eq!(1, response_trailers.stream_id);

  let trailer_literals = captured_hpack_literals(&response_trailers.payload);
  let compressed_trailer = captured_hpack_literal(&trailer_literals, "z");
  assert!(!compressed_trailer.name_huffman);
  assert!(compressed_trailer.value_huffman);
  assert_eq!(None, compressed_trailer.value);
  let raw_trailer = captured_hpack_literal(&trailer_literals, "w");
  assert!(!raw_trailer.name_huffman);
  assert!(!raw_trailer.value_huffman);
  assert_eq!(Some("x"), raw_trailer.value.as_deref());

  handle.join().expect("server thread");
}

#[test]
fn server_splits_large_http2_response_headers_to_peer_max_frame_size() {
  let max_frame_size = 16_384usize;
  let large_header_value = "a".repeat(max_frame_size * 2);
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| HttpResponse::ok("split headers").header("X-Large", large_header_value))
      .expect("serve split h2 headers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, max_frame_size as u32),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/large-response-headers", addr.to_string().as_bytes()),
  );

  assert_h2_header_block_split(&mut stream, max_frame_size, 0, H2_FLAG_END_HEADERS);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert!(response_body.payload.len() <= max_frame_size);
  assert_eq!(b"split headers", response_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn server_splits_large_http2_response_trailers_to_peer_max_frame_size() {
  let max_frame_size = 16_384usize;
  let large_trailer_value = "t".repeat(max_frame_size * 2);
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| {
        HttpResponse::ok("split trailers")
          .header("Trailer", "X-Large-Trailer")
          .trailer("X-Large-Trailer", large_trailer_value)
      })
      .expect("serve split h2 trailers");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_MAX_FRAME_SIZE, max_frame_size as u32),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/large-response-trailers", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);
  assert!(response_headers.payload.len() <= max_frame_size);

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(0, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert!(response_body.payload.len() <= max_frame_size);
  assert_eq!(b"split trailers", response_body.payload.as_slice());

  assert_h2_header_block_split(
    &mut stream,
    max_frame_size,
    H2_FLAG_END_STREAM,
    H2_FLAG_END_HEADERS,
  );

  handle.join().expect("server thread");
}

#[test]
fn server_pauses_http2_response_data_until_stream_window_update() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("abcdefghij")
          .header("Trailer", "X-Flow")
          .trailer("X-Flow", "complete")
      })
      .expect("serve flow-controlled h2 response");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 5),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/stream-window", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let first_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_data.frame_type);
  assert_eq!(1, first_data.stream_id);
  assert_eq!(b"abcde", first_data.payload.as_slice());
  assert_eq!(0, first_data.flags & H2_FLAG_END_STREAM);

  let blocked = try_read_h2_frame(&mut stream).expect_err("server must wait for stream credit");
  assert!(
    matches!(blocked.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    "unexpected read error: {blocked}"
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    1,
    &h2_window_update(5),
  );

  let second_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_data.frame_type);
  assert_eq!(1, second_data.stream_id);
  assert_eq!(b"fghij", second_data.payload.as_slice());
  assert_eq!(0, second_data.flags & H2_FLAG_END_STREAM);

  let trailers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, trailers.frame_type);
  assert_eq!(1, trailers.stream_id);
  assert_eq!(H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM, trailers.flags);

  handle.join().expect("server thread");
}

#[test]
fn server_pauses_http2_response_data_until_connection_window_update() {
  let body = vec![b'x'; 65_536];
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| HttpResponse::ok(body))
      .expect("serve connection-flow-controlled h2 response");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 70_000),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/connection-window", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let mut received = 0usize;
  while received < 65_535 {
    let frame = read_h2_frame(&mut stream);
    assert_eq!(H2_FRAME_DATA, frame.frame_type);
    assert_eq!(1, frame.stream_id);
    assert_eq!(0, frame.flags & H2_FLAG_END_STREAM);
    received += frame.payload.len();
  }
  assert_eq!(65_535, received);

  let blocked = try_read_h2_frame(&mut stream).expect_err("server must wait for connection credit");
  assert!(
    matches!(blocked.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    "unexpected read error: {blocked}"
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    0,
    &h2_window_update(1),
  );

  let final_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, final_data.frame_type);
  assert_eq!(1, final_data.stream_id);
  assert_eq!(1, final_data.payload.len());
  assert_eq!(H2_FLAG_END_STREAM, final_data.flags & H2_FLAG_END_STREAM);

  handle.join().expect("server thread");
}

#[test]
fn server_preserves_http2_request_frames_received_while_response_is_flow_control_blocked() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        if request.target() == "/queued" {
          tx.send((
            request.body().to_vec(),
            request.trailer("x-queued").map(str::to_string),
          ))
          .expect("send queued h2 request details");
          HttpResponse::ok("ok")
        } else {
          HttpResponse::ok("abcdefghij")
        }
      })
      .expect("serve multiplexed flow-controlled h2 responses");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 5),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/blocked", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let first_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_data.frame_type);
  assert_eq!(1, first_data.stream_id);
  assert_eq!(b"abcde", first_data.payload.as_slice());

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &h2_post_headers(b"/queued", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 3, b"saved");
  let trailers = h2_literal_new_name(b"x-queued", b"trailer");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &trailers,
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    1,
    &h2_window_update(5),
  );

  let second_data = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_data.frame_type);
  assert_eq!(1, second_data.stream_id);
  assert_eq!(b"fghij", second_data.payload.as_slice());
  assert_eq!(H2_FLAG_END_STREAM, second_data.flags & H2_FLAG_END_STREAM);

  let goaway = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(3, h2_goaway_last_stream_id(&goaway));

  let queued_response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, queued_response_headers.frame_type);
  assert_eq!(3, queued_response_headers.stream_id);
  let queued_response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, queued_response_body.frame_type);
  assert_eq!(3, queued_response_body.stream_id);
  assert_eq!(b"ok", queued_response_body.payload.as_slice());

  assert_eq!(
    (b"saved".to_vec(), Some("trailer".to_string())),
    rx.recv().expect("receive queued h2 request details")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_cancels_blocked_http2_response_after_reset_and_serves_next_stream() {
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
          .expect("send h2 target");
        if request.target() == "/reset-response" {
          HttpResponse::ok("abcdefghij")
        } else {
          HttpResponse::ok("after")
        }
      })
      .expect("serve reset h2 response sequence");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 5),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/reset-response", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let first_data = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_data.frame_type);
  assert_eq!(1, first_data.stream_id);
  assert_eq!(b"abcde", first_data.payload.as_slice());

  let blocked = try_read_h2_frame(&mut stream).expect_err("server must wait for stream credit");
  assert!(
    matches!(blocked.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    "unexpected read error: {blocked}"
  );

  write_h2_frame(&mut stream, H2_FRAME_RST_STREAM, 0, 1, &0u32.to_be_bytes());
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/after-reset", addr.to_string().as_bytes()),
  );

  let next_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, next_headers.frame_type);
  assert_eq!(3, next_headers.stream_id);
  let next_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, next_body.frame_type);
  assert_eq!(3, next_body.stream_id);
  assert_eq!(b"after", next_body.payload.as_slice());

  assert_eq!(
    vec!["/reset-response".to_string(), "/after-reset".to_string()],
    vec![
      rx.recv().expect("reset h2 target"),
      rx.recv().expect("next h2 target"),
    ]
  );

  handle.join().expect("server thread");
}

#[test]
fn server_applies_http2_window_update_to_other_stream_while_response_is_blocked() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        if request.target() == "/blocked" {
          HttpResponse::ok("aa")
        } else {
          HttpResponse::ok("ok")
        }
      })
      .expect("serve window-updated h2 responses");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 0),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/blocked", addr.to_string().as_bytes()),
  );

  let blocked_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, blocked_headers.frame_type);
  assert_eq!(1, blocked_headers.stream_id);
  let blocked = try_read_h2_frame(&mut stream).expect_err("server must wait for stream credit");
  assert!(
    matches!(blocked.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    "unexpected read error: {blocked}"
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/other", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    3,
    &h2_window_update(2),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    1,
    &h2_window_update(2),
  );

  let blocked_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, blocked_body.frame_type);
  assert_eq!(1, blocked_body.stream_id);
  assert_eq!(b"aa", blocked_body.payload.as_slice());

  let goaway = read_h2_frame(&mut stream);
  assert_eq!(3, h2_goaway_last_stream_id(&goaway));

  let other_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, other_headers.frame_type);
  assert_eq!(3, other_headers.stream_id);
  let other_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, other_body.frame_type);
  assert_eq!(3, other_body.stream_id);
  assert_eq!(b"ok", other_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn server_applies_http2_initial_window_settings_to_all_streams_while_response_is_blocked() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        if request.target() == "/blocked" {
          HttpResponse::ok("aa")
        } else {
          HttpResponse::ok("ok")
        }
      })
      .expect("serve settings-adjusted h2 responses");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_millis(200)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 0),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/blocked", addr.to_string().as_bytes()),
  );

  let blocked_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, blocked_headers.frame_type);
  assert_eq!(1, blocked_headers.stream_id);
  let blocked = try_read_h2_frame(&mut stream).expect_err("server must wait for stream credit");
  assert!(
    matches!(blocked.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
    "unexpected read error: {blocked}"
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/other", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 2),
  );

  let settings_ack = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_SETTINGS, settings_ack.frame_type);
  assert_eq!(H2_FLAG_ACK, settings_ack.flags);

  let blocked_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, blocked_body.frame_type);
  assert_eq!(1, blocked_body.stream_id);
  assert_eq!(b"aa", blocked_body.payload.as_slice());

  let goaway = read_h2_frame(&mut stream);
  assert_eq!(3, h2_goaway_last_stream_id(&goaway));

  let other_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, other_headers.frame_type);
  assert_eq!(3, other_headers.stream_id);
  let other_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, other_body.frame_type);
  assert_eq!(3, other_body.stream_id);
  assert_eq!(b"ok", other_body.payload.as_slice());

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_response_window_update_overflow() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || server.accept_one(|_| HttpResponse::ok("overflow response")));

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake_with_settings(
    &mut stream,
    &h2_setting(H2_SETTINGS_INITIAL_WINDOW_SIZE, 1),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_get_headers(b"/window-overflow", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_WINDOW_UPDATE,
    0,
    1,
    &h2_window_update(0x7fff_ffff),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, &[]);

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("overflowing WINDOW_UPDATE should reject response");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(
    error.to_string().contains("overflow"),
    "unexpected error: {error}"
  );
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
fn server_sends_http2_window_updates_while_consuming_large_request_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.body().len(),
          request.trailer("x-large-body").map(str::to_string),
        ))
        .expect("send large h2 request details");
        HttpResponse::ok("large body")
      })
      .expect("serve large h2 request");
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
    &h2_post_headers(b"/large-body", addr.to_string().as_bytes()),
  );

  let first_chunk = vec![b'a'; 65_535];
  for chunk in first_chunk.chunks(H2_DEFAULT_MAX_FRAME_SIZE) {
    write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, chunk);
  }

  read_h2_window_updates_until(&mut stream, 65_535, 65_535, 1);

  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"tail");
  let trailers = h2_literal_new_name(b"x-large-body", b"complete");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &trailers,
  );

  read_h2_window_updates_until(&mut stream, 4, 4, 1);

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(b"large body", response_body.payload.as_slice());

  assert_eq!(
    (65_539, Some("complete".to_string())),
    rx.recv().expect("receive large h2 body details")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_zero_window_update_increment() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_WINDOW_UPDATE, 0, 0, &0u32.to_be_bytes());
}

#[test]
fn server_ignores_http2_prior_knowledge_unknown_connection_frames_around_bounded_streams() {
  const H2_FRAME_UNKNOWN_EXTENSION: u8 = 0xb;

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
          .expect("send h2 target");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve bounded h2 requests around unknown frames");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(
    &mut stream,
    H2_FRAME_UNKNOWN_EXTENSION,
    0,
    0,
    b"connection metadata",
  );
  assert!(
    rx.try_recv().is_err(),
    "connection-level extension frame must not dispatch a request"
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/first-after-extension", addr.to_string().as_bytes()),
  );

  let first_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, first_headers.frame_type);
  assert_eq!(1, first_headers.stream_id);
  let first_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, first_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, first_body.flags);
  assert_eq!(1, first_body.stream_id);
  assert_eq!(
    b"served /first-after-extension",
    first_body.payload.as_slice()
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_UNKNOWN_EXTENSION,
    0,
    0,
    b"between streams",
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    3,
    &h2_get_headers(b"/second-after-extension", addr.to_string().as_bytes()),
  );

  let second_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, second_headers.frame_type);
  assert_eq!(3, second_headers.stream_id);
  let second_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, second_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, second_body.flags);
  assert_eq!(3, second_body.stream_id);
  assert_eq!(
    b"served /second-after-extension",
    second_body.payload.as_slice()
  );

  assert_eq!(
    "/first-after-extension",
    rx.recv().expect("receive first h2 target")
  );
  assert_eq!(
    "/second-after-extension",
    rx.recv().expect("receive second h2 target")
  );

  let goaway = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(3, h2_goaway_last_stream_id(&goaway));

  handle.join().expect("server thread");
}

#[test]
fn server_ignores_http2_prior_knowledge_unknown_stream_frames_without_exposing_payload() {
  const H2_FRAME_UNKNOWN_EXTENSION: u8 = 0xb;

  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.body().to_vec(),
          request.trailers().to_vec(),
          request.trailer("x-upload-status").map(str::to_string),
        ))
        .expect("send parsed h2 request");
        HttpResponse::ok("clean response")
      })
      .expect("serve h2 request with unknown stream frames");
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
    &h2_post_headers(b"/upload-with-extension", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"visible ");
  write_h2_frame(
    &mut stream,
    H2_FRAME_UNKNOWN_EXTENSION,
    0,
    1,
    b"not request body",
  );
  write_raw_h2_frame(&mut stream, H2_FRAME_DATA, 0, 0x8000_0001, b"body");
  write_raw_h2_frame(
    &mut stream,
    H2_FRAME_UNKNOWN_EXTENSION,
    0,
    0x8000_0001,
    &h2_literal_new_name(b"x-hidden", b"not-a-trailer"),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_literal_new_name(b"x-upload-status", b"stored"),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"clean response", response_body.payload.as_slice());

  assert_eq!(
    (
      b"visible body".to_vec(),
      vec![("x-upload-status".to_string(), "stored".to_string())],
      Some("stored".to_string()),
    ),
    rx.recv().expect("receive parsed h2 request")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_ignores_http2_prior_knowledge_priority_before_request_headers() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send h2 target");
        HttpResponse::ok("priority ignored")
      })
      .expect("serve h2 request after priority frame");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  write_h2_frame(&mut stream, H2_FRAME_PRIORITY, 0, 1, &[0, 0, 0, 0, 16]);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/priority-before-headers", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"priority ignored", response_body.payload.as_slice());

  assert_eq!(
    "/priority-before-headers",
    rx.recv().expect("receive h2 target")
  );

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_priority_with_zero_stream_id() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_PRIORITY, 0, 0, &[0, 0, 0, 0, 16]);
}

#[test]
fn server_rejects_http2_prior_knowledge_priority_with_invalid_payload_length() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_PRIORITY, 0, 1, &[0, 0, 0, 0]);
}

#[test]
fn server_rejects_http2_prior_knowledge_push_promise_without_handler() {
  let mut payload = 2u32.to_be_bytes().to_vec();
  payload.extend(h2_get_headers(b"/promised", b"localhost"));
  assert_invalid_h2_frame_without_handler(H2_FRAME_PUSH_PROMISE, H2_FLAG_END_HEADERS, 1, &payload);
}

#[test]
fn server_rejects_http2_prior_knowledge_connection_push_promise_without_handler() {
  let mut payload = 2u32.to_be_bytes().to_vec();
  payload.extend(h2_get_headers(b"/promised", b"localhost"));
  assert_invalid_h2_frame_without_handler(H2_FRAME_PUSH_PROMISE, H2_FLAG_END_HEADERS, 0, &payload);
}

#[test]
fn server_rejects_http2_prior_knowledge_connection_specific_request_headers_without_handler() {
  for (name, value) in [
    (b"connection".as_slice(), b"close".as_slice()),
    (b"keep-alive".as_slice(), b"timeout=5".as_slice()),
    (b"proxy-connection".as_slice(), b"keep-alive".as_slice()),
    (b"transfer-encoding".as_slice(), b"chunked".as_slice()),
    (b"upgrade".as_slice(), b"h2c".as_slice()),
    (b"Connection".as_slice(), b"close".as_slice()),
    (b"TE".as_slice(), b"gzip".as_slice()),
  ] {
    let mut headers = h2_get_headers(b"/forbidden-headers", b"localhost");
    headers.extend(h2_literal_new_name(name, value));

    assert_invalid_h2_headers_without_handler(&headers);
  }
}

#[test]
fn server_accepts_http2_prior_knowledge_te_trailers_request_header() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("te").map(str::to_string))
          .expect("send h2 TE header");
        HttpResponse::ok("accepted")
      })
      .expect("serve h2 TE trailers request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  let mut headers = h2_get_headers(b"/te-trailers", addr.to_string().as_bytes());
  headers.extend(h2_literal_new_name(b"te", b"trailers"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &headers,
  );

  let response_headers = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"accepted", response_body.payload.as_slice());

  assert_eq!(Some("trailers".to_string()), rx.recv().expect("TE header"));
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_invalid_te_request_header_without_handler() {
  let mut headers = h2_get_headers(b"/invalid-te", b"localhost");
  headers.extend(h2_literal_new_name(b"te", b"gzip"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_truncated_push_promise_without_handler() {
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

  let declared_length = 4usize;
  let mut header = [0; 9];
  header[0] = ((declared_length >> 16) & 0xff) as u8;
  header[1] = ((declared_length >> 8) & 0xff) as u8;
  header[2] = (declared_length & 0xff) as u8;
  header[3] = H2_FRAME_PUSH_PROMISE;
  header[4] = H2_FLAG_END_HEADERS;
  header[5..9].copy_from_slice(&1u32.to_be_bytes());
  stream.write_all(&header).expect("write h2 frame head");
  stream
    .write_all(&[0, 0])
    .expect("write truncated h2 frame payload");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("truncated PUSH_PROMISE should reject connection");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_data_beyond_receive_window() {
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
    &h2_post_headers(b"/window-exhaustion", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_DATA,
    H2_FLAG_END_STREAM,
    1,
    &vec![b'x'; 65_536],
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("DATA beyond receive window should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_decodes_http2_prior_knowledge_hpack_huffman_request_headers_and_trailers() {
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
          request.header("x-huffman").map(str::to_string),
          request.trailer("x-huff-trailer").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed huffman h2 request");
        HttpResponse::ok("decoded")
      })
      .expect("serve huffman h2 request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  let mut headers = vec![0x83, 0x86];
  headers.extend(h2_literal_indexed_name_huffman(4, H2_HUFFMAN_PATH));
  headers.extend(h2_literal_indexed_name_huffman(1, H2_HUFFMAN_LOCALHOST));
  headers.extend(h2_literal_new_name_huffman(
    H2_HUFFMAN_X_HUFFMAN,
    H2_HUFFMAN_DECODED_HEADER,
  ));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"huffman body");

  let trailers = h2_literal_new_name_huffman(H2_HUFFMAN_X_HUFF_TRAILER, H2_HUFFMAN_DECODED_TRAILER);
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
  assert_eq!(b"decoded", response_body.payload.as_slice());

  assert_eq!(
    (
      "POST".to_string(),
      "/huffman-path".to_string(),
      Some("localhost".to_string()),
      Some("decoded-header".to_string()),
      Some("decoded-trailer".to_string()),
      vec![("x-huff-trailer".to_string(), "decoded-trailer".to_string())]
    ),
    rx.recv().expect("receive parsed huffman h2 request")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_decodes_http2_prior_knowledge_hpack_dynamic_headers_and_trailers() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request.header("x-dynamic").map(str::to_string),
          request.trailer("x-dynamic").map(str::to_string),
          request.trailers().to_vec(),
        ))
        .expect("send parsed h2 dynamic request");
        HttpResponse::ok("decoded")
      })
      .expect("serve h2 dynamic request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  let mut headers = vec![0x83, 0x86];
  headers.extend(h2_literal_indexed_name(4, b"/dynamic"));
  headers.extend(h2_literal_indexed_name(1, addr.to_string().as_bytes()));
  headers.extend(h2_literal_new_name_incremental(b"x-dynamic", b"from-table"));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &headers,
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_indexed_header(62),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(H2_FLAG_END_HEADERS, response_headers.flags);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"decoded", response_body.payload.as_slice());

  assert_eq!(
    (
      "/dynamic".to_string(),
      Some("from-table".to_string()),
      Some("from-table".to_string()),
      vec![("x-dynamic".to_string(), "from-table".to_string())]
    ),
    rx.recv().expect("receive parsed h2 dynamic request")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_evicts_http2_prior_knowledge_hpack_dynamic_entries() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("x-second").map(str::to_string))
          .expect("send parsed h2 eviction request");
        HttpResponse::ok("evicted")
      })
      .expect("serve h2 eviction request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  complete_h2_server_handshake(&mut stream);

  let mut headers = h2_table_size_update(64);
  headers.extend([0x82, 0x86]);
  headers.extend(h2_literal_indexed_name(4, b"/evict"));
  headers.extend(h2_literal_indexed_name(1, addr.to_string().as_bytes()));
  headers.extend(h2_literal_new_name_incremental(b"x-first", b"one"));
  headers.extend(h2_literal_new_name_incremental(
    b"x-second",
    b"abcdefghijklmnopqrstu",
  ));
  headers.extend(h2_indexed_header(62));
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &headers,
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);

  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"evicted", response_body.payload.as_slice());

  assert_eq!(
    Some("abcdefghijklmnopqrstu".to_string()),
    rx.recv().expect("receive parsed h2 eviction request")
  );
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_invalid_hpack_dynamic_index_before_handler() {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_indexed_name(4, b"/bad-dynamic-index"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));
  headers.extend(h2_indexed_header(62));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_invalid_hpack_dynamic_size_update_before_handler() {
  let mut headers = h2_table_size_update(4097);
  headers.extend([0x82, 0x86]);
  headers.extend(h2_literal_indexed_name(4, b"/bad-dynamic-size"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_truncated_hpack_dynamic_index_before_handler() {
  let mut headers = vec![0x82, 0x86];
  headers.extend(h2_literal_indexed_name(4, b"/truncated-dynamic-index"));
  headers.extend(h2_literal_indexed_name(1, b"localhost"));
  headers.push(0xff);

  assert_invalid_h2_headers_without_handler(&headers);
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
fn server_rejects_http2_prior_knowledge_hpack_huffman_request_trailer_before_handler() {
  assert_invalid_h2_request_trailers_without_handler(&h2_literal_new_name_huffman(
    H2_HUFFMAN_X_HUFF_TRAILER,
    &[0xff],
  ));
}

#[test]
fn server_rejects_http2_prior_knowledge_forbidden_request_trailer_name() {
  for name in [
    b"cache-control".as_slice(),
    b"max-forwards",
    b"content-length".as_slice(),
    b"transfer-encoding",
    b"connection",
    b"keep-alive",
    b"proxy-connection",
    b"host",
    b"te",
    b"trailer",
    b"upgrade",
  ] {
    assert_invalid_h2_request_trailers_without_handler(&h2_literal_new_name(name, b"blocked"));
  }
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
fn server_rejects_http2_prior_knowledge_interleaved_frame_during_request_trailers() {
  assert_invalid_h2_request_trailer_sequence_without_handler(|stream, addr| {
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &h2_post_headers(b"/interleaved-trailers", addr.to_string().as_bytes()),
    );
    write_h2_frame(stream, H2_FRAME_DATA, 0, 1, b"body");
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_STREAM,
      1,
      &h2_literal_new_name(b"x-trace", b"partial"),
    );
    write_h2_frame(stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"interleaved");
  });
}

#[test]
fn server_rejects_http2_prior_knowledge_request_trailer_continuation_on_wrong_stream() {
  assert_invalid_h2_request_trailer_sequence_without_handler(|stream, addr| {
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &h2_post_headers(b"/wrong-stream-trailers", addr.to_string().as_bytes()),
    );
    write_h2_frame(stream, H2_FRAME_DATA, 0, 1, b"body");
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_STREAM,
      1,
      &h2_literal_new_name(b"x-trace", b"partial"),
    );
    write_h2_frame(
      stream,
      H2_FRAME_CONTINUATION,
      H2_FLAG_END_HEADERS,
      3,
      &h2_literal_new_name(b"x-second", b"wrong-stream"),
    );
  });
}

#[test]
fn server_rejects_http2_prior_knowledge_eof_during_request_trailers() {
  assert_invalid_h2_request_trailer_sequence_without_handler(|stream, addr| {
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_HEADERS,
      1,
      &h2_post_headers(b"/eof-during-trailers", addr.to_string().as_bytes()),
    );
    write_h2_frame(stream, H2_FRAME_DATA, 0, 1, b"body");
    write_h2_frame(
      stream,
      H2_FRAME_HEADERS,
      H2_FLAG_END_STREAM,
      1,
      &h2_literal_new_name(b"x-trace", b"partial"),
    );
  });
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
fn server_rejects_http2_prior_knowledge_even_client_stream_id() {
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
    2,
    &h2_get_headers(b"/even-stream", addr.to_string().as_bytes()),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("even h2 stream id should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(
    error
      .to_string()
      .contains("invalid HTTP/2 client stream id"),
    "unexpected error: {error}"
  );
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_lower_client_stream_id_after_higher_stream() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.serve_requests(2, |request| {
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
    3,
    &h2_get_headers(b"/higher", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/lower", addr.to_string().as_bytes()),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("lower h2 stream id should reject connection");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(
    error
      .to_string()
      .contains("invalid HTTP/2 client stream id"),
    "unexpected error: {error}"
  );
  assert_eq!(
    "/higher",
    rx.recv()
      .expect("higher h2 stream can be served before lower stream violation")
      .target()
  );
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_reused_closed_client_stream_id() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server.serve_requests(2, |request| {
      tx.send(request.target().to_string())
        .expect("send h2 target");
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
    &h2_get_headers(b"/first", addr.to_string().as_bytes()),
  );
  let _ = read_h2_frame_skipping_window_updates(&mut stream);
  let _ = read_h2_frame_skipping_window_updates(&mut stream);
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/reused", addr.to_string().as_bytes()),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("reused closed h2 stream id should reject connection");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(
    error
      .to_string()
      .contains("HTTP/2 frame arrived after stream close"),
    "unexpected error: {error}"
  );
  assert_eq!("/first", rx.recv().expect("first h2 request"));
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
fn server_rejects_http2_prior_knowledge_connect_without_path_or_scheme_before_handler() {
  let mut headers = h2_literal_indexed_name(2, b"CONNECT");
  headers.extend(h2_literal_indexed_name(1, b"example.test:443"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_connect_authority_form_before_handler() {
  let mut headers = h2_literal_indexed_name(2, b"CONNECT");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, b"example.test:443"));
  headers.extend(h2_literal_indexed_name(1, b"example.test:443"));

  assert_invalid_h2_headers_without_handler(&headers);
}

#[test]
fn server_rejects_http2_prior_knowledge_hpack_huffman_eos_symbol_before_handler() {
  assert_invalid_h2_headers_without_handler(&h2_get_headers_with_huffman_path(&[
    0xff, 0xff, 0xff, 0xff,
  ]));
}

#[test]
fn server_rejects_http2_prior_knowledge_hpack_huffman_invalid_padding_before_handler() {
  assert_invalid_h2_headers_without_handler(&h2_get_headers_with_huffman_path(&[0x00]));
}

#[test]
fn server_rejects_http2_prior_knowledge_hpack_huffman_truncated_code_before_handler() {
  assert_invalid_h2_headers_without_handler(&h2_get_headers_with_huffman_path(&[0xfe]));
}

#[test]
fn server_rejects_http2_prior_knowledge_hpack_huffman_overlong_padding_before_handler() {
  assert_invalid_h2_headers_without_handler(&h2_get_headers_with_huffman_path(&[0xff]));
}

#[test]
fn server_rejects_http2_prior_knowledge_hpack_huffman_non_utf8_before_handler() {
  assert_invalid_h2_headers_without_handler(&h2_get_headers_with_huffman_path(H2_HUFFMAN_NON_UTF8));
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
fn server_accepts_http2_prior_knowledge_options_origin_form_with_end_stream_once() {
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
        .expect("send h2 origin-form OPTIONS request");
        HttpResponse::ok("resource options")
      })
      .expect("serve h2 origin-form OPTIONS request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  complete_h2_server_handshake(&mut stream);
  let mut headers = h2_literal_indexed_name(2, b"OPTIONS");
  headers.push(0x86);
  headers.extend(h2_literal_indexed_name(4, b"/resource"));
  headers.extend(h2_literal_indexed_name(1, addr.to_string().as_bytes()));
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
  assert_eq!(
    0x88, response_headers.payload[0],
    "200 response must use the HTTP/2 static status entry"
  );

  let response_body = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(1, response_body.stream_id);
  assert_eq!(b"resource options", response_body.payload.as_slice());

  let request = rx.recv().expect("receive h2 origin-form OPTIONS request");
  assert_eq!(
    (
      "OPTIONS".to_string(),
      "/resource".to_string(),
      Some(addr.to_string())
    ),
    request
  );
  assert!(rx.try_recv().is_err(), "handler must run exactly once");
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
fn server_advertises_bounded_http2_prior_knowledge_concurrent_stream_limit() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |_| HttpResponse::ok("unused"))
      .expect_err("client closes before any h2 request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  let settings = read_h2_server_settings_during_handshake(&mut stream, &[]);
  assert_eq!(
    Some(2),
    h2_setting_value(&settings, H2_SETTINGS_MAX_CONCURRENT_STREAMS)
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  assert_eq!(
    ErrorKind::UnexpectedEof,
    handle.join().expect("server thread").kind()
  );
}

#[test]
fn server_advertises_bounded_http2_prior_knowledge_header_list_size() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| HttpResponse::ok("unused"))
      .expect_err("client closes before any h2 request")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  let settings = read_h2_server_settings_during_handshake(&mut stream, &[]);
  assert_eq!(
    Some(H2_SERVER_MAX_HEADER_LIST_SIZE),
    h2_setting_value(&settings, H2_SETTINGS_MAX_HEADER_LIST_SIZE)
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  assert_eq!(
    ErrorKind::UnexpectedEof,
    handle.join().expect("server thread").kind()
  );
}

#[test]
fn server_accepts_http2_prior_knowledge_enable_push_zero_and_advertises_existing_settings() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string())
          .expect("send h2 target");
        HttpResponse::ok("push disabled")
      })
      .expect("serve h2 request with enable-push zero")
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  let settings =
    read_h2_server_settings_during_handshake(&mut stream, &h2_setting(H2_SETTINGS_ENABLE_PUSH, 0));
  assert_eq!(
    Some(1),
    h2_setting_value(&settings, H2_SETTINGS_MAX_CONCURRENT_STREAMS)
  );
  assert_eq!(
    Some(H2_DEFAULT_MAX_FRAME_SIZE as u32),
    h2_setting_value(&settings, H2_SETTINGS_MAX_FRAME_SIZE)
  );
  assert_eq!(
    Some(H2_SERVER_MAX_HEADER_LIST_SIZE),
    h2_setting_value(&settings, H2_SETTINGS_MAX_HEADER_LIST_SIZE)
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    1,
    &h2_get_headers(b"/enable-push-zero", addr.to_string().as_bytes()),
  );

  let response_headers = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_HEADERS, response_headers.frame_type);
  assert_eq!(1, response_headers.stream_id);
  let response_body = read_h2_frame_skipping_window_updates(&mut stream);
  assert_eq!(H2_FRAME_DATA, response_body.frame_type);
  assert_eq!(H2_FLAG_END_STREAM, response_body.flags);
  assert_eq!(b"push disabled", response_body.payload.as_slice());

  assert_eq!("/enable-push-zero", rx.recv().expect("h2 target"));
  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_enable_push_two_before_handler() {
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
  stream.write_all(H2_PREFACE).expect("write h2 preface");
  write_h2_frame(
    &mut stream,
    H2_FRAME_SETTINGS,
    0,
    0,
    &h2_setting(H2_SETTINGS_ENABLE_PUSH, 2),
  );
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown h2 write");

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("invalid h2 enable-push setting should reject connection");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(error.to_string().contains("SETTINGS_ENABLE_PUSH"));
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_oversized_request_headers_before_handler() {
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

  let mut headers = h2_post_headers(b"/oversized-headers", addr.to_string().as_bytes());
  headers.extend(h2_literal_indexed_name_sized(
    2,
    &vec![b'a'; H2_SERVER_MAX_HEADER_LIST_SIZE as usize],
  ));
  write_h2_header_block(&mut stream, 1, true, &headers);
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("oversized h2 request headers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
}

#[test]
fn server_rejects_http2_prior_knowledge_oversized_request_trailers_before_handler() {
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
    &h2_post_headers(b"/oversized-trailers", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, 0, 1, b"body");

  let trailers =
    h2_literal_indexed_name_sized(2, &vec![b'a'; H2_SERVER_MAX_HEADER_LIST_SIZE as usize]);
  let split = trailers.len() / 2;
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_STREAM,
    1,
    &trailers[..split],
  );
  match try_write_h2_frame(
    &mut stream,
    H2_FRAME_CONTINUATION,
    H2_FLAG_END_HEADERS,
    1,
    &trailers[split..],
  ) {
    Ok(()) => {}
    Err(err)
      if matches!(
        err.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
      ) => {}
    Err(err) => panic!("write oversized h2 trailer continuation: {err}"),
  }
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let error = handle
    .join()
    .expect("server thread")
    .expect_err("oversized h2 request trailers should reject request");
  assert_eq!(ErrorKind::InvalidData, error.kind());
  assert!(rx.try_recv().is_err());
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
fn server_allows_http2_prior_knowledge_interleaving_up_to_advertised_stream_limit() {
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
      .expect("serve bounded h2 requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect h2 server");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set h2 read timeout");
  let settings = read_h2_server_settings_during_handshake(&mut stream, &[]);
  assert_eq!(
    Some(2),
    h2_setting_value(&settings, H2_SETTINGS_MAX_CONCURRENT_STREAMS)
  );

  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    1,
    &h2_post_headers(b"/first", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &h2_post_headers(b"/second", addr.to_string().as_bytes()),
  );
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 3, b"two");
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"one");

  let mut response_streams = Vec::new();
  while response_streams.len() < 2 {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      response_streams.push(frame.stream_id);
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

  handle.join().expect("server thread");
}

#[test]
fn server_rejects_http2_prior_knowledge_streams_above_active_limit_before_handler() {
  let server = rttp::Http::server("127.0.0.1:0")
    .expect("bind server")
    .with_read_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send allowed h2 target");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve allowed h2 streams")
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
    &h2_post_headers(b"/first", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS,
    3,
    &h2_post_headers(b"/second", addr.to_string().as_bytes()),
  );
  write_h2_frame(
    &mut stream,
    H2_FRAME_HEADERS,
    H2_FLAG_END_HEADERS | H2_FLAG_END_STREAM,
    5,
    &h2_get_headers(b"/over-limit", addr.to_string().as_bytes()),
  );

  let reset = read_h2_frame(&mut stream);
  assert_eq!(H2_FRAME_RST_STREAM, reset.frame_type);
  assert_eq!(0, reset.flags);
  assert_eq!(5, reset.stream_id);
  assert_eq!(
    H2_ERROR_REFUSED_STREAM.to_be_bytes(),
    reset.payload.as_slice()
  );
  assert!(rx.try_recv().is_err());

  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 1, b"");
  write_h2_frame(&mut stream, H2_FRAME_DATA, H2_FLAG_END_STREAM, 3, b"");

  let mut response_streams = Vec::new();
  while response_streams.len() < 2 {
    let frame = read_h2_frame(&mut stream);
    if frame.frame_type == H2_FRAME_DATA && frame.flags & H2_FLAG_END_STREAM == H2_FLAG_END_STREAM {
      response_streams.push(frame.stream_id);
    }
  }
  response_streams.sort_unstable();
  assert_eq!(vec![1, 3], response_streams);

  let mut targets = vec![
    rx.recv().expect("first allowed h2 target"),
    rx.recv().expect("second allowed h2 target"),
  ];
  targets.sort();
  assert_eq!(vec!["/first".to_string(), "/second".to_string()], targets);
  assert!(rx.try_recv().is_err());

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
fn server_rejects_http2_prior_knowledge_reset_stream_with_zero_stream_id() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_RST_STREAM, 0, 0, &0u32.to_be_bytes());
}

#[test]
fn server_rejects_http2_prior_knowledge_reset_stream_with_invalid_payload_length() {
  assert_invalid_h2_frame_without_handler(H2_FRAME_RST_STREAM, 0, 1, &[0, 0, 0]);
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
      "Trailer: X-Trace, X-Signature\r\n",
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
fn conditional_request_helpers_evaluate_if_none_match_get_as_not_modified() {
  let metadata = HttpConditionalMetadata::new()
    .entity_tag(HttpEntityTag::strong("abc"))
    .last_modified(UNIX_EPOCH + Duration::from_secs(784_111_777));

  let outcome = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-None-Match: W/\"different\", \"abc\"\r\n",
      "\r\n",
    ),
    metadata,
  );

  assert_eq!(HttpConditionalRequestOutcome::NotModified, outcome);
}

#[test]
fn conditional_request_helpers_evaluate_if_none_match_unsafe_method_as_precondition_failed() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("abc"));

  let outcome = conditional_outcome_for(
    concat!(
      "PUT /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-None-Match: *\r\n",
      "Content-Length: 0\r\n",
      "\r\n",
    ),
    metadata,
  );

  assert_eq!(HttpConditionalRequestOutcome::PreconditionFailed, outcome);
}

#[test]
fn conditional_request_helpers_evaluate_if_match_with_strong_comparison() {
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("abc"));

  let weak_match = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-Match: W/\"abc\"\r\n",
      "\r\n",
    ),
    metadata.clone(),
  );
  let strong_match = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-Match: \"abc\"\r\n",
      "\r\n",
    ),
    metadata,
  );

  assert_eq!(
    HttpConditionalRequestOutcome::PreconditionFailed,
    weak_match
  );
  assert_eq!(HttpConditionalRequestOutcome::Proceed, strong_match);
}

#[test]
fn conditional_request_helpers_evaluate_http_dates_and_precedence() {
  let metadata = HttpConditionalMetadata::new()
    .entity_tag(HttpEntityTag::strong("abc"))
    .last_modified(UNIX_EPOCH + Duration::from_secs(784_111_777));

  let stale_if_unmodified_since = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-Unmodified-Since: Sun, 06 Nov 1994 08:49:36 GMT\r\n",
      "\r\n",
    ),
    metadata.clone(),
  );
  let fresh_if_modified_since = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "\r\n",
    ),
    metadata.clone(),
  );
  let if_none_match_takes_precedence = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-None-Match: \"different\"\r\n",
      "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "\r\n",
    ),
    metadata,
  );

  assert_eq!(
    HttpConditionalRequestOutcome::PreconditionFailed,
    stale_if_unmodified_since
  );
  assert_eq!(
    HttpConditionalRequestOutcome::NotModified,
    fresh_if_modified_since
  );
  assert_eq!(
    HttpConditionalRequestOutcome::Proceed,
    if_none_match_takes_precedence
  );
}

#[test]
fn conditional_request_helpers_compare_http_dates_at_second_precision() {
  let metadata = HttpConditionalMetadata::new()
    .last_modified(UNIX_EPOCH + Duration::from_secs(784_111_777) + Duration::from_millis(500));

  let outcome = conditional_outcome_for(
    concat!(
      "GET /cached HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "\r\n",
    ),
    metadata,
  );

  assert_eq!(HttpConditionalRequestOutcome::NotModified, outcome);
}

#[test]
fn conditional_response_helpers_include_available_validators_and_preserve_304_framing() {
  let metadata = HttpConditionalMetadata::new()
    .entity_tag(HttpEntityTag::weak("abc"))
    .last_modified(UNIX_EPOCH + Duration::from_secs(784_111_777));
  let mut not_modified = Vec::new();
  let mut precondition_failed = Vec::new();

  HttpResponse::not_modified(&metadata)
    .body("ignored")
    .write_to(&mut not_modified)
    .expect("serialize 304 response");
  HttpResponse::precondition_failed()
    .write_to(&mut precondition_failed)
    .expect("serialize 412 response");

  assert_eq!(
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "ETag: W/\"abc\"\r\n",
      "Last-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "Connection: close\r\n",
      "\r\n",
    )
    .as_bytes(),
    not_modified.as_slice()
  );
  assert_eq!(
    b"HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    precondition_failed.as_slice()
  );
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

fn conditional_outcome_for(
  request: &str,
  metadata: HttpConditionalMetadata,
) -> HttpConditionalRequestOutcome {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let outcome = request.evaluate_conditional(&metadata);
        tx.send(outcome).expect("send conditional outcome");
        HttpResponse::ok("")
      })
      .expect("serve one request");
  });

  let _response = send_request(addr, request.as_bytes());
  let outcome = rx.recv().expect("receive conditional outcome");
  handle.join().expect("server thread");
  outcome
}
