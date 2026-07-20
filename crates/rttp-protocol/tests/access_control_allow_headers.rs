use rttp_protocol::access_control_allow_headers::{
  AccessControlAllowHeaders, MAX_ACCESS_CONTROL_ALLOW_HEADERS_FIELD_NAMES,
  MAX_ACCESS_CONTROL_ALLOW_HEADERS_VALUE_BYTES,
};

#[test]
fn access_control_allow_headers_parses_normalized_field_names() {
  let allow_headers = AccessControlAllowHeaders::parse("X-Request-Id, ETag")
    .expect("valid Access-Control-Allow-Headers");

  assert_eq!(allow_headers.field_names(), ["x-request-id", "etag"]);
  assert!(!allow_headers.is_wildcard());
  assert_eq!(allow_headers.header_value(), "x-request-id, etag");
}

#[test]
fn access_control_allow_headers_accepts_wildcard_as_the_only_member() {
  let allow_headers =
    AccessControlAllowHeaders::parse("*").expect("wildcard Access-Control-Allow-Headers");

  assert!(allow_headers.is_wildcard());
  assert!(allow_headers.field_names().is_empty());
  assert_eq!(allow_headers.header_value(), "*");
}

#[test]
fn access_control_allow_headers_combines_multiple_header_fields() {
  let allow_headers =
    AccessControlAllowHeaders::parse_values(["X-Request-Id, ETag", "X-RateLimit-Remaining"])
      .expect("multiple Access-Control-Allow-Headers fields");

  assert_eq!(
    allow_headers.field_names(),
    ["x-request-id", "etag", "x-ratelimit-remaining"]
  );
}

#[test]
fn access_control_allow_headers_rejects_malformed_members() {
  for value in [
    "",
    "X-Request-Id,",
    ",X-Request-Id",
    "X-Request-Id,,ETag",
    "X Request Id",
    "X-Request-Id\rETag",
    "*, X-Request-Id",
    "X-Request-Id, *",
    "*, *",
  ] {
    assert!(
      AccessControlAllowHeaders::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_headers_rejects_case_insensitive_duplicates() {
  for values in [
    vec!["X-Request-Id, x-request-id"],
    vec!["X-Request-Id", "X-REQUEST-ID"],
  ] {
    assert!(
      AccessControlAllowHeaders::parse_values(values).is_err(),
      "duplicate field names must be rejected"
    );
  }
}

#[test]
fn access_control_allow_headers_enforces_value_and_field_name_bounds() {
  assert!(AccessControlAllowHeaders::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_HEADERS_VALUE_BYTES + 1)
  )
  .is_err());

  let at_limit = (0..MAX_ACCESS_CONTROL_ALLOW_HEADERS_FIELD_NAMES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlAllowHeaders::parse(at_limit).is_ok());

  let too_many = (0..=MAX_ACCESS_CONTROL_ALLOW_HEADERS_FIELD_NAMES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlAllowHeaders::parse(too_many).is_err());
}
