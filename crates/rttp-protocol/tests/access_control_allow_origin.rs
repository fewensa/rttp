use rttp_protocol::access_control_allow_origin::{
  AccessControlAllowOrigin, MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES,
};
use rttp_protocol::origin::{Origin, OriginScheme};

#[test]
fn access_control_allow_origin_parses_wildcard_null_and_tuple_origins() {
  let wildcard =
    AccessControlAllowOrigin::parse("*").expect("wildcard Access-Control-Allow-Origin");
  assert!(wildcard.is_wildcard());
  assert_eq!(None, wildcard.origin());
  assert_eq!("*", wildcard.header_value());

  let null = AccessControlAllowOrigin::parse("null").expect("null Access-Control-Allow-Origin");
  assert_eq!(Some(&Origin::Null), null.origin());
  assert_eq!("null", null.header_value());

  let tuple = AccessControlAllowOrigin::parse("https://example.test:8443")
    .expect("tuple Access-Control-Allow-Origin");
  let origin = tuple.origin().expect("tuple origin should be present");
  let origin = origin.tuple().expect("tuple origin should parse");
  assert_eq!(OriginScheme::Https, origin.scheme());
  assert_eq!("example.test", origin.host());
  assert_eq!(Some(8443), origin.port());
  assert_eq!("https://example.test:8443", tuple.header_value());
}

#[test]
fn access_control_allow_origin_rejects_duplicate_and_malformed_values() {
  for values in [
    vec!["*", "null"],
    vec!["https://example.test", "https://other.test"],
    vec!["https://example.test, https://other.test"],
    vec!["https://example.test\r\n"],
    vec!["https://example.test/path"],
    vec!["ftp://example.test"],
  ] {
    assert!(
      AccessControlAllowOrigin::parse_values(values.iter().copied()).is_err(),
      "{values:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_origin_enforces_the_value_bound() {
  assert!(AccessControlAllowOrigin::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES + 1)
  )
  .is_err());
}
