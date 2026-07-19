use rttp_protocol::origin::{Origin, MAX_ORIGIN_VALUE_BYTES};

#[test]
fn origin_parses_null_and_http_origins() {
  let null = Origin::parse("null").expect("null Origin must parse");
  let http = Origin::parse("http://example.test:8080").expect("HTTP Origin must parse");
  let https = Origin::parse("https://example.test").expect("HTTPS Origin must parse");

  assert_eq!(Origin::Null, null);
  assert_eq!("null", null.header_value());
  assert_eq!("http://example.test:8080", http.header_value());
  assert_eq!("https://example.test", https.header_value());
}

#[test]
fn origin_rejects_invalid_singleton_values() {
  for value in [
    "",
    "http://",
    "ftp://example.test",
    "https://example.test/path",
    "https://example.test?query",
    "https://example.test#fragment",
    "https://user@example.test",
    "https://example.test, https://other.test",
    "https://example.test\r\nX-Injected: true",
  ] {
    assert!(Origin::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn origin_rejects_duplicate_singleton_fields() {
  assert!(Origin::parse_values(["https://example.test", "null"]).is_err());
}

#[test]
fn origin_enforces_value_bounds_without_panicking() {
  assert!(Origin::parse("a".repeat(MAX_ORIGIN_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_ORIGIN_VALUE_BYTES + 1);
  assert!(Origin::parse_values(["null", oversized_duplicate.as_str()]).is_err());
}
