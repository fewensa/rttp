use rttp_protocol::access_control_allow_methods::{
  AccessControlAllowMethods, MAX_ACCESS_CONTROL_ALLOW_METHODS_METHODS,
  MAX_ACCESS_CONTROL_ALLOW_METHODS_VALUE_BYTES,
};

#[test]
fn access_control_allow_methods_parses_normalized_methods() {
  let allow_methods = AccessControlAllowMethods::parse("get, POST, patch")
    .expect("valid Access-Control-Allow-Methods");

  assert_eq!(allow_methods.methods(), ["GET", "POST", "PATCH"]);
  assert!(!allow_methods.is_wildcard());
  assert_eq!(allow_methods.header_value(), "GET, POST, PATCH");
}

#[test]
fn access_control_allow_methods_accepts_wildcard() {
  let allow_methods =
    AccessControlAllowMethods::parse("*").expect("wildcard Access-Control-Allow-Methods");

  assert!(allow_methods.is_wildcard());
  assert!(allow_methods.methods().is_empty());
  assert_eq!(allow_methods.header_value(), "*");
}

#[test]
fn access_control_allow_methods_combines_and_deduplicates_multiple_header_fields() {
  let allow_methods = AccessControlAllowMethods::parse_values(["GET, post", "PATCH, GET"])
    .expect("multiple Access-Control-Allow-Methods fields");

  assert_eq!(allow_methods.methods(), ["GET", "POST", "PATCH"]);
}

#[test]
fn access_control_allow_methods_preserves_wildcard_with_methods() {
  let allow_methods =
    AccessControlAllowMethods::parse("*, GET").expect("wildcard and methods should be preserved");

  assert!(allow_methods.is_wildcard());
  assert_eq!(allow_methods.methods(), ["GET"]);
  assert_eq!(allow_methods.header_value(), "*, GET");
}

#[test]
fn access_control_allow_methods_rejects_malformed_values() {
  for value in ["", "GET,", ",GET", "GET,,POST", "GET POST", "GET\rPOST"] {
    assert!(
      AccessControlAllowMethods::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_methods_enforces_value_and_method_count_bounds() {
  assert!(AccessControlAllowMethods::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_METHODS_VALUE_BYTES + 1)
  )
  .is_err());

  let too_many = std::iter::repeat_n("GET", MAX_ACCESS_CONTROL_ALLOW_METHODS_METHODS + 1)
    .collect::<Vec<_>>()
    .join(",");
  assert!(AccessControlAllowMethods::parse(too_many).is_err());

  assert!(AccessControlAllowMethods::parse_values(std::iter::repeat_n(
    "*",
    MAX_ACCESS_CONTROL_ALLOW_METHODS_METHODS + 1,
  ))
  .is_err());
}
