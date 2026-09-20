use rttp_protocol::permissions_policy_report_only::{
  PermissionsPolicyReportOnly, PermissionsPolicyReportOnlyAllowlist,
  PermissionsPolicyReportOnlyAllowlistMember, MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS,
  MAX_PERMISSIONS_POLICY_DIRECTIVES, MAX_PERMISSIONS_POLICY_VALUE_BYTES,
};

#[test]
fn permissions_policy_report_only_parses_dictionary_with_permissions_policy_model() {
  let policy = PermissionsPolicyReportOnly::parse(
    r#"geolocation=(self "https://maps.example.test"), camera=()"#,
  )
  .expect("Permissions-Policy-Report-Only should parse");

  assert_eq!(policy.len(), 2);
  assert!(!policy.is_empty());
  assert_eq!(policy.directives()[0].feature(), "geolocation");
  assert_eq!(
    policy.directives()[0].allowlist(),
    &PermissionsPolicyReportOnlyAllowlist::Members(vec![
      PermissionsPolicyReportOnlyAllowlistMember::SelfToken,
      PermissionsPolicyReportOnlyAllowlistMember::Origin("https://maps.example.test".to_owned()),
    ])
  );
  assert_eq!(policy.directives()[1].feature(), "camera");
  assert!(policy.directives()[1].allowlist().is_empty());
  assert_eq!(
    policy.header_value(),
    r#"geolocation=(self "https://maps.example.test"), camera=()"#
  );
}

#[test]
fn permissions_policy_report_only_combines_fields_in_wire_order() {
  let policy =
    PermissionsPolicyReportOnly::parse_values(["geolocation=(self)", "camera=(), fullscreen=*"])
      .expect("combined Permissions-Policy-Report-Only fields should parse");

  assert_eq!(policy.len(), 3);
  assert_eq!(policy.directives()[0].feature(), "geolocation");
  assert_eq!(policy.directives()[1].feature(), "camera");
  assert_eq!(policy.directives()[2].feature(), "fullscreen");
  assert_eq!(
    policy.header_value(),
    "geolocation=self, camera=(), fullscreen=*"
  );
}

#[test]
fn permissions_policy_report_only_rejects_malformed_and_duplicate_values() {
  for value in [
    "",
    "   ",
    "geolocation",
    "geolocation=src",
    r#"geolocation="'none'""#,
    "geolocation=(*)",
    r#"geolocation=(* "https://example.test")"#,
    "geolocation=self;foo=bar",
    "geolocation=self, geolocation=()",
    "geolocation=(self self)",
  ] {
    assert!(
      PermissionsPolicyReportOnly::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    PermissionsPolicyReportOnly::parse_values(["geolocation=self", "geolocation=()"]).is_err(),
    "duplicate feature keys across fields must be rejected"
  );
}

#[test]
fn permissions_policy_report_only_enforces_shared_size_and_member_bounds() {
  assert!(
    PermissionsPolicyReportOnly::parse("x".repeat(MAX_PERMISSIONS_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_PERMISSIONS_POLICY_VALUE_BYTES + 1);
  assert!(
    PermissionsPolicyReportOnly::parse_values(["geolocation=self", oversized_duplicate.as_str()])
      .is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_PERMISSIONS_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=self"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = PermissionsPolicyReportOnly::parse(&at_limit).expect("256 directives should parse");
  assert_eq!(parsed.len(), MAX_PERMISSIONS_POLICY_DIRECTIVES);

  let too_many = (0..=MAX_PERMISSIONS_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=self"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    PermissionsPolicyReportOnly::parse(&too_many).is_err(),
    "more than 256 directives must be rejected"
  );

  let members_at_limit = (0..MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS)
    .map(|index| format!("\"https://origin{index}.example.test\""))
    .collect::<Vec<_>>()
    .join(" ");
  let parsed_members =
    PermissionsPolicyReportOnly::parse(format!("geolocation=({members_at_limit})"))
      .expect("256 allowlist members should parse");
  assert_eq!(
    parsed_members
      .directive("geolocation")
      .unwrap()
      .allowlist()
      .members()
      .len(),
    MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS
  );

  let members_too_many = (0..=MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS)
    .map(|index| format!("\"https://origin{index}.example.test\""))
    .collect::<Vec<_>>()
    .join(" ");
  assert!(
    PermissionsPolicyReportOnly::parse(format!("geolocation=({members_too_many})")).is_err(),
    "more than 256 allowlist members must be rejected"
  );
}
