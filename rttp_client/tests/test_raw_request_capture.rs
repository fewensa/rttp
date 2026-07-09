mod support;

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::types::Proxy;
use rttp_client::HttpClient;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn client() -> HttpClient {
  HttpClient::new()
}

fn capture_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_raw_http_request();
  request(format!("http://{}", addr));
  handle.join().expect("raw request capture server")
}

fn capture_optional_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
  request(format!("http://{}", addr));
  handle.join().expect("optional raw request capture server")
}

fn capture_proxy_request(request: impl FnOnce(Proxy)) -> Vec<u8> {
  let (addr, handle) = support::capture_raw_http_request();
  request(Proxy::http("127.0.0.1", u32::from(addr.port())));
  handle.join().expect("raw proxy request capture server")
}

fn request_text(request: &[u8]) -> String {
  String::from_utf8(request.to_vec()).expect("request should be utf-8")
}

fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
  request.lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    if header_name.eq_ignore_ascii_case(name) {
      Some(value.trim())
    } else {
      None
    }
  })
}

fn request_body(request: &[u8]) -> &[u8] {
  let body_start = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| position + 4)
    .expect("request should contain header terminator");
  &request[body_start..]
}

fn read_request_head(stream: &mut TcpStream) -> Vec<u8> {
  let mut request = Vec::new();
  let mut byte = [0u8; 1];
  while !request.ends_with(b"\r\n\r\n") {
    stream.read_exact(&mut byte).expect("read request head");
    request.push(byte[0]);
  }
  request
}

fn content_length(request: &[u8]) -> usize {
  request
    .split(|byte| *byte == b'\n')
    .find_map(|line| {
      let line = String::from_utf8_lossy(line);
      let (name, value) = line.split_once(':')?;
      if name.eq_ignore_ascii_case("Content-Length") {
        Some(value.trim().parse().expect("valid content length"))
      } else {
        None
      }
    })
    .unwrap_or(0)
}

fn spawn_streaming_then_capture_server() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind streaming reuse server");
  let addr = listener.local_addr().expect("streaming reuse server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept streaming request");
    let first_head = read_request_head(&mut stream);
    let mut body = vec![0u8; content_length(&first_head)];
    stream.read_exact(&mut body).expect("read streaming body");
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: keep-alive\r\n\r\nuploaded")
      .expect("write streaming response");

    let (mut stream, _) = listener.accept().expect("accept follow-up request");
    let second_head = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecond")
      .expect("write follow-up response");
    second_head
  });
  (addr, handle)
}

fn spawn_chunked_streaming_then_capture_server() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind chunked streaming reuse server");
  let addr = listener
    .local_addr()
    .expect("chunked streaming reuse server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept chunked streaming request");
    let _first_head = read_request_head(&mut stream);
    let mut body = Vec::new();
    let mut byte = [0u8; 1];
    while !body.ends_with(b"\r\n0\r\n\r\n") {
      stream.read_exact(&mut byte).expect("read chunked body");
      body.push(byte[0]);
    }
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: keep-alive\r\n\r\nuploaded")
      .expect("write chunked streaming response");

    let (mut stream, _) = listener.accept().expect("accept follow-up request");
    let second_head = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecond")
      .expect("write follow-up response");
    second_head
  });
  (addr, handle)
}

fn spawn_chunked_trailer_capture_server() -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind chunked trailer capture server");
  let addr = listener
    .local_addr()
    .expect("chunked trailer capture server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept chunked trailer request");
    let mut request = read_request_head(&mut stream);
    let mut body = Vec::new();
    let mut byte = [0u8; 1];
    loop {
      stream.read_exact(&mut byte).expect("read chunked body");
      body.push(byte[0]);
      let saw_final_chunk =
        body.starts_with(b"0\r\n") || body.windows(5).any(|window| window == b"\r\n0\r\n");
      if saw_final_chunk && body.ends_with(b"\r\n\r\n") {
        break;
      }
    }
    request.extend_from_slice(&body);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
      .expect("write chunked trailer response");
    request
  });
  (addr, handle)
}

#[test]
fn get_with_query_parameters_sends_request_target_without_body() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/search", base_url))
      .para("name=Julia")
      .para(("debug", "true"))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("GET /search?name=Julia&debug=true HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn range_helpers_emit_single_byte_range_headers() {
  let bounded = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .range(10, 19)
      .expect("bounded range should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let bounded = request_text(&bounded);
  assert_eq!(Some("bytes=10-19"), header_value(&bounded, "Range"));

  let open_ended = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .range_from(20)
      .emit()
      .expect("request should succeed");
  });
  let open_ended = request_text(&open_ended);
  assert_eq!(Some("bytes=20-"), header_value(&open_ended, "Range"));

  let suffix = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .range_suffix(128)
      .expect("suffix range should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let suffix = request_text(&suffix);
  assert_eq!(Some("bytes=-128"), header_value(&suffix, "Range"));
}

#[test]
fn range_helper_rejects_malformed_inputs_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .range(20, 10)
      .expect_err("inverted range should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "malformed range helper should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .range_suffix(0)
      .expect_err("empty suffix range should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "malformed suffix helper should not open a socket"
  );
}

#[test]
fn manual_range_header_remains_available_as_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .header(("Range", "bytes=5-9"))
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some("bytes=5-9"), header_value(&request, "Range"));
}

#[test]
fn conditional_request_helpers_emit_validator_headers() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .if_none_match(r#""abc123""#)
      .expect("etag should be accepted")
      .if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("http date should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some(r#""abc123""#), header_value(&request, "If-None-Match"));
  assert_eq!(
    Some("Sun, 06 Nov 1994 08:49:37 GMT"),
    header_value(&request, "If-Modified-Since")
  );

  let request = capture_request(|base_url| {
    client()
      .put()
      .url(format!("{}/asset", base_url))
      .if_match(r#"W/"weak-tag""#)
      .expect("weak etag syntax should be accepted")
      .if_unmodified_since("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("http date should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some(r#"W/"weak-tag""#), header_value(&request, "If-Match"));
  assert_eq!(
    Some("Sun, 06 Nov 1994 08:49:37 GMT"),
    header_value(&request, "If-Unmodified-Since")
  );
}

#[test]
fn conditional_request_helpers_reject_obvious_malformed_inputs_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_none_match("abc123")
      .expect_err("unquoted etag should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed etag helper should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_match(r#""one", "two""#)
      .expect_err("etag lists should stay on manual header escape hatch");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed etag helper should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_modified_since("not a date")
      .expect_err("invalid http date should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed http date helper should not open a socket"
  );
}

#[test]
fn manual_conditional_headers_remain_available_as_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .header(("If-None-Match", r#""one", "two""#))
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some(r#""one", "two""#),
    header_value(&request, "If-None-Match")
  );
}

#[test]
fn streaming_fixed_framing_does_not_leak_into_later_emit() {
  let (addr, handle) = spawn_streaming_then_capture_server();
  let mut client = client();

  client
    .post()
    .url(format!("http://{}/upload", addr))
    .emit_streaming_fixed("hello".as_bytes(), 5)
    .expect("streaming upload should succeed");

  client
    .get()
    .url(format!("http://{}/second", addr))
    .emit()
    .expect("follow-up request should succeed");

  let second = handle.join().expect("streaming reuse server");
  let second = request_text(&second);

  assert!(second.starts_with("GET /second HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&second, "Content-Length"));
  assert_eq!(None, header_value(&second, "Transfer-Encoding"));
}

#[test]
fn streaming_chunked_framing_does_not_leak_into_later_emit() {
  let (addr, handle) = spawn_chunked_streaming_then_capture_server();
  let mut client = client();

  client
    .post()
    .url(format!("http://{}/upload", addr))
    .emit_streaming_chunked("hello".as_bytes())
    .expect("chunked streaming upload should succeed");

  client
    .get()
    .url(format!("http://{}/second", addr))
    .emit()
    .expect("follow-up request should succeed");

  let second = handle.join().expect("chunked streaming reuse server");
  let second = request_text(&second);

  assert!(second.starts_with("GET /second HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&second, "Content-Length"));
  assert_eq!(None, header_value(&second, "Transfer-Encoding"));
}

#[test]
fn streaming_chunked_upload_sends_advertised_request_trailers_after_final_chunk() {
  let (addr, handle) = spawn_chunked_trailer_capture_server();

  client()
    .post()
    .url(format!("http://{}/upload", addr))
    .trailer(("X-Trace", "trace-123"))
    .expect("request trailer should be accepted")
    .trailer(("X-Signature", "sha256=abc"))
    .expect("request trailer should be accepted")
    .emit_streaming_chunked("hello trailers".as_bytes())
    .expect("chunked streaming upload should succeed");

  let request = handle.join().expect("chunked trailer capture server");
  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /upload HTTP/1.1\r\n"));
  assert_eq!(Some("chunked"), header_value(&text, "Transfer-Encoding"));
  assert_eq!(Some("X-Trace, X-Signature"), header_value(&text, "Trailer"));
  assert_eq!(
    b"e\r\nhello trailers\r\n0\r\nX-Trace: trace-123\r\nX-Signature: sha256=abc\r\n\r\n",
    body
  );
}

#[test]
fn streaming_fixed_upload_with_request_trailers_keeps_fixed_length_framing() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/upload", base_url))
      .trailer(("X-Trace", "trace-123"))
      .expect("request trailer should be accepted")
      .emit_streaming_fixed("hello".as_bytes(), 5)
      .expect("fixed streaming upload should succeed");
  });

  let text = request_text(&request);

  assert_eq!(Some("5"), header_value(&text, "Content-Length"));
  assert_eq!(None, header_value(&text, "Transfer-Encoding"));
  assert_eq!(None, header_value(&text, "Trailer"));
  assert_eq!(b"hello", request_body(&request));
}

#[test]
fn request_trailer_rejects_forbidden_field_names() {
  for name in [
    "Host",
    "Content-Length",
    "Transfer-Encoding",
    "Trailer",
    "Connection",
    "Upgrade",
    "Proxy-Authorization",
    "Proxy-Connection",
  ] {
    let error = client()
      .post()
      .trailer((name, "blocked"))
      .expect_err("forbidden request trailer should be rejected");
    assert!(
      error.to_string().contains("Forbidden request trailer"),
      "unexpected error for {name}: {error}"
    );
  }
}

#[test]
fn http_proxy_request_sends_absolute_form_request_target() {
  let request = capture_proxy_request(|proxy| {
    client()
      .get()
      .url("http://example.test/path?x=1")
      .proxy(proxy)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let request_line = text.lines().next().expect("request line");

  assert_eq!("GET http://example.test/path?x=1 HTTP/1.1", request_line);
}

#[test]
fn direct_http_request_sends_origin_form_request_target() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/path?x=1", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let request_line = text.lines().next().expect("request line");

  assert_eq!("GET /path?x=1 HTTP/1.1", request_line);
}

#[test]
fn head_without_body_omits_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .head()
      .url(format!("{}/metadata", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("HEAD /metadata HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn delete_without_body_omits_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .delete()
      .url(format!("{}/resource", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("DELETE /resource HTTP/1.1\r\n"));
  assert_eq!(None, header_value(&text, "Content-Type"));
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn bodyless_request_preserves_explicit_content_type_without_content_length() {
  let request = capture_request(|base_url| {
    client()
      .head()
      .url(format!("{}/metadata", base_url))
      .content_type("application/json")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);

  assert!(text.starts_with("HEAD /metadata HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/json"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(None, header_value(&text, "Content-Length"));
  assert_eq!(b"", request_body(&request));
}

#[test]
fn post_para_sends_form_urlencoded_body_and_matching_content_length() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/submit", base_url))
      .para("name=Julia")
      .para(("debug", "true"))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /submit HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/x-www-form-urlencoded"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(b"name=Julia&debug=true", body);
}

#[test]
fn raw_body_without_explicit_content_type_sends_text_plain() {
  let raw_body = "plain body";
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/raw", base_url))
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /raw HTTP/1.1\r\n"));
  assert_eq!(Some("text/plain"), header_value(&text, "Content-Type"));
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn raw_json_preserves_explicit_content_type_and_content_length() {
  let raw_body = r#"{"from":"rttp"}"#;
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/json", base_url))
      .content_type("application/json")
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /json HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/json"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn raw_body_preserves_existing_query_parameters_in_request_target() {
  let raw_body = "plain body";
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/raw?trace=abc&debug=true", base_url))
      .raw(raw_body)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /raw?trace=abc&debug=true HTTP/1.1\r\n"));
  assert_eq!(Some("text/plain"), header_value(&text, "Content-Type"));
  assert_eq!(
    Some(raw_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(raw_body.as_bytes(), body);
}

#[test]
fn binary_body_without_explicit_content_type_sends_octet_stream() {
  let binary_body = vec![0, 1, 2, 3];
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/binary", base_url))
      .binary(binary_body.clone())
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /binary HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/octet-stream"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(binary_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(binary_body.as_slice(), body);
}

#[test]
fn binary_body_preserves_existing_query_parameters_in_request_target() {
  let binary_body = vec![0, 1, 2, 3];
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/binary?trace=abc&debug=true", base_url))
      .binary(binary_body.clone())
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);

  assert!(text.starts_with("POST /binary?trace=abc&debug=true HTTP/1.1\r\n"));
  assert_eq!(
    Some("application/octet-stream"),
    header_value(&text, "Content-Type")
  );
  assert_eq!(
    Some(binary_body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert_eq!(binary_body.as_slice(), body);
}

#[test]
fn multipart_form_body_sends_generated_content_type_and_content_length() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/form", base_url))
      .form("name=Julia")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let body = request_body(&request);
  let content_type = header_value(&text, "Content-Type").expect("content type header");

  assert!(text.starts_with("POST /form HTTP/1.1\r\n"));
  assert!(content_type.starts_with("multipart/form-data; boundary="));
  assert_eq!(
    Some(body.len().to_string().as_str()),
    header_value(&text, "Content-Length")
  );
  assert!(body.starts_with(b"-----------------------------"));
  assert!(body.ends_with(b"--\r\n"));
}

#[test]
fn custom_common_headers_are_not_overwritten_by_auto_headers() {
  let request = capture_request(|base_url| {
    let authority = base_url
      .strip_prefix("http://")
      .expect("test URL should be http");
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", authority))
      .header("User-Agent: custom-agent/1.0")
      .header("Accept: application/json")
      .header("Connection: keep-alive")
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert_eq!(Some("custom-agent/1.0"), header_value(&text, "User-Agent"));
  assert_eq!(Some("application/json"), header_value(&text, "Accept"));
  assert_eq!(Some("keep-alive"), header_value(&text, "Connection"));
}

#[test]
fn matching_explicit_host_header_is_preserved() {
  let request = capture_request(|base_url| {
    let authority = base_url
      .strip_prefix("http://")
      .expect("test URL should be http");
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", authority))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
fn conflicting_explicit_host_header_is_rejected_before_sending_request() {
  let request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/headers", base_url))
      .header(("Host", "example.test"))
      .emit()
      .expect_err("conflicting host should be rejected");

    assert!(error.is_builder());
    assert!(error.to_string().contains("Host header"));
  });

  assert_eq!(b"", request.as_slice());
}

#[test]
fn missing_host_header_is_generated_from_url_authority() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/headers", base_url))
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let authority = text
    .lines()
    .find_map(|line| line.strip_prefix("Host: "))
    .expect("generated host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn async_matching_explicit_host_header_is_preserved() {
  let request = {
    let (addr, handle) = support::capture_raw_http_request();
    block_on(async {
      let base_url = format!("http://{}", addr);
      let authority = base_url
        .strip_prefix("http://")
        .expect("test URL should be http");
      client()
        .get()
        .url(format!("{}/headers", base_url))
        .header(("Host", authority))
        .rasync()
        .await
        .expect("request should succeed");
    });
    handle.join().expect("raw request capture server")
  };

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn async_conflicting_explicit_host_header_is_rejected_before_sending_request() {
  let request = {
    let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
    block_on(async {
      let error = client()
        .get()
        .url(format!("http://{}/headers", addr))
        .header(("Host", "example.test"))
        .rasync()
        .await
        .expect_err("conflicting host should be rejected");

      assert!(error.is_builder());
      assert!(error.to_string().contains("Host header"));
    });
    handle.join().expect("optional raw request capture server")
  };

  assert_eq!(b"", request.as_slice());
}

#[test]
#[cfg(feature = "async")]
fn async_missing_host_header_is_generated_from_url_authority() {
  let request = {
    let (addr, handle) = support::capture_raw_http_request();
    block_on(async {
      client()
        .get()
        .url(format!("http://{}/headers", addr))
        .rasync()
        .await
        .expect("request should succeed");
    });
    handle.join().expect("raw request capture server")
  };

  let text = request_text(&request);
  let authority = header_value(&text, "Host").expect("generated host header");

  assert!(authority.starts_with("127.0.0.1:"));
  assert!(text.starts_with("GET /headers HTTP/1.1\r\n"));
}

#[test]
fn connect_method_uses_authority_form_request_target() {
  let request = capture_request(|base_url| {
    client()
      .method("CONNECT")
      .url(base_url)
      .emit()
      .expect("request should succeed");
  });

  let text = request_text(&request);
  let request_line = text.lines().next().expect("request line");
  let host = header_value(&text, "Host").expect("host header");

  assert_eq!(format!("CONNECT {} HTTP/1.1", host), request_line);
}
