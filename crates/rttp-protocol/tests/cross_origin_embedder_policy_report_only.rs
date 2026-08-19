use rttp_protocol::cross_origin_embedder_policy_report_only::{
  CrossOriginEmbedderPolicyReportOnly, MAX_CROSS_ORIGIN_EMBEDDER_POLICY_REPORT_ONLY_VALUE_BYTES,
};

#[test]
fn cross_origin_embedder_policy_report_only_parses_standard_directives() {
  assert_eq!(
    CrossOriginEmbedderPolicyReportOnly::UnsafeNone,
    CrossOriginEmbedderPolicyReportOnly::parse("unsafe-none").expect("unsafe-none should parse")
  );
  assert_eq!(
    CrossOriginEmbedderPolicyReportOnly::RequireCorp,
    CrossOriginEmbedderPolicyReportOnly::parse("require-corp").expect("require-corp should parse")
  );
  assert_eq!(
    CrossOriginEmbedderPolicyReportOnly::Credentialless,
    CrossOriginEmbedderPolicyReportOnly::parse("credentialless")
      .expect("credentialless should parse")
  );
  assert_eq!(
    "unsafe-none",
    CrossOriginEmbedderPolicyReportOnly::UnsafeNone.header_value()
  );
  assert_eq!(
    "require-corp",
    CrossOriginEmbedderPolicyReportOnly::RequireCorp.header_value()
  );
  assert_eq!(
    "credentialless",
    CrossOriginEmbedderPolicyReportOnly::Credentialless.header_value()
  );
}

#[test]
fn cross_origin_embedder_policy_report_only_accepts_http_optional_whitespace_padding() {
  for value in [
    "\trequire-corp\t",
    " \trequire-corp\t ",
    "require-corp\t",
    "\trequire-corp",
  ] {
    assert_eq!(
      CrossOriginEmbedderPolicyReportOnly::RequireCorp,
      CrossOriginEmbedderPolicyReportOnly::parse(value)
        .expect("OWS-padded require-corp should parse")
    );
  }
}

#[test]
fn cross_origin_embedder_policy_report_only_accepts_well_formed_parameters_as_syntax() {
  assert_eq!(
    CrossOriginEmbedderPolicyReportOnly::RequireCorp,
    CrossOriginEmbedderPolicyReportOnly::parse(r#"require-corp; report-to="coep""#)
      .expect("parameterized require-corp should parse")
  );
  assert_eq!(
    CrossOriginEmbedderPolicyReportOnly::Credentialless,
    CrossOriginEmbedderPolicyReportOnly::parse("credentialless; foo=1")
      .expect("unknown well-formed parameters should parse as syntax")
  );
  assert_eq!(
    "require-corp",
    CrossOriginEmbedderPolicyReportOnly::parse(r#"require-corp; report-to="coep""#)
      .expect("parameterized require-corp should parse")
      .header_value()
  );
}

#[test]
fn cross_origin_embedder_policy_report_only_rejects_case_variants() {
  for value in ["REQUIRE-CORP", "Credentialless", "Unsafe-None"] {
    assert!(
      CrossOriginEmbedderPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn cross_origin_embedder_policy_report_only_rejects_empty_duplicate_malformed_and_oversized_values()
{
  for value in [
    "",
    "   ",
    "unknown",
    "require-corp, credentialless",
    "\"require-corp\"",
    "require-corp\r\nX: y",
    "require-corp\u{7f}",
  ] {
    assert!(
      CrossOriginEmbedderPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    CrossOriginEmbedderPolicyReportOnly::parse_values(["require-corp", "credentialless"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    CrossOriginEmbedderPolicyReportOnly::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    CrossOriginEmbedderPolicyReportOnly::parse(
      "a".repeat(MAX_CROSS_ORIGIN_EMBEDDER_POLICY_REPORT_ONLY_VALUE_BYTES + 1)
    )
    .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn cross_origin_embedder_policy_report_only_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_CROSS_ORIGIN_EMBEDDER_POLICY_REPORT_ONLY_VALUE_BYTES + 1);

  assert!(
    CrossOriginEmbedderPolicyReportOnly::parse_values(["require-corp", oversized.as_str()])
      .is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
