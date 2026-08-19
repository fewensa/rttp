use rttp_protocol::content_security_policy_report_only::{
  ContentSecurityPolicyReportOnly, MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS,
  MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_VALUE_BYTES,
};

#[test]
fn content_security_policy_report_only_preserves_opaque_policy_values() {
  let value = "default-src 'self'; report-to csp-endpoint";
  let policy = ContentSecurityPolicyReportOnly::parse(value).expect("CSP report-only should parse");

  assert_eq!(policy.as_str(), value);
  assert_eq!(policy.header_value(), value);
  assert_eq!(policy.as_ref(), value);
  assert_eq!(policy.header_values(), [value]);
}

#[test]
fn content_security_policy_report_only_preserves_multiple_policy_fields() {
  let policy =
    ContentSecurityPolicyReportOnly::parse_values(["default-src 'self'", "object-src 'none'"])
      .expect("multiple CSP report-only fields should parse");

  assert_eq!(policy.as_str(), "default-src 'self'");
  assert_eq!(policy.header_value(), "default-src 'self'");
  assert_eq!(
    policy.header_values(),
    ["default-src 'self'", "object-src 'none'"]
  );
}

#[test]
fn content_security_policy_report_only_rejects_absent_empty_malformed_and_oversized_values() {
  assert!(
    ContentSecurityPolicyReportOnly::parse_values([]).is_err(),
    "absent values must be rejected by the parser"
  );
  assert!(
    ContentSecurityPolicyReportOnly::parse("").is_err(),
    "empty values must be rejected"
  );
  assert!(
    ContentSecurityPolicyReportOnly::parse("default-src 'self'\r\nX-Test: y").is_err(),
    "CRLF controls must be rejected"
  );
  assert!(
    ContentSecurityPolicyReportOnly::parse("default-src 'self'\u{7f}").is_err(),
    "DEL controls must be rejected"
  );
  assert!(
    ContentSecurityPolicyReportOnly::parse(
      "x".repeat(MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_VALUE_BYTES + 1),
    )
    .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn content_security_policy_report_only_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_VALUE_BYTES + 1);

  assert!(
    ContentSecurityPolicyReportOnly::parse_values(["default-src 'self'", oversized.as_str()])
      .is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}

#[test]
fn content_security_policy_report_only_rejects_too_many_repeated_fields() {
  let fields = vec!["default-src 'self'"; MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS];
  let policy = ContentSecurityPolicyReportOnly::parse_values(fields.iter().copied())
    .expect("bounded fields should parse");

  assert_eq!(
    policy.header_values().len(),
    MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS
  );

  let too_many = vec!["default-src 'self'"; MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS + 1];

  assert!(
    ContentSecurityPolicyReportOnly::parse_values(too_many.iter().copied()).is_err(),
    "too many repeated fields must be rejected"
  );
}
