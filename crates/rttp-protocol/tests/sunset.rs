use rttp_protocol::sunset::parse_sunset;
use std::time::{Duration, UNIX_EPOCH};

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
