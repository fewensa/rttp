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
fn access_control_expose_headers_ignores_empty_list_elements() {
  let expose_headers = AccessControlExposeHeaders::parse("X-Request-Id,, ETag,")
    .expect("empty list elements should be ignored");

  assert_eq!(expose_headers.field_names(), ["x-request-id", "etag"]);
}

#[test]
fn access_control_expose_headers_preserves_wildcard_with_field_names() {
  let expose_headers = AccessControlExposeHeaders::parse("*, X-Request-Id")
    .expect("wildcard and field names should be preserved");

  assert!(expose_headers.is_wildcard());
  assert_eq!(expose_headers.field_names(), ["x-request-id"]);
  assert_eq!(expose_headers.header_value(), "*, x-request-id");
}

#[test]
fn access_control_expose_headers_deduplicates_field_names_across_header_fields() {
  let expose_headers =
    AccessControlExposeHeaders::parse_values(["X-Request-Id, ETag", "x-request-id, X-Request-Id"])
      .expect("duplicate field names should be deduplicated");

  assert_eq!(expose_headers.field_names(), ["x-request-id", "etag"]);
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
fn access_control_expose_headers_rejects_malformed_values() {
  for value in ["", "X Request Id"] {
    assert!(
      AccessControlExposeHeaders::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
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
