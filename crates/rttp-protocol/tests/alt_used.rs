use rttp_protocol::alt_used::{AltUsed, MAX_ALT_USED_VALUE_BYTES};

#[test]
fn alt_used_parses_host_port_and_ipv6_authorities() {
  let name = AltUsed::parse("example.test").expect("reg-name Alt-Used must parse");
  let with_port = AltUsed::parse("example.test:8443").expect("host:port Alt-Used must parse");
  let ipv4 = AltUsed::parse("127.0.0.1:443").expect("IPv4 Alt-Used must parse");
  let ipv6 = AltUsed::parse("[2001:db8::1]").expect("IPv6 Alt-Used must parse");
  let ipv6_port = AltUsed::parse("[2001:db8::1]:8443").expect("IPv6 host:port Alt-Used must parse");

  assert_eq!("example.test", name.host());
  assert_eq!(None, name.port());
  assert_eq!("example.test", name.header_value());
  assert_eq!("example.test", with_port.host());
  assert_eq!(Some("8443"), with_port.port());
  assert_eq!("example.test:8443", with_port.header_value());
  assert_eq!("127.0.0.1", ipv4.host());
  assert_eq!(Some("443"), ipv4.port());
  assert_eq!("[2001:db8::1]", ipv6.host());
  assert_eq!(None, ipv6.port());
  assert_eq!("[2001:db8::1]", ipv6.header_value());
  assert_eq!("[2001:db8::1]", ipv6_port.host());
  assert_eq!(Some("8443"), ipv6_port.port());
  assert_eq!("[2001:db8::1]:8443", ipv6_port.header_value());
}

#[test]
fn alt_used_trims_http_optional_whitespace() {
  let alt_used = AltUsed::parse("\texample.test:443 ").expect("OWS-padded Alt-Used must parse");

  assert_eq!("example.test", alt_used.host());
  assert_eq!(Some("443"), alt_used.port());
  assert_eq!("example.test:443", alt_used.header_value());
}

#[test]
fn alt_used_rejects_malformed_and_duplicate_values() {
  for value in [
    "",
    "   ",
    "/path",
    "https://example.test",
    "user@example.test",
    "example.test?",
    "example.test#frag",
    "example.test:",
    "2001:db8::1",
    "[]",
    "[::1",
    "::1]",
    "[not-an-ip]",
    "foo%GG",
    "example.test:80:443",
  ] {
    assert!(AltUsed::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(AltUsed::parse_values([]).is_err());
  assert!(AltUsed::parse_values(["example.test", "other.test"]).is_err());
  assert!(AltUsed::parse_values(["example.test", "example.test"]).is_err());
}

#[test]
fn alt_used_enforces_value_bounds_without_panicking() {
  assert!(AltUsed::parse("a".repeat(MAX_ALT_USED_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_ALT_USED_VALUE_BYTES + 1);
  assert!(AltUsed::parse_values(["example.test", oversized_duplicate.as_str()]).is_err());
}
