use rttp_protocol::access_control_expose_headers::{
  AccessControlExposeHeaders, MAX_ACCESS_CONTROL_EXPOSE_HEADERS_FIELD_NAMES,
  MAX_ACCESS_CONTROL_EXPOSE_HEADERS_VALUE_BYTES,
};

#[test]
fn access_control_expose_headers_parses_normalized_field_names() {
  let expose_headers = AccessControlExposeHeaders::parse("X-Request-Id, ETag")
    .expect("valid Access-Control-Expose-Headers");

  assert_eq!(expose_headers.field_names(), ["x-request-id", "etag"]);
  assert!(!expose_headers.is_wildcard());
  assert_eq!(expose_headers.header_value(), "x-request-id, etag");
}

#[test]
fn access_control_expose_headers_accepts_wildcard() {
  let expose_headers =
    AccessControlExposeHeaders::parse("*").expect("wildcard Access-Control-Expose-Headers");

  assert!(expose_headers.is_wildcard());
  assert!(expose_headers.field_names().is_empty());
  assert_eq!(expose_headers.header_value(), "*");
}

#[test]
fn access_control_expose_headers_combines_multiple_header_fields() {
  let expose_headers =
    AccessControlExposeHeaders::parse_values(["X-Request-Id, ETag", "X-RateLimit-Remaining"])
      .expect("multiple Access-Control-Expose-Headers fields");

  assert_eq!(
    expose_headers.field_names(),
    ["x-request-id", "etag", "x-ratelimit-remaining"]
  );
}

#[test]
fn access_control_expose_headers_rejects_malformed_duplicate_and_mixed_wildcard_values() {
  for value in [
    "",
    "X-Request-Id,",
    "X Request Id",
    "X-Request-Id, x-request-id",
    "*, X-Request-Id",
    "X-Request-Id, *",
    "*, *",
  ] {
    assert!(
      AccessControlExposeHeaders::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(AccessControlExposeHeaders::parse_values(["X-Request-Id", "x-request-id"]).is_err());
  assert!(AccessControlExposeHeaders::parse_values(["*", "X-Request-Id"]).is_err());
  assert!(AccessControlExposeHeaders::parse_values(["X-Request-Id", "*"]).is_err());
}

#[test]
fn access_control_expose_headers_enforces_value_and_field_count_bounds() {
  assert!(AccessControlExposeHeaders::parse(
    "x".repeat(MAX_ACCESS_CONTROL_EXPOSE_HEADERS_VALUE_BYTES + 1)
  )
  .is_err());

  let too_many = std::iter::repeat_n("x", MAX_ACCESS_CONTROL_EXPOSE_HEADERS_FIELD_NAMES + 1)
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlExposeHeaders::parse(too_many).is_err());
}
