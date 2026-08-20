use rttp_protocol::cross_origin_opener_policy::CrossOriginOpenerPolicy;
use rttp_protocol::cross_origin_opener_policy_report_only::{
  CrossOriginOpenerPolicyReportOnly, CrossOriginOpenerPolicyReportOnlyBareItem,
  MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_VALUE_BYTES,
};

#[test]
fn cross_origin_opener_policy_report_only_reuses_canonical_coop_directives() {
  for (value, policy) in [
    ("unsafe-none", CrossOriginOpenerPolicy::UnsafeNone),
    (
      "same-origin-allow-popups",
      CrossOriginOpenerPolicy::SameOriginAllowPopups,
    ),
    ("same-origin", CrossOriginOpenerPolicy::SameOrigin),
    (
      "noopener-allow-popups",
      CrossOriginOpenerPolicy::NoopenerAllowPopups,
    ),
  ] {
    let report_only = CrossOriginOpenerPolicyReportOnly::parse(value)
      .expect("canonical COOP directive should parse as report-only");
    assert_eq!(policy, report_only.policy());
    assert_eq!(policy, CrossOriginOpenerPolicy::parse(value).expect("COOP"));
    assert_eq!(value, report_only.header_value());
    assert_eq!(value, policy.header_value());
    assert_eq!(None, report_only.report_to());
    assert!(report_only.parameters().is_empty());
  }
}

#[test]
fn cross_origin_opener_policy_report_only_preserves_reporting_parameters() {
  let policy =
    CrossOriginOpenerPolicyReportOnly::parse(r#"same-origin; report-to="coop"; endpoint="canary""#)
      .expect("parameterized report-only COOP should parse");

  assert_eq!(CrossOriginOpenerPolicy::SameOrigin, policy.policy());
  assert_eq!(Some("coop"), policy.report_to());
  assert_eq!(
    ["report-to", "endpoint"],
    policy
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    &CrossOriginOpenerPolicyReportOnlyBareItem::String("coop".to_string()),
    policy.parameters()[0].value()
  );
  assert_eq!(
    r#"same-origin; report-to="coop"; endpoint="canary""#,
    policy.header_value()
  );
}

#[test]
fn cross_origin_opener_policy_report_only_formats_boolean_parameters() {
  let present = CrossOriginOpenerPolicyReportOnly::parse("same-origin; flag")
    .expect("boolean-true parameter should parse");
  assert_eq!("same-origin; flag", present.header_value());
  assert_eq!(
    &CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(true),
    present.parameters()[0].value()
  );

  let absent = CrossOriginOpenerPolicyReportOnly::parse("same-origin; flag=?0")
    .expect("boolean-false parameter should parse");
  assert_eq!("same-origin; flag=?0", absent.header_value());
  assert_eq!(
    &CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(false),
    absent.parameters()[0].value()
  );
}

#[test]
fn cross_origin_opener_policy_report_only_accepts_http_optional_whitespace_padding() {
  for value in [
    "\tsame-origin\t",
    " \tsame-origin\t ",
    "same-origin\t",
    "\tsame-origin",
  ] {
    let policy =
      CrossOriginOpenerPolicyReportOnly::parse(value).expect("OWS-padded same-origin should parse");
    assert_eq!(CrossOriginOpenerPolicy::SameOrigin, policy.policy());
  }
}

#[test]
fn cross_origin_opener_policy_report_only_rejects_case_variants() {
  for value in ["SAME-ORIGIN", "Same-Origin-Allow-Popups", "Unsafe-None"] {
    assert!(
      CrossOriginOpenerPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn cross_origin_opener_policy_report_only_rejects_empty_duplicate_malformed_and_oversized_values() {
  for value in [
    "",
    "   ",
    "unknown",
    "same-origin-plus-coep",
    "same-origin, same-origin-allow-popups",
    "\"same-origin\"",
    "same-origin\r\nX: y",
    "same-origin\u{7f}",
    r#"same-origin; report-to="coop"; report-to="other""#,
  ] {
    assert!(
      CrossOriginOpenerPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    CrossOriginOpenerPolicyReportOnly::parse_values(["same-origin", "same-origin-allow-popups"])
      .is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    CrossOriginOpenerPolicyReportOnly::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    CrossOriginOpenerPolicyReportOnly::parse(
      "a".repeat(MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_VALUE_BYTES + 1)
    )
    .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn cross_origin_opener_policy_report_only_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_VALUE_BYTES + 1);

  assert!(
    CrossOriginOpenerPolicyReportOnly::parse_values(["same-origin", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
