#![cfg(feature = "http2")]

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::HttpResponse;
use rttp_client::HttpClient;

const H2_FRAME_DATA: u8 = 0x0;
const H2_FRAME_HEADERS: u8 = 0x1;
const H2_FRAME_PRIORITY: u8 = 0x2;
const H2_FRAME_RST_STREAM: u8 = 0x3;
const H2_FRAME_SETTINGS: u8 = 0x4;
const H2_FRAME_PUSH_PROMISE: u8 = 0x5;
const H2_FRAME_PING: u8 = 0x6;
const H2_FRAME_GOAWAY: u8 = 0x7;
const H2_FRAME_WINDOW_UPDATE: u8 = 0x8;
const H2_FLAG_END_STREAM: u8 = 0x1;
const H2_FLAG_ACK: u8 = 0x1;
const H2_FLAG_END_HEADERS: u8 = 0x4;
const H2_FLAG_PADDED: u8 = 0x8;
const H2_FLAG_PRIORITY: u8 = 0x20;
const H2_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const H2_SETTINGS_ENABLE_PUSH: u16 = 0x2;
const H2_SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
const H2_SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
const H2_SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
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
      .contains("HTTP/2 prior-knowledge CONNECT/proxy tunneling is unsupported"),
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
    .header(("Upgrade", "websocket"))
    .header(("X-Boundary", "present"))
    .emit_http2_prior_knowledge()
    .expect("h2 connection header boundary response");

  assert_eq!("clean h2c response", response.body().string().unwrap());
  assert_eq!(
    (None, None, None, None, Some("present".to_string())),
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
