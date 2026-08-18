use rttp_protocol::content_security_policy::{
  ContentSecurityPolicy, MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES,
};

#[test]
fn content_security_policy_preserves_opaque_policy_values() {
  let value = "default-src 'self'; object-src 'none'";
  let policy = ContentSecurityPolicy::parse(value).expect("CSP should parse");

  assert_eq!(policy.as_str(), value);
  assert_eq!(policy.header_value(), value);
  assert_eq!(policy.as_ref(), value);
}

#[test]
fn content_security_policy_rejects_absent_empty_duplicate_malformed_and_oversized_values() {
  assert!(
    ContentSecurityPolicy::parse_values([]).is_err(),
    "absent singleton values must be rejected by the parser"
  );
  assert!(
    ContentSecurityPolicy::parse("").is_err(),
    "empty values must be rejected"
  );
  assert!(
    ContentSecurityPolicy::parse("default-src 'self'\r\nX-Test: y").is_err(),
    "CRLF controls must be rejected"
  );
  assert!(
    ContentSecurityPolicy::parse("default-src 'self'\u{7f}").is_err(),
    "DEL controls must be rejected"
  );
  assert!(
    ContentSecurityPolicy::parse_values(["default-src 'self'", "object-src 'none'"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    ContentSecurityPolicy::parse("x".repeat(MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn content_security_policy_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES + 1);

  assert!(
    ContentSecurityPolicy::parse_values(["default-src 'self'", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
