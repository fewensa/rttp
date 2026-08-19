use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::retry_after::{RetryAfter, MAX_RETRY_AFTER_VALUE_BYTES};

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const RFC_850_DATE: &str = "Sunday, 06-Nov-94 08:49:37 GMT";
const U64_MAX: &str = "18446744073709551615";
const RETRY_AT: Duration = Duration::from_secs(784_111_777);

#[test]
fn retry_after_parses_delta_seconds() {
  for (value, expected) in [("0", 0), ("120", 120), (U64_MAX, u64::MAX)] {
    let retry_after = RetryAfter::parse(value).expect("delta-seconds should parse");

    assert_eq!(Some(expected), retry_after.delta_seconds());
    assert_eq!(None, retry_after.http_date());
    assert_eq!(value, retry_after.header_value());
  }
}

#[test]
fn retry_after_parses_http_dates() {
  for value in [IMF_FIXDATE, RFC_850_DATE] {
    let retry_after = RetryAfter::parse(value).expect("HTTP-date should parse");

    assert_eq!(None, retry_after.delta_seconds());
    assert_eq!(Some(UNIX_EPOCH + RETRY_AT), retry_after.http_date());
    assert_eq!(IMF_FIXDATE, retry_after.header_value());
  }
}

#[test]
fn retry_after_accepts_http_optional_whitespace_padding() {
  for value in ["\t120\t", " 120 ", " \t120\t "] {
    assert_eq!(
      RetryAfter::DeltaSeconds(120),
      RetryAfter::parse(value).expect("OWS-padded delta-seconds should parse")
    );
  }

  let padded_date = format!(" \t{IMF_FIXDATE}\t ");
  assert_eq!(
    RetryAfter::HttpDate(UNIX_EPOCH + RETRY_AT),
    RetryAfter::parse(padded_date).expect("OWS-padded HTTP-date should parse")
  );
}

#[test]
fn retry_after_rejects_empty_malformed_control_and_duplicate_values() {
  assert!(RetryAfter::parse_values([]).is_err());
  assert!(RetryAfter::parse_values(["60", "120"]).is_err());

  for value in [
    "",
    " ",
    "-1",
    "+1",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    "18446744073709551616",
    "Sun, 06 Nov 1994 08:49:37 PST",
    "Sun, 06 Nov 1994 08:49:37 GMT, Mon, 07 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT\r\nX: y",
    "Sun, 06 Nov 1994 08:49:37 GMT\u{7f}",
  ] {
    assert!(
      RetryAfter::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn retry_after_enforces_value_bounds() {
  let exact_bound = "0".repeat(MAX_RETRY_AFTER_VALUE_BYTES);
  let oversized = "1".repeat(MAX_RETRY_AFTER_VALUE_BYTES + 1);

  assert_eq!(
    RetryAfter::DeltaSeconds(0),
    RetryAfter::parse(exact_bound).expect("exactly bounded value should parse")
  );
  assert!(RetryAfter::parse(oversized).is_err());
}

#[test]
fn retry_after_checks_duplicate_values_against_its_bound() {
  let oversized = "1".repeat(MAX_RETRY_AFTER_VALUE_BYTES + 1);

  assert!(
    RetryAfter::parse_values(["60", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
