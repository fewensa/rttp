use rttp_protocol::permissions_policy::{
  PermissionsPolicy, PermissionsPolicyAllowlist, PermissionsPolicyAllowlistMember,
  MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS, MAX_PERMISSIONS_POLICY_DIRECTIVES,
  MAX_PERMISSIONS_POLICY_VALUE_BYTES,
};

#[test]
fn permissions_policy_parses_rfc_example_and_reformats() {
  let policy =
    PermissionsPolicy::parse(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
      .expect("W3C Permissions-Policy example should parse");

  assert_eq!(policy.len(), 2);
  assert!(!policy.is_empty());
  assert_eq!(policy.directives()[0].feature(), "geolocation");
  assert_eq!(
    policy.directives()[0].allowlist(),
    &PermissionsPolicyAllowlist::Members(vec![
      PermissionsPolicyAllowlistMember::SelfToken,
      PermissionsPolicyAllowlistMember::Origin("https://maps.example.test".to_owned()),
    ])
  );
  assert_eq!(policy.directives()[1].feature(), "camera");
  assert!(policy.directives()[1].allowlist().is_empty());
  assert_eq!(policy.directives()[1].allowlist().members(), []);
  assert_eq!(
    policy.header_value(),
    r#"geolocation=(self "https://maps.example.test"), camera=()"#
  );
}

#[test]
fn permissions_policy_parses_special_token_allowlists() {
  let wildcard = PermissionsPolicy::parse("fullscreen=*").expect("fullscreen=* should parse");
  assert!(wildcard
    .directive("fullscreen")
    .unwrap()
    .allowlist()
    .is_all_origins());
  assert_eq!(wildcard.header_value(), "fullscreen=*");

  let self_token =
    PermissionsPolicy::parse("microphone=self").expect("microphone=self should parse");
  let allowlist = self_token.directive("microphone").unwrap().allowlist();
  assert!(!allowlist.is_all_origins());
  assert_eq!(
    allowlist.members(),
    [PermissionsPolicyAllowlistMember::SelfToken]
  );
  assert!(allowlist.members()[0].is_self());
  assert_eq!(self_token.header_value(), "microphone=self");
}

#[test]
fn permissions_policy_parses_origin_allowlists_and_canonicalizes_them() {
  let single_origin = PermissionsPolicy::parse(r#"geolocation="https://example.test""#)
    .expect("bare quoted origin should parse");
  assert_eq!(
    single_origin
      .directive("geolocation")
      .unwrap()
      .allowlist()
      .members(),
    [PermissionsPolicyAllowlistMember::Origin(
      "https://example.test".to_owned()
    )]
  );
  assert_eq!(
    single_origin.header_value(),
    r#"geolocation=("https://example.test")"#
  );

  let with_port = PermissionsPolicy::parse(r#"geolocation="https://example.test:8443""#)
    .expect("non-default port origin should parse");
  assert_eq!(
    with_port
      .directive("geolocation")
      .unwrap()
      .allowlist()
      .members()[0]
      .origin(),
    Some("https://example.test:8443")
  );

  let multiple =
    PermissionsPolicy::parse(r#"geolocation=("https://a.example.test" "https://b.example.test")"#)
      .expect("inner list of origins should parse");
  assert_eq!(
    multiple
      .directive("geolocation")
      .unwrap()
      .allowlist()
      .members()
      .len(),
    2
  );
  assert_eq!(
    multiple.header_value(),
    r#"geolocation=("https://a.example.test" "https://b.example.test")"#
  );
}

#[test]
fn permissions_policy_retains_unknown_feature_tokens() {
  let policy = PermissionsPolicy::parse("unknown-feature=self, xr-spatial-tracking=*")
    .expect("unknown well-formed feature tokens should be retained");
  assert_eq!(policy.directives()[0].feature(), "unknown-feature");
  assert_eq!(policy.directives()[1].feature(), "xr-spatial-tracking");
  assert!(policy.directive("unknown-feature").is_some());
  assert!(policy.directive("UNKNOWN-FEATURE").is_none());
  assert_eq!(
    policy.header_value(),
    "unknown-feature=self, xr-spatial-tracking=*"
  );
}

#[test]
fn permissions_policy_accepts_and_drops_report_to_parameters() {
  let policy = PermissionsPolicy::parse(
    r#"payment=();report-to="payments", geolocation=self;report-to="geo""#,
  )
  .expect("report-to parameters should be accepted as syntax");
  assert!(policy.directive("payment").unwrap().allowlist().is_empty());
  assert_eq!(policy.header_value(), r#"payment=(), geolocation=self"#);
}

#[test]
fn permissions_policy_combines_fields_in_wire_order() {
  let policy = PermissionsPolicy::parse_values(["geolocation=(self)", "camera=(), fullscreen=*"])
    .expect("combined Permissions-Policy fields should parse");

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
fn permissions_policy_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    "geolocation",
    "geolocation=src",
    r#"geolocation="'none'""#,
    r#"geolocation=("'none'")"#,
    "geolocation=(*)",
    r#"geolocation=(* "https://example.test")"#,
    "geolocation=?1",
    "geolocation=1",
    "geolocation=1.5",
    "geolocation=:YWJj:",
    "geolocation=@123",
    "geolocation=https://example.test",
    r#"geolocation="https://example.test/path""#,
    r#"geolocation="null""#,
    r#"geolocation="ftp://example.test""#,
    r#"geolocation="https://example.test:443""#,
    "geolocation=self;foo=bar",
    "geolocation=self;report-to=5",
    r#"geolocation=(self);report-to="x";foo=1"#,
    "geolocation 'self' https://example.test",
    "geolocation=() camera=()",
    "Geolocation=self",
    "geolocation=(self src)",
    "geolocation=(self self)",
    r#"geolocation=("https://example.test" "https://example.test")"#,
    "geolocation=self, geolocation=()",
  ] {
    assert!(
      PermissionsPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn permissions_policy_rejects_empty_field_sets_and_cross_field_duplicates() {
  assert!(
    PermissionsPolicy::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    PermissionsPolicy::parse_values(["geolocation=self", "geolocation=()"]).is_err(),
    "duplicate feature keys across fields must be rejected"
  );
}

#[test]
fn permissions_policy_enforces_value_directive_and_member_bounds() {
  assert!(
    PermissionsPolicy::parse("x".repeat(MAX_PERMISSIONS_POLICY_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_PERMISSIONS_POLICY_VALUE_BYTES + 1);
  assert!(
    PermissionsPolicy::parse_values(["geolocation=self", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_PERMISSIONS_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=self"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = PermissionsPolicy::parse(&at_limit).expect("256 directives should parse");
  assert_eq!(parsed.len(), MAX_PERMISSIONS_POLICY_DIRECTIVES);

  let too_many = (0..=MAX_PERMISSIONS_POLICY_DIRECTIVES)
    .map(|index| format!("feature{index}=self"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    PermissionsPolicy::parse(&too_many).is_err(),
    "more than 256 directives must be rejected"
  );

  let members_at_limit = (0..MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS)
    .map(|index| format!("\"https://origin{index}.example.test\""))
    .collect::<Vec<_>>()
    .join(" ");
  let parsed_members = PermissionsPolicy::parse(format!("geolocation=({members_at_limit})"))
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
    PermissionsPolicy::parse(format!("geolocation=({members_too_many})")).is_err(),
    "more than 256 allowlist members must be rejected"
  );
}
