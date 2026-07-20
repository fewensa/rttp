use rttp_protocol::access_control_max_age::{
  AccessControlMaxAge, MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES,
};

#[test]
fn access_control_max_age_parses_unsigned_u64_boundaries() {
  let zero = AccessControlMaxAge::parse("0").expect("zero delta-seconds");
  assert_eq!(zero.seconds(), 0);
  assert_eq!(zero.header_value(), "0");

  let maximum =
    AccessControlMaxAge::parse(u64::MAX.to_string()).expect("maximum u64 delta-seconds");
  assert_eq!(maximum.seconds(), u64::MAX);
  assert_eq!(maximum.header_value(), u64::MAX.to_string());
}

#[test]
fn access_control_max_age_rejects_duplicate_and_invalid_values() {
  assert!(AccessControlMaxAge::parse_values(["60", "120"]).is_err());

  for value in [
    "",
    " ",
    "60, 120",
    "+60",
    "-60",
    "60.0",
    "sixy",
    "18446744073709551616",
  ] {
    assert!(
      AccessControlMaxAge::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_max_age_enforces_value_bounds() {
  assert!(
    AccessControlMaxAge::parse("0".repeat(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES + 1)).is_err()
  );
}
