use rttp_protocol::reporting_endpoints::{
  ReportingEndpoints, MAX_REPORTING_ENDPOINTS, MAX_REPORTING_ENDPOINTS_TOTAL_BYTES,
  MAX_REPORTING_ENDPOINTS_VALUE_BYTES,
};

#[test]
fn reporting_endpoints_parses_valid_multi_field_dictionaries() {
  let endpoints = ReportingEndpoints::parse_values([
    r#"default="https://reports.example/default""#,
    r#"csp="https://reports.example/csp""#,
  ])
  .expect("valid multi-field Reporting-Endpoints should parse");

  assert_eq!(
    vec![
      ("default", "https://reports.example/default"),
      ("csp", "https://reports.example/csp"),
    ],
    endpoints.endpoints()
  );
  assert_eq!(
    Some("https://reports.example/csp"),
    endpoints.endpoint("csp")
  );
  assert_eq!(
    r#"default="https://reports.example/default", csp="https://reports.example/csp""#,
    endpoints.header_value()
  );
}

#[test]
fn reporting_endpoints_unescapes_quoted_urls_and_round_trips() {
  let endpoints = ReportingEndpoints::parse(r#"default="https://reports.example/a\"b\\c""#)
    .expect("escaped Reporting-Endpoints URL should parse");

  assert_eq!(
    Some(r#"https://reports.example/a"b\c"#),
    endpoints.endpoint("default")
  );
  assert_eq!(
    r#"default="https://reports.example/a\"b\\c""#,
    endpoints.header_value()
  );

  let rebuilt =
    ReportingEndpoints::from_endpoints([("default", r#"https://reports.example/a"b\c"#)])
      .expect("escaped URL construction should succeed");
  assert_eq!(endpoints, rebuilt);
  assert_eq!(
    ReportingEndpoints::parse(rebuilt.header_value()).expect("formatted value should reparse"),
    rebuilt
  );
}

#[test]
fn reporting_endpoints_rejects_malformed_dictionaries_and_names() {
  for value in [
    "",
    " ",
    r#"default=https://reports.example/default"#,
    r#"Default="https://reports.example/default""#,
    r#"default="https://reports.example/default","#,
    r#"default="https://reports.example/unclosed"#,
    r#"default="https://reports.example/\q""#,
    r#"default="https://reports.example/\"#,
    "default=\"https://reports.example/\u{007f}\"",
  ] {
    assert!(
      ReportingEndpoints::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    ReportingEndpoints::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn reporting_endpoints_rejects_duplicate_names_across_fields() {
  assert!(
    ReportingEndpoints::parse(
      r#"default="https://reports.example/default", default="https://reports.example/other""#
    )
    .is_err(),
    "duplicate names in one field must be rejected"
  );
  assert!(
    ReportingEndpoints::parse_values([
      r#"default="https://reports.example/default""#,
      r#"default="https://reports.example/other""#,
    ])
    .is_err(),
    "duplicate names across fields must be rejected"
  );
}

#[test]
fn reporting_endpoints_enforces_value_total_and_member_bounds() {
  assert!(
    ReportingEndpoints::parse("x".repeat(MAX_REPORTING_ENDPOINTS_VALUE_BYTES + 1)).is_err(),
    "oversized field values must be rejected"
  );
  assert!(
    ReportingEndpoints::parse_values([
      r#"default="https://reports.example/default""#,
      "x".repeat(MAX_REPORTING_ENDPOINTS_VALUE_BYTES + 1).as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );

  let half = MAX_REPORTING_ENDPOINTS_TOTAL_BYTES / 2 + 1;
  let first = format!(r#"a="{}""#, "x".repeat(half));
  let second = format!(r#"b="{}""#, "x".repeat(half));
  assert!(
    first.len() <= MAX_REPORTING_ENDPOINTS_VALUE_BYTES,
    "each cumulative fixture field stays under the per-value limit"
  );
  assert!(
    first.len() + second.len() > MAX_REPORTING_ENDPOINTS_TOTAL_BYTES,
    "combined fixture fields must exceed the total-size limit"
  );
  assert!(
    ReportingEndpoints::parse_values([first.as_str(), second.as_str()]).is_err(),
    "cumulative oversized dictionaries must be rejected"
  );

  assert!(
    ReportingEndpoints::from_endpoints(
      (0..=MAX_REPORTING_ENDPOINTS)
        .map(|index| (format!("endpoint{index}"), "https://reports.example/")),
    )
    .is_err(),
    "excessive endpoint counts must be rejected"
  );
  assert!(
    ReportingEndpoints::from_endpoints(
      (0..MAX_REPORTING_ENDPOINTS)
        .map(|index| (format!("endpoint{index}"), "https://reports.example/")),
    )
    .is_ok(),
    "the canonical member-count limit must remain accepted"
  );
}
