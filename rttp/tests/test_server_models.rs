use rttp::server::{HttpRequest, HttpResponse};

#[test]
fn parses_http_request_target_headers_and_body() {
  let raw = concat!(
    "POST /submit?name=Rttp&debug=true HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain\r\n",
    "X-Trace-Id: abc-123\r\n",
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
