use std::io::Write;
use std::net::TcpStream;

use rttp_protocol::entity_tag::{MAX_CONDITIONAL_ENTITY_TAGS, MAX_IF_MATCH_VALUE_BYTES};
use rttp_server::server::{
  HttpIfMatch, HttpIfMatchParseError, HttpRequest, HttpResponse, HttpServer, Request,
};

fn request_with_headers(headers: &str) -> HttpRequest {
  let raw = format!("PUT /asset HTTP/1.1\r\nHost: example.test\r\n{headers}\r\n");
  HttpRequest::parse(raw.as_bytes()).expect("request should parse")
}

fn serve_request_once<F>(raw: &[u8], max_request_head_bytes: usize, handler: F)
where
  F: FnOnce(Request) -> HttpResponse,
{
  let server = HttpServer::bind(("127.0.0.1", 0))
    .expect("server should bind")
    .with_max_request_head_bytes(max_request_head_bytes)
    .expect("request-head limit should be valid");
  let mut client = TcpStream::connect(server.local_addr().expect("server address should exist"))
    .expect("client should connect");
  client.write_all(raw).expect("client should send request");
  server
    .accept_one(handler)
    .expect("server should accept request");
}

#[test]
fn http_request_if_match_parses_absent_ordered_and_wildcard_values() {
  let absent = request_with_headers("");
  assert_eq!(
    None,
    absent.if_match().expect("missing field should be valid")
  );

  let ordered = request_with_headers("If-Match: \"one\"\r\nif-match: W/\"two\"\r\n");
  let parsed = ordered
    .if_match()
    .expect("ordered field should parse")
    .expect("field should be present");
  assert!(!parsed.is_wildcard());
  assert_eq!(2, parsed.entity_tags().len());
  assert_eq!("one", parsed.entity_tags()[0].opaque_tag());
  assert!(!parsed.entity_tags()[0].is_weak());
  assert_eq!("two", parsed.entity_tags()[1].opaque_tag());
  assert!(parsed.entity_tags()[1].is_weak());
  assert_eq!("\"one\", W/\"two\"", parsed.header_value());
  assert_eq!(Some("\"one\""), ordered.header("IF-MATCH"));

  let wildcard = request_with_headers("IF-MATCH: *\r\n");
  let parsed = wildcard
    .if_match()
    .expect("wildcard field should parse")
    .expect("field should be present");
  assert!(parsed.is_wildcard());
  assert!(parsed.entity_tags().is_empty());
  assert_eq!("*", parsed.header_value());
  assert_eq!(Some("*"), wildcard.header("if-match"));
}

#[test]
fn http_request_if_match_rejects_invalid_values_and_preserves_raw_headers() {
  for value in [
    "",
    "not-a-tag",
    "*, \"one\"",
    "\"one\", \"one\"",
    "\"one\",",
    "W/abc",
    "\"one\t\"",
  ] {
    let request = request_with_headers(&format!("If-Match: {value}\r\n"));
    let result: Result<Option<HttpIfMatch>, HttpIfMatchParseError> = request.if_match();
    assert!(result.is_err(), "If-Match should reject {value:?}");
    assert_eq!(Some(value), request.header("If-Match"));
  }

  let duplicate = request_with_headers("If-Match: \"one\"\r\nif-match: \"one\"\r\n");
  let result: Result<Option<HttpIfMatch>, HttpIfMatchParseError> = duplicate.if_match();
  assert!(result.is_err(), "duplicate entity tags should be rejected");
  assert_eq!(Some("\"one\""), duplicate.header("If-Match"));

  let too_many = (0..=MAX_CONDITIONAL_ENTITY_TAGS)
    .map(|index| format!("\"tag-{index}\""))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_request = request_with_headers(&format!("If-Match: {too_many}\r\n"));
  let result: Result<Option<HttpIfMatch>, HttpIfMatchParseError> = too_many_request.if_match();
  assert!(result.is_err(), "the item limit should be enforced");
  assert_eq!(Some(too_many.as_str()), too_many_request.header("If-Match"));
}

#[test]
fn request_if_match_rejects_control_bytes_and_preserves_raw_header() {
  let raw = b"PUT /asset HTTP/1.1\r\nHost: example.test\r\nIf-Match: \"\t\"\r\n\r\n";
  serve_request_once(raw, raw.len(), |request| {
    let result: Result<Option<HttpIfMatch>, HttpIfMatchParseError> = request.if_match();
    assert!(result.is_err(), "control bytes should be rejected");
    assert_eq!(Some("\"\t\""), request.header("If-Match"));
    HttpResponse::ok("")
  });
}

#[test]
fn request_if_match_preserves_raw_over_limit_values_and_returns_typed_error() {
  let value = format!("\"{}\"", "a".repeat(MAX_IF_MATCH_VALUE_BYTES - 1));
  assert_eq!(MAX_IF_MATCH_VALUE_BYTES + 1, value.len());
  let raw = format!("PUT /asset HTTP/1.1\r\nHost: example.test\r\nIf-Match: {value}\r\n\r\n");

  let max_request_head_bytes = raw.len();
  serve_request_once(&raw.into_bytes(), max_request_head_bytes, |request| {
    let result: Result<Option<HttpIfMatch>, HttpIfMatchParseError> = request.if_match();
    assert!(result.is_err(), "the size limit should be enforced");
    assert_eq!(Some(value.as_str()), request.header("If-Match"));
    HttpResponse::ok("")
  });
}

#[test]
fn if_match_parser_rejects_values_beyond_the_shared_size_limit() {
  let value = format!("\"{}\"", "a".repeat(MAX_IF_MATCH_VALUE_BYTES - 1));
  let _: HttpIfMatchParseError = HttpIfMatch::parse(value)
    .expect_err("over-limit If-Match values should return the typed parse error");
}
