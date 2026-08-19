use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::if_modified_since::{IfModifiedSince, MAX_IF_MODIFIED_SINCE_VALUE_BYTES};

#[test]
fn if_modified_since_parses_http_dates_and_round_trips_canonical_form() {
  let instant = UNIX_EPOCH + Duration::from_secs(784_111_777);

  for value in [
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sunday, 06-Nov-94 08:49:37 GMT",
    "Sun Nov  6 08:49:37 1994",
  ] {
    let parsed = IfModifiedSince::parse(value).expect("HTTP-date should parse");
    assert_eq!(instant, parsed.datetime());
    assert_eq!(
      "Sun, 06 Nov 1994 08:49:37 GMT",
      parsed.header_value(),
      "header_value must emit canonical IMF-fixdate"
    );
  }
}

#[test]
fn if_modified_since_accepts_http_optional_whitespace_padding() {
  for value in [
    "\tSun, 06 Nov 1994 08:49:37 GMT\t",
    " Sun, 06 Nov 1994 08:49:37 GMT ",
    " \tSun, 06 Nov 1994 08:49:37 GMT\t ",
  ] {
    let parsed = IfModifiedSince::parse(value).expect("OWS-padded If-Modified-Since should parse");
    assert_eq!("Sun, 06 Nov 1994 08:49:37 GMT", parsed.header_value());
  }
}

#[test]
fn if_modified_since_rejects_duplicate_and_invalid_values() {
  assert!(IfModifiedSince::parse_values([
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT",
  ])
  .is_err());
  assert!(IfModifiedSince::parse_values([]).is_err());

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
      IfModifiedSince::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn if_modified_since_enforces_value_bounds() {
  let oversized =
    "Sun, 06 Nov 1994 08:49:37 GMT".repeat(MAX_IF_MODIFIED_SINCE_VALUE_BYTES / 29 + 1);
  assert!(
    IfModifiedSince::parse(oversized).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn if_modified_since_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_IF_MODIFIED_SINCE_VALUE_BYTES + 1);

  assert!(
    IfModifiedSince::parse_values(["Sun, 06 Nov 1994 08:49:37 GMT", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
