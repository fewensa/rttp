use rttp::server::{HttpRequest, HttpResponse};

#[test]
fn parses_http_request_target_headers_and_body() {
  let raw = concat!(
    "POST /submit?name=Rttp&debug=true HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain\r\n",
    "X-Trace-Id: abc-123\r\n",
    "Content-Length: 11\r\n",
    "\r\n",
    "hello=world"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("POST", request.method());
  assert_eq!("/submit", request.path());
  assert_eq!(Some("name=Rttp&debug=true"), request.query());
  assert_eq!("HTTP/1.1", request.version());
  assert_eq!(Some("example.test"), request.header("host"));
  assert_eq!(Some("text/plain"), request.header("Content-Type"));
  assert_eq!(Some("abc-123"), request.header("x-trace-id"));
  assert_eq!(b"hello=world", request.body());
}

#[test]
fn parses_body_only_when_content_length_matches() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!(b"hello", request.body());
}

#[test]
fn parses_fixed_length_request_with_duplicate_matching_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!(b"hello", request.body());
}

#[test]
fn rejects_request_body_shorter_than_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hel"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "request body length does not match Content-Length",
    error.to_string()
  );
}

#[test]
fn rejects_request_body_longer_than_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "helloGET /next HTTP/1.1\r\n\r\n"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "request body length does not match Content-Length",
    error.to_string()
  );
}

#[test]
fn rejects_malformed_request_line_and_request_metadata() {
  for raw in [
    b"GET  HTTP/1.1\r\nHost: example.test\r\n\r\n".as_slice(),
    b"GET / HTTP/2.0\r\nHost: example.test\r\n\r\n",
    b"GE(T / HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET /bad path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET http://:80/path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET http://example.test:port/path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"CONNECT example.test HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"CONNECT example.test:port HTTP/1.1\r\nHost: example.test\r\n\r\n",
  ] {
    let _error = HttpRequest::parse(raw).expect_err("request should be rejected");
  }
}

#[test]
fn rejects_invalid_and_folded_request_headers() {
  for raw in [
    b"GET / HTTP/1.1\r\nBad Header: value\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: bad\rvalue\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test\r\n folded: value\r\n\r\n",
  ] {
    let _error = HttpRequest::parse(raw).expect_err("request should be rejected");
  }
}

#[test]
fn rejects_http_11_request_without_host_header() {
  let error =
    HttpRequest::parse(b"GET / HTTP/1.1\r\n\r\n").expect_err("request should be rejected");

  assert_eq!(
    "HTTP/1.1 request requires exactly one Host header",
    error.to_string()
  );
}

#[test]
fn rejects_http_11_request_with_multiple_host_headers() {
  let error = HttpRequest::parse(
    concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "hOSt: other.test\r\n",
      "\r\n"
    )
    .as_bytes(),
  )
  .expect_err("request should be rejected");

  assert_eq!(
    "HTTP/1.1 request requires exactly one Host header",
    error.to_string()
  );
}

#[test]
fn rejects_http_11_request_with_invalid_host_header_value() {
  for raw in [
    b"GET / HTTP/1.1\r\nHost: \r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: http://example.test\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test/path\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test:port\r\n\r\n",
  ] {
    let error = HttpRequest::parse(raw).expect_err("request should be rejected");

    assert_eq!("invalid Host header", error.to_string());
  }
}

#[test]
fn accepts_http_10_request_without_host_header() {
  let request = HttpRequest::parse(b"GET /legacy HTTP/1.0\r\n\r\n").expect("request should parse");

  assert_eq!("HTTP/1.0", request.version());
  assert_eq!(None, request.header("host"));
}

#[test]
fn rejects_conflicting_duplicate_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "Content-Length: 6\r\n",
    "\r\n",
    "hello"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!("conflicting Content-Length headers", error.to_string());
}

#[test]
fn rejects_transfer_encoding_request_even_with_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "Transfer-Encoding conflicts with Content-Length",
    error.to_string()
  );
}

#[test]
fn parses_http_request_without_query_or_body() {
  let raw = concat!(
    "GET /health HTTP/1.0\r\n",
    "Host: example.test\r\n",
    "Connection: close\r\n",
    "\r\n"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("GET", request.method());
  assert_eq!("/health", request.path());
  assert_eq!(None, request.query());
  assert_eq!("HTTP/1.0", request.version());
  assert_eq!(Some("close"), request.header("connection"));
  assert!(request.body().is_empty());
}

#[test]
fn serializes_http_response_status_headers_content_length_and_body() {
  let response = HttpResponse::new(201, "Created")
    .header("Content-Type", "application/json")
    .header("Connection", "close")
    .body(r#"{"ok":true}"#);

  let serialized = response.to_bytes();

  assert_eq!(
    concat!(
      "HTTP/1.1 201 Created\r\n",
      "Content-Type: application/json\r\n",
      "Connection: close\r\n",
      "Content-Length: 11\r\n",
      "\r\n",
      r#"{"ok":true}"#
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn serializes_at_most_one_connection_header() {
  let response = HttpResponse::new(200, "OK")
    .header("Connection", "keep-alive")
    .header("Connection", "close")
    .body("ok");

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
  let connection_headers = serialized
    .lines()
    .filter(|line| line.to_ascii_lowercase().starts_with("connection:"))
    .count();

  assert_eq!(1, connection_headers);
  assert!(serialized.contains("\r\nConnection: close\r\n"));
  assert!(!serialized.contains("\r\nConnection: keep-alive\r\n"));
}

#[test]
fn serializes_chunked_response_body_when_transfer_encoding_is_chunked() {
  let response = HttpResponse::new(200, "OK")
    .header("Transfer-Encoding", "chunked")
    .body("hello");

  let serialized = response.to_bytes();

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\n",
      "hello\r\n",
      "0\r\n",
      "\r\n"
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn rejects_response_headers_with_crlf() {
  let result = std::panic::catch_unwind(|| {
    let _response = HttpResponse::new(302, "Found").header("Location", "/safe\r\nX-Evil: true");
  });

  assert!(result.is_err());
}

#[test]
fn serializes_empty_http_response_without_content_length_for_204() {
  let response = HttpResponse::new(204, "No Content");

  let serialized = response.to_bytes();

  assert_eq!(b"HTTP/1.1 204 No Content\r\n\r\n", serialized.as_slice());
}

#[test]
fn serializes_empty_http_response_without_content_length_for_1xx() {
  let response = HttpResponse::new(101, "Switching Protocols");

  let serialized = response.to_bytes();

  assert_eq!(
    b"HTTP/1.1 101 Switching Protocols\r\n\r\n",
    serialized.as_slice()
  );
}
