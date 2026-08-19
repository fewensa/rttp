use rttp_protocol::access_control_request_method::{
  AccessControlRequestMethod, MAX_ACCESS_CONTROL_REQUEST_METHOD_VALUE_BYTES,
};

#[test]
fn access_control_request_method_parses_canonical_method() {
  let request_method =
    AccessControlRequestMethod::parse("delete").expect("valid Access-Control-Request-Method");

  assert_eq!(request_method.method(), "DELETE");
  assert_eq!(request_method.header_value(), "DELETE");
}

#[test]
fn access_control_request_method_preserves_canonical_uppercase() {
  let request_method =
    AccessControlRequestMethod::parse("  get  ").expect("valid Access-Control-Request-Method");

  assert_eq!(request_method.method(), "GET");
  assert_eq!(request_method.header_value(), "GET");
}

#[test]
fn access_control_request_method_parse_values_accepts_single_field() {
  let request_method = AccessControlRequestMethod::parse_values(["patch"])
    .expect("single Access-Control-Request-Method field");

  assert_eq!(request_method.method(), "PATCH");
}

#[test]
fn access_control_request_method_rejects_empty_and_malformed_values() {
  for value in [
    "",
    " ",
    "GET, POST",
    "GET,",
    ",GET",
    "GET POST",
    "GET\rPOST",
    "*",
  ] {
    assert!(
      AccessControlRequestMethod::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_request_method_rejects_duplicate_header_fields() {
  assert!(AccessControlRequestMethod::parse_values(["GET", "POST"]).is_err());
}

#[test]
fn access_control_request_method_enforces_value_bounds() {
  assert!(AccessControlRequestMethod::parse(
    "x".repeat(MAX_ACCESS_CONTROL_REQUEST_METHOD_VALUE_BYTES + 1)
  )
  .is_err());
}
