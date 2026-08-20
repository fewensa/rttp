use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::http_date::{
  ResponseDate, ResponseExpires, ResponseLastModified, MAX_RESPONSE_HTTP_DATE_VALUE_BYTES,
};

const UNIX_SECONDS: u64 = 784_111_777;

#[test]
fn response_http_date_primitives_accept_supported_forms_and_format_canonically() {
  for value in [
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sunday, 06-Nov-94 08:49:37 GMT",
    "Sun Nov  6 08:49:37 1994",
    "\tSun, 06 Nov 1994 08:49:37 GMT ",
  ] {
    assert_eq!(
      UNIX_EPOCH + Duration::from_secs(UNIX_SECONDS),
      ResponseDate::parse(value)
        .unwrap_or_else(|err| panic!("Date should parse {value:?}: {err}"))
        .datetime()
    );
    assert_eq!(
      UNIX_EPOCH + Duration::from_secs(UNIX_SECONDS),
      ResponseExpires::parse(value)
        .unwrap_or_else(|err| panic!("Expires should parse {value:?}: {err}"))
        .datetime()
    );
    assert_eq!(
      UNIX_EPOCH + Duration::from_secs(UNIX_SECONDS),
      ResponseLastModified::parse(value)
        .unwrap_or_else(|err| panic!("Last-Modified should parse {value:?}: {err}"))
        .datetime()
    );
  }

  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    ResponseExpires::new(UNIX_EPOCH + Duration::from_secs(UNIX_SECONDS)).header_value()
  );
}

#[test]
fn response_http_date_primitives_reject_malformed_duplicate_and_control_values() {
  for value in ["", "not a date", "Sun, 06 Nov 1994 08:49:37 PST"] {
    assert!(
      ResponseDate::parse(value).is_err(),
      "Date should reject {value:?}"
    );
    assert!(
      ResponseExpires::parse(value).is_err(),
      "Expires should reject {value:?}"
    );
    assert!(
      ResponseLastModified::parse(value).is_err(),
      "Last-Modified should reject {value:?}"
    );
  }

  assert!(ResponseDate::parse_values([
    "Sun, 06 Nov 1994 08:49:37 GMT",
    "Sun, 06 Nov 1994 08:49:38 GMT",
  ])
  .is_err());
  assert!(ResponseExpires::parse("Sun, 06 Nov 1994 08:49:37 GMT\r").is_err());
  assert!(ResponseLastModified::parse("Sun, 06 Nov 1994 08:49:37 GMT\u{7f}").is_err());
}

#[test]
fn response_http_date_primitives_reject_oversized_values() {
  let oversized = "x".repeat(MAX_RESPONSE_HTTP_DATE_VALUE_BYTES + 1);

  assert!(ResponseDate::parse(&oversized).is_err());
  assert!(ResponseExpires::parse(&oversized).is_err());
  assert!(ResponseLastModified::parse(&oversized).is_err());
}
