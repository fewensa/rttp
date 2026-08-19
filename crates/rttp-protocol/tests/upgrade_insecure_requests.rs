use rttp_protocol::upgrade_insecure_requests::{
  UpgradeInsecureRequests, MAX_UPGRADE_INSECURE_REQUESTS_VALUE_BYTES,
};

#[test]
fn upgrade_insecure_requests_parses_signal_value() {
  let metadata = UpgradeInsecureRequests::parse("1").expect("valid Upgrade-Insecure-Requests");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn upgrade_insecure_requests_parse_values_accepts_single_ows_padded_field() {
  let metadata = UpgradeInsecureRequests::parse_values(["\t1 "])
    .expect("single Upgrade-Insecure-Requests field");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn upgrade_insecure_requests_rejects_malformed_values() {
  for value in [
    "", " ", "\t", "0", "true", "?1", "1, 1", "01", "1.0", "TRUE", "1;q=1",
  ] {
    assert!(
      UpgradeInsecureRequests::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn upgrade_insecure_requests_rejects_control_bytes() {
  for value in ["1\r", "1\n", "1\u{7f}"] {
    assert!(
      UpgradeInsecureRequests::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn upgrade_insecure_requests_rejects_duplicate_header_fields() {
  assert!(UpgradeInsecureRequests::parse_values(["1", "1"]).is_err());
}

#[test]
fn upgrade_insecure_requests_rejects_empty_value_lists() {
  assert!(UpgradeInsecureRequests::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn upgrade_insecure_requests_enforces_value_bounds() {
  let oversized = "1".repeat(MAX_UPGRADE_INSECURE_REQUESTS_VALUE_BYTES + 1);
  assert!(UpgradeInsecureRequests::parse(oversized).is_err());
}
