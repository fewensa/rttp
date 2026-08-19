use rttp_protocol::document_policy_report_only::{
  DocumentPolicyReportOnly, DocumentPolicyReportOnlyValue, MAX_DOCUMENT_POLICY_DIRECTIVES,
  MAX_DOCUMENT_POLICY_TOTAL_BYTES, MAX_DOCUMENT_POLICY_VALUE_BYTES,
};

#[test]
fn document_policy_report_only_parses_dictionary_with_document_policy_model() {
  let policy =
    DocumentPolicyReportOnly::parse("oversized-images=2.0, unsized-media=?0, *;report-to=default")
      .expect("Document-Policy-Report-Only should parse");

  assert_eq!(policy.len(), 3);
  assert!(!policy.is_empty());
  assert_eq!(policy.directives()[0].name(), "oversized-images");
  assert_eq!(
    policy.directive("oversized-images").unwrap().value(),
    &DocumentPolicyReportOnlyValue::Decimal("2.0".to_owned())
  );
  assert_eq!(
    policy.directive("unsized-media").unwrap().value(),
    &DocumentPolicyReportOnlyValue::Boolean(false)
  );
  assert_eq!(policy.directive("*").unwrap().report_to(), Some("default"));
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
}

#[test]
fn document_policy_report_only_combines_fields_in_wire_order() {
  let policy = DocumentPolicyReportOnly::parse_values([
    "oversized-images=2.0, unsized-media=?0",
    "*;report-to=default",
  ])
  .expect("combined Document-Policy-Report-Only fields should parse");

  assert_eq!(policy.len(), 3);
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
}

#[test]
fn document_policy_report_only_rejects_malformed_and_duplicate_values() {
  for value in [
    "",
    "   ",
    "oversized-images=()",
    "oversized-images=(1 2)",
    "oversized-images=\"2.0\"",
    "oversized-images=:MjA=:",
    "oversized-images=@123",
    "oversized-images=%\"2.0\"",
    "oversized-images=+2.0",
    "oversized-images=1.",
    "oversized-images=1.2345",
    "oversized-images=1;foo=bar",
    "oversized-images=1;report-to=5",
    "oversized-images=1;report-to=first;report-to=second",
    "Oversized-Images=2.0",
    "oversized-images=2.0, oversized-images=3.0",
    "oversized-images=2.0,, unsized-media=?0",
  ] {
    assert!(
      DocumentPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    DocumentPolicyReportOnly::parse_values(["oversized-images=2.0", "oversized-images=3.0"])
      .is_err(),
    "duplicate directive names across fields must be rejected"
  );
}

#[test]
fn document_policy_report_only_enforces_shared_size_and_member_bounds() {
  assert!(
    DocumentPolicyReportOnly::parse("x".repeat(MAX_DOCUMENT_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_limit = (0..MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = DocumentPolicyReportOnly::parse(&at_limit).expect("256 directives should parse");
  assert_eq!(parsed.len(), MAX_DOCUMENT_POLICY_DIRECTIVES);

  let too_many = (0..=MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    DocumentPolicyReportOnly::parse(&too_many).is_err(),
    "more than 256 directives must be rejected"
  );

  let first = format!("first={}", "a".repeat(40 * 1024));
  let second = format!("second={}", "b".repeat(40 * 1024));
  assert!(first.len() + second.len() > MAX_DOCUMENT_POLICY_TOTAL_BYTES);
  assert!(
    DocumentPolicyReportOnly::parse_values([first.as_str(), second.as_str()]).is_err(),
    "fields that exceed the cumulative bound together must be rejected"
  );
}
