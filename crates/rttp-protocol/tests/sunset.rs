use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::sunset::{
  format_sunset, parse_sunset, parse_sunset_values, MAX_SUNSET_VALUE_BYTES,
};

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const RFC_850_DATE: &str = "Sunday, 06-Nov-94 08:49:37 GMT";
const ASCTIME_DATE: &str = "Sun Nov  6 08:49:37 1994";
const SUNSET_AT: Duration = Duration::from_secs(784_111_777);

#[test]
fn parses_imf_fixdate_sunset_values() {
  assert_eq!(
    UNIX_EPOCH + Duration::from_secs(784_111_777),
    parse_sunset("Sun, 06 Nov 1994 08:49:37 GMT").expect("Sunset should parse")
  );
}

#[test]
fn rejects_invalid_sunset_values() {
  assert!(parse_sunset("not a date").is_err());
}

#[test]
fn sunset_parses_http_dates_and_round_trips_canonical_imf() {
  let expected = UNIX_EPOCH + SUNSET_AT;

  for value in [IMF_FIXDATE, RFC_850_DATE, ASCTIME_DATE] {
    let parsed = parse_sunset(value).expect("HTTP-date should parse");
    assert_eq!(expected, parsed);
    assert_eq!(IMF_FIXDATE, format_sunset(parsed));
    assert_eq!(
      parsed,
      parse_sunset(format_sunset(parsed)).expect("canonical IMF-fixdate should reparse")
    );
  }
}

#[test]
fn sunset_accepts_http_optional_whitespace_padding() {
  for value in [
    format!(" \t{IMF_FIXDATE}\t "),
    format!("\t{ASCTIME_DATE} "),
    format!(" {RFC_850_DATE}\t"),
    format!("\t{IMF_FIXDATE}\t"),
    format!(" {IMF_FIXDATE} "),
  ] {
    let parsed = parse_sunset(&value).expect("OWS-padded HTTP-date should parse");
    assert_eq!(UNIX_EPOCH + SUNSET_AT, parsed);
    assert_eq!(IMF_FIXDATE, format_sunset(parsed));
  }
}

#[test]
fn sunset_absent_values_are_none() {
  assert_eq!(
    None,
    parse_sunset_values(std::iter::empty()).expect("absent Sunset should parse")
  );
  assert_eq!(
    None,
    parse_sunset_values([]).expect("absent Sunset should parse")
  );
}

#[test]
fn sunset_rejects_empty_malformed_control_and_duplicate_values() {
  assert!(parse_sunset_values([IMF_FIXDATE, IMF_FIXDATE]).is_err());

  for value in [
    "",
    " ",
    "not a date",
    "Sun, 06 Nov 1994 08:49:37 PST",
    "Sun, 06 Nov 1994 08:49:37 GMT, Mon, 07 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT\r\nX: y",
    "Sun, 06 Nov 1994 08:49:37 GMT\u{7f}",
  ] {
    assert!(parse_sunset(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn sunset_validates_ows_and_controls_on_every_value() {
  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let value = format!("{IMF_FIXDATE}{}", char::from(control));
    assert!(
      parse_sunset(&value).is_err(),
      "control {control:?} must be rejected"
    );
  }

  assert!(parse_sunset_values([IMF_FIXDATE, "bad\0value"]).is_err());
  assert!(parse_sunset_values([IMF_FIXDATE, "bad\rvalue"]).is_err());
  assert!(parse_sunset_values([IMF_FIXDATE, "bad\u{7f}value"]).is_err());
}

#[test]
fn sunset_enforces_exact_per_field_size_boundary() {
  let padding = " ".repeat(MAX_SUNSET_VALUE_BYTES - IMF_FIXDATE.len());
  let exact = format!("{padding}{IMF_FIXDATE}");
  let oversized = format!(" {exact}");

  assert_eq!(
    UNIX_EPOCH + SUNSET_AT,
    parse_sunset(&exact).expect("OWS-padded exact date bound should parse")
  );
  assert!(parse_sunset(&oversized).is_err());
}

#[test]
fn sunset_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_SUNSET_VALUE_BYTES + 1);

  assert!(
    parse_sunset_values([IMF_FIXDATE, oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
