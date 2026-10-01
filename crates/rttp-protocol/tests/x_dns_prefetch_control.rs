use rttp_protocol::x_dns_prefetch_control::{
  XDnsPrefetchControl, MAX_X_DNS_PREFETCH_CONTROL_VALUE_BYTES,
};

#[test]
fn x_dns_prefetch_control_parses_on_and_off_case_insensitively() {
  for (value, expected, canonical) in [
    ("on", XDnsPrefetchControl::On, "on"),
    ("ON", XDnsPrefetchControl::On, "on"),
    ("On", XDnsPrefetchControl::On, "on"),
    ("off", XDnsPrefetchControl::Off, "off"),
    ("OFF", XDnsPrefetchControl::Off, "off"),
    ("Off", XDnsPrefetchControl::Off, "off"),
  ] {
    let metadata = XDnsPrefetchControl::parse(value).expect("valid value should parse");
    assert_eq!(expected, metadata);
    assert_eq!(canonical, metadata.header_value());
  }
}

#[test]
fn x_dns_prefetch_control_accepts_ows_and_rejects_malformed_values() {
  for value in ["\ton\t", " on ", "\toFf "] {
    assert!(
      XDnsPrefetchControl::parse(value).is_ok(),
      "OWS-padded value {value:?} should parse"
    );
  }

  for value in [
    "",
    "   ",
    "true",
    "on, off",
    "on; foo",
    "\"on\"",
    "on\tinner",
    "on\r\nX: y",
    "on\u{7f}",
  ] {
    assert!(
      XDnsPrefetchControl::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn x_dns_prefetch_control_rejects_duplicate_fields_and_empty_sets() {
  assert!(XDnsPrefetchControl::parse_values(["on", "off"]).is_err());
  assert!(XDnsPrefetchControl::parse_values([]).is_err());
}

#[test]
fn x_dns_prefetch_control_enforces_value_bounds_on_all_fields() {
  let oversized = "x".repeat(MAX_X_DNS_PREFETCH_CONTROL_VALUE_BYTES + 1);

  assert!(XDnsPrefetchControl::parse(&oversized).is_err());
  assert!(XDnsPrefetchControl::parse_values(["on", oversized.as_str()]).is_err());
}
