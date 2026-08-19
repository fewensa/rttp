use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::deprecation::{Deprecation, MAX_DEPRECATION_VALUE_BYTES};

#[test]
fn deprecation_parses_structured_boolean_items() {
  let deprecated = Deprecation::parse("?1").expect("true boolean should parse");
  assert_eq!(deprecated, Deprecation::Boolean(true));
  assert_eq!(deprecated.boolean(), Some(true));
  assert_eq!(deprecated.date(), None);
  assert_eq!(deprecated.header_value(), "?1");

  let not_deprecated = Deprecation::parse("?0").expect("false boolean should parse");
  assert_eq!(not_deprecated, Deprecation::Boolean(false));
  assert_eq!(not_deprecated.boolean(), Some(false));
  assert_eq!(not_deprecated.header_value(), "?0");
}

#[test]
fn deprecation_parses_structured_date_items() {
  let instant = UNIX_EPOCH + Duration::from_secs(1_688_169_599);
  let deprecation = Deprecation::parse("@1688169599").expect("date item should parse");
  assert_eq!(deprecation, Deprecation::Date(instant));
  assert_eq!(deprecation.date(), Some(instant));
  assert_eq!(deprecation.boolean(), None);
  assert_eq!(deprecation.header_value(), "@1688169599");

  let epoch = Deprecation::parse("@0").expect("epoch date should parse");
  assert_eq!(epoch, Deprecation::Date(UNIX_EPOCH));
  assert_eq!(epoch.header_value(), "@0");
}

#[test]
fn deprecation_parses_negative_date_when_system_time_can_represent_it() {
  let before_epoch = match UNIX_EPOCH.checked_sub(Duration::from_secs(1)) {
    Some(time) => time,
    None => return,
  };
  let deprecation =
    Deprecation::parse("@-1").expect("negative date should parse when representable");
  assert_eq!(deprecation, Deprecation::Date(before_epoch));
  assert_eq!(deprecation.header_value(), "@-1");
}

#[test]
fn deprecation_accepts_http_optional_whitespace_padding() {
  for value in ["\t?1\t", " ?1 ", " \t?1\t ", "?1\t", "\t?1"] {
    let deprecation = Deprecation::parse(value).expect("OWS-padded boolean should parse");
    assert_eq!(deprecation, Deprecation::Boolean(true));
    assert_eq!(deprecation.header_value(), "?1");
  }
}

#[test]
fn deprecation_rejects_historical_and_invalid_grammar() {
  assert!(Deprecation::parse_values(["?1", "?0"]).is_err());
  assert!(Deprecation::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "true",
    "false",
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "1688169599",
    "@1688169599.0",
    "?1, ?1",
    "?1;foo=?1",
    "?2",
    "(?1)",
    "\"?1\"",
    ":YWJj:",
    "%\"deprecated\"",
    "1.0",
    "?1\r\nX: y",
    "?1\u{7f}",
    "?1\0",
  ] {
    assert!(
      Deprecation::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn deprecation_enforces_value_bounds() {
  assert!(Deprecation::parse("?".to_owned() + &"1".repeat(MAX_DEPRECATION_VALUE_BYTES)).is_err());
}

#[test]
fn deprecation_checks_duplicate_values_against_its_bound() {
  let oversized = "1".repeat(MAX_DEPRECATION_VALUE_BYTES + 1);

  assert!(
    Deprecation::parse_values(["?1", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
