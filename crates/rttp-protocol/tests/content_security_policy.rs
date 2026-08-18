use rttp_protocol::content_security_policy::{
  ContentSecurityPolicy, MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES,
};

#[test]
fn parses_and_preserves_policy_text() {
  let value = "default-src 'self'; img-src https:";
  let policy = ContentSecurityPolicy::parse(value).expect("CSP should parse");

  assert_eq!(policy.as_str(), value);
  assert_eq!(policy.header_value(), value);
  assert_eq!(policy.as_ref(), value);
}

#[test]
fn parse_values_accepts_singleton_policy() {
  let policy = ContentSecurityPolicy::parse_values(["default-src\t'self'"])
    .expect("CSP with HTAB should parse");

  assert_eq!(policy.header_value(), "default-src\t'self'");
}

#[test]
fn rejects_absent_empty_duplicate_and_oversized_values() {
  assert!(ContentSecurityPolicy::parse_values([]).is_err());
  assert!(ContentSecurityPolicy::parse("").is_err());
  assert!(
    ContentSecurityPolicy::parse_values(["default-src 'self'", "script-src 'none'"]).is_err()
  );

  let oversized = "a".repeat(MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES + 1);
  assert!(ContentSecurityPolicy::parse(oversized.as_str()).is_err());
  assert!(ContentSecurityPolicy::parse_values(["default-src 'self'", oversized.as_str()]).is_err());
}

#[test]
fn rejects_invalid_control_bytes_but_allows_htab() {
  assert!(ContentSecurityPolicy::parse("default-src\t'self'").is_ok());
  assert!(ContentSecurityPolicy::parse("default-src\n'self'").is_err());
  assert!(ContentSecurityPolicy::parse("default-src\u{7f}'self'").is_err());
}
