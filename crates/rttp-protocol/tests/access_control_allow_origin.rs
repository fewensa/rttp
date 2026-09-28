use rttp_protocol::access_control_allow_origin::{
  AccessControlAllowOrigin, MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES,
};
use rttp_protocol::origin::{Origin, OriginScheme};

#[test]
fn access_control_allow_origin_parses_exact_wildcard_null_and_tuple_origins() {
  let wildcard =
    AccessControlAllowOrigin::parse("*").expect("wildcard Access-Control-Allow-Origin");
  assert!(wildcard.is_wildcard());
  assert_eq!(None, wildcard.origin());
  assert_eq!("*", wildcard.header_value());

  let null = AccessControlAllowOrigin::parse("null").expect("null Access-Control-Allow-Origin");
  assert!(!null.is_wildcard());
  assert_eq!(Some(&Origin::Null), null.origin());
  assert_eq!("null", null.header_value());

  let tuple = AccessControlAllowOrigin::parse("https://example.test:8443")
    .expect("tuple Access-Control-Allow-Origin");
  assert!(!tuple.is_wildcard());
  let origin = tuple.origin().expect("tuple origin should be present");
  let origin = origin.tuple().expect("tuple origin should parse");
  assert_eq!(OriginScheme::Https, origin.scheme());
  assert_eq!("example.test", origin.host());
  assert_eq!(Some(8443), origin.port());
  assert_eq!("https://example.test:8443", tuple.header_value());
}

#[test]
fn access_control_allow_origin_accepts_outer_sp_htab_ows() {
  for value in [
    " * ",
    "\t*\t",
    " \t*\t ",
    "*\t",
    "\t*",
    " null ",
    "\tnull\t",
    " https://example.test ",
    "\thttps://example.test\t",
    " \thttp://example.test:8080\t ",
  ] {
    let parsed = AccessControlAllowOrigin::parse(value)
      .unwrap_or_else(|_| panic!("OWS-padded Access-Control-Allow-Origin should parse: {value:?}"));
    let expected = value.trim_matches([' ', '\t']);
    let canonical = match expected {
      "*" => "*".to_string(),
      "null" => "null".to_string(),
      other => Origin::parse(other)
        .expect("trimmed origin must parse")
        .header_value(),
    };
    assert_eq!(
      canonical,
      parsed.header_value(),
      "unexpected canonical for {value:?}"
    );
    assert_eq!(
      expected == "*",
      parsed.is_wildcard(),
      "wildcard flag mismatch for {value:?}"
    );
  }
}

#[test]
fn access_control_allow_origin_is_policy_free_singleton_metadata() {
  // Parsing accepts both wildcard and concrete origins without applying CORS policy.
  let wildcard = AccessControlAllowOrigin::parse("*").expect("wildcard");
  let origin = AccessControlAllowOrigin::parse("https://example.test").expect("origin");
  assert!(wildcard.is_wildcard());
  assert!(!origin.is_wildcard());
  assert!(wildcard.origin().is_none());
  assert!(origin.origin().is_some());

  // A single field value is required; policy decisions remain with callers.
  assert!(AccessControlAllowOrigin::parse_values(["*"]).is_ok());
  assert!(AccessControlAllowOrigin::parse_values(["https://example.test"]).is_ok());
  assert!(AccessControlAllowOrigin::parse_values(["null"]).is_ok());
}

#[test]
fn access_control_allow_origin_rejects_duplicate_empty_comma_and_malformed_values() {
  assert!(AccessControlAllowOrigin::parse_values([]).is_err());
  assert!(AccessControlAllowOrigin::parse_values(["*", "*"]).is_err());
  assert!(AccessControlAllowOrigin::parse_values(["null", "null"]).is_err());
  assert!(AccessControlAllowOrigin::parse_values(["*", "null"]).is_err());
  assert!(
    AccessControlAllowOrigin::parse_values(["https://example.test", "https://other.test"]).is_err()
  );

  for value in [
    "",
    " ",
    "\t",
    "   ",
    "*, *",
    "*, https://example.test",
    "https://example.test, https://other.test",
    "https://example.test,",
    ",https://example.test",
    "null, null",
    "*null",
    "null*",
    "**",
    "Null",
    "NULL",
    "nUlL",
  ] {
    assert!(
      AccessControlAllowOrigin::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

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
fn access_control_allow_origin_rejects_internal_ows_controls_paths_and_invalid_schemes() {
  for value in [
    "https://example. test",
    "https://example.\ttest",
    "https ://example.test",
    "https:// example.test",
    "* *",
    "*\t*",
    "https://example.test/path",
    "https://example.test/",
    "https://example.test?query",
    "https://example.test#fragment",
    "https://user@example.test",
    "ftp://example.test",
    "file://example.test",
    "ws://example.test",
    "wss://example.test",
    "HTTPS+TCP://example.test",
    "http://",
    "https://",
    "https://example.test:",
    "https://example.test:abc",
    "https://example.test:1.5",
    "https://example.test:65536",
    "https://example.test:999999999999",
    "https://[::1]:",
    "https://[::1]:65536",
    "https://[::1]8443",
    "https://example.test\n",
    "https://example.test\r",
    "https://example.test\0",
    "https://example.test\x7f",
    "https://example.test\r\nX-Injected: true",
    "*\n",
    "null\r",
    "null\0",
    "https://exämple.test",
    "https://例子.test",
  ] {
    assert!(
      AccessControlAllowOrigin::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_allow_origin_rejects_all_ascii_controls_except_horizontal_tab() {
  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let value = format!("*{}", char::from(control));
    assert!(
      AccessControlAllowOrigin::parse(&value).is_err(),
      "control {control:#04x} must be rejected"
    );
    let origin_value = format!("https://example.test{}", char::from(control));
    assert!(
      AccessControlAllowOrigin::parse(&origin_value).is_err(),
      "control {control:#04x} in origin must be rejected"
    );
  }

  assert!(AccessControlAllowOrigin::parse_values(["*", "bad\0value"]).is_err());
  assert!(AccessControlAllowOrigin::parse_values(["*", "bad\rvalue"]).is_err());
  assert!(AccessControlAllowOrigin::parse_values(["*", "bad\u{7f}value"]).is_err());
}

#[test]
fn access_control_allow_origin_covers_scheme_host_port_and_ip_boundaries() {
  for (value, expected) in [
    ("http://example.test:80", "http://example.test"),
    ("https://example.test:443", "https://example.test"),
    ("http://example.test:8080", "http://example.test:8080"),
    ("https://example.test:8443", "https://example.test:8443"),
    ("HTTP://EXAMPLE.TEST:8080", "http://example.test:8080"),
    ("hTtPs://EXAMPLE.TEST:443", "https://example.test"),
    ("http://127.0.0.1", "http://127.0.0.1"),
    ("http://127.0.0.1:8080", "http://127.0.0.1:8080"),
    ("https://127.0.0.1:443", "https://127.0.0.1"),
    ("http://[0:0:0:0:0:0:0:1]", "http://[::1]"),
    ("https://[0:0:0:0:0:0:0:1]:8443", "https://[::1]:8443"),
    ("https://[::1]:443", "https://[::1]"),
    ("http://[2001:db8::1]:80", "http://[2001:db8::1]"),
  ] {
    let parsed = AccessControlAllowOrigin::parse(value)
      .unwrap_or_else(|_| panic!("boundary origin should parse: {value:?}"));
    assert!(!parsed.is_wildcard());
    assert_eq!(
      expected,
      parsed.header_value(),
      "unexpected canonical form for {value:?}"
    );
  }
}

#[test]
fn access_control_allow_origin_enforces_exact_value_bounds() {
  let padding = " ".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES - 1);
  let exact_wildcard = format!("{padding}*");
  let exact_null = format!(
    "{}null",
    " ".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES - 4)
  );
  let exact_origin = format!(
    "{}https://example.test",
    " ".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES - "https://example.test".len())
  );
  let oversized = "x".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES + 1);
  let oversized_padded = format!(" {exact_wildcard}");

  assert_eq!(
    MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES,
    exact_wildcard.len()
  );
  assert_eq!(
    MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES,
    exact_null.len()
  );
  assert_eq!(
    MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES,
    exact_origin.len()
  );
  assert_eq!(
    MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES + 1,
    oversized_padded.len()
  );

  assert_eq!(
    "*",
    AccessControlAllowOrigin::parse(&exact_wildcard)
      .expect("exact 64 KiB OWS-padded wildcard should parse")
      .header_value()
  );
  assert_eq!(
    "null",
    AccessControlAllowOrigin::parse(&exact_null)
      .expect("exact 64 KiB OWS-padded null should parse")
      .header_value()
  );
  assert_eq!(
    "https://example.test",
    AccessControlAllowOrigin::parse(&exact_origin)
      .expect("exact 64 KiB OWS-padded origin should parse")
      .header_value()
  );
  assert!(AccessControlAllowOrigin::parse(&oversized).is_err());
  assert!(AccessControlAllowOrigin::parse(&oversized_padded).is_err());
  assert!(AccessControlAllowOrigin::parse(
    "x".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES + 1)
  )
  .is_err());
}

#[test]
fn access_control_allow_origin_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES + 1);
  assert!(
    AccessControlAllowOrigin::parse_values(["*", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}

#[test]
fn access_control_allow_origin_header_value_round_trips_and_stays_canonical() {
  for value in [
    "*",
    "null",
    "http://example.test",
    "https://example.test",
    "http://example.test:8080",
    "https://example.test:8443",
    "http://example.test:80",
    "https://example.test:443",
    "HTTP://EXAMPLE.TEST:8080",
    "http://127.0.0.1:8080",
    "https://[0:0:0:0:0:0:0:1]:8443",
    " \thttps://example.test\t ",
    "\t*\t",
    " null ",
  ] {
    let parsed = AccessControlAllowOrigin::parse(value)
      .unwrap_or_else(|_| panic!("value should parse for round-trip: {value:?}"));
    let canonical = parsed.header_value();
    let round_trip = AccessControlAllowOrigin::parse(&canonical)
      .expect("canonical header_value should round-trip");
    assert_eq!(round_trip, parsed, "round-trip inequality for {value:?}");
    assert_eq!(
      round_trip.header_value(),
      canonical,
      "canonical output must be stable for {value:?}"
    );
    assert_eq!(
      round_trip.is_wildcard(),
      parsed.is_wildcard(),
      "wildcard flag must round-trip for {value:?}"
    );
    assert_eq!(round_trip.origin(), parsed.origin());
  }
}
