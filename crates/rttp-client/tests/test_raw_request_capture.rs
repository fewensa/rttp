use rttp_test_support as support;

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::types::{Auth, Header, Proxy};
use rttp_client::{HttpClient, SecPurpose};
use rttp_protocol::authorization::MAX_AUTHORIZATION_VALUE_BYTES;
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

fn capture_optional_proxy_request(request: impl FnOnce(Proxy)) -> Vec<u8> {
  let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
  request(Proxy::http("127.0.0.1", u32::from(addr.port())));
  handle
    .join()
    .expect("optional raw proxy request capture server")
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

fn request_head_text(request: &[u8]) -> String {
  let head_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .map(|position| position + 4)
    .expect("request should contain header terminator");
  request_text(&request[..head_end])
}

#[test]
fn outbound_headers_reject_invalid_names_and_values_before_connecting() {
  let invalid_headers = [
    Header::new("X Request", "safe"),
    Header::new("X:Request", "safe"),
    Header::new(" X-Request", "safe"),
    Header::new("X-Request", "safe\r\ninjected"),
    Header::new("X-Request", "safe\r"),
    Header::new("X-Request", "safe\n"),
    Header::new("X-Request", "safe\0value"),
    Header::new("X-Request", "safe\u{1}value"),
    Header::new("X-Request", "safe\u{b}value"),
    Header::new("X-Request", "safe\u{1f}value"),
    Header::new("X-Request", "safe\u{7f}value"),
  ];

  for header in invalid_headers {
    let request = capture_optional_request(|base_url| {
      let error = client()
        .get()
        .url(format!("{}/invalid-header", base_url))
        .header(header)
        .emit()
        .expect_err("invalid outbound header must be rejected");
      assert!(error.is_builder());
    });
    assert!(request.is_empty(), "invalid header must not open a socket");
  }
}

#[test]
fn outbound_header_convenience_apis_reject_line_breaks_before_connecting() {
  let tuple_request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/invalid-tuple-header", base_url))
      .header(("X-Request", "safe\nvalue"))
      .emit()
      .expect_err("tuple header with a line break must be rejected");
    assert!(error.is_builder());
  });
  assert!(tuple_request.is_empty());

  let raw_request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/invalid-raw-header", base_url))
      .header("X-Request: safe\nInjected: value")
      .emit()
      .expect_err("raw header with a line break must be rejected");
    assert!(error.is_builder());
  });
  assert!(raw_request.is_empty());

  let empty_name_request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/empty-raw-header-name", base_url))
      .header(": safe")
      .emit()
      .expect_err("raw header with an empty name must be rejected");
    assert!(error.is_builder());
  });
  assert!(empty_name_request.is_empty());
}

#[test]
fn outbound_headers_preserve_permitted_visible_bytes_and_horizontal_tabs() {
  let mut visible_value = (0x20..=0x7e).collect::<Vec<u8>>();
  visible_value.extend_from_slice("\tobsé".as_bytes());
  let visible_value = String::from_utf8(visible_value).expect("valid UTF-8 header value");

  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/valid-header", base_url))
      .header(Header::new("X-Token!#$%&'*+-.^_`|~", &visible_value))
      .header(Header::new("X-Tab", "\tinside\t"))
      .emit()
      .expect("valid outbound headers should be sent");
  });

  let visible_header = format!("X-Token!#$%&'*+-.^_`|~: {visible_value}\r\n");
  assert!(request
    .windows(visible_header.len())
    .any(|window| window == visible_header.as_bytes()));
  assert!(request
    .windows(b"X-Tab: \tinside\t\r\n".len())
    .any(|window| window == b"X-Tab: \tinside\t\r\n"));
}

#[test]
fn outbound_trailers_reject_untrimmed_invalid_bytes() {
  for trailer in [
    Header::new(" X-Trace", "safe"),
    Header::new("X-Trace", "safe\r"),
    Header::new("X-Trace", "safe\n"),
    Header::new("X-Trace", "safe\0value"),
  ] {
    let mut client = client();
    let error = client
      .trailer(trailer)
      .expect_err("invalid outbound trailer must be rejected");
    assert!(error.is_builder());
  }
}

#[test]
fn outbound_upgrade_protocols_emit_validated_upgrade_metadata() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/upgrade", base_url))
      .upgrade_protocols(["websocket", "h2c"])
      .expect("valid Upgrade protocols should be accepted")
      .emit()
      .expect("request should be sent");
  });
  let request = request_text(&request);

  assert_eq!(header_value(&request, "Upgrade"), Some("websocket, h2c"));
  assert_ne!(header_value(&request, "Connection"), Some("Upgrade"));
}

#[test]
fn outbound_upgrade_protocols_reject_invalid_values_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/invalid-upgrade", base_url))
      .upgrade_protocols(["web socket"])
      .expect_err("invalid Upgrade protocol must be rejected");
    assert!(error.is_builder());
  });

  assert!(request.is_empty(), "invalid Upgrade must not open a socket");
}

#[test]
fn outbound_sec_purpose_emits_validated_metadata() {
  let purpose = SecPurpose::from_tokens(["prefetch", "vendor-ext"])
    .expect("valid Sec-Purpose tokens should parse");
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/sec-purpose", base_url))
      .sec_purpose(&purpose)
      .emit()
      .expect("request should be sent");
  });
  let request = request_text(&request);

  assert_eq!(
    header_value(&request, "Sec-Purpose"),
    Some("prefetch, vendor-ext")
  );
}

#[cfg(feature = "async")]
#[test]
fn async_outbound_headers_are_rejected_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let error = block_on(
      client()
        .get()
        .url(format!("{}/invalid-async-header", base_url))
        .header(("X-Request", "safe\rvalue"))
        .rasync(),
    )
    .expect_err("invalid async outbound header must be rejected");
    assert!(error.is_builder());
  });
  assert!(request.is_empty());
}

#[cfg(feature = "http2")]
#[test]
fn http2_outbound_headers_are_rejected_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/invalid-http2-header", base_url))
      .header(("X-Request", "safe\u{7f}value"))
      .emit_http2_prior_knowledge()
      .expect_err("invalid HTTP/2 outbound header must be rejected");
    assert!(error.is_builder());
  });
  assert!(request.is_empty());
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
fn authorization_helpers_emit_basic_bearer_and_custom_scheme_credentials() {
  for (scheme, credentials, expected) in [
    ("Basic", "dXNlcjpzZWNyZXQ=", "Basic dXNlcjpzZWNyZXQ="),
    ("Bearer", "token-123", "Bearer token-123"),
    ("ApiKey", "v1:client-42", "ApiKey v1:client-42"),
  ] {
    let request = capture_request(|base_url| {
      client()
        .get()
        .url(format!("{}/asset", base_url))
        .authorization(scheme, credentials)
        .expect("authorization metadata should be accepted")
        .emit()
        .expect("request should succeed");
    });
    assert_eq!(
      Some(expected),
      header_value(&request_text(&request), "Authorization")
    );
  }
}

#[test]
fn authorization_helper_rejects_invalid_or_oversized_metadata_before_connecting() {
  for (scheme, credentials) in [
    ("bad scheme", "token".to_string()),
    ("Bearer", "".to_string()),
    ("Bearer", " \t ".to_string()),
    ("Bearer", "token\rnext".to_string()),
    ("Bearer", "token\nnext".to_string()),
    ("Bearer", "token\0next".to_string()),
    ("Bearer", "x".repeat(64 * 1024 + 1)),
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .authorization(scheme, &credentials)
        .expect_err("invalid authorization metadata should be rejected");
      assert!(error.is_builder());
      if !credentials.is_empty() {
        assert!(!error.to_string().contains(&credentials));
      }
    });
    assert!(
      request.is_empty(),
      "invalid metadata should not open a socket"
    );
  }
}

#[test]
fn raw_headers_remain_an_escape_hatch_for_custom_authorization_schemes() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .header((
        "Authorization",
        "Signature keyId=\"client\",algorithm=\"hs2019\"",
      ))
      .emit()
      .expect("request should succeed");
  });
  assert_eq!(
    Some("Signature keyId=\"client\",algorithm=\"hs2019\""),
    header_value(&request_text(&request), "Authorization")
  );
}

#[test]
fn auth_facade_rejects_oversized_bearer_before_connecting_without_exposing_token() {
  let token = "x".repeat(MAX_AUTHORIZATION_VALUE_BYTES);
  let request = capture_optional_request(|base_url| {
    let error = client()
      .get()
      .url(format!("{}/asset", base_url))
      .auth(Auth::bearer(&token))
      .emit()
      .expect_err("oversized Authorization metadata should be rejected");
    assert!(error.is_builder());
    assert!(!error.to_string().contains(&token));
  });

  assert!(
    request.is_empty(),
    "oversized Authorization metadata should not open a socket"
  );
}

#[test]
fn proxy_auth_rejects_oversized_basic_before_connecting_without_exposing_credentials() {
  let username = "proxy-user";
  let password = "x".repeat(MAX_AUTHORIZATION_VALUE_BYTES);
  let request = capture_optional_proxy_request(|proxy| {
    let error = client()
      .get()
      .url("http://example.test/asset")
      .proxy(
        Proxy::builder(proxy.type_().clone())
          .host(proxy.host())
          .port(proxy.port())
          .username(username)
          .password(&password),
      )
      .emit()
      .expect_err("oversized Proxy-Authorization metadata should be rejected");
    assert!(error.is_builder());
    let message = error.to_string();
    assert!(!message.contains(username));
    assert!(!message.contains(&password));
  });

  assert!(
    request.is_empty(),
    "oversized Proxy-Authorization metadata should not open a proxy socket"
  );
}

#[test]
fn proxy_debug_redacts_credentials() {
  let proxy = Proxy::http_with_authorization("127.0.0.1", 8080, "proxy-user", "proxy-secret");
  let debug = format!("{proxy:?}");

  assert!(debug.contains("127.0.0.1"));
  assert!(!debug.contains("proxy-user"));
  assert!(!debug.contains("proxy-secret"));
  assert!(debug.contains("[REDACTED]"));
}

#[test]
fn facade_debug_redacts_sensitive_header_values() {
  let mut client = HttpClient::new();
  client
    .auth(Auth::bearer("origin-token"))
    .header(("Proxy-Authorization", "Basic cHJveHk6c2VjcmV0"))
    .header(("Cookie", "session=private"))
    .header(("Idempotency-Key", "charge-2026-08-19-9f3c"))
    .header(("Accept", "application/json"));

  let debug = format!("{client:?}");
  assert!(debug.contains("Authorization"));
  assert!(debug.contains("Proxy-Authorization"));
  assert!(debug.contains("Cookie"));
  assert!(debug.contains("Idempotency-Key"));
  assert!(debug.contains("Accept"));
  assert!(debug.contains("application/json"));
  assert!(debug.contains("[REDACTED]"));
  for secret in [
    "origin-token",
    "cHJveHk6c2VjcmV0",
    "session=private",
    "charge-2026-08-19-9f3c",
  ] {
    assert!(!debug.contains(secret));
  }
}

#[test]
fn accept_encoding_helpers_emit_validated_codings_and_quality_values() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .accept_gzip()
      .expect("gzip should be accepted")
      .accept_br_with_q("0.8")
      .expect("br quality should be accepted")
      .accept_identity_with_q("0")
      .expect("identity quality should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("gzip, br;q=0.8, identity;q=0"),
    header_value(&request, "Accept-Encoding")
  );
}

#[test]
fn expect_continue_helper_emits_metadata_without_gating_the_request_body() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/upload", base_url))
      .expect_continue()
      .raw("request body")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some("100-continue"), header_value(&request, "Expect"));
  assert!(request.ends_with("request body"));
}

#[test]
fn raw_expect_extension_header_remains_an_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/upload", base_url))
      .header(("Expect", "preview=sha256; chunk=1"))
      .raw("request body")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("preview=sha256; chunk=1"),
    header_value(&request, "Expect")
  );
  assert!(request.ends_with("request body"));
}

#[test]
fn accept_encoding_helpers_reject_invalid_members_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding_with_q("bad coding", "1.1")
      .expect_err("invalid coding should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "invalid Accept-Encoding helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding_with_q("gzip", "1.1")
      .expect_err("invalid q-value should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "invalid Accept-Encoding q-value should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let oversized_coding = "a".repeat(64 * 1024 + 1);
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding(&oversized_coding)
      .expect_err("oversized first coding should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "oversized first Accept-Encoding coding should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding("gzip, br")
      .expect_err("comma-bearing coding should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "comma-bearing Accept-Encoding coding should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding("gzip;q=0")
      .expect_err("parameterized coding should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "parameterized Accept-Encoding coding should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_encoding_with_q("gzip", "0.8, br")
      .expect_err("comma-bearing q-value should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "comma-bearing Accept-Encoding q-value should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .accept_gzip_with_q("0.8, br")
      .expect_err("comma-bearing gzip q-value should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "comma-bearing gzip Accept-Encoding q-value should not open a socket"
  );
}

#[test]
fn want_digest_helpers_emit_rfc_9530_dictionary_members() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .want_content_digest("sha-256")
      .expect("content digest algorithm should be accepted")
      .want_content_digest_with_q("sha-512", "8")
      .expect("content digest preference should be accepted")
      .want_repr_digest("sha-256")
      .expect("representation digest algorithm should be accepted")
      .want_repr_digest_with_q("sha-512", "0")
      .expect("representation digest preference should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("sha-256=10, sha-512=8"),
    header_value(&request, "Want-Content-Digest")
  );
  assert_eq!(
    Some("sha-256=10, sha-512=0"),
    header_value(&request, "Want-Repr-Digest")
  );
}

#[test]
fn want_digest_helpers_reject_invalid_or_excessive_values_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/asset", base_url))
      .want_content_digest("bad algorithm")
      .expect_err("invalid digest algorithm should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid digest preference helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/asset", base_url))
      .want_content_digest_with_q("sha-256", "1.0")
      .expect_err("malformed digest preference should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid digest preference helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/asset", base_url))
      .want_repr_digest_with_q("sha-256", "11")
      .expect_err("invalid digest preference should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid digest preference helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/asset", base_url));
    client
      .want_content_digest("sha-256")
      .expect("first digest algorithm should be accepted");
    assert!(client
      .want_content_digest("SHA-256")
      .expect_err("duplicate digest algorithm should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid digest preference helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let oversized_algorithm = "a".repeat(64 * 1024 + 1);
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/asset", base_url))
      .want_repr_digest(&oversized_algorithm)
      .expect_err("oversized digest algorithm should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid digest preference helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/asset", base_url));
    for index in 0..32 {
      client
        .want_repr_digest(format!("algorithm{index}"))
        .expect("digest algorithm within the limit should be accepted");
    }
    assert!(client
      .want_repr_digest("one-too-many")
      .expect_err("too many digest algorithms should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "excessive digest preference helper input should not open a socket"
  );
}

#[test]
fn signature_helpers_emit_canonical_fields_and_reject_malformed_input_before_connecting() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/signed", base_url))
      .signature_input(
        r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#,
      )
      .expect("Signature-Input should be accepted")
      .signature("sig1=:YWJj:")
      .expect("Signature should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some(r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#),
    header_value(&request, "Signature-Input")
  );
  assert_eq!(Some("sig1=:YWJj:"), header_value(&request, "Signature"));

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .post()
      .url(format!("{}/signed", base_url))
      .signature("not-a-signature")
      .expect_err("malformed Signature should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed Signature helper input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .post()
      .url(format!("{}/signed", base_url))
      .signature_input("not-an-input")
      .expect_err("malformed Signature-Input should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed Signature-Input helper input should not open a socket"
  );
}

#[test]
fn raw_want_digest_headers_remain_available_for_extended_syntax() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .header(("Want-Content-Digest", "sha-256;example=custom"))
      .header(("Want-Repr-Digest", "sha-512;example=custom"))
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("sha-256;example=custom"),
    header_value(&request, "Want-Content-Digest")
  );
  assert_eq!(
    Some("sha-512;example=custom"),
    header_value(&request, "Want-Repr-Digest")
  );
}

#[test]
fn accept_helpers_emit_validated_media_ranges_and_quality_values() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/document", base_url))
      .accept_json()
      .expect("JSON should be accepted")
      .accept_html_with_q("0.8")
      .expect("HTML quality should be accepted")
      .accept("text/plain; charset=utf-8; q=0.5")
      .expect("parameterized media range should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("application/json, text/html;q=0.8, text/plain; charset=utf-8; q=0.5"),
    header_value(&request, "Accept")
  );
}

#[test]
fn cache_control_helpers_emit_bounded_request_directives() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/document", base_url))
      .cache_control_no_cache()
      .expect("no-cache should be accepted")
      .cache_control_no_store()
      .expect("no-store should be accepted")
      .cache_control_max_age(60)
      .expect("max-age should be accepted")
      .cache_control_extension_with_value("community", "private")
      .expect("extension should be accepted")
      .cache_control_extension("immutable")
      .expect("valueless extension should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("no-cache, no-store, max-age=60, community=private, immutable"),
    header_value(&request, "Cache-Control")
  );
}

#[test]
fn cache_control_helpers_reject_invalid_or_excessive_values_before_connecting() {
  for (name, value) in [
    ("bad name", "value".to_string()),
    ("community", "bad\r\nvalue".to_string()),
    ("community", "a".repeat(64 * 1024 + 1)),
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/document", base_url))
        .cache_control_extension_with_value(name, value)
        .expect_err("invalid Cache-Control extension should be rejected");
      assert!(error.is_builder());
    });
    assert!(
      request.is_empty(),
      "invalid Cache-Control metadata should not open a socket"
    );
  }

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/document", base_url));
    for index in 0..256 {
      client
        .cache_control_extension(format!("extension-{index}"))
        .expect("bounded Cache-Control directive should be accepted");
    }
    let error = client
      .cache_control_extension("overflow")
      .expect_err("too many Cache-Control directives should be rejected");
    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "excessive Cache-Control directives should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/document", base_url))
      .cache_control_extension("max-age")
      .expect_err("dedicated Cache-Control directives should not be extensions");
    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "reserved Cache-Control directive names should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/document", base_url))
      .cache_control_extension_with_value("no-store", "1")
      .expect_err("dedicated Cache-Control directives should not be extensions");
    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "reserved Cache-Control directive names should not open a socket"
  );
}

#[test]
fn accept_helpers_reject_invalid_values_before_connecting() {
  for value in [
    "text",
    "text/html; q=0.8; q=0.5",
    "text/html; q=1.001",
    "text/html\n;level=1",
    "text/html\r;level=1",
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/document", base_url))
        .accept(value)
        .expect_err("invalid Accept media range should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "invalid Accept helper input should not open a socket"
    );
  }

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/document", base_url));
    for index in 0..32 {
      client
        .accept(format!("application/x-{index}"))
        .expect("bounded Accept media range should be accepted");
    }
    let error = client
      .accept("application/x-overflow")
      .expect_err("too many media ranges should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "oversized Accept media range list should not open a socket"
  );

  let oversized = format!("application/{}", "a".repeat(64 * 1024));
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/document", base_url))
      .accept(&oversized)
      .expect_err("oversized media range should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "oversized Accept helper input should not open a socket"
  );
}

#[test]
fn manual_accept_header_remains_available_as_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/document", base_url))
      .header(("Accept", "application/example; feature=?experimental"))
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("application/example; feature=?experimental"),
    header_value(&request, "Accept")
  );
}

#[test]
fn te_helpers_emit_validated_codings_and_trailers() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .te("gzip")
      .expect("transfer coding should be accepted")
      .te_with_q("deflate", "0.5")
      .expect("transfer coding quality should be accepted")
      .te_trailers()
      .expect("trailers should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("gzip, deflate;q=0.5, trailers"),
    header_value(&request, "TE")
  );
  assert_eq!(Some("Close, TE"), header_value(&request, "Connection"));
}

#[test]
fn te_helpers_accept_multiple_codings_and_inline_qvalues_in_one_call() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .te("gzip;q=0.5")
      .expect("inline q-value should be accepted")
      .te("deflate, br")
      .expect("comma-separated codings should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("gzip;q=0.5, deflate, br"),
    header_value(&request, "TE")
  );
  assert_eq!(Some("Close, TE"), header_value(&request, "Connection"));
}

#[test]
fn te_helpers_reject_duplicate_codings_within_one_call_before_connecting() {
  for value in ["gzip, GZIP", "gzip, gzip;q=0.5"] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .te(value)
        .expect_err("duplicate codings in one call should be rejected");

      assert!(error.is_builder());
    });
    assert!(
      request.is_empty(),
      "duplicate TE input should not open a socket"
    );
  }
}

#[test]
fn te_helpers_reject_invalid_members_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .te_with_q("bad coding", "1.1")
      .expect_err("invalid coding should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid TE input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .te_with_q("gzip", "1.1")
      .expect_err("invalid q-value should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid TE q-value should not open a socket"
  );

  let mut trailers_client = client();
  let error = trailers_client
    .get()
    .url("http://example.test/asset")
    .te_with_q("trailers", "0.5")
    .expect_err("trailers must not carry a q-value");
  assert!(error.is_builder());

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .te("Chunked")
      .expect_err("chunked must not be advertised in TE");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "chunked TE input should not open a socket"
  );
}

#[test]
fn trailer_header_helper_declares_validated_fields_without_enabling_trailer_streaming() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/upload", base_url))
      .trailer_header(["X-Checksum", "x-signature", "X-Checksum"])
      .expect("Trailer field names should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);
  assert_eq!(
    Some("x-checksum, x-signature"),
    header_value(&request, "Trailer")
  );
  assert_eq!(None, header_value(&request, "TE"));

  for field in [
    "Content-Length",
    "TE",
    "bad field",
    "X-Good\r\nInjected: yes",
  ] {
    let mut client = client();
    let error = client
      .post()
      .url("http://example.test/upload")
      .trailer_header([field])
      .expect_err("invalid Trailer field name should be rejected");
    assert!(
      error.is_builder(),
      "{field:?} should be rejected before connecting"
    );
  }
}

#[test]
fn priority_helper_emits_bounded_known_and_extension_metadata() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .priority("u=1, i, x=token")
      .expect("Priority should be accepted")
      .emit()
      .expect("request should succeed");
  });

  assert_eq!(
    Some("u=1, i, x=token"),
    header_value(&request_text(&request), "Priority")
  );
}

#[test]
fn priority_helper_rejects_oversized_parameter_sets_before_connecting() {
  let too_many = (0..257)
    .map(|index| format!("x{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .priority(&too_many)
      .expect_err("too many Priority parameters should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "invalid Priority should not open a socket"
  );
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
fn accept_language_helper_emits_bounded_language_ranges() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/localized", base_url))
      .accept_language(["en-US", "fr-CA; q=0.8", "de; q=1.", "*"])
      .expect("language ranges should be accepted")
      .emit()
      .expect("request should succeed");
  });

  assert_eq!(
    Some("en-US, fr-CA; q=0.8, de; q=1., *"),
    header_value(&request_text(&request), "Accept-Language")
  );
}

#[test]
fn accept_language_helper_rejects_invalid_values_before_connecting() {
  for value in ["en_US", "en; q=1.001", "en, EN"] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/localized", base_url))
        .accept_language([value])
        .expect_err("invalid language range should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "invalid Accept-Language helper input should not open a socket"
    );
  }

  let oversized = format!("{}en", " ".repeat(64 * 1024));
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/localized", base_url))
      .accept_language([oversized.as_str()])
      .expect_err("oversized language value should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "oversized Accept-Language helper input should not open a socket"
  );
}

#[test]
fn te_and_prefer_helpers_emit_bounded_request_metadata() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/metadata", base_url))
      .te_trailers()
      .expect("trailers should be accepted")
      .te_with_q("deflate", "0.5")
      .expect("TE coding should be accepted")
      .prefer("respond-async")
      .expect("token preference should be accepted")
      .prefer_with_value("return", "minimal")
      .expect("valued preference should be accepted")
      .prefer_with_value("wait", "30")
      .expect("wait should be accepted")
      .prefer_with_value("example-extension", "enabled")
      .expect("unknown extension should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("trailers, deflate;q=0.5"),
    header_value(&request, "TE")
  );
  assert_eq!(Some("Close, TE"), header_value(&request, "Connection"));
  assert_eq!(
    Some("respond-async, return=minimal, wait=30, example-extension=enabled"),
    header_value(&request, "Prefer")
  );
}

#[test]
fn te_and_prefer_helpers_reject_invalid_values_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/metadata", base_url))
      .te_with_q("bad coding", "1")
      .expect_err("invalid TE coding should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid TE input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/metadata", base_url))
      .prefer_with_value("return", "bad value")
      .expect_err("invalid preference value should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid Prefer input should not open a socket"
  );
}

#[test]
fn te_helpers_reject_duplicate_overflow_and_oversized_values_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client
      .get()
      .url(format!("{}/metadata", base_url))
      .te("gzip")
      .expect("first TE coding should be accepted");
    assert!(client
      .te("GZIP")
      .expect_err("duplicate TE coding should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "duplicate TE input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/metadata", base_url));
    for index in 0..32 {
      client
        .te(format!("coding-{index}"))
        .expect("TE coding within the limit should be accepted");
    }
    assert!(client
      .te("coding-33")
      .expect_err("the 33rd TE coding should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "33rd TE member should not open a socket"
  );

  let oversized = "x".repeat(64 * 1024 + 1);
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/metadata", base_url))
      .te(oversized)
      .expect_err("oversized TE coding should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "oversized TE input should not open a socket"
  );
}

#[test]
fn te_helpers_reject_cross_call_duplicates_across_inline_and_multi_coding_forms() {
  for values in [
    ["gzip", "gzip;q=0.5"],
    ["gzip;q=0.5", "GZIP"],
    ["gzip, deflate", "gzip"],
    ["gzip, deflate", "deflate;q=0.5"],
    ["gzip, deflate", "gzip, deflate"],
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      client
        .get()
        .url(format!("{}/metadata", base_url))
        .te(values[0])
        .expect("first TE call should be accepted");
      assert!(client
        .te(values[1])
        .expect_err("cross-call duplicate TE coding should be rejected")
        .is_builder());
    });
    assert!(
      request.is_empty(),
      "cross-call duplicate TE input should not open a socket"
    );
  }
}

#[test]
fn te_helpers_reject_multi_coding_overflow_beyond_the_member_bound() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/metadata", base_url));
    for index in 0..31 {
      client
        .te(format!("coding-{index}"))
        .expect("TE coding within the limit should be accepted");
    }
    assert!(client
      .te("final-a, final-b")
      .expect_err("31 codings plus two more must exceed the 32-member bound")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "TE member overflow input should not open a socket"
  );
}

#[test]
fn prefer_helpers_reject_invalid_wait_and_bound_values_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/metadata", base_url))
      .prefer("wait")
      .expect_err("valueless wait preference should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "invalid Prefer input should not open a socket"
  );

  for (name, value) in [
    ("wait", "-1"),
    ("wait", "1.5"),
    ("wait", "abc"),
    ("return", "not a token"),
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      assert!(client
        .get()
        .url(format!("{}/metadata", base_url))
        .prefer_with_value(name, value)
        .expect_err("invalid Prefer input should be rejected")
        .is_builder());
    });
    assert!(
      request.is_empty(),
      "invalid Prefer input should not open a socket"
    );
  }

  let request = capture_optional_request(|base_url| {
    let oversized_value = "a".repeat(8 * 1024 + 1);
    let mut client = client();
    assert!(client
      .get()
      .url(format!("{}/metadata", base_url))
      .prefer_with_value("extension", oversized_value)
      .expect_err("oversized preference value should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "oversized Prefer input should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    client.get().url(format!("{}/metadata", base_url));
    for index in 0..32 {
      client
        .prefer(format!("extension{index}"))
        .expect("preference within the limit should be accepted");
    }
    assert!(client
      .prefer("one-too-many")
      .expect_err("excessive preferences should be rejected")
      .is_builder());
  });
  assert!(
    request.is_empty(),
    "excessive Prefer input should not open a socket"
  );
}

#[test]
fn forwarded_helper_emits_bounded_forwarding_metadata() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .forwarded(r#"for=192.0.2.60;by=203.0.113.43;host=example.test;proto="https""#)
      .expect("first forwarding element should be accepted")
      .forwarded(r#"for="[2001:db8:cafe::17]""#)
      .expect("second forwarding element should be accepted")
      .emit()
      .expect("request should succeed");
  });

  assert_eq!(
    Some(
      r#"for=192.0.2.60; by=203.0.113.43; host=example.test; proto=https, for="[2001:db8:cafe::17]""#
    ),
    header_value(&request_text(&request), "Forwarded")
  );
}

#[test]
fn forwarded_helper_rejects_duplicate_or_excessive_metadata_before_connecting() {
  for value in ["for=192.0.2.60;for=198.51.100.17", "for="] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .forwarded(value)
        .expect_err("invalid forwarding metadata should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "invalid Forwarded input should not open a socket"
    );
  }

  let excessive = (0..257)
    .map(|index| format!("for=192.0.2.{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .forwarded(excessive.as_str())
      .expect_err("too many forwarding elements should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "excessive Forwarded input should not open a socket"
  );

  let first = format!("for={}", "a".repeat(64 * 1024 - 4));
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .forwarded(first.as_str())
      .expect("a bounded first element should be accepted")
      .forwarded("for=second")
      .expect_err("combined Forwarded metadata should remain bounded");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "oversized Forwarded output should not open a socket"
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
fn max_forwards_helper_emits_bounded_trace_and_options_headers() {
  let trace = capture_request(|base_url| {
    client()
      .trace()
      .url(format!("{}/diagnostics", base_url))
      .max_forwards("0")
      .expect("zero Max-Forwards should be accepted")
      .emit()
      .expect("TRACE request should succeed");
  });
  let trace = request_text(&trace);
  assert!(trace.starts_with("TRACE /diagnostics HTTP/1.1\r\n"));
  assert_eq!(Some("0"), header_value(&trace, "Max-Forwards"));

  let options = capture_request(|base_url| {
    client()
      .options()
      .url(format!("{}/diagnostics", base_url))
      .max_forwards("4294967295")
      .expect("u32::MAX Max-Forwards should be accepted")
      .emit()
      .expect("OPTIONS request should succeed");
  });
  let options = request_text(&options);
  assert!(options.starts_with("OPTIONS /diagnostics HTTP/1.1\r\n"));
  assert_eq!(Some("4294967295"), header_value(&options, "Max-Forwards"));
}

#[test]
fn max_forwards_helper_rejects_invalid_values_before_connecting() {
  for value in ["-1", "1.5", "", "4294967296", "99999999999"] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .trace()
        .url(format!("{}/diagnostics", base_url))
        .max_forwards(value)
        .expect_err("invalid Max-Forwards should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "invalid Max-Forwards helper input should not open a socket"
    );
  }

  let oversized = "0".repeat(64 * 1024 + 1);
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .options()
      .url(format!("{}/diagnostics", base_url))
      .max_forwards(oversized.as_str())
      .expect_err("oversized Max-Forwards should be rejected");

    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "oversized Max-Forwards helper input should not open a socket"
  );
}

#[test]
fn manual_max_forwards_header_remains_available_as_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .trace()
      .url(format!("{}/diagnostics", base_url))
      .header(("Max-Forwards", "unusual-value"))
      .emit()
      .expect("manual Max-Forwards header should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("unusual-value"),
    header_value(&request, "Max-Forwards")
  );
}

#[test]
fn idempotency_key_helper_emits_canonical_metadata() {
  for (value, expected) in [
    ("charge-2026-08-19-9f3c", "charge-2026-08-19-9f3c"),
    (
      "urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00",
      "urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00",
    ),
    (" \tcharge-2026-08-19-9f3c\t ", "charge-2026-08-19-9f3c"),
  ] {
    let request = capture_request(|base_url| {
      client()
        .post()
        .url(format!("{}/charges", base_url))
        .idempotency_key(value)
        .expect("idempotency key should be accepted")
        .emit()
        .expect("request should succeed");
    });
    let request = request_text(&request);
    assert_eq!(Some(expected), header_value(&request, "Idempotency-Key"));
  }
}

#[test]
fn idempotency_key_helper_replaces_existing_fields() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/charges", base_url))
      .header(("Idempotency-Key", "legacy-key"))
      .idempotency_key("charge-2026-08-19-9f3c")
      .expect("idempotency key should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);
  assert_eq!(
    Some("charge-2026-08-19-9f3c"),
    header_value(&request, "Idempotency-Key")
  );
  assert!(
    !request.contains("legacy-key"),
    "the typed helper must replace an existing same-name field"
  );
}

#[test]
fn idempotency_key_helper_rejects_invalid_values_before_connecting() {
  for value in [
    "",
    " ",
    "key with space",
    "key\r\nX-Injected: 1",
    "key\rX: y",
    "key\nX: y",
    "key\0value",
    "key\u{7f}value",
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .post()
        .url(format!("{}/charges", base_url))
        .idempotency_key(value)
        .expect_err("invalid idempotency key should be rejected");
      assert!(error.is_builder());
      if !value.trim().is_empty() {
        assert!(!error.to_string().contains(value));
      }
    });
    assert!(
      request.is_empty(),
      "invalid idempotency key must not open a socket"
    );
  }
}

#[test]
fn idempotency_key_helper_rejects_oversized_values_before_connecting() {
  let oversized = "x".repeat(64 * 1024 + 1);
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .post()
      .url(format!("{}/charges", base_url))
      .idempotency_key(oversized.as_str())
      .expect_err("oversized idempotency key should be rejected");
    assert!(error.is_builder());
    assert!(!error.to_string().contains(&oversized[..64]));
  });
  assert!(
    request.is_empty(),
    "oversized idempotency key must not open a socket"
  );
}

#[test]
fn raw_idempotency_key_header_remains_available_as_escape_hatch() {
  let request = capture_request(|base_url| {
    client()
      .post()
      .url(format!("{}/charges", base_url))
      .header(("Idempotency-Key", "opaque custom value"))
      .emit()
      .expect("manual Idempotency-Key header should succeed");
  });
  let request = request_text(&request);
  assert_eq!(
    Some("opaque custom value"),
    header_value(&request, "Idempotency-Key")
  );
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
fn if_range_helpers_emit_single_validator_headers() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .range(10, 19)
      .expect("bounded range should be accepted")
      .if_range_etag(r#""abc123""#)
      .expect("strong etag should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some("bytes=10-19"), header_value(&request, "Range"));
  assert_eq!(Some(r#""abc123""#), header_value(&request, "If-Range"));

  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .range_from(20)
      .if_range_date("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("http date should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some("bytes=20-"), header_value(&request, "Range"));
  assert_eq!(
    Some("Sun, 06 Nov 1994 08:49:37 GMT"),
    header_value(&request, "If-Range")
  );
}

#[test]
fn if_range_helpers_reject_obvious_malformed_inputs_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_range_etag(r#"W/"weak-tag""#)
      .expect_err("weak etag should be rejected for If-Range");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed If-Range etag helper should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_range_etag("*")
      .expect_err("wildcard etag should be rejected for If-Range");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed If-Range etag helper should not open a socket"
  );

  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .if_range_date("not a date")
      .expect_err("invalid http date should be rejected");

    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "malformed If-Range date helper should not open a socket"
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
fn conditional_etag_helpers_reject_oversized_validators_before_connecting() {
  let oversized = format!("\"{}\"", "a".repeat(64 * 1024));

  for helper in ["If-Match", "If-None-Match", "If-Range"] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let request = client.get().url(format!("{}/asset", base_url));
      let error = match helper {
        "If-Match" => request.if_match(&oversized),
        "If-None-Match" => request.if_none_match(&oversized),
        "If-Range" => request.if_range_etag(&oversized),
        _ => unreachable!("test helper names are exhaustive"),
      }
      .expect_err("oversized entity tag should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "oversized {helper} helper input should not open a socket"
    );
  }
}

#[test]
fn conditional_http_date_helpers_reject_oversized_and_duplicate_dates_before_connecting() {
  let oversized = "0".repeat(64 * 1024 + 1);

  for helper in ["If-Modified-Since", "If-Unmodified-Since"] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let request = client.get().url(format!("{}/asset", base_url));
      let error = match helper {
        "If-Modified-Since" => request.if_modified_since(&oversized),
        "If-Unmodified-Since" => request.if_unmodified_since(&oversized),
        _ => unreachable!("test helper names are exhaustive"),
      }
      .expect_err("oversized http date should be rejected");

      assert!(error.is_builder());
    });

    assert!(
      request.is_empty(),
      "oversized {helper} helper input should not open a socket"
    );
  }
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

  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .header(("If-Range", r#"W/"manual-weak""#))
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some(r#"W/"manual-weak""#),
    header_value(&request, "If-Range")
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
fn http_proxy_request_sends_proxy_authorization_in_header_block() {
  let request = capture_proxy_request(|proxy| {
    client()
      .post()
      .url("http://example.test/path?x=1")
      .proxy(
        Proxy::builder(proxy.type_().clone())
          .host(proxy.host())
          .port(proxy.port())
          .username("proxy-user")
          .password("proxy-secret"),
      )
      .raw("Proxy-Authorization: body-value")
      .emit()
      .expect("request should succeed");
  });

  let head = request_head_text(&request);
  assert_eq!(
    Some("Basic cHJveHktdXNlcjpwcm94eS1zZWNyZXQ="),
    header_value(&head, "Proxy-Authorization")
  );
  assert_eq!(b"Proxy-Authorization: body-value", request_body(&request));
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

#[test]
fn preflight_metadata_helpers_emit_validated_request_headers() {
  let request = capture_request(|base_url| {
    client()
      .options()
      .url(format!("{}/asset", base_url))
      .origin("https://spa.example.test")
      .expect("Origin should be accepted")
      .access_control_request_method("PUT")
      .expect("preflight method should be accepted")
      .access_control_request_headers(["X-Request-Id", "Content-Type"])
      .expect("preflight field names should be accepted")
      .access_control_request_private_network()
      .expect("private-network preflight metadata should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(
    Some("https://spa.example.test"),
    header_value(&request, "Origin")
  );
  assert_eq!(
    Some("PUT"),
    header_value(&request, "Access-Control-Request-Method")
  );
  assert_eq!(
    Some("x-request-id, content-type"),
    header_value(&request, "Access-Control-Request-Headers")
  );
  assert_eq!(
    Some("true"),
    header_value(&request, "Access-Control-Request-Private-Network")
  );
}

#[test]
fn save_data_helper_emits_on_request_token() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/catalog", base_url))
      .save_data()
      .expect("Save-Data should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);

  assert_eq!(Some("on"), header_value(&request, "Save-Data"));
}

#[test]
fn upgrade_insecure_requests_helper_emits_signal_value_without_rewriting_target() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/page", base_url))
      .upgrade_insecure_requests()
      .expect("Upgrade-Insecure-Requests should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);
  let request_line = request.lines().next().expect("request line");

  assert!(
    request_line.starts_with("GET /page HTTP/1.1"),
    "helper must keep the original origin-form target: {request_line}"
  );
  assert_eq!(
    Some("1"),
    header_value(&request, "Upgrade-Insecure-Requests")
  );
  assert!(header_value(&request, "Host").is_some());
  assert_eq!(
    1,
    request
      .lines()
      .filter(|line| line
        .to_ascii_lowercase()
        .starts_with("upgrade-insecure-requests:"))
      .count()
  );
}

#[test]
fn origin_helper_emits_null_and_normalized_tuple_origins() {
  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .origin("null")
      .expect("null Origin should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);
  assert_eq!(Some("null"), header_value(&request, "Origin"));

  let request = capture_request(|base_url| {
    client()
      .get()
      .url(format!("{}/asset", base_url))
      .origin("https://example.test:443")
      .expect("default-port Origin should be accepted")
      .emit()
      .expect("request should succeed");
  });
  let request = request_text(&request);
  assert_eq!(
    Some("https://example.test"),
    header_value(&request, "Origin")
  );
}

#[test]
fn preflight_metadata_helpers_reject_invalid_input_before_connecting() {
  for value in [
    "https://example.test/path".to_string(),
    "https://example.test?query".to_string(),
    "ftp://example.test".to_string(),
    "a".repeat(64 * 1024 + 1),
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .origin(value)
        .expect_err("invalid Origin should be rejected");
      assert!(error.is_builder());
    });
    assert!(
      request.is_empty(),
      "invalid Origin should not open a socket"
    );
  }

  for value in ["*", "GET, POST", "GET POST", ""] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .access_control_request_method(value)
        .expect_err("invalid preflight method should be rejected");
      assert!(error.is_builder());
    });
    assert!(
      request.is_empty(),
      "invalid preflight method should not open a socket"
    );
  }

  for field_names in [
    vec!["X-Request-Id", "x-request-id"],
    vec!["bad field"],
    vec!["X-Id", "X-Id\rInjected: yes"],
  ] {
    let request = capture_optional_request(|base_url| {
      let mut client = client();
      let error = client
        .get()
        .url(format!("{}/asset", base_url))
        .access_control_request_headers(field_names)
        .expect_err("invalid preflight field names should be rejected");
      assert!(error.is_builder());
    });
    assert!(
      request.is_empty(),
      "invalid preflight field names should not open a socket"
    );
  }

  let too_many = (0..257)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>();
  let request = capture_optional_request(|base_url| {
    let mut client = client();
    let error = client
      .get()
      .url(format!("{}/asset", base_url))
      .access_control_request_headers(&too_many)
      .expect_err("too many preflight field names should be rejected");
    assert!(error.is_builder());
  });
  assert!(
    request.is_empty(),
    "excessive preflight field names should not open a socket"
  );
}
