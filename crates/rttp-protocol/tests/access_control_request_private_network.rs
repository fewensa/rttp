use rttp_protocol::access_control_request_private_network::{
  AccessControlRequestPrivateNetwork, MAX_ACCESS_CONTROL_REQUEST_PRIVATE_NETWORK_VALUE_BYTES,
};

#[test]
fn access_control_request_private_network_parses_true_request_form() {
  let metadata = AccessControlRequestPrivateNetwork::parse("true")
    .expect("valid Access-Control-Request-Private-Network");

  assert_eq!("true", metadata.header_value());
}

#[test]
fn access_control_request_private_network_parse_values_accepts_single_field() {
  let metadata = AccessControlRequestPrivateNetwork::parse_values(["\ttrue "])
    .expect("single Access-Control-Request-Private-Network field");

  assert_eq!("true", metadata.header_value());
}

#[test]
fn access_control_request_private_network_rejects_malformed_values() {
  for value in ["", "false", "TRUE", "True", "?1", "1", "true, true"] {
    assert!(
      AccessControlRequestPrivateNetwork::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn access_control_request_private_network_rejects_control_bytes() {
  for value in ["true\r", "true\n", "true\u{7f}"] {
    assert!(
      AccessControlRequestPrivateNetwork::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn access_control_request_private_network_rejects_duplicate_header_fields() {
  assert!(AccessControlRequestPrivateNetwork::parse_values(["true", "true"]).is_err());
}

#[test]
fn access_control_request_private_network_enforces_value_bounds() {
  let oversized = "t".repeat(MAX_ACCESS_CONTROL_REQUEST_PRIVATE_NETWORK_VALUE_BYTES + 1);
  assert!(AccessControlRequestPrivateNetwork::parse(oversized).is_err());
}
