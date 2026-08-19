use rttp_protocol::document_policy::{
  DocumentPolicy, DocumentPolicyValue, MAX_DOCUMENT_POLICY_DIRECTIVES,
  MAX_DOCUMENT_POLICY_TOTAL_BYTES, MAX_DOCUMENT_POLICY_VALUE_BYTES,
};

#[test]
fn document_policy_parses_mixed_dictionary_and_reformats() {
  let policy =
    DocumentPolicy::parse("oversized-images=2.0, unsized-media=?0, sync-xhr=?0, js-profiling")
      .expect("WICG-style Document-Policy dictionary should parse");

  assert_eq!(policy.len(), 4);
  assert!(!policy.is_empty());
  assert_eq!(policy.directives()[0].name(), "oversized-images");
  assert_eq!(
    policy.directives()[0].value(),
    &DocumentPolicyValue::Decimal("2.0".to_owned())
  );
  assert_eq!(
    policy.directives()[1].value(),
    &DocumentPolicyValue::Boolean(false)
  );
  assert_eq!(
    policy.directives()[2].value(),
    &DocumentPolicyValue::Boolean(false)
  );
  assert_eq!(
    policy.directives()[3].value(),
    &DocumentPolicyValue::Boolean(true)
  );
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, sync-xhr=?0, js-profiling"
  );
}

#[test]
fn document_policy_parses_typed_values_and_canonicalizes_them() {
  let policy =
    DocumentPolicy::parse("oversized-images=2.00, lazy-load-image-count=5, unsized-media=state")
      .expect("typed values should parse");
  assert_eq!(
    policy.directive("oversized-images").unwrap().value(),
    &DocumentPolicyValue::Decimal("2.0".to_owned())
  );
  assert_eq!(
    policy.directive("lazy-load-image-count").unwrap().value(),
    &DocumentPolicyValue::Integer(5)
  );
  assert_eq!(
    policy.directive("unsized-media").unwrap().value(),
    &DocumentPolicyValue::Token("state".to_owned())
  );
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, lazy-load-image-count=5, unsized-media=state"
  );
}

#[test]
fn document_policy_retains_unknown_directive_names() {
  let policy = DocumentPolicy::parse("unknown-feature=2.0, xr-testing=?0")
    .expect("unknown well-formed directive names should be retained");
  assert_eq!(policy.directives()[0].name(), "unknown-feature");
  assert_eq!(policy.directives()[1].name(), "xr-testing");
  assert!(policy.directive("unknown-feature").is_some());
  assert!(policy.directive("UNKNOWN-FEATURE").is_none());
  assert_eq!(policy.header_value(), "unknown-feature=2.0, xr-testing=?0");
}

#[test]
fn document_policy_retains_star_default_reporting_member() {
  let policy = DocumentPolicy::parse("*;report-to=default")
    .expect("star default-reporting member should parse");
  let star = policy.directive("*").expect("star directive present");
  assert_eq!(star.value(), &DocumentPolicyValue::Boolean(true));
  assert_eq!(star.report_to(), Some("default"));
  assert_eq!(policy.header_value(), "*;report-to=default");
}

#[test]
fn document_policy_accepts_token_and_string_report_to() {
  let token = DocumentPolicy::parse("something=1.0;report-to=endpoint1")
    .expect("token report-to should parse");
  assert_eq!(
    token.directive("something").unwrap().report_to(),
    Some("endpoint1")
  );
  assert_eq!(token.header_value(), "something=1.0;report-to=endpoint1");

  let quoted = DocumentPolicy::parse("something=1.0;report-to=\"endpoint1\"")
    .expect("string report-to should parse");
  assert_eq!(
    quoted.directive("something").unwrap().report_to(),
    Some("endpoint1")
  );
  assert_eq!(
    quoted.header_value(),
    "something=1.0;report-to=\"endpoint1\""
  );

  let none = DocumentPolicy::parse("oversized-images=2.0;report-to=none")
    .expect("none report-to should parse");
  assert_eq!(
    none.directive("oversized-images").unwrap().report_to(),
    Some("none")
  );

  let quoted_semicolon = DocumentPolicy::parse(r#"oversized-images=1;report-to="first;second""#)
    .expect("quoted report-to may contain a semicolon");
  assert_eq!(
    quoted_semicolon
      .directive("oversized-images")
      .unwrap()
      .report_to(),
    Some("first;second")
  );

  let per_directive =
    DocumentPolicy::parse("oversized-images=1;report-to=first, unsized-media=?0;report-to=second")
      .expect("the same parameter name on different directives should parse");
  assert_eq!(
    per_directive
      .directive("oversized-images")
      .unwrap()
      .report_to(),
    Some("first")
  );
  assert_eq!(
    per_directive
      .directive("unsized-media")
      .unwrap()
      .report_to(),
    Some("second")
  );
}

#[test]
fn document_policy_combines_fields_in_wire_order() {
  let policy = DocumentPolicy::parse_values([
    "oversized-images=2.0, unsized-media=?0",
    "*;report-to=default",
  ])
  .expect("combined Document-Policy fields should parse");

  assert_eq!(policy.len(), 3);
  assert_eq!(policy.directives()[0].name(), "oversized-images");
  assert_eq!(policy.directives()[1].name(), "unsized-media");
  assert_eq!(policy.directives()[2].name(), "*");
  assert_eq!(
    policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
}

#[test]
fn document_policy_rejects_invalid_members() {
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
    "oversized-images=1;report-to=?0",
    "oversized-images=1;report-to=:YWJj:",
    "oversized-images=1;report-to=\"a\";foo=1",
    "oversized-images=1;report-to=first;report-to=second",
    "oversized-images=1;report-to=\"first\";report-to=\"second\"",
    "*;report-to=first;report-to=second",
    "Oversized-Images=2.0",
    "oversized-images=2.0, oversized-images=3.0",
    "oversized-images=2.0,, unsized-media=?0",
    "oversized-images=2.0 unsized-media=?0",
  ] {
    assert!(
      DocumentPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn document_policy_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    DocumentPolicy::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    DocumentPolicy::parse_values(["oversized-images=2.0", "oversized-images=3.0"]).is_err(),
    "duplicate directive names across fields must be rejected"
  );
}

#[test]
fn document_policy_enforces_value_directive_and_size_bounds() {
  assert!(
    DocumentPolicy::parse("x".repeat(MAX_DOCUMENT_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_DOCUMENT_POLICY_VALUE_BYTES + 1);
  assert!(
    DocumentPolicy::parse_values(["oversized-images=2.0", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = DocumentPolicy::parse(&at_limit).expect("256 directives should parse");
  assert_eq!(parsed.len(), MAX_DOCUMENT_POLICY_DIRECTIVES);

  let too_many = (0..=MAX_DOCUMENT_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    DocumentPolicy::parse(&too_many).is_err(),
    "more than 256 directives must be rejected"
  );

  let first = format!("first={}", "a".repeat(40 * 1024));
  let second = format!("second={}", "b".repeat(40 * 1024));
  assert!(
    first.len() <= MAX_DOCUMENT_POLICY_VALUE_BYTES
      && second.len() <= MAX_DOCUMENT_POLICY_VALUE_BYTES,
    "fixture fields must fit per-field bounds"
  );
  assert!(
    first.len() + second.len() > MAX_DOCUMENT_POLICY_TOTAL_BYTES,
    "fixture fields must exceed the cumulative bound together"
  );
  assert!(
    DocumentPolicy::parse_values([first.as_str(), second.as_str()]).is_err(),
    "fields that fit individually but exceed the cumulative bound together must be rejected"
  );
}
