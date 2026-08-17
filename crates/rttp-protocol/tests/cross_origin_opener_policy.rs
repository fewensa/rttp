use rttp_protocol::cross_origin_opener_policy::{
  CrossOriginOpenerPolicy, MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES,
};

#[test]
fn cross_origin_opener_policy_parses_standard_directives_case_insensitively() {
  for (value, expected) in [
    ("unsafe-none", CrossOriginOpenerPolicy::UnsafeNone),
    ("same-origin", CrossOriginOpenerPolicy::SameOrigin),
    (
      "same-origin-allow-popups",
      CrossOriginOpenerPolicy::SameOriginAllowPopups,
    ),
    (
      "noopener-allow-popups",
      CrossOriginOpenerPolicy::NoopenerAllowPopups,
    ),
  ] {
    assert_eq!(
      expected,
      CrossOriginOpenerPolicy::parse(value).expect("value should parse")
    );
  }

  for (value, expected) in [
    ("UNSAFE-NONE", CrossOriginOpenerPolicy::UnsafeNone),
    ("Same-Origin", CrossOriginOpenerPolicy::SameOrigin),
    (
      "SAME-ORIGIN-ALLOW-POPUPS",
      CrossOriginOpenerPolicy::SameOriginAllowPopups,
    ),
    (
      "Noopener-Allow-Popups",
      CrossOriginOpenerPolicy::NoopenerAllowPopups,
    ),
  ] {
    assert_eq!(
      expected,
      CrossOriginOpenerPolicy::parse(value).expect("mixed-case value should parse")
    );
  }

  assert_eq!(
    "unsafe-none",
    CrossOriginOpenerPolicy::UnsafeNone.header_value()
  );
  assert_eq!(
    "same-origin",
    CrossOriginOpenerPolicy::SameOrigin.header_value()
  );
  assert_eq!(
    "same-origin-allow-popups",
    CrossOriginOpenerPolicy::SameOriginAllowPopups.header_value()
  );
  assert_eq!(
    "noopener-allow-popups",
    CrossOriginOpenerPolicy::NoopenerAllowPopups.header_value()
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
fn cross_origin_opener_policy_rejects_empty_duplicate_malformed_and_ambiguous_values() {
  for value in [
    "",
    "   ",
    "same origin",
    "same-origin-plus-coep",
    "cross-origin",
    "same-origin, same-origin",
    "same-origin; report-to=endpoint",
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
    CrossOriginOpenerPolicy::parse_values(["same-origin", "same-origin"]).is_err(),
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
