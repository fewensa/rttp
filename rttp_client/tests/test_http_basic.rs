mod support;

use std::collections::HashMap;
use std::io::{self, Cursor, Read, Write};
use std::net::TcpListener;
use std::thread;

use rttp_client::types::{Auth, Para, Proxy, RoUrl};
use rttp_client::{Config, HttpClient};

fn client() -> HttpClient {
  HttpClient::new()
}

fn spawn_streaming_upload_capture_server() -> (std::net::SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upload capture server");
  let addr = listener.local_addr().expect("upload capture addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upload");
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
      stream.read_exact(&mut byte).expect("read request head");
      request.push(byte[0]);
    }
    while !request.ends_with(b"\r\n0\r\n\r\n") {
      if stream.read_exact(&mut byte).is_err() {
        return request;
      }
      request.push(byte[0]);
    }
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nuploaded")
      .expect("write response");
    request
  });
  (addr, handle)
}

fn spawn_fixed_upload_capture_server() -> (std::net::SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixed upload capture server");
  let addr = listener.local_addr().expect("fixed upload capture addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept fixed upload");
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
      stream.read_exact(&mut byte).expect("read request head");
      request.push(byte[0]);
    }
    let mut body = [0u8; 12];
    stream.read_exact(&mut body).expect("read fixed body");
    request.extend_from_slice(&body);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
      .expect("write response");
    request
  });
  (addr, handle)
}

struct FailingReader {
  sent: bool,
}

impl Read for FailingReader {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.sent {
      return Err(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "client cancelled",
      ));
    }
    self.sent = true;
    buf[..5].copy_from_slice(b"hello");
    Ok(5)
  }
}

#[test]
fn test_http() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client().url(format!("http://{}/get", addr)).emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
  println!("{}", response);
}

#[test]
fn test_streaming_chunked_upload_writes_incremental_framing() {
  let (addr, handle) = spawn_streaming_upload_capture_server();
  let payload = vec![b'a'; 96 * 1024];
  let response = client()
    .post()
    .url(format!("http://{}/upload", addr))
    .emit_streaming_chunked(Cursor::new(payload.clone()))
    .expect("stream chunked upload");

  assert_eq!("uploaded", response.body().string().unwrap());

  let request = handle.join().expect("upload server thread");
  let request = String::from_utf8(request).expect("request utf8");
  assert!(request.starts_with("POST /upload HTTP/1.1\r\n"));
  assert!(request.contains("\r\nTransfer-Encoding: chunked\r\n"));
  assert!(!request.contains("\r\nContent-Length:"));
  assert!(request.ends_with("\r\n0\r\n\r\n"));
}

#[test]
fn test_streaming_fixed_upload_writes_content_length() {
  let (addr, handle) = spawn_fixed_upload_capture_server();
  let response = client()
    .post()
    .url(format!("http://{}/fixed", addr))
    .emit_streaming_fixed(Cursor::new(b"request body".to_vec()), 12)
    .expect("stream fixed upload");

  assert_eq!("OK", response.body().string().unwrap());

  let request = handle.join().expect("expect server thread");
  assert!(String::from_utf8_lossy(&request).contains("\r\nContent-Length: 12\r\n"));
  assert!(request.ends_with(b"request body"));
}

#[test]
fn test_streaming_upload_returns_reader_eof_error() {
  let (addr, _handle) = spawn_streaming_upload_capture_server();
  let error = client()
    .post()
    .url(format!("http://{}/cancel", addr))
    .emit_streaming_chunked(FailingReader { sent: false })
    .expect_err("streaming reader error should abort upload");

  assert!(error.to_string().contains("client cancelled"));
}

#[test]
fn test_multi() {
  let (addr, _handle) = support::spawn_http_server();
  let mut para_map = HashMap::new();
  para_map.insert("id", "1");
  para_map.insert("relation", "eq");
  let response = client()
    .method("post")
    .url(RoUrl::with(format!("http://{}/?id=1&name=jack#none", addr)).para("name=Julia"))
    .path("post")
    .header("User-Agent: Mozilla/5.0")
    .header(("Host", addr.to_string().as_str()))
    .para("name=Chico")
    .para(&"name=文".to_string())
    .para(para_map)
    .form(("debug", "true", "name=Form"))
    .cookie("token=123234")
    .cookie("uid=abcdef")
    .content_type("application/x-www-form-urlencoded")
    .encode(true)
    .traditional(true)
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_gzip() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .header(("Accept-Encoding", "gzip, deflate"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_invalid_gzip_returns_error_instead_of_panicking() {
  let (addr, _handle) = support::spawn_invalid_gzip_server();
  let result =
    std::panic::catch_unwind(|| client().get().url(format!("http://{}/gzip", addr)).emit());

  assert!(result.is_ok());
  assert!(result.unwrap().is_err());
}

#[test]
fn test_chunked() {
  let (addr, _handle) = support::spawn_chunked_server();
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(
    Some(&"chunked".to_string()),
    response.header_value("Transfer-Encoding")
  );
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some("abc"),
    response.trailer("x-trace").map(|h| h.value().as_str())
  );
  assert_eq!(
    Some("signed"),
    response.trailer("X-SIGNATURE").map(|h| h.value().as_str())
  );
}

#[test]
fn test_chunked_valid_extension_preserves_trailers_without_leaking_extension() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "4;foo=bar\r\nWiki\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ));

  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("Wiki", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some("abc"),
    response.trailer("x-trace").map(|h| h.value().as_str())
  );
  assert_eq!(
    Some("signed"),
    response.trailer_value("X-SIGNATURE").map(String::as_str)
  );
  assert!(response.trailer("foo").is_none());
  assert!(response.trailer_value("foo").is_none());
}

#[test]
fn test_socket2_server_chunked_trailers_are_exposed_case_insensitively() {
  let (addr, _handle) = support::spawn_socket2_chunked_trailer_server();
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("socket2 chunked body", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some("abc"),
    response.trailer("x-trace").map(|h| h.value().as_str())
  );
  assert_eq!(
    Some("abc"),
    response.trailer_value("X-TRACE").map(String::as_str)
  );
  assert_eq!(
    Some("signed"),
    response.trailer("X-SIGNATURE").map(|h| h.value().as_str())
  );
  assert_eq!(
    Some("signed"),
    response.trailer_value("x-signature").map(String::as_str)
  );
}

#[test]
fn test_chunked_without_trailers_exposes_empty_trailers() {
  let (addr, _handle) = support::spawn_chunked_server_without_trailers();
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
  assert!(response.trailers().is_empty());
  assert!(response.trailer("x-trace").is_none());
}

#[test]
fn test_chunked_with_trailers_decodes_body() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=bar\r\nchunked\r\n",
    "6\r\n body!\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ));

  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("chunked body!", response.body().string().unwrap());
}

#[test]
fn test_chunked_quoted_extensions_are_accepted() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=\"bar;baz\";answer=42\r\nchunked\r\n",
    "6;empty;quoted=\"\\\\\\\"\"\r\n body!\r\n",
    "0;done=\"yes\"\r\n",
    "X-Trace: abc\r\n",
    "\r\n"
  ));

  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(
    Some("abc"),
    response.trailer("x-trace").map(|h| h.value().as_str())
  );
}

#[test]
fn test_chunked_quoted_extensions_accept_obs_text() {
  let mut response = b"HTTP/1.1 200 OK\r\n\
Transfer-Encoding: chunked\r\n\
Connection: close\r\n\
\r\n\
7;meta=\""
    .to_vec();
  response.push(0xff);
  response.extend_from_slice(b"\"\r\nchunked\r\n0\r\n\r\n");

  let (addr, _handle) = support::spawn_chunked_response_server(response);
  let response = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .unwrap();

  assert_eq!("chunked", response.body().string().unwrap());
}

#[test]
fn test_transfer_encoding_chunked_with_content_length_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 2\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("ambiguous response framing should be rejected");

  assert!(
    error
      .to_string()
      .contains("Transfer-Encoding conflicts with Content-Length"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_non_chunked_transfer_coding_before_chunked_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: gzip, chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("unsupported transfer coding should be rejected");

  assert!(
    error
      .to_string()
      .contains("Unsupported Transfer-Encoding response body"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_malformed_extension_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;bad name=value\r\nchunked\r\n",
    "0\r\n",
    "\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("malformed chunk extension should be rejected");

  assert!(
    error.to_string().contains("Invalid chunk extension"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_malformed_size_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "z\r\n",
    "OK\r\n",
    "0\r\n",
    "\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("malformed chunk size should be rejected");

  assert!(
    error.to_string().starts_with("error receive response")
      && error.to_string().contains("Invalid chunk size"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_missing_crlf_after_chunk_data_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK",
    "0\r\n",
    "\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("missing chunk data terminator should be rejected");

  assert!(
    error.to_string().contains("Invalid chunk terminator"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_missing_final_zero_chunk_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("missing final zero chunk should be rejected");

  assert!(
    error.to_string().starts_with("error receive response")
      && error.to_string().contains("Unexpected end of chunked body"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_truncated_trailer_block_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "X-Trace: abc"
  ));

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("truncated chunk trailer block should be rejected");

  assert!(
    error.to_string().starts_with("error receive response")
      && error.to_string().contains("Unexpected end of chunked body"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_oversized_extension_is_rejected() {
  let extension = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7;foo={extension}\r\n\
     chunked\r\n\
     0\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("oversized chunk extension should be rejected");

  assert!(
    error
      .to_string()
      .contains("chunked response line is too large"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_chunked_oversized_trailer_is_rejected() {
  let trailer = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7\r\n\
     chunked\r\n\
     0\r\n\
     X-Trace: {trailer}\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("oversized chunk trailer should be rejected");

  assert!(
    error
      .to_string()
      .contains("chunked response line is too large"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_forbidden_chunked_response_trailer_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "WWW-Authenticate: unsafe\r\n",
    "\r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  let error = client()
    .get()
    .url(format!("http://{}/chunked", addr))
    .emit()
    .expect_err("forbidden chunk trailer should be rejected");

  assert!(
    error.to_string().contains("Forbidden trailer header"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_duplicate_set_cookie_headers_are_preserved() {
  let (addr, _handle) = support::spawn_duplicate_set_cookie_server();
  let response = client()
    .get()
    .url(format!("http://{}/cookies", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  let duplicate_headers = response
    .headers()
    .iter()
    .filter(|header| {
      header.name().eq_ignore_ascii_case("set-cookie")
        || header.name().eq_ignore_ascii_case("cache-control")
    })
    .map(|header| (header.name().as_str(), header.value().as_str()))
    .collect::<Vec<_>>();
  assert_eq!(
    vec![
      ("Set-Cookie", "session=abc; Path=/; HttpOnly"),
      ("cache-control", "no-cache"),
      ("SET-COOKIE", "theme=dark; Path=/; SameSite=Lax"),
      ("Cache-Control", "max-age=60")
    ],
    duplicate_headers
  );
  assert_eq!(
    vec![
      &"session=abc; Path=/; HttpOnly".to_string(),
      &"theme=dark; Path=/; SameSite=Lax".to_string()
    ],
    response.header_values("set-cookie")
  );
  assert_eq!(
    vec![&"no-cache".to_string(), &"max-age=60".to_string()],
    response.header_values("CACHE-CONTROL")
  );
  assert_eq!(
    Some(&"session=abc; Path=/; HttpOnly".to_string()),
    response.header_value("set-cookie")
  );
  assert_eq!(2, response.cookies().len());
  assert!(response.cookie("session").is_some());
  assert!(response.cookie("theme").is_some());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("content-type")
  );
}

#[test]
fn test_content_length_response_does_not_wait_for_eof() {
  let (addr, _handle) = support::spawn_keep_alive_server();
  let response = client()
    .get()
    .config(Config::builder().read_timeout(100))
    .url(format!("http://{}/keep-alive", addr))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_sync_client_skips_100_continue_before_final_response() {
  let (addr, _handle) = support::spawn_continue_then_ok_server();
  let response = client()
    .post()
    .url(format!("http://{}/continue", addr))
    .header(("Expect", "100-continue"))
    .raw("request body")
    .emit()
    .unwrap();

  assert_eq!(200, response.code());
  assert_eq!("OK", response.reason());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("Content-Type")
  );
  assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
  assert!(response.header_value("X-Interim").is_none());
  assert_eq!("final body", response.body().string().unwrap());
}

#[test]
fn test_sync_client_waits_for_100_continue_before_sending_body() {
  let (addr, handle) = support::spawn_expect_continue_gate_server();
  let response = client()
    .post()
    .url(format!("http://{}/continue-gate", addr))
    .header(("Expect", "100-continue"))
    .raw("request body")
    .emit()
    .unwrap();

  assert_eq!(200, response.code());
  assert_eq!("accepted", response.body().string().unwrap());

  let request = handle.join().expect("expect continue gate thread");
  assert!(!request.is_empty(), "body was sent before 100 Continue");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(b"request body"));
}

#[test]
fn test_sync_client_does_not_send_body_when_expect_continue_gets_final_response() {
  let (addr, handle) = support::spawn_expect_continue_reject_gate_server();
  let response = client()
    .post()
    .url(format!("http://{}/continue-reject", addr))
    .header(("Expect", "100-continue"))
    .raw("request body")
    .emit()
    .unwrap();

  assert_eq!(417, response.code());
  assert_eq!("Expectation Failed", response.body().string().unwrap());

  let request = handle.join().expect("expect continue reject gate thread");
  assert!(
    !request.is_empty(),
    "body was sent before final expectation response"
  );
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(!request.ends_with(b"request body"));
}

#[test]
fn test_sync_client_skips_103_early_hints_before_final_response() {
  let (addr, _handle) = support::spawn_informational_then_ok_server("103 Early Hints");
  let response = client()
    .get()
    .url(format!("http://{}/early-hints", addr))
    .emit()
    .unwrap();

  assert_eq!(200, response.code());
  assert_eq!("OK", response.reason());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("Content-Type")
  );
  assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
  assert!(response.header_value("X-Interim").is_none());
  assert_eq!("final body", response.body().string().unwrap());
}

#[test]
fn test_sync_client_returns_101_switching_protocols_as_terminal_response() {
  let (addr, _handle) = support::spawn_switching_protocols_server();
  let response = client()
    .get()
    .url(format!("http://{}/upgrade", addr))
    .emit()
    .unwrap();

  assert_eq!(101, response.code());
  assert_eq!("Switching Protocols", response.reason());
  assert_eq!(
    Some(&"Upgrade".to_string()),
    response.header_value("Connection")
  );
  assert_eq!(
    Some(&"websocket".to_string()),
    response.header_value("Upgrade")
  );
  assert_eq!(
    Some(&"test-accept".to_string()),
    response.header_value("Sec-WebSocket-Accept")
  );
  assert_eq!("", response.body().string().unwrap());
}

#[test]
fn test_upload() {
  let (addr, _handle) = support::spawn_http_server();
  let response = client()
    .method("post")
    .url(format!("http://{}/post", addr))
    .form(("debug", "true", "name=Form"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_raw_json() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("post")
    .url(format!("http://{}/post?raw=json", addr))
    .para("name=Chico")
    .content_type("application/json")
    .raw(r#"  {"from": "rttp"} "#)
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_raw_form_urlencoded() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("post")
    .url(format!("http://{}/post", addr))
    .para(Para::with_form("name", "Chico"))
    .raw("name=Nick&name=Wendy")
    .content_type("application/x-www-form-urlencoded")
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(feature = "tls-rustls")]
fn test_https() {
  let (addr, _handle) = support::spawn_tls_server();
  let response = client()
    .get()
    .url(format!("https://{}/", addr))
    .config(
      Config::builder()
        .verify_ssl_cert(false)
        .verify_ssl_hostname(false),
    )
    .para(Para::with_form("q", "News"))
    .emit();
  assert!(response.is_ok());
}

#[test]
fn test_http_with_url() {
  let (addr, _handle) = support::spawn_http_server();
  client()
    .method("get")
    .url(
      RoUrl::with(format!("http://{}", addr))
        .path("/get")
        .para(("name", "Chico")),
    )
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(any(feature = "tls-rustls", feature = "tls-native"))]
#[ignore]
fn test_with_proxy_http() {
  client()
    .get()
    .url("https://example.test")
    .proxy(Proxy::http("127.0.0.1", 1081))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_with_proxy_socks5() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) = support::spawn_socks5_proxy_server();
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .proxy(Proxy::socks5("127.0.0.1", proxy_addr.port().into()))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_with_proxy_socks5_auth() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) =
    support::spawn_socks5_proxy_server_with_credentials("username", "password");
  let response = client()
    .get()
    .url(format!("http://{}/get", addr))
    .proxy(Proxy::socks5_with_authorization(
      "127.0.0.1",
      proxy_addr.port().into(),
      "username",
      "password",
    ))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
}

#[test]
fn test_auto_redirect() {
  let (addr, _handle) = support::spawn_redirect_server();
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/", addr))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert!(response.ok());
}

fn assert_redirect_resolves_to_target<F>(location: F, expected_target: &str)
where
  F: FnOnce(std::net::SocketAddr) -> String + Send + 'static,
{
  let (addr, _handle) = support::spawn_redirect_target_echo_server(location);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect/from?old=1", addr))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(expected_target, response.body().string().unwrap());
}

fn assert_redirect_error_has_url_context(
  error: &rttp_client::error::Error,
  message: &str,
  expected_path: &str,
) {
  assert!(error.is_redirect());
  let error_message = error.to_string();
  assert!(error_message.contains(message));
  assert!(error_message.contains(" for url (http://"));
  assert!(error_message.contains(expected_path));
  assert!(!error_message.contains("Authorization"));
  assert!(!error_message.contains("Bearer secret"));
  assert!(!error_message.contains("Cookie"));
  assert!(!error_message.contains("session=secret"));
  assert!(!error_message.contains("Proxy-Authorization"));
  assert!(!error_message.contains("proxy-secret"));
  assert_eq!(
    expected_path,
    error.url().expect("redirect error url").path()
  );
}

struct CapturedRequest {
  method: String,
  target: String,
  headers: HashMap<String, String>,
  body: Vec<u8>,
}

fn captured_request(request: Vec<u8>) -> CapturedRequest {
  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .expect("captured request headers");
  let header = String::from_utf8_lossy(&request[..header_end]);
  let mut lines = header.lines();
  let request_line = lines.next().expect("captured request line");
  let mut request_line_parts = request_line.split_whitespace();
  let method = request_line_parts
    .next()
    .expect("captured request method")
    .to_string();
  let target = request_line_parts
    .nth(0)
    .expect("captured request target")
    .to_string();
  let headers = lines
    .filter_map(|line| {
      let (name, value) = line.split_once(':')?;
      Some((name.to_ascii_lowercase(), value.trim().to_string()))
    })
    .collect();
  let body = request[header_end + 4..].to_vec();

  CapturedRequest {
    method,
    target,
    headers,
    body,
  }
}

fn captured_redirected_post(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .post()
    .url(format!("http://{}/redirect", addr))
    .raw("redirect-body")
    .emit();

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

fn captured_redirected_put(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .put()
    .url(format!("http://{}/redirect", addr))
    .raw("redirect-body")
    .emit();

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

fn captured_redirected_head(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .head()
    .url(format!("http://{}/redirect", addr))
    .emit();

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[test]
fn test_auto_redirect_301_post_becomes_get_without_body_or_body_framing() {
  let request = captured_redirected_post(301, "Moved Permanently");

  assert_eq!("GET", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"", request.body.as_slice());
  assert!(!request.headers.contains_key("content-length"));
  assert!(!request.headers.contains_key("content-type"));
  assert!(!request.headers.contains_key("transfer-encoding"));
}

#[test]
fn test_auto_redirect_302_post_becomes_get_without_body_or_body_framing() {
  let request = captured_redirected_post(302, "Found");

  assert_eq!("GET", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"", request.body.as_slice());
  assert!(!request.headers.contains_key("content-length"));
  assert!(!request.headers.contains_key("content-type"));
  assert!(!request.headers.contains_key("transfer-encoding"));
}

#[test]
fn test_auto_redirect_301_put_preserves_method_body_and_body_framing() {
  let request = captured_redirected_put(301, "Moved Permanently");

  assert_eq!("PUT", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"redirect-body", request.body.as_slice());
  assert_eq!(
    Some("13"),
    request.headers.get("content-length").map(String::as_str)
  );
  assert_eq!(
    Some("text/plain"),
    request.headers.get("content-type").map(String::as_str)
  );
}

#[test]
fn test_auto_redirect_302_put_preserves_method_body_and_body_framing() {
  let request = captured_redirected_put(302, "Found");

  assert_eq!("PUT", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"redirect-body", request.body.as_slice());
  assert_eq!(
    Some("13"),
    request.headers.get("content-length").map(String::as_str)
  );
  assert_eq!(
    Some("text/plain"),
    request.headers.get("content-type").map(String::as_str)
  );
}

#[test]
fn test_auto_redirect_303_post_becomes_get_without_body_or_body_framing() {
  let request = captured_redirected_post(303, "See Other");

  assert_eq!("GET", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"", request.body.as_slice());
  assert!(!request.headers.contains_key("content-length"));
  assert!(!request.headers.contains_key("content-type"));
  assert!(!request.headers.contains_key("transfer-encoding"));
}

#[test]
fn test_auto_redirect_303_post_allows_same_url_after_method_changes() {
  let (addr, _handle) = support::spawn_same_url_303_redirect_method_echo_server();
  let response = client()
    .config(Config::builder().auto_redirect(true).max_redirect(1))
    .post()
    .url(format!("http://{}/submit", addr))
    .raw("redirect-body")
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("GET /submit HTTP/1.1", response.body().string().unwrap());
}

#[test]
fn test_auto_redirect_303_head_preserves_method() {
  let request = captured_redirected_head(303, "See Other");

  assert_eq!("HEAD", request.method);
  assert_eq!("/final?via=redirect", request.target);
}

#[test]
fn test_auto_redirect_307_post_preserves_method_body_and_body_framing() {
  let request = captured_redirected_post(307, "Temporary Redirect");

  assert_eq!("POST", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"redirect-body", request.body.as_slice());
  assert_eq!(
    Some("13"),
    request.headers.get("content-length").map(String::as_str)
  );
  assert_eq!(
    Some("text/plain"),
    request.headers.get("content-type").map(String::as_str)
  );
}

#[test]
fn test_auto_redirect_308_post_preserves_method_body_and_body_framing() {
  let request = captured_redirected_post(308, "Permanent Redirect");

  assert_eq!("POST", request.method);
  assert_eq!("/final?via=redirect", request.target);
  assert_eq!(b"redirect-body", request.body.as_slice());
  assert_eq!(
    Some("13"),
    request.headers.get("content-length").map(String::as_str)
  );
  assert_eq!(
    Some("text/plain"),
    request.headers.get("content-type").map(String::as_str)
  );
}

#[test]
fn test_auto_redirect_resolves_absolute_location() {
  assert_redirect_resolves_to_target(|addr| format!("http://{}/final", addr), "/final");
}

#[test]
fn test_auto_redirect_resolves_absolute_path_location() {
  assert_redirect_resolves_to_target(|_| "/absolute-path".to_string(), "/absolute-path");
}

#[test]
fn test_auto_redirect_resolves_relative_child_location() {
  assert_redirect_resolves_to_target(|_| "relative-child".to_string(), "/redirect/relative-child");
}

#[test]
fn test_auto_redirect_resolves_parent_relative_location() {
  assert_redirect_resolves_to_target(|_| "../sibling".to_string(), "/sibling");
}

#[test]
fn test_auto_redirect_preserves_trailing_slash_for_dot_segment_location() {
  assert_redirect_resolves_to_target(|_| ".".to_string(), "/redirect/");
}

#[test]
fn test_auto_redirect_preserves_trailing_slash_for_parent_dot_segment_location() {
  assert_redirect_resolves_to_target(|_| "/a/b/..".to_string(), "/a/");
}

#[test]
fn test_auto_redirect_resolves_query_only_location() {
  assert_redirect_resolves_to_target(|_| "?query-only".to_string(), "/redirect/from?query-only");
}

#[test]
fn test_auto_redirect_resolves_location_with_fragment_without_sending_fragment() {
  assert_redirect_resolves_to_target(
    |_| "fragment-child#section".to_string(),
    "/redirect/fragment-child",
  );
}

#[test]
fn test_auto_redirect_resolves_absolute_location_with_fragment_without_sending_fragment() {
  assert_redirect_resolves_to_target(
    |addr| format!("http://{}/absolute-fragment#section", addr),
    "/absolute-fragment",
  );
}

#[test]
fn test_auto_redirect_preserves_percent_encoded_path_and_query_octets() {
  assert_redirect_resolves_to_target(
    |_| "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b".to_string(),
    "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b",
  );
}

#[test]
fn test_auto_redirect_uses_preserved_percent_encoded_path_as_relative_base() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/files/%2e%2e/a/"),
      ("/files/%2e%2e/a/", "next%2fhop?token=%2e%2e"),
    ],
    3,
  );
  let response = client()
    .config(Config::builder().auto_redirect(true).max_redirect(2))
    .get()
    .url(format!("http://{}/start", addr))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(
    "/files/%2e%2e/a/next%2fhop?token=%2e%2e",
    response.body().string().unwrap()
  );
}

#[test]
fn test_auto_redirect_preserves_chain_that_finishes_within_max_redirect() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![("/start", "/hop-one"), ("/hop-one", "/final?done=1")],
    3,
  );
  let response = client()
    .config(Config::builder().auto_redirect(true).max_redirect(2))
    .get()
    .url(format!("http://{}/start", addr))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("/final?done=1", response.body().string().unwrap());
}

#[test]
fn test_auto_redirect_enforces_max_redirect_bound() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/hop-one"),
      ("/hop-one", "/hop-two"),
      ("/hop-two", "/final"),
    ],
    3,
  );
  let error = client()
    .config(Config::builder().auto_redirect(true).max_redirect(2))
    .get()
    .url(format!("http://{}/start", addr))
    .emit()
    .expect_err("redirect chain should exceed max_redirect");

  assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-two");
}

#[test]
fn test_auto_redirect_with_zero_max_redirect_fails_before_first_hop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/start", "/final?done=1")], 1);
  let error = client()
    .config(Config::builder().auto_redirect(true).max_redirect(0))
    .get()
    .url(format!("http://{}/start", addr))
    .header(("Authorization", "Bearer secret"))
    .header(("Cookie", "session=secret"))
    .header(("Proxy-Authorization", "Basic proxy-secret"))
    .emit()
    .expect_err("max_redirect=0 should reject the first redirect");

  assert_redirect_error_has_url_context(&error, "too many redirects", "/start");
}

#[test]
fn test_auto_redirect_with_one_max_redirect_fails_on_second_hop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![("/start", "/hop-one"), ("/hop-one", "/final?done=1")],
    2,
  );
  let error = client()
    .config(Config::builder().auto_redirect(true).max_redirect(1))
    .get()
    .url(format!("http://{}/start", addr))
    .emit()
    .expect_err("max_redirect=1 should allow one redirect and reject the second");

  assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-one");
}

#[test]
fn test_auto_redirect_uses_default_max_redirect_when_enabled() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/hop-one"),
      ("/hop-one", "/hop-two"),
      ("/hop-two", "/hop-three"),
      ("/hop-three", "/hop-four"),
      ("/hop-four", "/hop-five"),
      ("/hop-five", "/final"),
    ],
    6,
  );
  let error = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/start", addr))
    .emit()
    .expect_err("auto_redirect default max should reject the sixth redirect");

  assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-five");
}

#[test]
fn test_auto_redirect_detects_a_b_a_loop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/a", "/b"), ("/b", "/a")], 3);
  let error = client()
    .config(Config::builder().auto_redirect(true).max_redirect(10))
    .get()
    .url(format!("http://{}/a", addr))
    .header(("Authorization", "Bearer secret"))
    .header(("Cookie", "session=secret"))
    .header(("Proxy-Authorization", "Basic proxy-secret"))
    .emit()
    .expect_err("A -> B -> A should be detected as a loop");

  assert_redirect_error_has_url_context(&error, "infinite redirect loop detected", "/b");
}

#[test]
fn test_auto_redirect_detects_self_redirect() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/self", "/self")], 1);
  let error = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/self", addr))
    .emit()
    .expect_err("self redirect should be detected as a loop");

  assert_redirect_error_has_url_context(&error, "infinite redirect loop detected", "/self");
}

#[test]
fn test_auto_redirect_detects_loop_after_relative_location_is_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/redirect/from?old=1", "?old=1")], 8);
  let error = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect/from?old=1", addr))
    .emit()
    .expect_err("redirect should resolve back to current URL");

  assert!(error.is_redirect());
  assert!(error
    .to_string()
    .contains("infinite redirect loop detected"));
}

#[test]
fn test_auto_redirect_detects_loop_after_dot_segments_are_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/a/current", "../a/current")], 8);
  let error = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/a/current", addr))
    .emit()
    .expect_err("redirect should normalize back to current URL");

  assert!(error.is_redirect());
  assert!(error
    .to_string()
    .contains("infinite redirect loop detected"));
}

#[test]
fn test_auto_redirect_strips_sensitive_headers_for_cross_authority_location() {
  let (origin_addr, _target_addr, _handle) =
    support::spawn_cross_authority_redirect_header_echo_server();
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect", origin_addr))
    .header(("Authorization", "Bearer secret"))
    .header(("Cookie", "session=secret"))
    .header(("Proxy-Authorization", "Basic proxy-secret"))
    .header(("X-Trace", "trace-123"))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(
    "authorization=\ncookie=\nproxy-authorization=\nx-trace=trace-123",
    response.body().string().unwrap()
  );
}

#[test]
fn test_auto_redirect_strips_sensitive_headers_and_userinfo_for_cross_authority_location() {
  let (origin_addr, _target_addr, _handle) =
    support::spawn_cross_authority_redirect_userinfo_echo_server();
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect", origin_addr))
    .header(("Authorization", "Bearer secret"))
    .header(("Cookie", "session=secret"))
    .header(("Proxy-Authorization", "Basic proxy-secret"))
    .header(("X-Trace", "trace-123"))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(
    "request-target=/final\nauthorization=\ncookie=\nproxy-authorization=\nx-trace=trace-123",
    response.body().string().unwrap()
  );
}

#[test]
fn test_auto_redirect_preserves_sensitive_headers_for_same_authority_location() {
  let (addr, _handle) = support::spawn_same_authority_redirect_header_echo_server();
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect", addr))
    .header(("Authorization", "Bearer secret"))
    .header(("Cookie", "session=secret"))
    .header(("Proxy-Authorization", "Basic proxy-secret"))
    .header(("X-Trace", "trace-123"))
    .emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(
    "authorization=Bearer secret\ncookie=session=secret\nproxy-authorization=Basic proxy-secret\nx-trace=trace-123",
    response.body().string().unwrap()
  );
}

#[test]
fn test_http_proxy_uses_absolute_form_for_http_requests() {
  let (addr, _handle) = support::spawn_http_proxy_server();
  let response = client()
    .get()
    .url("http://example.test/proxy?q=1")
    .proxy(Proxy::http("127.0.0.1", u32::from(addr.port())))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!(
    "GET http://example.test/proxy?q=1 HTTP/1.1",
    response.body().string().unwrap()
  );
}

#[test]
fn test_http_proxy_with_auth_uses_proxy_authorization_header() {
  let (addr, _handle) = support::spawn_http_proxy_auth_echo_server();
  let response = client()
    .get()
    .url("http://example.test/proxy?q=1")
    .proxy(Proxy::http_with_authorization(
      "127.0.0.1",
      u32::from(addr.port()),
      "user",
      "secret",
    ))
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
}

#[test]
#[cfg(feature = "tls-rustls")]
fn test_https_proxy_with_auth_uses_connect_tunnel() {
  let (proxy_addr, target_addr, _proxy_handle) =
    support::spawn_https_proxy_server_with_credentials("user", "secret");
  let response = client()
    .get()
    .url(format!("https://localhost:{}/", target_addr.port()))
    .proxy(Proxy::http_with_authorization(
      "127.0.0.1",
      u32::from(proxy_addr.port()),
      "user",
      "secret",
    ))
    .config(
      Config::builder()
        .verify_ssl_cert(false)
        .verify_ssl_hostname(false),
    )
    .emit();
  assert!(response.is_ok());

  let response = response.unwrap();
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_connection_closed() {
  let (addr, _handle) = support::spawn_http_server_count(5);
  let mut client = client();
  let resp0 = client.url(format!("http://{}/get", addr)).emit();
  assert!(resp0.is_ok());
  let resp1 = client.post().url(format!("http://{}/post", addr)).emit();
  assert!(resp1.is_err());
  let resp2 = self::client().url(format!("http://{}/get", addr)).emit();
  assert!(resp2.is_ok());
  let resp3 = self::client()
    .post()
    .url(format!("http://{}/post", addr))
    .emit();
  assert!(resp3.is_ok());
  let resp4 = client
    .reset()
    .post()
    .url(format!("http://{}/post", addr))
    .emit();
  assert!(resp4.is_ok());
}

#[test]
fn test_eof_delimited_response_body_is_read_to_connection_close() {
  let (addr, _handle) = support::spawn_eof_delimited_response_server("connection delimited");
  let response = client().url(format!("http://{}/eof", addr)).emit();

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("connection delimited", response.body().string().unwrap());
  assert_eq!(
    Some(&"close".to_string()),
    response.header_value("Connection")
  );
}

#[test]
fn test_truncated_content_length_response_is_rejected() {
  let (addr, _handle) = support::spawn_truncated_content_length_server();
  let error = client()
    .url(format!("http://{}/truncated", addr))
    .emit()
    .expect_err("truncated fixed-length body should be rejected");

  assert!(
    error.to_string().contains("failed to fill whole buffer")
      || error.to_string().contains("unexpected end of file"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_basic_auth() {
  let (addr, _handle) = support::spawn_auth_echo_server();
  let response = client()
    .get()
    .url(format!("http://{}/", addr))
    .auth(Auth::basic("user", "secret"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  // base64("user:secret") = "dXNlcjpzZWNyZXQ="
  assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
}

#[test]
fn test_bearer_auth() {
  let (addr, _handle) = support::spawn_auth_echo_server();
  let response = client()
    .get()
    .url(format!("http://{}/", addr))
    .auth(Auth::bearer("my-token-abc"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("Bearer my-token-abc", response.body().string().unwrap());
}
