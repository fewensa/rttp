use rttp_protocol::access_control_allow_private_network::{
  AccessControlAllowPrivateNetwork, MAX_ACCESS_CONTROL_ALLOW_PRIVATE_NETWORK_VALUE_BYTES,
};

#[test]
fn access_control_allow_private_network_parses_the_true_token() {
  for value in ["true", " true ", "\ttrue\t"] {
    let metadata =
      AccessControlAllowPrivateNetwork::parse(value).expect("the true token should parse");
    assert_eq!("true", metadata.header_value());
  }
}

#[test]
fn access_control_allow_private_network_rejects_duplicate_and_malformed_values() {
  for values in [
    vec!["true", "true"],
    vec![""],
    vec!["  "],
    vec!["false"],
    vec!["TRUE"],
    vec!["True"],
    vec!["true, true"],
    vec!["true\r\n"],
    vec!["true\n"],
  ] {
    assert!(
      AccessControlAllowPrivateNetwork::parse_values(values.iter().copied()).is_err(),
      "{values:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_private_network_rejects_control_bytes() {
  for value in ["true\0", "true\r", "true\n", "true\u{7f}"] {
    assert!(
      AccessControlAllowPrivateNetwork::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_private_network_enforces_the_value_bound() {
  assert!(AccessControlAllowPrivateNetwork::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_PRIVATE_NETWORK_VALUE_BYTES + 1)
  )
  .is_err());
}
