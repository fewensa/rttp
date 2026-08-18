use rttp_protocol::host::{Host, MAX_HOST_VALUE_BYTES};

#[test]
fn host_parses_name_port_and_ipv6_authorities() {
  let name = Host::parse("example.test").expect("reg-name Host must parse");
  let with_port = Host::parse("example.test:8080").expect("host:port Host must parse");
  let ipv4 = Host::parse("127.0.0.1:443").expect("IPv4 Host must parse");
  let ipv6 = Host::parse("[::1]").expect("IPv6 Host must parse");
  let ipv6_port = Host::parse("[2001:db8::1]:8443").expect("IPv6 host:port Host must parse");
  let ipvfuture = Host::parse("[v1.fe80::a]:8080").expect("IPvFuture Host must parse");

  assert_eq!("example.test", name.host());
  assert_eq!(None, name.port());
  assert_eq!("example.test", name.header_value());
  assert_eq!("example.test", with_port.host());
  assert_eq!(Some("8080"), with_port.port());
  assert_eq!("example.test:8080", with_port.header_value());
  assert_eq!("127.0.0.1", ipv4.host());
  assert_eq!(Some("443"), ipv4.port());
  assert_eq!("[::1]", ipv6.host());
  assert_eq!(None, ipv6.port());
  assert_eq!("[::1]", ipv6.header_value());
  assert_eq!("[2001:db8::1]", ipv6_port.host());
  assert_eq!(Some("8443"), ipv6_port.port());
  assert_eq!("[2001:db8::1]:8443", ipv6_port.header_value());
  assert_eq!("[v1.fe80::a]", ipvfuture.host());
  assert_eq!(Some("8080"), ipvfuture.port());
  assert_eq!("[v1.fe80::a]:8080", ipvfuture.header_value());
}

#[test]
fn host_trims_http_optional_whitespace() {
  let host = Host::parse("\texample.test:80 ").expect("OWS-padded Host must parse");

  assert_eq!("example.test", host.host());
  assert_eq!(Some("80"), host.port());
  assert_eq!("example.test:80", host.header_value());
}

#[test]
fn host_accepts_inbound_reg_name_characters() {
  for value in ["foo_bar.example", "foo~bar", "foo%2Dbar", "foo!bar"] {
    let host = Host::parse(value).expect("inbound-legal host must parse");
    assert_eq!(value, host.header_value());
  }
}

#[test]
fn host_rejects_empty_path_userinfo_and_malformed_values() {
  for value in [
    "",
    "   ",
    "\t",
    "/",
    "/path",
    "example.test/path",
    "user@example.test",
    "example.test?",
    "example.test#frag",
    "example.test:",
    "2001:db8::1",
    "[]",
    "[::1",
    "::1]",
    "[not-an-ip]",
    "[:::]",
    "[foo bar]:80",
    "foo%2",
    "foo%GG",
    "example.test:80:443",
  ] {
    assert!(Host::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    Host::parse_values([]).is_err(),
    "empty field set must be rejected"
  );
}

#[test]
fn host_rejects_duplicate_singleton_fields() {
  assert!(Host::parse_values(["example.test", "other.test"]).is_err());
  assert!(Host::parse_values(["example.test", "example.test"]).is_err());
}

#[test]
fn host_enforces_value_bounds_without_panicking() {
  assert!(Host::parse("a".repeat(MAX_HOST_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_HOST_VALUE_BYTES + 1);
  assert!(Host::parse_values(["example.test", oversized_duplicate.as_str()]).is_err());
}
