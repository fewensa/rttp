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
const FRAME_PING: u8 = 0x6;
const FRAME_GOAWAY: u8 = 0x7;
const FRAME_WINDOW_UPDATE: u8 = 0x8;
const FRAME_CONTINUATION: u8 = 0x9;

const FLAG_END_STREAM: u8 = 0x1;
const FLAG_END_HEADERS: u8 = 0x4;
const FLAG_PADDED: u8 = 0x8;
const FLAG_ACK: u8 = 0x1;
const FLAG_PRIORITY: u8 = 0x20;

const SETTING_ENABLE_PUSH: u16 = 0x2;
const SETTING_INITIAL_WINDOW_SIZE: u16 = 0x4;
const SETTING_MAX_FRAME_SIZE: u16 = 0x5;

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
  assert_eq!(
    b"/hello?via=h2",
    find_header_value(&request_header_block, b":path")
      .expect("request path")
      .value
      .as_slice()
  );
}

#[test]
fn prior_knowledge_request_literals_use_huffman_only_when_smaller() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");
  let compressible_value = "a".repeat(24);
  let expected_header_value = compressible_value.clone();
  let expected_trailer_value = compressible_value.clone();

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request(&mut stream);

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    let request_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, request_body.frame_type);
    assert_eq!(0, request_body.flags);
    assert_eq!(1, request_body.stream_id);
    assert_eq!(b"huffman body", request_body.payload.as_slice());

    let request_trailers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_trailers.frame_type);
    assert_eq!(FLAG_END_HEADERS | FLAG_END_STREAM, request_trailers.flags);
    assert_eq!(1, request_trailers.stream_id);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"ok");

    (request_headers.payload, request_trailers.payload)
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/huffman-literals", addr))
    .header(("X-H".to_string(), compressible_value))
    .header(("X-R", "0"))
    .trailer(("X-T".to_string(), expected_trailer_value.clone()))
    .expect("configure request trailer")
    .raw("huffman body")
    .emit_http2_prior_knowledge()
    .expect("h2 POST response");

  assert_eq!(200, response.code());
  assert_eq!("ok", response.body().string().unwrap());

  let (request_header_block, request_trailer_block) = handle.join().expect("h2 peer thread");
  let encoded_header =
    find_literal_new_name_value(&request_header_block, b"x-h").expect("huffman request header");
  assert!(encoded_header.huffman);
  assert_eq!(expected_header_value.as_bytes(), encoded_header.value);

  let raw_header =
    find_literal_new_name_value(&request_header_block, b"x-r").expect("raw request header");
  assert!(!raw_header.huffman);
  assert_eq!(b"0", raw_header.value.as_slice());

  let encoded_trailer =
    find_literal_new_name_value(&request_trailer_block, b"x-t").expect("huffman request trailer");
  assert!(encoded_trailer.huffman);
  assert_eq!(expected_trailer_value.as_bytes(), encoded_trailer.value);
}

#[test]
fn prior_knowledge_post_sends_headers_then_body_data_frame() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request(&mut stream);

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    let request_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, request_body.frame_type);
    assert_eq!(FLAG_END_STREAM, request_body.flags);
    assert_eq!(1, request_body.stream_id);
    assert_eq!(b"{\"ok\":true}", request_body.payload.as_slice());

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"created");

    request_headers.payload
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/submit", addr))
    .content_type("application/json")
    .header(("X-Trace", "abc123"))
    .raw("{\"ok\":true}")
    .emit_http2_prior_knowledge()
    .expect("h2 POST response");

  assert_eq!(200, response.code());
  assert_eq!("HTTP/2", response.version());
  assert_eq!("created", response.body().string().unwrap());

  let request_header_block = handle.join().expect("h2 peer thread");
  assert!(request_header_block.contains(&0x83));
  assert_eq!(
    b"/submit",
    find_header_value(&request_header_block, b":path")
      .expect("request path")
      .value
      .as_slice()
  );
  assert_eq!(
    b"application/json",
    find_header_value(&request_header_block, b"content-type")
      .expect("content-type request header")
      .value
      .as_slice()
  );
  assert_eq!(
    b"11",
    find_header_value(&request_header_block, b"content-length")
      .expect("content-length request header")
      .value
      .as_slice()
  );
  assert_eq!(
    b"abc123",
    find_header_value(&request_header_block, b"x-trace")
      .expect("x-trace request header")
      .value
      .as_slice()
  );
}

#[test]
fn prior_knowledge_post_sends_request_trailers_after_body_data_frame() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request(&mut stream);

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    let request_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, request_body.frame_type);
    assert_eq!(0, request_body.flags);
    assert_eq!(1, request_body.stream_id);
    assert_eq!(b"trace body", request_body.payload.as_slice());

    let request_trailers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_trailers.frame_type);
    assert_eq!(FLAG_END_HEADERS | FLAG_END_STREAM, request_trailers.flags);
    assert_eq!(1, request_trailers.stream_id);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"created");

    (request_headers.payload, request_trailers.payload)
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/submit-with-tail", addr))
    .header(("Trailer", "X-Trace"))
    .trailer(("X-Trace", "abc"))
    .expect("configure request trailer")
    .raw("trace body")
    .emit_http2_prior_knowledge()
    .expect("h2 POST response");

  assert_eq!(200, response.code());
  assert_eq!("created", response.body().string().unwrap());

  let (request_header_block, request_trailer_block) = handle.join().expect("h2 peer thread");
  assert_eq!(
    b"/submit-with-tail",
    find_header_value(&request_header_block, b":path")
      .expect("request path")
      .value
      .as_slice()
  );
  assert_eq!(
    b"X-Trace",
    find_header_value(&request_header_block, b"trailer")
      .expect("trailer request header")
      .value
      .as_slice()
  );
  assert_eq!(
    b"abc",
    find_header_value(&request_trailer_block, b"x-trace")
      .expect("request trailer")
      .value
      .as_slice()
  );
}

#[test]
fn prior_knowledge_post_splits_body_data_frames_at_default_peer_max_frame_size() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");
  let body = "x".repeat(16 * 1024 + 7);
  let expected_body = body.clone();

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request(&mut stream);

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    let first_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, first_body.frame_type);
    assert_eq!(0, first_body.flags);
    assert_eq!(1, first_body.stream_id);
    assert_eq!(16 * 1024, first_body.payload.len());

    let second_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, second_body.frame_type);
    assert_eq!(FLAG_END_STREAM, second_body.flags);
    assert_eq!(1, second_body.stream_id);
    assert_eq!(7, second_body.payload.len());

    let mut full_body = first_body.payload;
    full_body.extend_from_slice(&second_body.payload);
    assert_eq!(expected_body.as_bytes(), full_body.as_slice());

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"created");
  });

  let response = HttpClient::new()
    .post()
    .url(format!("http://{}/upload", addr))
    .raw(&body)
    .emit_http2_prior_knowledge()
    .expect("h2 POST response");

  assert_eq!(200, response.code());
  assert_eq!("created", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_splits_large_request_headers_at_peer_max_frame_size() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");
  let peer_max_frame_size = 16 * 1024;
  let large_header_value = "{".repeat(peer_max_frame_size + 512);
  let expected_header_value = large_header_value.clone();

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request_with_settings(
      &mut stream,
      &settings_payload(SETTING_MAX_FRAME_SIZE, peer_max_frame_size as u32),
    );

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_STREAM, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);
    assert_eq!(peer_max_frame_size, request_headers.payload.len());

    let request_continuation = read_frame(&mut stream);
    assert_eq!(FRAME_CONTINUATION, request_continuation.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_continuation.flags);
    assert_eq!(1, request_continuation.stream_id);
    assert!(request_continuation.payload.len() <= peer_max_frame_size);
    assert!(!request_continuation.payload.is_empty());

    let mut header_block = request_headers.payload;
    header_block.extend_from_slice(&request_continuation.payload);
    let large_header =
      find_header_value(&header_block, b"x-large-header").expect("large request header");
    assert!(!large_header.huffman);
    assert_eq!(expected_header_value.as_bytes(), large_header.value);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"split");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/large-headers", addr))
    .header(("X-Large-Header".to_string(), large_header_value))
    .emit_http2_prior_knowledge()
    .expect("h2 GET response");

  assert_eq!(200, response.code());
  assert_eq!("split", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_get_splits_large_huffman_request_headers_at_peer_max_frame_size() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");
  let peer_max_frame_size = 16 * 1024;
  let large_header_value = "a".repeat(peer_max_frame_size * 3);
  let expected_header_value = large_header_value.clone();

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request_with_settings(
      &mut stream,
      &settings_payload(SETTING_MAX_FRAME_SIZE, peer_max_frame_size as u32),
    );

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_STREAM, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);
    assert_eq!(peer_max_frame_size, request_headers.payload.len());

    let request_continuation = read_frame(&mut stream);
    assert_eq!(FRAME_CONTINUATION, request_continuation.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_continuation.flags);
    assert_eq!(1, request_continuation.stream_id);
    assert!(request_continuation.payload.len() <= peer_max_frame_size);
    assert!(!request_continuation.payload.is_empty());

    let mut header_block = request_headers.payload;
    header_block.extend_from_slice(&request_continuation.payload);
    let encoded_header =
      find_literal_new_name_value(&header_block, b"x-h").expect("large huffman request header");
    assert!(encoded_header.huffman);
    assert_eq!(expected_header_value.as_bytes(), encoded_header.value);

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"split");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/large-huffman-headers", addr))
    .header(("X-H".to_string(), large_header_value))
    .emit_http2_prior_knowledge()
    .expect("h2 GET response");

  assert_eq!(200, response.code());
  assert_eq!("split", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_put_and_patch_send_body_data_frames() {
  for method in ["PUT", "PATCH"] {
    let request_header_block = emit_prior_knowledge_body_request(method);

    assert!(request_header_block
      .windows(method.len())
      .any(|window| window == method.as_bytes()));
    assert_eq!(
      b"/resource",
      find_header_value(&request_header_block, b":path")
        .expect("request path")
        .value
        .as_slice()
    );
  }
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
fn prior_knowledge_rejects_initial_settings_with_invalid_payload_length() {
  let (addr, handle) = spawn_initial_settings_peer(0, 0, &[0, 1, 2, 3, 4]);

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/invalid-settings-length", addr))
    .emit_http2_prior_knowledge()
    .expect_err("invalid SETTINGS payload length must fail");

  assert!(err.to_string().contains("invalid HTTP/2 SETTINGS frame"));
  handle.join().expect("invalid settings length peer thread");
}

#[test]
fn prior_knowledge_rejects_initial_settings_with_invalid_max_frame_size() {
  let payload = settings_payload(SETTING_MAX_FRAME_SIZE, 16_777_216);
  let (addr, handle) = spawn_initial_settings_peer(0, 0, &payload);

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/invalid-max-frame-size", addr))
    .emit_http2_prior_knowledge()
    .expect_err("invalid SETTINGS_MAX_FRAME_SIZE must fail");

  assert!(err.to_string().contains("SETTINGS_MAX_FRAME_SIZE"));
  handle.join().expect("invalid max frame size peer thread");
}

#[test]
fn prior_knowledge_rejects_initial_settings_with_invalid_enable_push() {
  let payload = settings_payload(SETTING_ENABLE_PUSH, 2);
  let (addr, handle) = spawn_initial_settings_peer(0, 0, &payload);

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/invalid-enable-push", addr))
    .emit_http2_prior_knowledge()
    .expect_err("invalid SETTINGS_ENABLE_PUSH must fail");

  assert!(err.to_string().contains("SETTINGS_ENABLE_PUSH"));
  handle.join().expect("invalid enable push peer thread");
}

#[test]
fn prior_knowledge_rejects_initial_settings_with_invalid_initial_window_size() {
  let payload = settings_payload(SETTING_INITIAL_WINDOW_SIZE, 2_147_483_648);
  let (addr, handle) = spawn_initial_settings_peer(0, 0, &payload);

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/invalid-initial-window-size", addr))
    .emit_http2_prior_knowledge()
    .expect_err("invalid SETTINGS_INITIAL_WINDOW_SIZE must fail");

  assert!(err.to_string().contains("SETTINGS_INITIAL_WINDOW_SIZE"));
  handle
    .join()
    .expect("invalid initial window size peer thread");
}

#[test]
fn prior_knowledge_rejects_initial_settings_ack_with_payload() {
  let payload = settings_payload(SETTING_MAX_FRAME_SIZE, 16 * 1024);
  let (addr, handle) = spawn_initial_settings_peer(FLAG_ACK, 0, &payload);

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/settings-ack-payload", addr))
    .emit_http2_prior_knowledge()
    .expect_err("SETTINGS ACK with payload must fail");

  assert!(err.to_string().contains("SETTINGS ACK"));
  handle.join().expect("settings ack payload peer thread");
}

#[test]
fn prior_knowledge_rejects_subsequent_settings_with_invalid_payload() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);
    write_frame(
      &mut stream,
      FRAME_SETTINGS,
      0,
      0,
      &settings_payload(SETTING_ENABLE_PUSH, 2),
    );
  });

  let err = HttpClient::new()
    .get()
    .url(format!("http://{}/invalid-subsequent-settings", addr))
    .emit_http2_prior_knowledge()
    .expect_err("invalid subsequent SETTINGS must fail");

  assert!(err.to_string().contains("SETTINGS_ENABLE_PUSH"));
  handle
    .join()
    .expect("invalid subsequent settings peer thread");
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
      &h2_literal_new_name(b":status", b"103"),
    );
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS,
      1,
      &[
        0x88, 0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"final body");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/early-hints", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response after informational headers");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
  assert_eq!("final body", response.body().string().unwrap());
  assert!(response.trailers().is_empty());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_exposes_response_trailers_after_data_without_changing_headers() {
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
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"hello");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_STREAM | FLAG_END_HEADERS,
      1,
      &h2_literal_new_name(b"x-trace", b"abc123"),
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with trailers");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
  assert_eq!("hello", response.body().string().unwrap());
  assert_eq!(1, response.trailers().len());
  assert_eq!(
    Some(&"abc123".to_string()),
    response.trailer_value("X-Trace")
  );
  assert!(response.header("x-trace").is_none());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_decodes_padded_response_headers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    let mut payload = vec![3, 0x88];
    payload.extend_from_slice(&h2_literal_new_name(b"x-padded", b"headers"));
    payload.extend_from_slice(&[0, 0, 0]);

    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_PADDED | FLAG_END_HEADERS,
      1,
      &payload,
    );
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"ok");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/padded-headers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with padded headers");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"headers".to_string()),
    response.header_value("X-Padded")
  );
  assert_eq!("ok", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_decodes_padded_data_without_appending_padding() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(
      &mut stream,
      FRAME_DATA,
      FLAG_PADDED | FLAG_END_STREAM,
      1,
      &[4, b'b', b'o', b'd', b'y', 0, 0, 0, 0],
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/padded-data", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with padded data");

  assert_eq!(200, response.code());
  assert_eq!("body", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_decodes_padded_response_trailers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    let mut trailer_payload = vec![2];
    trailer_payload.extend_from_slice(&h2_literal_new_name(b"x-trace", b"padded"));
    trailer_payload.extend_from_slice(&[0, 0]);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"trailer body");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_PADDED | FLAG_END_HEADERS | FLAG_END_STREAM,
      1,
      &trailer_payload,
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/padded-trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with padded trailers");

  assert_eq!(200, response.code());
  assert_eq!("trailer body", response.body().string().unwrap());
  assert_eq!(
    Some(&"padded".to_string()),
    response.trailer_value("X-Trace")
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_ignores_response_headers_priority_metadata() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    let mut payload = vec![0, 0, 0, 0, 16, 0x88];
    payload.extend_from_slice(&h2_literal_new_name(b"x-priority", b"ignored"));

    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_PRIORITY | FLAG_END_HEADERS,
      1,
      &payload,
    );
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"priority");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/priority-headers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with priority headers");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"ignored".to_string()),
    response.header_value("X-Priority")
  );
  assert_eq!("priority", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_malformed_padding_lengths() {
  let (data_addr, data_handle) = spawn_malformed_padding_peer(FRAME_DATA);
  let data_error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-data-padding", data_addr))
    .emit_http2_prior_knowledge()
    .expect_err("DATA padding longer than payload must fail");
  assert!(
    data_error.to_string().contains("padding"),
    "unexpected error: {data_error}"
  );
  data_handle.join().expect("bad data padding peer thread");

  let (headers_addr, headers_handle) = spawn_malformed_padding_peer(FRAME_HEADERS);
  let headers_error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-headers-padding", headers_addr))
    .emit_http2_prior_knowledge()
    .expect_err("HEADERS padding longer than payload must fail");
  assert!(
    headers_error.to_string().contains("padding"),
    "unexpected error: {headers_error}"
  );
  headers_handle
    .join()
    .expect("bad headers padding peer thread");
}

#[test]
fn prior_knowledge_decodes_response_headers_split_across_continuation_frames() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, 0, 1, &[0x88]);
    write_frame(
      &mut stream,
      FRAME_CONTINUATION,
      FLAG_END_HEADERS,
      1,
      &h2_literal_new_name(b"x-split", b"headers"),
    );
    write_frame(
      &mut stream,
      FRAME_DATA,
      FLAG_END_STREAM,
      1,
      b"split header body",
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/split-headers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with continued headers");

  assert_eq!(200, response.code());
  assert_eq!(
    Some(&"headers".to_string()),
    response.header_value("X-Split")
  );
  assert_eq!("split header body", response.body().string().unwrap());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_decodes_response_trailers_split_across_continuation_frames() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"split trailer body");
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_STREAM, 1, &[]);
    write_frame(
      &mut stream,
      FRAME_CONTINUATION,
      FLAG_END_HEADERS,
      1,
      &h2_literal_new_name(b"x-trace", b"continued"),
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/split-trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response with continued trailers");

  assert_eq!(200, response.code());
  assert_eq!("split trailer body", response.body().string().unwrap());
  assert_eq!(
    Some(&"continued".to_string()),
    response.trailer_value("X-Trace")
  );
  assert!(response.header("x-trace").is_none());
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_continuation_without_pending_header_block() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(
      &mut stream,
      FRAME_CONTINUATION,
      FLAG_END_HEADERS,
      1,
      &[0x88],
    );
  });

  let error = HttpClient::new()
    .get()
    .url(format!("http://{}/orphan-continuation", addr))
    .emit_http2_prior_knowledge()
    .expect_err("orphan CONTINUATION frame should be rejected");

  assert!(
    error.to_string().contains("unexpected HTTP/2 CONTINUATION"),
    "unexpected error: {error}"
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_interrupted_continuation_sequence() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, 0, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"not allowed here");
  });

  let error = HttpClient::new()
    .get()
    .url(format!("http://{}/interrupted-continuation", addr))
    .emit_http2_prior_knowledge()
    .expect_err("interrupted CONTINUATION sequence should be rejected");

  assert!(
    error.to_string().contains("expected HTTP/2 CONTINUATION"),
    "unexpected error: {error}"
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_continuation_on_wrong_stream_during_header_block() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, 0, 1, &[0x88]);
    write_frame(
      &mut stream,
      FRAME_CONTINUATION,
      FLAG_END_HEADERS,
      3,
      &[
        0x0f, 16, 10, b't', b'e', b'x', b't', b'/', b'p', b'l', b'a', b'i', b'n',
      ],
    );
  });

  let error = HttpClient::new()
    .get()
    .url(format!("http://{}/wrong-stream-continuation", addr))
    .emit_http2_prior_knowledge()
    .expect_err("wrong-stream CONTINUATION sequence should be rejected");

  assert!(
    error.to_string().contains("expected HTTP/2 CONTINUATION"),
    "unexpected error: {error}"
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_eof_before_end_headers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, 0, 1, &[0x88]);
  });

  let error = HttpClient::new()
    .get()
    .url(format!("http://{}/eof-before-end-headers", addr))
    .emit_http2_prior_knowledge()
    .expect_err("EOF before END_HEADERS should be rejected");

  assert!(
    error.to_string().contains("incomplete HTTP/2 header block"),
    "unexpected error: {error}"
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_response_trailer_pseudo_headers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"hello");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_STREAM | FLAG_END_HEADERS,
      1,
      &[0x88],
    );
  });

  let error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-trailers", addr))
    .emit_http2_prior_knowledge()
    .expect_err("trailer pseudo-header should be rejected");

  assert!(error.to_string().contains("Invalid trailer header"));
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
fn prior_knowledge_rejects_invalid_window_update_frames() {
  let (zero_addr, zero_handle) = spawn_window_update_peer(1, &[0]);
  let zero_error = HttpClient::new()
    .get()
    .url(format!("http://{}/zero-window-update", zero_addr))
    .emit_http2_prior_knowledge()
    .expect_err("zero WINDOW_UPDATE increment must fail");
  assert!(
    zero_error.to_string().contains("WINDOW_UPDATE"),
    "unexpected error: {zero_error}"
  );
  zero_handle.join().expect("zero window update peer thread");

  let (overflow_addr, overflow_handle) = spawn_window_update_peer(0, &[0x7fff_ffff]);
  let overflow_error = HttpClient::new()
    .get()
    .url(format!("http://{}/overflow-window-update", overflow_addr))
    .emit_http2_prior_knowledge()
    .expect_err("overflowing WINDOW_UPDATE increment must fail");
  assert!(
    overflow_error.to_string().contains("overflow"),
    "unexpected error: {overflow_error}"
  );
  overflow_handle
    .join()
    .expect("overflow window update peer thread");
}

#[test]
fn prior_knowledge_acks_ping_before_consuming_final_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    stream
      .set_read_timeout(Some(Duration::from_secs(2)))
      .expect("set h2 peer read timeout");
    complete_h2_request_handshake(&mut stream);

    let ping_payload = *b"rttp-png";
    write_frame(&mut stream, FRAME_PING, 0, 0, &ping_payload);

    let ping_ack = read_frame(&mut stream);
    assert_eq!(FRAME_PING, ping_ack.frame_type);
    assert_eq!(FLAG_ACK, ping_ack.flags);
    assert_eq!(0, ping_ack.stream_id);
    assert_eq!(ping_payload, ping_ack.payload.as_slice());

    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"pong");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/ping", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response after ping");

  assert_eq!(200, response.code());
  assert_eq!("pong", response.body().string().unwrap());
  handle.join().expect("ping peer thread");
}

#[test]
fn prior_knowledge_ignores_ping_ack_and_consumes_final_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    write_frame(&mut stream, FRAME_PING, FLAG_ACK, 0, b"rttp-ack");
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"ignored ack");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/ping-ack", addr))
    .emit_http2_prior_knowledge()
    .expect("h2 response after ping ack");

  assert_eq!(200, response.code());
  assert_eq!("ignored ack", response.body().string().unwrap());
  handle.join().expect("ping ack peer thread");
}

#[test]
fn prior_knowledge_rejects_malformed_ping_frames() {
  let (stream_addr, stream_handle) = spawn_ping_peer(0, 1, b"rttp-png");
  let stream_error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-ping-stream", stream_addr))
    .emit_http2_prior_knowledge()
    .expect_err("ping on non-zero stream must fail");
  assert!(stream_error.to_string().contains("PING"));
  stream_handle.join().expect("bad ping stream peer thread");

  let (length_addr, length_handle) = spawn_ping_peer(0, 0, b"short");
  let length_error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-ping-length", length_addr))
    .emit_http2_prior_knowledge()
    .expect_err("ping with invalid payload length must fail");
  assert!(length_error.to_string().contains("PING"));
  length_handle.join().expect("bad ping length peer thread");

  let (ack_length_addr, ack_length_handle) = spawn_ping_peer(FLAG_ACK, 0, b"short");
  let ack_length_error = HttpClient::new()
    .get()
    .url(format!("http://{}/bad-ping-ack-length", ack_length_addr))
    .emit_http2_prior_knowledge()
    .expect_err("ping ack with invalid payload length must fail");
  assert!(
    ack_length_error
      .to_string()
      .contains("invalid HTTP/2 PING frame"),
    "unexpected error: {ack_length_error}"
  );
  ack_length_handle
    .join()
    .expect("bad ping ack length peer thread");
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

#[test]
fn prior_knowledge_decodes_hpack_huffman_response_headers_and_trailers() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    let mut headers = vec![0x88];
    headers.extend_from_slice(&h2_literal_huffman_new_name(
      &[0xf2, 0xb4, 0xf6, 0xcb, 0x2f],
      &[0x3f, 0x55, 0xa7, 0xb6, 0x59, 0x7f],
    ));
    let trailers = h2_literal_huffman_new_name(
      &[0xf2, 0xb2, 0x46, 0x6a, 0x3f],
      &[0x4d, 0x83, 0x35, 0x0b, 0x4f, 0x6c, 0xb2, 0xff],
    );

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &headers);
    write_frame(&mut stream, FRAME_DATA, 0, 1, b"huffman body");
    write_frame(
      &mut stream,
      FRAME_HEADERS,
      FLAG_END_HEADERS | FLAG_END_STREAM,
      1,
      &trailers,
    );
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{}/huffman", addr))
    .emit_http2_prior_knowledge()
    .expect("huffman h2 response");

  assert_eq!(200, response.code());
  assert_eq!("huffman body", response.body().string().unwrap());
  assert_eq!(
    Some(&"ok-huff".to_string()),
    response.header_value("x-huff")
  );
  assert_eq!(
    Some(&"trail-huff".to_string()),
    response.trailer_value("x-tail")
  );
  handle.join().expect("h2 peer thread");
}

#[test]
fn prior_knowledge_rejects_malformed_hpack_huffman_strings() {
  let cases: &[(&str, &[u8], &str)] = &[
    (
      "eos-data",
      &[0x88, 0, 0x84, 0xff, 0xff, 0xff, 0xff, 1, b'x'],
      "HPACK Huffman EOS symbol used as data",
    ),
    (
      "invalid-padding",
      &[0x88, 0, 0x81, 0x00, 1, b'x'],
      "invalid HPACK Huffman padding",
    ),
    (
      "truncated-code",
      &[0x88, 0, 0x81, 0xfe, 1, b'x'],
      "truncated HPACK Huffman code",
    ),
    (
      "overlong-padding",
      &[0x88, 0, 0x81, 0xff, 1, b'x'],
      "overlong HPACK Huffman padding",
    ),
    (
      "invalid-utf8",
      &[0x88, 0, 0x84, 0xff, 0xff, 0xfb, 0xbf, 1, b'x'],
      "invalid utf-8 sequence",
    ),
  ];

  for (path, header_block, expected) in cases {
    let (addr, handle) = spawn_h2_prior_knowledge_peer_with_response(header_block, &[b""]);

    let error = HttpClient::new()
      .get()
      .url(format!("http://{}/{}", addr, path))
      .emit_http2_prior_knowledge()
      .expect_err("malformed Huffman string should be rejected");

    assert!(
      error.to_string().contains(expected),
      "expected {expected:?}, got {error}"
    );
    handle.join().expect("h2 peer thread");
  }
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

fn spawn_initial_settings_peer(
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");
  let payload = payload.to_vec();

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

    write_frame(&mut stream, FRAME_SETTINGS, flags, stream_id, &payload);
  });

  (addr, handle)
}

fn spawn_window_update_peer(
  stream_id: u32,
  increments: &'static [u32],
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    for increment in increments {
      write_frame(
        &mut stream,
        FRAME_WINDOW_UPDATE,
        0,
        stream_id,
        &increment.to_be_bytes(),
      );
    }
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"ignored");
  });

  (addr, handle)
}

fn complete_h2_request_handshake(stream: &mut impl ReadWrite) {
  complete_h2_handshake_without_request(stream);

  let request_headers = read_frame(stream);
  assert_eq!(FRAME_HEADERS, request_headers.frame_type);
  assert_eq!(FLAG_END_STREAM | FLAG_END_HEADERS, request_headers.flags);
  assert_eq!(1, request_headers.stream_id);
}

fn complete_h2_handshake_without_request(stream: &mut impl ReadWrite) {
  complete_h2_handshake_without_request_with_settings(stream, &[]);
}

fn complete_h2_handshake_without_request_with_settings(
  stream: &mut impl ReadWrite,
  settings: &[u8],
) {
  let mut preface = [0; 24];
  stream
    .read_exact(&mut preface)
    .expect("read client preface");
  assert_eq!(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", &preface);

  let client_settings = read_frame(stream);
  assert_eq!(FRAME_SETTINGS, client_settings.frame_type);
  assert_eq!(0, client_settings.stream_id);
  assert_eq!(0, client_settings.payload.len());

  write_frame(stream, FRAME_SETTINGS, 0, 0, settings);

  let client_settings_ack = read_frame(stream);
  assert_eq!(FRAME_SETTINGS, client_settings_ack.frame_type);
  assert_eq!(FLAG_ACK, client_settings_ack.flags);
  assert_eq!(0, client_settings_ack.stream_id);
  assert_eq!(0, client_settings_ack.payload.len());
}

fn emit_prior_knowledge_body_request(method: &str) -> Vec<u8> {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_handshake_without_request(&mut stream);

    let request_headers = read_frame(&mut stream);
    assert_eq!(FRAME_HEADERS, request_headers.frame_type);
    assert_eq!(FLAG_END_HEADERS, request_headers.flags);
    assert_eq!(1, request_headers.stream_id);

    let request_body = read_frame(&mut stream);
    assert_eq!(FRAME_DATA, request_body.frame_type);
    assert_eq!(FLAG_END_STREAM, request_body.flags);
    assert_eq!(1, request_body.stream_id);
    assert_eq!(b"update-body", request_body.payload.as_slice());

    write_frame(&mut stream, FRAME_SETTINGS, FLAG_ACK, 0, &[]);
    write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
    write_frame(&mut stream, FRAME_DATA, FLAG_END_STREAM, 1, b"updated");

    request_headers.payload
  });

  let response = HttpClient::new()
    .method(method)
    .url(format!("http://{}/resource", addr))
    .raw("update-body")
    .emit_http2_prior_knowledge()
    .expect("h2 request body response");

  assert_eq!(200, response.code());
  assert_eq!("updated", response.body().string().unwrap());
  handle.join().expect("h2 peer thread")
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

fn settings_payload(identifier: u16, value: u32) -> Vec<u8> {
  let mut payload = Vec::with_capacity(6);
  payload.extend_from_slice(&identifier.to_be_bytes());
  payload.extend_from_slice(&value.to_be_bytes());
  payload
}

fn h2_literal_new_name(name: &[u8], value: &[u8]) -> Vec<u8> {
  let mut block = Vec::new();
  block.push(0);
  h2_string(&mut block, name);
  h2_string(&mut block, value);
  block
}

fn h2_literal_huffman_new_name(name: &[u8], value: &[u8]) -> Vec<u8> {
  let mut block = Vec::new();
  block.push(0);
  h2_huffman_string(&mut block, name);
  h2_huffman_string(&mut block, value);
  block
}

struct TestHpackString {
  huffman: bool,
  value: Vec<u8>,
}

fn find_literal_new_name_value(block: &[u8], expected_name: &[u8]) -> Option<TestHpackString> {
  find_test_header_value(block, expected_name, true)
}

fn find_header_value(block: &[u8], expected_name: &[u8]) -> Option<TestHpackString> {
  find_test_header_value(block, expected_name, false)
}

fn find_test_header_value(
  block: &[u8],
  expected_name: &[u8],
  new_name_only: bool,
) -> Option<TestHpackString> {
  let mut cursor = 0;
  while cursor < block.len() {
    let byte = block[cursor];
    if byte & 0x80 == 0x80 {
      decode_test_integer(block, &mut cursor, 7);
      continue;
    }
    assert_eq!(0, byte & 0x20, "dynamic table update in request block");

    let name_index = decode_test_integer(block, &mut cursor, 4);
    let name = if name_index == 0 {
      Some(decode_test_string(block, &mut cursor))
    } else {
      test_static_name(name_index).map(|name| TestHpackString {
        huffman: false,
        value: name.to_vec(),
      })
    };
    let value = decode_test_string(block, &mut cursor);
    if (!new_name_only || name_index == 0)
      && name
        .as_ref()
        .is_some_and(|name| name.value.as_slice() == expected_name)
    {
      return Some(value);
    }
  }
  None
}

fn test_static_name(index: usize) -> Option<&'static [u8]> {
  match index {
    1 => Some(b":authority"),
    2 => Some(b":method"),
    4 => Some(b":path"),
    _ => None,
  }
}

fn decode_test_integer(block: &[u8], cursor: &mut usize, prefix_bits: u8) -> usize {
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

fn decode_test_string(block: &[u8], cursor: &mut usize) -> TestHpackString {
  let huffman = block[*cursor] & 0x80 == 0x80;
  let len = decode_test_integer(block, cursor, 7);
  let end = *cursor + len;
  let encoded = &block[*cursor..end];
  *cursor = end;
  let value = if huffman {
    decode_test_huffman_ascii_string(encoded)
  } else {
    encoded.to_vec()
  };
  TestHpackString { huffman, value }
}

fn decode_test_huffman_ascii_string(encoded: &[u8]) -> Vec<u8> {
  let mut value = Vec::new();
  let mut code = 0u32;
  let mut code_len = 0u8;

  for byte in encoded {
    for bit_offset in (0..8).rev() {
      code = (code << 1) | (((byte >> bit_offset) & 1) as u32);
      code_len += 1;

      if let Some(symbol) = test_huffman_ascii_symbol(code, code_len) {
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
      "invalid test HPACK Huffman padding"
    );
    assert!(code_len <= 7, "overlong test HPACK Huffman padding");
  }

  value
}

fn test_huffman_ascii_symbol(code: u32, code_len: u8) -> Option<u8> {
  TEST_HPACK_HUFFMAN_ASCII
    .iter()
    .find_map(|&(candidate, candidate_len, symbol)| {
      (candidate == code && candidate_len == code_len).then_some(symbol)
    })
}

const TEST_HPACK_HUFFMAN_ASCII: &[(u32, u8, u8)] = &[
  (0x14, 6, b' '),
  (0x3f8, 10, b'!'),
  (0x3f9, 10, b'"'),
  (0xffa, 12, b'#'),
  (0x1ff9, 13, b'$'),
  (0x15, 6, b'%'),
  (0xf8, 8, b'&'),
  (0x7fa, 11, b'\''),
  (0x3fa, 10, b'('),
  (0x3fb, 10, b')'),
  (0xf9, 8, b'*'),
  (0x7fb, 11, b'+'),
  (0xfa, 8, b','),
  (0x16, 6, b'-'),
  (0x17, 6, b'.'),
  (0x18, 6, b'/'),
  (0x0, 5, b'0'),
  (0x1, 5, b'1'),
  (0x2, 5, b'2'),
  (0x19, 6, b'3'),
  (0x1a, 6, b'4'),
  (0x1b, 6, b'5'),
  (0x1c, 6, b'6'),
  (0x1d, 6, b'7'),
  (0x1e, 6, b'8'),
  (0x1f, 6, b'9'),
  (0x5c, 7, b':'),
  (0xfb, 8, b';'),
  (0x7ffc, 15, b'<'),
  (0x20, 6, b'='),
  (0xffb, 12, b'>'),
  (0x3fc, 10, b'?'),
  (0x1ffa, 13, b'@'),
  (0x21, 6, b'A'),
  (0x5d, 7, b'B'),
  (0x5e, 7, b'C'),
  (0x5f, 7, b'D'),
  (0x60, 7, b'E'),
  (0x61, 7, b'F'),
  (0x62, 7, b'G'),
  (0x63, 7, b'H'),
  (0x64, 7, b'I'),
  (0x65, 7, b'J'),
  (0x66, 7, b'K'),
  (0x67, 7, b'L'),
  (0x68, 7, b'M'),
  (0x69, 7, b'N'),
  (0x6a, 7, b'O'),
  (0x6b, 7, b'P'),
  (0x6c, 7, b'Q'),
  (0x6d, 7, b'R'),
  (0x6e, 7, b'S'),
  (0x6f, 7, b'T'),
  (0x70, 7, b'U'),
  (0x71, 7, b'V'),
  (0x72, 7, b'W'),
  (0xfc, 8, b'X'),
  (0x73, 7, b'Y'),
  (0xfd, 8, b'Z'),
  (0x1ffb, 13, b'['),
  (0x1ffc, 13, b']'),
  (0x3ffc, 14, b'^'),
  (0x22, 6, b'_'),
  (0x7ffd, 15, b'`'),
  (0x3, 5, b'a'),
  (0x23, 6, b'b'),
  (0x4, 5, b'c'),
  (0x24, 6, b'd'),
  (0x5, 5, b'e'),
  (0x25, 6, b'f'),
  (0x26, 6, b'g'),
  (0x27, 6, b'h'),
  (0x6, 5, b'i'),
  (0x74, 7, b'j'),
  (0x75, 7, b'k'),
  (0x28, 6, b'l'),
  (0x29, 6, b'm'),
  (0x2a, 6, b'n'),
  (0x7, 5, b'o'),
  (0x2b, 6, b'p'),
  (0x76, 7, b'q'),
  (0x2c, 6, b'r'),
  (0x8, 5, b's'),
  (0x9, 5, b't'),
  (0x2d, 6, b'u'),
  (0x77, 7, b'v'),
  (0x78, 7, b'w'),
  (0x79, 7, b'x'),
  (0x7a, 7, b'y'),
  (0x7b, 7, b'z'),
  (0x7ffe, 15, b'{'),
  (0x7fc, 11, b'|'),
  (0x3ffd, 14, b'}'),
  (0x1ffd, 13, b'~'),
];

fn h2_string(block: &mut Vec<u8>, value: &[u8]) {
  assert!(
    value.len() < 128,
    "test HPACK helper only encodes short strings"
  );
  block.push(value.len() as u8);
  block.extend_from_slice(value);
}

fn h2_huffman_string(block: &mut Vec<u8>, value: &[u8]) {
  assert!(
    value.len() < 128,
    "test HPACK helper only encodes short strings"
  );
  block.push(0x80 | value.len() as u8);
  block.extend_from_slice(value);
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

fn spawn_ping_peer(
  flags: u8,
  stream_id: u32,
  payload: &'static [u8],
) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);
    write_frame(&mut stream, FRAME_PING, flags, stream_id, payload);
  });

  (addr, handle)
}

fn spawn_malformed_padding_peer(frame_type: u8) -> (SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 peer");
  let addr = listener.local_addr().expect("h2 peer addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept h2 client");
    complete_h2_request_handshake(&mut stream);

    match frame_type {
      FRAME_DATA => {
        write_frame(&mut stream, FRAME_HEADERS, FLAG_END_HEADERS, 1, &[0x88]);
        write_frame(
          &mut stream,
          FRAME_DATA,
          FLAG_PADDED | FLAG_END_STREAM,
          1,
          &[2, b'x'],
        );
      }
      FRAME_HEADERS => write_frame(
        &mut stream,
        FRAME_HEADERS,
        FLAG_PADDED | FLAG_END_HEADERS,
        1,
        &[2, 0x88],
      ),
      _ => unreachable!("unexpected frame type"),
    }
  });

  (addr, handle)
}
