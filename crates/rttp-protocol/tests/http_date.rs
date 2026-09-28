use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::http_date::{
  ResponseDate, ResponseExpires, ResponseLastModified, MAX_RESPONSE_HTTP_DATE_VALUE_BYTES,
};

const UNIX_SECONDS: u64 = 784_111_777;

const IMF_FIXDATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const RFC_850: &str = "Sunday, 06-Nov-94 08:49:37 GMT";
const ASCTIME: &str = "Sun Nov  6 08:49:37 1994";
const CANONICAL: &str = IMF_FIXDATE;

#[test]
fn response_http_date_primitives_accept_forms_and_round_trip_canonically() {
  for value in [
    IMF_FIXDATE,
    RFC_850,
    ASCTIME,
    "\tSun, 06 Nov 1994 08:49:37 GMT ",
  ] {
    let expected = UNIX_EPOCH + Duration::from_secs(UNIX_SECONDS);

    let date = ResponseDate::parse(value).unwrap();
    assert_eq!(expected, date.datetime());
    assert_eq!(CANONICAL, date.header_value());
    assert_eq!(
      expected,
      ResponseDate::parse(date.header_value()).unwrap().datetime()
    );

    let expires = ResponseExpires::parse(value).unwrap();
    assert_eq!(expected, expires.datetime());
    assert_eq!(CANONICAL, expires.header_value());
    assert_eq!(
      expected,
      ResponseExpires::parse(expires.header_value())
        .unwrap()
        .datetime()
    );

    let last_modified = ResponseLastModified::parse(value).unwrap();
    assert_eq!(expected, last_modified.datetime());
    assert_eq!(CANONICAL, last_modified.header_value());
    assert_eq!(
      expected,
      ResponseLastModified::parse(last_modified.header_value())
        .unwrap()
        .datetime()
    );
  }
}

#[test]
fn response_http_date_primitives_reject_empty_combined_and_duplicates() {
  for values in [
    [""].as_slice(),
    [IMF_FIXDATE, ", ", IMF_FIXDATE].as_slice(),
    [IMF_FIXDATE, IMF_FIXDATE].as_slice(),
  ] {
    assert!(ResponseDate::parse_values(values.iter().copied()).is_err());
    assert!(ResponseExpires::parse_values(values.iter().copied()).is_err());
    assert!(ResponseLastModified::parse_values(values.iter().copied()).is_err());
  }
}

#[test]
fn response_http_date_primitives_validate_ows_and_controls_on_every_value() {
  let ows = " \tSun, 06 Nov 1994 08:49:37 GMT\t ";
  assert!(ResponseDate::parse(ows).is_ok());
  assert!(ResponseExpires::parse(ows).is_ok());
  assert!(ResponseLastModified::parse(ows).is_ok());

  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let value = format!("{IMF_FIXDATE}{}", char::from(control));
    assert!(ResponseDate::parse(&value).is_err(), "control {control:?}");
    assert!(
      ResponseExpires::parse(&value).is_err(),
      "control {control:?}"
    );
    assert!(
      ResponseLastModified::parse(&value).is_err(),
      "control {control:?}"
    );
  }

  assert!(ResponseDate::parse_values([IMF_FIXDATE, "bad\0value"]).is_err());
  assert!(ResponseExpires::parse_values([IMF_FIXDATE, "bad\rvalue"]).is_err());
  assert!(ResponseLastModified::parse_values([IMF_FIXDATE, "bad\u{7f}value"]).is_err());
}

#[test]
fn response_http_date_primitives_enforce_exact_per_field_size_boundary() {
  let padding = " ".repeat(MAX_RESPONSE_HTTP_DATE_VALUE_BYTES - IMF_FIXDATE.len());
  let exact = format!("{padding}{IMF_FIXDATE}");
  let oversized = format!(" {exact}");
  assert!(ResponseDate::parse(&exact).is_ok());
  assert!(ResponseExpires::parse(&exact).is_ok());
  assert!(ResponseLastModified::parse(&exact).is_ok());
  assert!(ResponseDate::parse(&oversized).is_err());
  assert!(ResponseExpires::parse(&oversized).is_err());
  assert!(ResponseLastModified::parse(&oversized).is_err());
}
