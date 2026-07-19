use rttp_protocol::cross_origin_resource_policy::{
  CrossOriginResourcePolicy, MAX_CROSS_ORIGIN_RESOURCE_POLICY_VALUE_BYTES,
};

#[test]
fn cross_origin_resource_policy_parses_standard_values_case_insensitively() {
  assert_eq!(
    CrossOriginResourcePolicy::SameOrigin,
    CrossOriginResourcePolicy::parse("SAME-ORIGIN").expect("same-origin should parse")
  );
  assert_eq!(
    CrossOriginResourcePolicy::SameSite,
    CrossOriginResourcePolicy::parse("same-site").expect("same-site should parse")
  );
  assert_eq!(
    CrossOriginResourcePolicy::CrossOrigin,
    CrossOriginResourcePolicy::parse("Cross-Origin").expect("cross-origin should parse")
  );
  assert_eq!(
    "same-origin",
    CrossOriginResourcePolicy::SameOrigin.header_value()
  );
}

#[test]
fn cross_origin_resource_policy_rejects_empty_duplicate_malformed_and_oversized_values() {
  for value in [
    "",
    "same origin",
    "same-origin, same-site",
    "unknown",
    "same-origin\r\nX: y",
  ] {
    assert!(
      CrossOriginResourcePolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    CrossOriginResourcePolicy::parse_values(["same-origin", "same-site"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    CrossOriginResourcePolicy::parse("a".repeat(MAX_CROSS_ORIGIN_RESOURCE_POLICY_VALUE_BYTES + 1))
      .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn cross_origin_resource_policy_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_CROSS_ORIGIN_RESOURCE_POLICY_VALUE_BYTES + 1);

  assert!(
    CrossOriginResourcePolicy::parse_values(["same-origin", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
