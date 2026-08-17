use rttp_protocol::cross_origin_opener_policy::{
  CrossOriginOpenerPolicy, MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES,
};

#[test]
fn cross_origin_opener_policy_parses_standard_directives() {
  assert_eq!(
    CrossOriginOpenerPolicy::UnsafeNone,
    CrossOriginOpenerPolicy::parse("unsafe-none").expect("unsafe-none should parse")
  );
  assert_eq!(
    CrossOriginOpenerPolicy::SameOriginAllowPopups,
    CrossOriginOpenerPolicy::parse("same-origin-allow-popups")
      .expect("same-origin-allow-popups should parse")
  );
  assert_eq!(
    CrossOriginOpenerPolicy::SameOrigin,
    CrossOriginOpenerPolicy::parse("same-origin").expect("same-origin should parse")
  );
  assert_eq!(
    CrossOriginOpenerPolicy::SameOriginPlusCoep,
    CrossOriginOpenerPolicy::parse("same-origin-plus-coep")
      .expect("same-origin-plus-coep should parse")
  );
  assert_eq!(
    "unsafe-none",
    CrossOriginOpenerPolicy::UnsafeNone.header_value()
  );
  assert_eq!(
    "same-origin-allow-popups",
    CrossOriginOpenerPolicy::SameOriginAllowPopups.header_value()
  );
  assert_eq!(
    "same-origin",
    CrossOriginOpenerPolicy::SameOrigin.header_value()
  );
  assert_eq!(
    "same-origin-plus-coep",
    CrossOriginOpenerPolicy::SameOriginPlusCoep.header_value()
  );
}

#[test]
fn cross_origin_opener_policy_accepts_http_optional_whitespace_padding() {
  for value in [
    "\tsame-origin\t",
    " \tsame-origin\t ",
    "same-origin\t",
    "\tsame-origin",
  ] {
    assert_eq!(
      CrossOriginOpenerPolicy::SameOrigin,
      CrossOriginOpenerPolicy::parse(value).expect("OWS-padded same-origin should parse")
    );
  }
}

#[test]
fn cross_origin_opener_policy_accepts_well_formed_parameters_as_syntax() {
  assert_eq!(
    CrossOriginOpenerPolicy::SameOriginPlusCoep,
    CrossOriginOpenerPolicy::parse(r#"same-origin-plus-coep; report-to="coop""#)
      .expect("parameterized same-origin-plus-coep should parse")
  );
  assert_eq!(
    CrossOriginOpenerPolicy::SameOrigin,
    CrossOriginOpenerPolicy::parse("same-origin; foo=1")
      .expect("unknown well-formed parameters should parse as syntax")
  );
  assert_eq!(
    "same-origin-plus-coep",
    CrossOriginOpenerPolicy::parse(r#"same-origin-plus-coep; report-to="coop""#)
      .expect("parameterized same-origin-plus-coep should parse")
      .header_value()
  );
}

#[test]
fn cross_origin_opener_policy_rejects_case_variants() {
  for value in ["SAME-ORIGIN", "Same-Origin-Allow-Popups", "Unsafe-None"] {
    assert!(
      CrossOriginOpenerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn cross_origin_opener_policy_rejects_empty_duplicate_malformed_and_oversized_values() {
  for value in [
    "",
    "   ",
    "unknown",
    "same-origin, same-origin-allow-popups",
    "\"same-origin\"",
    "same-origin\r\nX: y",
    "same-origin\u{7f}",
  ] {
    assert!(
      CrossOriginOpenerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    CrossOriginOpenerPolicy::parse_values(["same-origin", "same-origin-allow-popups"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    CrossOriginOpenerPolicy::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    CrossOriginOpenerPolicy::parse("a".repeat(MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES + 1))
      .is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn cross_origin_opener_policy_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES + 1);

  assert!(
    CrossOriginOpenerPolicy::parse_values(["same-origin", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
