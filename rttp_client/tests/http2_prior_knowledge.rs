#![cfg(feature = "http2")]

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread;
use std::time::Duration;

use rttp_client::types::Proxy;
use rttp_client::HttpClient;

const FRAME_DATA: u8 = 0x0;
const FRAME_HEADERS: u8 = 0x1;
const FRAME_RST_STREAM: u8 = 0x3;
const FRAME_SETTINGS: u8 = 0x4;
const FRAME_GOAWAY: u8 = 0x7;
const FRAME_WINDOW_UPDATE: u8 = 0x8;

const FLAG_END_STREAM: u8 = 0x1;
const FLAG_END_HEADERS: u8 = 0x4;
const FLAG_ACK: u8 = 0x1;

fn spawn_h2_prior_knowledge_peer() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    let mut preface = [0; 24];
    stream
      .read_exact(&mut preface)
      .expect("read client preface");
    assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", &preface);

    let client_settings = read_frame(&mut stream);
    assert_eq!(FRAME_SETTINGS, client_settings.frame_type);
    assert_eq!(0, client_settings.stream_id);
    assert_eq!(0, client_settings.payload.len());

    write_frame(&mut stream, FRAME_SETTINGS, 0, 0, &[]);

    let client_settings_ack = read_frame(&mut stream);
    assert_eq!(FRAME_SETTINGS, client_settings_ack.frame_type);
    assert_eq!(FLAG_ACK, client_settings_ack.flags);
    assert_eq!(0, client_settings_ack.stream_id);
    assert_eq!(0, client_settings_ack.payload.len());

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_STREAM | FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      &[
        0x88, 0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
    write_frame(
      &mut stream,
      FRAME_DATA,
      FLAG_END_STREAM,
      1,
      b"hello over h2",
    );

    request_headers.payload
  });

  (addr, handle)
}

#[test]
fn prior_knowledge_get_sends_h2_handshake_and_reads_single_response_stream() {
  let (addr, handle) = spawn_h2_prior_knowledge_peer();

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/hello?via=h2", addr))
    .emit_http2_prior_knowledge()
    .expect("single h2 GET response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
  assert_eq!("hello over h2", response.body().string().unwrap());

  let request_header_block = handle.join().expect("h2 peer thread");
  assert!(request_header_block.contains(&0x82));
  assert!(request_header_block.contains(&0x86));
  assert!(request_header_block
    .windows(b"/hello?via=h2".len())
    .any(|window| window == b"/hello?via=h2"));
}

#[test]
fn prior_knowledge_rejects_configured_proxy_before_connecting() {
  let err = HttpClient::new()
    .get()
    .url("http://127.0.0.1:9/proxy")
    .proxy(Proxy::http("127.0.0.1", 8080))
    .emit_http2_prior_knowledge()
    .expect_err("prior knowledge does not support proxies");

  assert!(err.is_builder());
  assert!(err.to_string().contains("does not support proxies"));
}

#[test]
fn prior_knowledge_decodes_content_length_from_hpack_static_index() {
  let (addr, handle) = spawn_h2_prior_knowledge_peer_with_response(
    &[0x88, 0x0f, 13, 2, b'1', b'3'],
    &[b"hello over h2"],
  );

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/content-length", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with content-length static index");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"13".to_string()),
    response.header_value("content-length")
  );
  assert_eq!("hello over h2", response.body().string().unwrap());

  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_skips_informational_headers_before_final_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      &[0x08, 3, b'1', b'0', b'3'],
    );
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"final h2");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/early-hints", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response after informational headers");

  assert_eq!(200, response.code());
  assert_eq!("final h2", response.body().string().unwrap());
  assert!(response.trailers().is_empty());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_ignores_interleaved_data_for_other_streams() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      &[
        0x88, 0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"hel");
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 3, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 3, b"ignored");
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"lo");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/interleaved", addr))
    .emit_http2_prior_knowledge()
    .expect("interleaved h2 response");

  assert_eq!(200, response.code());
  assert_eq!("hello", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_reports_stream_reset_and_goaway() {
  let (reset_addr, reset_handle) = spawn_control_frame_peer(FRAME_RST_STREAM);
  let reset_error = HttpClient::new()
    .get()
    .url(format!("http://{}/reset", reset_addr))
    .emit_http2_prior_knowledge()
    .expect_err("reset stream must fail the response");
  assert!(reset_error.to_string().contains("RST_STREAM"));
  reset_handle.join().expect("reset peer thread");

  let (goaway_addr, goaway_handle) = spawn_control_frame_peer(FRAME_GOAWAY);
  let goaway_error = HttpClient::new()
    .get()
    .url(format!("http://{}/goaway", goaway_addr))
    .emit_http2_prior_knowledge()
    .expect_err("goaway must fail the response");
  assert!(goaway_error.to_string().contains("GOAWAY"));
  goaway_handle.join().expect("goaway peer thread");
}

#[test]
fn prior_knowledge_get_continues_after_graceful_goaway_for_active_stream() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_GOAWAY, 0, 0, &[0, 0, 0, 1, 0, 0, 0, 0]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"still served");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/graceful-goaway", addr))
    .emit_http2_prior_knowledge()
    .expect("graceful GOAWAY permits active stream response");

  assert_eq!(200, response.code());
  assert_eq!("still served", response.body().string().unwrap());
  handle.join().expect("goaway peer thread");
}

#[test]
fn prior_knowledge_sends_window_updates_for_non_final_data_frames() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    let chunk = vec![b'x'; 32 * 1024];
    write_frame(&mut stream, FRAME_DATA, 0, 1, &chunk);

    let stream_update = read_frame(&mut stream);
    assert_eq!(FRAME_WINDOW_UPDATE, stream_update.frame_type);
    assert_eq!(1, stream_update.stream_id);
    assert_eq!(chunk.len() as u32, window_update_increment(&stream_update));

    let connection_update = read_frame(&mut stream);
    assert_eq!(FRAME_WINDOW_UPDATE, connection_update.frame_type);
    assert_eq!(0, connection_update.stream_id);
    assert_eq!(
      chunk.len() as u32,
      window_update_increment(&connection_update)
    );

    write_frame(
      &mut stream,
      FRAME_DATA,
      FLAG_END_STREAM,
      1,
      b" and final chunk",
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/window-update", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response requiring window updates");

  assert_eq!(
    32 * 1024 + b" and final chunk".len(),
    response.body().binary().len()
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_sends_window_updates_while_reading_large_response_body() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    for _ in 0..5 {
      write_frame(&mut stream, FRAME_DATA, 0, 1, &vec![b'x'; 16 * 1024]);
    }
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"done");

    let mut saw_stream_update = false;
    let mut saw_connection_update = false;
    for _ in 0..8 {
      let frame = read_frame(&mut stream);
      if frame.frame_type == FRAME_WINDOW_UPDATE && frame.stream_id == 1 {
        saw_stream_update = true;
      }
      if frame.frame_type == FRAME_WINDOW_UPDATE && frame.stream_id == 0 {
        saw_connection_update = true;
      }
      if saw_stream_update && saw_connection_update {
        return;
      }
    }
    panic!("client did not send stream and connection WINDOW_UPDATE frames");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/large", addr))
    .emit_http2_prior_knowledge()
    .expect("large h2 response");

  assert_eq!(200, response.code());
  assert_eq!(5 * 16 * 1024 + 4, response.body().binary().len());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_decodes_terminal_trailer_headers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"hello trailers");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS | FLAG_END_STREAM,
      1,
      &[
        0, 7, b'x', b'-', b't', b'r', b'a', b'c', b'e', 3, b'a', b'b', b'c',
      ],
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with trailer headers");

  assert_eq!(200, response.code());
  assert_eq!("hello trailers", response.body().string().unwrap());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("X-Trace"));
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_defers_small_window_updates_until_more_credit_is_needed() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_millis(200)))
      .expect("set h2 peer read timeout");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"small trailer body");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS | FLAG_END_STREAM,
      1,
      &[
        0, 7, b'x', b'-', b't', b'r', b'a', b'c', b'e', 3, b'a', b'b', b'c',
      ],
    );

    match try_read_frame(&mut stream) {
      Ok(Some(frame)) => panic!(
        "unexpected client frame after small trailer response: type={}, stream_id={}",
        frame.frame_type, frame.stream_id
      ),
      Ok(None) => {}
      Err(err) => panic!("unexpected frame read error: {err}"),
    }
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/small-trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with small trailer body");

  assert_eq!(200, response.code());
  assert_eq!("small trailer body", response.body().string().unwrap());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("X-Trace"));
  handle.join().expect("h2 peer thread");
}

struct Frame {
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: Vec<u8>,
}

fn read_frame(stream: &mut impl Read) -> Frame {
  let mut header = [0; 9];
  stream.read_exact(&mut header).expect("read frame header");
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload).expect("read frame payload");
  Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  }
}

fn try_read_frame(stream: &mut impl Read) -> io::Result<Option<Frame>> {
  let mut header = [0; 9];
  match stream.read_exact(&mut header) {
    Ok(()) => {}
    Err(err)
      if err.kind() == io::ErrorKind::WouldBlock
        || err.kind() == io::ErrorKind::TimedOut
        || err.kind() == io::ErrorKind::UnexpectedEof =>
    {
      return Ok(None);
    }
    Err(err) => return Err(err),
  }
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload)?;
  Ok(Some(Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  }))
}

fn spawn_h2_prior_knowledge_peer_with_response(
  header_block: &'static [u8],
  data_frames: &'static [&'static [u8]],
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      header_block,
    );
    for (index, data) in data_frames.iter().enumerate() {
      let flags = if index + 1 == data_frames.len() {
        FLAG_END_STREAM
      } else {
        0
      };
      write_frame(&mut stream, FRAME_DATA, flags, 1, data);
    }
  });

  (addr, handle)
}

fn complete_h2_request_handshake(stream: &mut impl ReadWrite) {
  let mut preface = [0; 24];
  stream
    .read_exact(&mut preface)
    .expect("read client preface");
  assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", &preface);

  let client_settings = read_frame(stream);
  assert_eq!(FRAME_SETTINGS, client_settings.frame_type);
  assert_eq!(0, client_settings.stream_id);
  assert_eq!(0, client_settings.payload.len());

  write_frame(stream, FRAME_SETTINGS, 0, 0, &[]);

  let client_settings_ack = read_frame(stream);
  assert_eq!(FRAME_SETTINGS, client_settings_ack.frame_type);
  assert_eq!(FLAG_ACK, client_settings_ack.flags);
  assert_eq!(0, client_settings_ack.stream_id);
  assert_eq!(0, client_settings_ack.payload.len());

  let request_headers = read_frame(stream);
  assert_eq!(FRAME_HEADERS, request_headers.frame_type);
  assert_eq!(FLAG_END_STREAM | FLAG_END_HEADERS, request_headers.flags);
  assert_eq!(1, request_headers.stream_id);
}

trait ReadWrite: Read + Write {}

impl<T> ReadWrite for T where T: Read + Write {}

fn window_update_increment(frame: &Frame) -> u32 {
  assert_eq!(4, frame.payload.len());
  u32::from_be_bytes([
    frame.payload[0] & 0x7f,
    frame.payload[1],
    frame.payload[2],
    frame.payload[3],
  ])
}

fn write_frame(stream: &mut impl Write, frame_type: u8, flags: u8, stream_id: u32, payload: &[u8]) {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header).expect("write frame header");
  stream.write_all(payload).expect("write frame payload");
  stream.flush().expect("flush frame");
}

fn spawn_control_frame_peer(frame_type: u8) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);
    match frame_type {
      FRAME_RST_STREAM => write_frame(&mut stream, FRAME_RST_STREAM, 0, 1, &0u32.to_be_bytes()),
      FRAME_GOAWAY => write_frame(&mut stream, FRAME_GOAWAY, 0, 0, &[0, 0, 0, 0, 0, 0, 0, 0]),
      _ => unreachable!("unexpected control frame"),
    }
  });

  (addr, handle)
}
