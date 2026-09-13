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
fn origin_trims_http_optional_whitespace() {
  let origin = Origin::parse("\thttps://example.test\t").expect("OWS-padded Origin must parse");

  assert_eq!("https://example.test", origin.header_value());
}

#[test]
fn origin_accepts_url_legal_host_characters() {
  let origin = Origin::parse("https://foo_bar.example").expect("URL-legal host must parse");

  assert_eq!("https://foo_bar.example", origin.header_value());
}

#[test]
fn origin_canonicalizes_tuple_components() {
  for (value, expected) in [
    ("http://example.test:80", "http://example.test"),
    ("https://example.test:443", "https://example.test"),
    ("http://example.test:8080", "http://example.test:8080"),
    ("https://example.test:8443", "https://example.test:8443"),
    ("HTTP://EXAMPLE.TEST:8080", "http://example.test:8080"),
    ("hTtPs://EXAMPLE.TEST:443", "https://example.test"),
    ("http://[0:0:0:0:0:0:0:1]", "http://[::1]"),
    ("https://[0:0:0:0:0:0:0:1]:8443", "https://[::1]:8443"),
  ] {
    let origin = Origin::parse(value).expect("tuple Origin must parse");
    assert_eq!(
      expected,
      origin.header_value(),
      "unexpected canonical form for {value:?}"
    );
  }
}

#[test]
fn origin_handles_opaque_null() {
  for value in ["null", "\tnull "] {
    let origin = Origin::parse(value).expect("opaque null Origin must parse");

    assert_eq!(Origin::Null, origin);
    assert_eq!("null", origin.header_value());
  }
}

#[test]
fn origin_rejects_invalid_singleton_values() {
  for value in [
    "",
    "   ",
    "http://",
    "ftp://example.test",
    "HTTPS+TCP://example.test",
    "https://example.test/path",
    "https://example.test?query",
    "https://example.test#fragment",
    "https://user@example.test",
    "https://example.test, https://other.test",
    "https://example.test:",
    "https://example.test:abc",
    "https://example.test:1.5",
    "https://example.test:65536",
    "https://example.test:999999999999",
    "https://[::1]:",
    "https://[::1]:65536",
    "https://[::1]8443",
    "https://example.test\n",
    "https://example.test\r",
    "https://example.test\0",
    "https://example.test\x7f",
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
