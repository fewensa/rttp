use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::memento_datetime::{MementoDatetime, MAX_MEMENTO_DATETIME_VALUE_BYTES};

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";

#[test]
fn memento_datetime_parses_one_imf_fixdate() {
  let datetime = MementoDatetime::parse(IMF_FIXDATE).expect("IMF-fixdate should parse");
  assert_eq!(
    datetime.datetime(),
    UNIX_EPOCH + Duration::from_secs(784_111_777)
  );
  assert_eq!(datetime.header_value(), IMF_FIXDATE);
}

#[test]
fn memento_datetime_accepts_http_optional_whitespace_padding() {
  for value in [
    format!("\t{IMF_FIXDATE}\t"),
    format!(" {IMF_FIXDATE} "),
    format!(" \t{IMF_FIXDATE}\t "),
  ] {
    let datetime = MementoDatetime::parse(&value).expect("OWS-padded IMF-fixdate should parse");
    assert_eq!(
      datetime.datetime(),
      UNIX_EPOCH + Duration::from_secs(784_111_777)
    );
    assert_eq!(datetime.header_value(), IMF_FIXDATE);
  }
}

#[test]
fn memento_datetime_rejects_empty_malformed_control_and_duplicate_values() {
  assert!(MementoDatetime::parse_values([]).is_err());
  assert!(MementoDatetime::parse_values([IMF_FIXDATE, IMF_FIXDATE]).is_err());

  for value in [
    "",
    " ",
    "not a date",
    "Sun, 06 Nov 1994 08:49:37 GMT, Mon, 07 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT\r\nX: y",
    "Sun, 06 Nov 1994 08:49:37 GMT\u{7f}",
  ] {
    assert!(
      MementoDatetime::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn memento_datetime_enforces_value_bounds() {
  assert!(MementoDatetime::parse("x".repeat(MAX_MEMENTO_DATETIME_VALUE_BYTES + 1)).is_err());
}

#[test]
fn memento_datetime_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_MEMENTO_DATETIME_VALUE_BYTES + 1);

  assert!(
    MementoDatetime::parse_values([IMF_FIXDATE, oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
