use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::retry_after::{RetryAfter, MAX_RETRY_AFTER_VALUE_BYTES};

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const RFC_850_DATE: &str = "Sunday, 06-Nov-94 08:49:37 GMT";
const ASCTIME_DATE: &str = "Sun Nov  6 08:49:37 1994";
const U64_MAX: &str = "18446744073709551615";
const U64_MAX_PLUS_ONE: &str = "18446744073709551616";
const RETRY_AT: Duration = Duration::from_secs(784_111_777);

#[test]
fn retry_after_parses_delta_seconds() {
  for (value, expected) in [("0", 0), ("120", 120), (U64_MAX, u64::MAX)] {
    let retry_after = RetryAfter::parse(value).expect("delta-seconds should parse");

    assert_eq!(Some(expected), retry_after.delta_seconds());
    assert_eq!(None, retry_after.http_date());
    assert_eq!(value, retry_after.header_value());
    assert_eq!(
      retry_after,
      RetryAfter::parse(retry_after.header_value()).expect("canonical delta should reparse")
    );
  }
}

#[test]
fn retry_after_parses_http_dates() {
  for value in [IMF_FIXDATE, RFC_850_DATE, ASCTIME_DATE] {
    let retry_after = RetryAfter::parse(value).expect("HTTP-date should parse");

    assert_eq!(None, retry_after.delta_seconds());
    assert_eq!(Some(UNIX_EPOCH + RETRY_AT), retry_after.http_date());
    assert_eq!(IMF_FIXDATE, retry_after.header_value());
    assert_eq!(
      retry_after,
      RetryAfter::parse(retry_after.header_value()).expect("canonical IMF-fixdate should reparse")
    );
  }
}

#[test]
fn retry_after_accepts_http_optional_whitespace_padding() {
  for value in ["\t120\t", " 120 ", " \t120\t ", "120\t", "\t120"] {
    let retry_after = RetryAfter::parse(value).expect("OWS-padded delta-seconds should parse");
    assert_eq!(RetryAfter::DeltaSeconds(120), retry_after);
    assert_eq!("120", retry_after.header_value());
  }

  for value in [
    format!(" \t{IMF_FIXDATE}\t "),
    format!("\t{ASCTIME_DATE} "),
    format!(" {RFC_850_DATE}\t"),
  ] {
    let retry_after = RetryAfter::parse(&value).expect("OWS-padded HTTP-date should parse");
    assert_eq!(RetryAfter::HttpDate(UNIX_EPOCH + RETRY_AT), retry_after);
    assert_eq!(IMF_FIXDATE, retry_after.header_value());
  }
}

#[test]
fn retry_after_rejects_empty_malformed_control_and_duplicate_values() {
  assert!(RetryAfter::parse_values([]).is_err());
  assert!(RetryAfter::parse_values(["60", "120"]).is_err());
  assert!(RetryAfter::parse_values([IMF_FIXDATE, IMF_FIXDATE]).is_err());

  for value in [
    "",
    " ",
    "-1",
    "+1",
    ".1",
    "1.",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    U64_MAX_PLUS_ONE,
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
fn retry_after_validates_ows_and_controls_on_every_value() {
  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let delta = format!("120{}", char::from(control));
    let date = format!("{IMF_FIXDATE}{}", char::from(control));
    assert!(
      RetryAfter::parse(&delta).is_err(),
      "delta control {control:?} must be rejected"
    );
    assert!(
      RetryAfter::parse(&date).is_err(),
      "date control {control:?} must be rejected"
    );
  }

  assert!(RetryAfter::parse_values(["60", "bad\0value"]).is_err());
  assert!(RetryAfter::parse_values([IMF_FIXDATE, "bad\rvalue"]).is_err());
  assert!(RetryAfter::parse_values(["120", "bad\u{7f}value"]).is_err());
}

#[test]
fn retry_after_enforces_value_bounds() {
  let exact_delta = "0".repeat(MAX_RETRY_AFTER_VALUE_BYTES);
  let oversized_delta = "1".repeat(MAX_RETRY_AFTER_VALUE_BYTES + 1);
  let delta_padding = " ".repeat(MAX_RETRY_AFTER_VALUE_BYTES - 1);
  let exact_padded_delta = format!("{delta_padding}0");
  let oversized_padded_delta = format!(" {exact_padded_delta}");

  assert_eq!(
    RetryAfter::DeltaSeconds(0),
    RetryAfter::parse(&exact_delta).expect("exactly bounded delta should parse")
  );
  assert_eq!(
    RetryAfter::DeltaSeconds(0),
    RetryAfter::parse(&exact_padded_delta).expect("OWS-padded exact delta bound should parse")
  );
  assert!(RetryAfter::parse(&oversized_delta).is_err());
  assert!(RetryAfter::parse(&oversized_padded_delta).is_err());

  let date_padding = " ".repeat(MAX_RETRY_AFTER_VALUE_BYTES - IMF_FIXDATE.len());
  let exact_date = format!("{date_padding}{IMF_FIXDATE}");
  let oversized_date = format!(" {exact_date}");

  assert_eq!(
    RetryAfter::HttpDate(UNIX_EPOCH + RETRY_AT),
    RetryAfter::parse(&exact_date).expect("OWS-padded exact date bound should parse")
  );
  assert!(RetryAfter::parse(&oversized_date).is_err());
}

#[test]
fn retry_after_checks_duplicate_values_against_its_bound() {
  let oversized = "1".repeat(MAX_RETRY_AFTER_VALUE_BYTES + 1);

  assert!(
    RetryAfter::parse_values(["60", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
  assert!(
    RetryAfter::parse_values([IMF_FIXDATE, oversized.as_str()]).is_err(),
    "oversized duplicate date fields must not bypass validation"
  );
}
