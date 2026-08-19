use rttp_protocol::access_control_request_headers::{
  AccessControlRequestHeaders, MAX_ACCESS_CONTROL_REQUEST_HEADERS_FIELD_NAMES,
  MAX_ACCESS_CONTROL_REQUEST_HEADERS_VALUE_BYTES,
};

#[test]
fn access_control_request_headers_parses_normalized_field_names() {
  let request_headers = AccessControlRequestHeaders::parse("X-Request-Id, Authorization")
    .expect("valid Access-Control-Request-Headers");

  assert_eq!(
    request_headers.field_names(),
    ["x-request-id", "authorization"]
  );
  assert_eq!(request_headers.len(), 2);
  assert!(!request_headers.is_empty());
  assert_eq!(
    request_headers.header_value(),
    "x-request-id, authorization"
  );
}

#[test]
fn access_control_request_headers_accepts_star_as_a_field_name() {
  let request_headers =
    AccessControlRequestHeaders::parse("*").expect("star Access-Control-Request-Headers");

  assert_eq!(request_headers.field_names(), ["*"]);
  assert_eq!(request_headers.header_value(), "*");
}

#[test]
fn access_control_request_headers_combines_multiple_header_fields() {
  let request_headers =
    AccessControlRequestHeaders::parse_values(["X-Request-Id, Authorization", "X-Custom"])
      .expect("multiple Access-Control-Request-Headers fields");

  assert_eq!(
    request_headers.field_names(),
    ["x-request-id", "authorization", "x-custom"]
  );
}

#[test]
fn access_control_request_headers_rejects_malformed_members() {
  for value in [
    "",
    "X-Id,",
    ",X-Id",
    "X-Id,,Y",
    "X Request Id",
    "X-Id\rY",
    "X-Id\nY",
  ] {
    assert!(
      AccessControlRequestHeaders::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_request_headers_rejects_case_insensitive_duplicates() {
  assert!(
    AccessControlRequestHeaders::parse("X-Id, x-id").is_err(),
    "duplicate field names must be rejected"
  );
}

#[test]
fn access_control_request_headers_enforces_value_and_field_name_bounds() {
  assert!(AccessControlRequestHeaders::parse(
    "x".repeat(MAX_ACCESS_CONTROL_REQUEST_HEADERS_VALUE_BYTES + 1)
  )
  .is_err());

  let at_limit = (0..MAX_ACCESS_CONTROL_REQUEST_HEADERS_FIELD_NAMES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlRequestHeaders::parse(at_limit).is_ok());

  let too_many = (0..=MAX_ACCESS_CONTROL_REQUEST_HEADERS_FIELD_NAMES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlRequestHeaders::parse(too_many).is_err());
}
