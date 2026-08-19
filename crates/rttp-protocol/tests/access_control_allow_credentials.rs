use rttp_protocol::access_control_allow_credentials::{
  AccessControlAllowCredentials, MAX_ACCESS_CONTROL_ALLOW_CREDENTIALS_VALUE_BYTES,
};

#[test]
fn access_control_allow_credentials_parses_the_true_token() {
  for value in ["true", "TRUE", "True", " true ", "\ttrue"] {
    let credentials =
      AccessControlAllowCredentials::parse(value).expect("the true token should parse");
    assert_eq!("true", credentials.header_value());
  }
}

#[test]
fn access_control_allow_credentials_rejects_duplicate_and_malformed_values() {
  for values in [
    vec!["true", "true"],
    vec![""],
    vec!["  "],
    vec!["false"],
    vec!["true, true"],
    vec!["true\r\n"],
    vec!["true\n"],
  ] {
    assert!(
      AccessControlAllowCredentials::parse_values(values.iter().copied()).is_err(),
      "{values:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_credentials_enforces_the_value_bound() {
  assert!(AccessControlAllowCredentials::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_CREDENTIALS_VALUE_BYTES + 1)
  )
  .is_err());
}
