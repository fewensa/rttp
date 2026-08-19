use rttp_protocol::strict_transport_security::{
  StrictTransportSecurity, MAX_STRICT_TRANSPORT_SECURITY_DIRECTIVES,
  MAX_STRICT_TRANSPORT_SECURITY_VALUE_BYTES,
};

#[test]
fn strict_transport_security_parses_max_age_and_flags() {
  let metadata = StrictTransportSecurity::parse("max-age=31536000; includeSubDomains; preload")
    .expect("HSTS flags should parse");

  assert_eq!(metadata.max_age(), 31536000);
  assert!(metadata.include_sub_domains());
  assert!(metadata.preload());
  assert_eq!(
    metadata.header_value(),
    "max-age=31536000; includeSubDomains; preload"
  );
}

#[test]
fn strict_transport_security_parses_max_age_only_and_zero() {
  let lifetime = StrictTransportSecurity::parse("max-age=60").expect("max-age should parse");
  assert_eq!(lifetime.max_age(), 60);
  assert!(!lifetime.include_sub_domains());
  assert!(!lifetime.preload());
  assert_eq!(lifetime.header_value(), "max-age=60");

  let zero = StrictTransportSecurity::parse("max-age=0").expect("max-age=0 should parse");
  assert_eq!(zero.max_age(), 0);
  assert_eq!(zero.header_value(), "max-age=0");

  let maximum = StrictTransportSecurity::parse(format!("max-age={}", u64::MAX))
    .expect("maximum u64 max-age should parse");
  assert_eq!(maximum.max_age(), u64::MAX);
}

#[test]
fn strict_transport_security_matches_directive_names_case_insensitively() {
  let metadata = StrictTransportSecurity::parse("MAX-AGE=10; includesubdomains; PreLoad")
    .expect("case-insensitive directive names should parse");

  assert_eq!(metadata.max_age(), 10);
  assert!(metadata.include_sub_domains());
  assert!(metadata.preload());
  assert_eq!(
    metadata.header_value(),
    "max-age=10; includeSubDomains; preload"
  );
}

#[test]
fn strict_transport_security_parses_quoted_max_age_and_canonicalizes_flag_order() {
  let metadata =
    StrictTransportSecurity::parse(r#"preload; max-age="31536000"; includeSubDomains"#)
      .expect("quoted max-age should parse");

  assert_eq!(metadata.max_age(), 31536000);
  assert!(metadata.include_sub_domains());
  assert!(metadata.preload());
  assert_eq!(
    metadata.header_value(),
    "max-age=31536000; includeSubDomains; preload"
  );

  let escaped =
    StrictTransportSecurity::parse(r#"max-age="31\536000""#).expect("quoted-pair max-age");
  assert_eq!(escaped.max_age(), 31536000);
}

#[test]
fn strict_transport_security_ignores_unknown_well_formed_directives() {
  let metadata = StrictTransportSecurity::parse("max-age=1; future=token; experimental")
    .expect("unknown well-formed directives should be ignored");

  assert_eq!(metadata.max_age(), 1);
  assert!(!metadata.include_sub_domains());
  assert!(!metadata.preload());
  assert_eq!(metadata.header_value(), "max-age=1");
}

#[test]
fn strict_transport_security_accepts_optional_whitespace_and_empty_slots() {
  for value in [
    "\tmax-age=15\t",
    " max-age = 15 ; includeSubDomains ",
    "max-age=15;;preload",
    ";max-age=15;",
    "max-age=15 ; ; includeSubDomains ;",
  ] {
    let metadata = StrictTransportSecurity::parse(value)
      .unwrap_or_else(|error| panic!("{value:?} should parse: {error}"));
    assert_eq!(metadata.max_age(), 15, "{value:?}");
  }
}

#[test]
fn strict_transport_security_rejects_duplicate_fields_after_bound_checks() {
  assert!(
    StrictTransportSecurity::parse_values(["max-age=1", "max-age=2"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    StrictTransportSecurity::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );

  let oversized = "a".repeat(MAX_STRICT_TRANSPORT_SECURITY_VALUE_BYTES + 1);
  assert!(
    StrictTransportSecurity::parse_values(["max-age=1", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}

#[test]
fn strict_transport_security_rejects_duplicate_directives_and_valued_flags() {
  for value in [
    "max-age=1; max-age=2",
    "max-age=1; includeSubDomains; includesubdomains",
    "max-age=1; preload; Preload",
    "max-age=1; future; FUTURE",
    "max-age=1; includeSubDomains=true",
    "max-age=1; preload=1",
    r#"max-age=1; includeSubDomains="""#,
  ] {
    assert!(
      StrictTransportSecurity::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn strict_transport_security_rejects_malformed_empty_and_oversized_values() {
  for value in [
    "",
    "   ",
    "includeSubDomains; preload",
    "max-age",
    "max-age=",
    "max-age=+60",
    "max-age=-60",
    "max-age=60.0",
    "max-age=one-year",
    "max-age=18446744073709551616",
    r#"max-age="unterminated"#,
    "max-age=60\r\nX: y",
    "max-age=60\u{7f}",
    "max-age=60, includeSubDomains",
  ] {
    assert!(
      StrictTransportSecurity::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    StrictTransportSecurity::parse("a".repeat(MAX_STRICT_TRANSPORT_SECURITY_VALUE_BYTES + 1))
      .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn strict_transport_security_enforces_directive_slot_boundary() {
  let mut value = String::from("max-age=0");
  for index in 0..MAX_STRICT_TRANSPORT_SECURITY_DIRECTIVES - 1 {
    value.push_str(&format!("; x{index}"));
  }
  let metadata =
    StrictTransportSecurity::parse(&value).expect("exact directive slot boundary should parse");
  assert_eq!(metadata.max_age(), 0);
  assert_eq!(metadata.header_value(), "max-age=0");

  value.push_str("; extra");
  assert!(
    StrictTransportSecurity::parse(&value).is_err(),
    "one slot beyond the directive boundary must be rejected"
  );
}
