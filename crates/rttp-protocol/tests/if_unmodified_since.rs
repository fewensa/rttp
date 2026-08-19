use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::if_unmodified_since::{IfUnmodifiedSince, MAX_IF_UNMODIFIED_SINCE_VALUE_BYTES};

#[test]
fn if_unmodified_since_parses_http_dates_and_round_trips_canonical_form() {
  let instant = UNIX_EPOCH + Duration::from_secs(784_111_777);

  for value in [
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sunday, 06-Nov-94 08:49:37 GMT",
    "Sun Nov  6 08:49:37 1994",
  ] {
    let parsed = IfUnmodifiedSince::parse(value).expect("HTTP-date should parse");
    assert_eq!(instant, parsed.datetime());
    assert_eq!(
      "Sun, 06 Nov 1994 08:49:37 GMT",
      parsed.header_value(),
      "header_value must emit canonical IMF-fixdate"
    );
  }
}

#[test]
fn if_unmodified_since_accepts_http_optional_whitespace_padding() {
  for value in [
    "\tSun, 06 Nov 1994 08:49:37 GMT\t",
    " Sun, 06 Nov 1994 08:49:37 GMT ",
    " \tSun, 06 Nov 1994 08:49:37 GMT\t ",
  ] {
    let parsed =
      IfUnmodifiedSince::parse(value).expect("OWS-padded If-Unmodified-Since should parse");
    assert_eq!("Sun, 06 Nov 1994 08:49:37 GMT", parsed.header_value());
  }
}

#[test]
fn if_unmodified_since_rejects_duplicate_and_invalid_values() {
  assert!(IfUnmodifiedSince::parse_values([
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT",
  ])
  .is_err());
  assert!(IfUnmodifiedSince::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "not-a-date",
    "08:49:37 06 Nov 1994",
    "Sun, 06 Nov 1994",
    "0\r\nX: y",
    "Sun, 06 Nov 1994 08:49:37 GMT\u{7f}",
  ] {
    assert!(
      IfUnmodifiedSince::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn if_unmodified_since_enforces_value_bounds() {
  let oversized =
    "Sun, 06 Nov 1994 08:49:37 GMT".repeat(MAX_IF_UNMODIFIED_SINCE_VALUE_BYTES / 29 + 1);
  assert!(
    IfUnmodifiedSince::parse(oversized).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn if_unmodified_since_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_IF_UNMODIFIED_SINCE_VALUE_BYTES + 1);

  assert!(
    IfUnmodifiedSince::parse_values(["Sun, 06 Nov 1994 08:49:37 GMT", oversized.as_str(),])
      .is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
