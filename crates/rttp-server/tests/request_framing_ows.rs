use rttp_server::server::{HttpParseError, HttpRequest};

const BODY: &[u8] = b"ABCD";
const CHUNKED_BODY: &[u8] = b"4\r\nABCD\r\n0\r\n\r\n";
const VT: char = '\u{000b}';
const FF: char = '\u{000c}';
const NBSP: char = '\u{00a0}';
const EM_SPACE: char = '\u{2003}';
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

fn raw_request(headers: &str, body: &[u8]) -> Vec<u8> {
  let mut raw = b"POST /upload HTTP/1.1\r\nHost: example.test\r\n".to_vec();
  raw.extend_from_slice(headers.as_bytes());
  raw.extend_from_slice(b"\r\n");
  raw.extend_from_slice(body);
  raw
}

fn parse_request(headers: &str, body: &[u8]) -> Result<HttpRequest, HttpParseError> {
  HttpRequest::parse(&raw_request(headers, body))
}

fn assert_fixed_body(headers: &str) {
  let request =
    parse_request(headers, BODY).expect("OWS-padded Content-Length request should parse");
  assert_eq!(BODY, request.body());
  assert_eq!(
    Some(BODY.len()),
    request.content_length().map(|length| length.len())
  );
}

fn assert_chunked_body(headers: &str) {
  let request =
    parse_request(headers, CHUNKED_BODY).expect("OWS-padded chunked request should parse");
  assert_eq!(BODY, request.body());
  assert_eq!(None, request.content_length());
}

fn assert_framing_rejected(headers: &str, body: &[u8], expected: &str) {
  let error = parse_request(headers, body).expect_err("malformed framing should fail to parse");
  let message = error.to_string();
  assert_eq!(expected, message);
  assert!(
    !message.contains("does not match Content-Length")
      && !message.contains("does not match Transfer-Encoding"),
    "malformed framing should fail before reading the body: {message}"
  );
}

#[test]
fn content_length_accepts_leading_and_trailing_sp_and_htab() {
  for value in ["4", " 4", "4 ", " 4 ", "\t4", "4\t", "\t4\t", " \t4\t "] {
    assert_fixed_body(&format!("Content-Length: {value}\r\n"));
  }
}

#[test]
fn content_length_accepts_sp_and_htab_around_comma_separated_tokens() {
  for value in ["4,4", "4, 4", "4,\t4", " 4 , 4 ", "4,\t 4\t"] {
    assert_fixed_body(&format!("Content-Length: {value}\r\n"));
  }
}

#[test]
fn matching_repeated_content_length_fields_accept_ows() {
  assert_fixed_body("Content-Length: 4\r\nContent-Length: 4\r\n");
  assert_fixed_body("Content-Length:  4  \r\nContent-Length:\t4\t\r\n");
  assert_fixed_body("Content-Length: 4, 4\r\nContent-Length:\t4\r\n");
}

#[test]
fn conflicting_content_length_fields_are_rejected_before_the_body() {
  assert_framing_rejected(
    "Content-Length: 4\r\nContent-Length: 5\r\n",
    BODY,
    "conflicting Content-Length headers",
  );
  assert_framing_rejected(
    "Content-Length: 4, 5\r\n",
    BODY,
    "conflicting Content-Length headers",
  );
}

#[test]
fn transfer_encoding_accepts_leading_and_trailing_sp_and_htab_for_chunked() {
  for value in [
    "chunked",
    " chunked",
    "chunked ",
    " chunked ",
    "\tchunked",
    "chunked\t",
    "\tchunked\t",
    " \tchunked\t ",
  ] {
    assert_chunked_body(&format!("Transfer-Encoding: {value}\r\n"));
  }
}

#[test]
fn content_length_rejects_non_ows_padding_before_reading_the_body() {
  for padding in [VT, FF, NBSP, EM_SPACE, IDEOGRAPHIC_SPACE] {
    let expected = if matches!(padding, VT | FF) {
      "invalid request header"
    } else {
      "invalid Content-Length header"
    };
    assert_framing_rejected(&format!("Content-Length: {padding}4\r\n"), BODY, expected);
    assert_framing_rejected(&format!("Content-Length: 4{padding}\r\n"), BODY, expected);
    assert_framing_rejected(
      &format!("Content-Length: 4{padding}, {padding}4\r\n"),
      BODY,
      expected,
    );
  }
}

#[test]
fn transfer_encoding_rejects_non_ows_padding_before_reading_the_body() {
  for padding in [VT, FF, NBSP, EM_SPACE, IDEOGRAPHIC_SPACE] {
    let expected = if matches!(padding, VT | FF) {
      "invalid request header"
    } else {
      "unsupported Transfer-Encoding request body"
    };
    assert_framing_rejected(
      &format!("Transfer-Encoding: {padding}chunked\r\n"),
      CHUNKED_BODY,
      expected,
    );
    assert_framing_rejected(
      &format!("Transfer-Encoding: chunked{padding}\r\n"),
      CHUNKED_BODY,
      expected,
    );
  }
}
