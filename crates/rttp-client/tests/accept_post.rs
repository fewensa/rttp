use rttp_client::response::{
  AcceptPost, AcceptPostParseError, MediaType, MediaTypeParameter, Response,
};
use rttp_client::types::RoUrl;

fn response_with_values(values: &[&str]) -> Response {
  let mut raw = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    raw.push_str("Accept-Post: ");
    raw.push_str(value);
    raw.push_str("\r\n");
  }
  raw.push_str("Content-Length: 0\r\n\r\n");
  Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("response should remain parseable")
}

#[test]
fn response_accept_post_uses_protocol_media_types_across_repeated_fields() {
  let response = response_with_values(&[
    r#"Text/Plain; title="a,b\"c", application/json"#,
    "text/plain; charset=utf-8",
  ]);
  let metadata = response
    .accept_post()
    .expect("Accept-Post should parse")
    .expect("Accept-Post should be present");

  let _: &[MediaType] = metadata.media_types();
  let _: &[MediaTypeParameter] = metadata.media_types()[0].parameters();
  assert_eq!(metadata.len(), 3);
  assert_eq!(metadata.media_types()[0].type_(), "Text");
  assert_eq!(metadata.media_types()[0].subtype(), "Plain");
  assert_eq!(metadata.media_types()[0].parameters()[0].value(), "a,b\"c");
  assert_eq!(metadata.media_types()[1].subtype(), "json");
  assert_eq!(metadata.media_types()[2].subtype(), "plain");
}

#[test]
fn response_accept_post_parse_failures_preserve_raw_headers() {
  for value in [
    "application/json,",
    "application/json; charset",
    "application/json\0",
  ] {
    let response = response_with_values(&[value]);
    assert!(response.accept_post().is_err(), "{value:?} should fail");
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Accept-Post")
    );
  }
}

#[test]
fn response_accept_post_is_absent_without_a_header_and_exports_parse_error() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Accept-Post should parse");
  assert_eq!(
    None,
    response
      .accept_post()
      .expect("absent header should be valid")
  );

  let _: AcceptPostParseError =
    AcceptPost::parse("application/json,").expect_err("malformed metadata should fail");
}

#[test]
fn response_accept_post_enforces_member_bound_without_losing_raw_header() {
  let value = std::iter::repeat_n("application/json", 257)
    .collect::<Vec<_>>()
    .join(", ");
  let response = response_with_values(&[&value]);
  assert!(response.accept_post().is_err());
  assert_eq!(Some(&value), response.header_value("Accept-Post"));
}

#[test]
fn response_accept_post_enforces_byte_bound_without_losing_raw_header() {
  let value = "x".repeat(64 * 1024 + 1);
  let response = response_with_values(&[&value]);
  assert!(response.accept_post().is_err());
  assert_eq!(Some(&value), response.header_value("Accept-Post"));
}
