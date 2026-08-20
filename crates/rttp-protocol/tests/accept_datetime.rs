use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::accept_datetime::{AcceptDatetime, MAX_ACCEPT_DATETIME_VALUE_BYTES};
use rttp_protocol::memento_datetime::MementoDatetime;

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const RFC_850_DATE: &str = "Sunday, 06-Nov-94 08:49:37 GMT";
const ASCTIME_DATE: &str = "Sun Nov  6 08:49:37 1994";

#[test]
fn accept_datetime_parses_http_dates_and_round_trips_canonical_form() {
  let instant = UNIX_EPOCH + Duration::from_secs(784_111_777);

  for value in [IMF_FIXDATE, RFC_850_DATE, ASCTIME_DATE] {
    let parsed = AcceptDatetime::parse(value).expect("HTTP-date should parse");
    assert_eq!(instant, parsed.datetime());
    assert_eq!(
      IMF_FIXDATE,
      parsed.header_value(),
      "header_value must emit canonical IMF-fixdate"
    );
  }
}

#[test]
fn accept_datetime_accepts_http_optional_whitespace_padding() {
  for value in [
    "\tSun, 06 Nov 1994 08:49:37 GMT\t",
    " Sun, 06 Nov 1994 08:49:37 GMT ",
    " \tSun, 06 Nov 1994 08:49:37 GMT\t ",
  ] {
    let parsed = AcceptDatetime::parse(value).expect("OWS-padded Accept-Datetime should parse");
    assert_eq!(IMF_FIXDATE, parsed.header_value());
  }
}

#[test]
fn accept_datetime_rejects_duplicate_and_invalid_values() {
  assert!(AcceptDatetime::parse_values([
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 GMT",
  ])
  .is_err());
  assert!(AcceptDatetime::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "not-a-date",
    "08:49:37 06 Nov 1994",
    "Sun, 06 Nov 1994",
    "Sun, 06 Nov 1994 08:49:37 GMT, Mon, 07 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:37 PST",
    "0\r\nX: y",
    "Sun, 06 Nov 1994 08:49:37 GMT\u{7f}",
  ] {
    assert!(
      AcceptDatetime::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_datetime_enforces_value_bounds() {
  let oversized = "Sun, 06 Nov 1994 08:49:37 GMT".repeat(MAX_ACCEPT_DATETIME_VALUE_BYTES / 29 + 1);
  assert!(
    AcceptDatetime::parse(oversized).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn accept_datetime_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_ACCEPT_DATETIME_VALUE_BYTES + 1);

  assert!(
    AcceptDatetime::parse_values(["Sun, 06 Nov 1994 08:49:37 GMT", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}

#[test]
fn accept_datetime_interoperates_with_memento_datetime_instants() {
  let instant = UNIX_EPOCH + Duration::from_secs(784_111_777);

  for (accept_form, memento_form) in [
    (IMF_FIXDATE, IMF_FIXDATE),
    (RFC_850_DATE, IMF_FIXDATE),
    (ASCTIME_DATE, RFC_850_DATE),
  ] {
    let accept = AcceptDatetime::parse(accept_form).expect("Accept-Datetime should parse");
    let memento = MementoDatetime::parse(memento_form).expect("Memento-Datetime should parse");
    assert_eq!(
      memento.datetime(),
      accept.datetime(),
      "the same HTTP-date instant must match across request and response metadata"
    );
    assert_eq!(
      memento.header_value(),
      accept.header_value(),
      "both sides must emit the same canonical IMF-fixdate"
    );
  }

  assert_eq!(
    AcceptDatetime::new(instant).header_value(),
    MementoDatetime::new(instant).header_value()
  );
}
