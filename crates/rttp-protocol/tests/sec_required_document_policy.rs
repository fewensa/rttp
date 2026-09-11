use rttp_protocol::sec_required_document_policy::{
  SecRequiredDocumentPolicy, SecRequiredDocumentPolicyValue, MAX_DOCUMENT_POLICY_DIRECTIVES,
  MAX_DOCUMENT_POLICY_TOTAL_BYTES, MAX_DOCUMENT_POLICY_VALUE_BYTES,
};

#[test]
fn sec_required_document_policy_parses_dictionary_with_document_policy_model() {
  let policy = SecRequiredDocumentPolicy::parse(
    "oversized-images=2.0, unsized-media=?0, *;report-to=default",
  )
  .expect("Sec-Required-Document-Policy should parse");

  assert_eq!(policy.len(), 3);
  assert!(!policy.is_empty());
  assert_eq!(policy.directives()[0].name(), "oversized-images");
  assert_eq!(
    policy.directive("oversized-images").unwrap().value(),
    &SecRequiredDocumentPolicyValue::Decimal("2.0".to_owned())
  );
  assert_eq!(
    policy.directive("unsized-media").unwrap().value(),
    &SecRequiredDocumentPolicyValue::Boolean(false)
  );
  assert_eq!(policy.directive("*").unwrap().report_to(), Some("default"));
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
}

#[test]
fn sec_required_document_policy_combines_fields_in_wire_order() {
  let policy = SecRequiredDocumentPolicy::parse_values([
    "oversized-images=2.0, unsized-media=?0",
    "*;report-to=default",
  ])
  .expect("combined Sec-Required-Document-Policy fields should parse");

  assert_eq!(policy.len(), 3);
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
}

#[test]
fn sec_required_document_policy_rejects_malformed_and_duplicate_values() {
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
    "oversized-images=2.0\r\nX-Injected: 1",
    "oversized-images=2.0\u{1}",
  ] {
    assert!(
      SecRequiredDocumentPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    SecRequiredDocumentPolicy::parse_values(["oversized-images=2.0", "oversized-images=3.0"])
      .is_err(),
    "duplicate directive names across fields must be rejected"
  );
}

#[test]
fn sec_required_document_policy_enforces_shared_size_and_member_bounds() {
  assert!(
    SecRequiredDocumentPolicy::parse("x".repeat(MAX_DOCUMENT_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_limit = (0..MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed =
    SecRequiredDocumentPolicy::parse(&at_limit).expect("256 directives should parse");
  assert_eq!(parsed.len(), MAX_DOCUMENT_POLICY_DIRECTIVES);

  let too_many = (0..=MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    SecRequiredDocumentPolicy::parse(&too_many).is_err(),
    "more than 256 directives must be rejected"
  );

  let first = format!("first={}", "a".repeat(40 * 1024));
  let second = format!("second={}", "b".repeat(40 * 1024));
  assert!(first.len() + second.len() > MAX_DOCUMENT_POLICY_TOTAL_BYTES);
  assert!(
    SecRequiredDocumentPolicy::parse_values([first.as_str(), second.as_str()]).is_err(),
    "fields that exceed the cumulative bound together must be rejected"
  );
}
