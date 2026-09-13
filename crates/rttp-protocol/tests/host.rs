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
fn host_accepts_ipvfuture_version_letter_casing() {
  for value in [
    "[v1.fe80::a]",
    "[V1.fe80::a]",
    "[vAb.xyz]:8080",
    "[VAb.xyz]:8080",
  ] {
    let host = Host::parse(value).expect("IPvFuture Host casing must parse");
    assert!(host.host().starts_with('['), "{value} must keep brackets");
    assert!(host.host().ends_with(']'), "{value} must keep brackets");
  }

  let upper = Host::parse("[V1.fe80::a]:443").expect("uppercase IPvFuture must parse");
  assert_eq!("[V1.fe80::a]", upper.host());
  assert_eq!(Some("443"), upper.port());
  assert_eq!("[V1.fe80::a]:443", upper.header_value());
}

#[test]
fn host_accepts_percent_encoded_reg_name_hex_casing() {
  for value in [
    "foo%2Dbar",
    "foo%2dbar",
    "ex%61mple.test",
    "ex%61Mple.%54est",
  ] {
    let host = Host::parse(value).expect("percent-encoded reg-name must parse");
    assert_eq!(value, host.host());
    assert_eq!(None, host.port());
    assert_eq!(value, host.header_value());
  }

  let with_port = Host::parse("foo%2dbar:8080").expect("pct-encoded host:port must parse");
  assert_eq!("foo%2dbar", with_port.host());
  assert_eq!(Some("8080"), with_port.port());
}

#[test]
fn host_accepts_bracketed_ipv6_with_and_without_port() {
  for value in [
    "[::1]",
    "[::ffff:127.0.0.1]",
    "[2001:db8::1]:443",
    "[::1]:1",
  ] {
    let host = Host::parse(value).expect("bracketed IPv6 Host must parse");
    assert!(host.host().starts_with('['));
    assert!(host.host().ends_with(']'));
  }

  let mapped = Host::parse("[::ffff:192.0.2.1]:8443").expect("IPv4-mapped IPv6 must parse");
  assert_eq!("[::ffff:192.0.2.1]", mapped.host());
  assert_eq!(Some("8443"), mapped.port());
  assert_eq!("[::ffff:192.0.2.1]:8443", mapped.header_value());
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
    "user:pass@example.test",
    "example.test?",
    "example.test?q=1",
    "example.test#frag",
    "example.test#",
    "example.test:",
    "2001:db8::1",
    "[]",
    "[::1",
    "::1]",
    "[::1]]",
    "[[::1]]",
    "[[::1]",
    "[not-an-ip]",
    "[:::]",
    "[foo bar]:80",
    "[::1]:",
    "[::1]80",
    "[v.fe80::a]",
    "[v1.]",
    "[1.fe80::a]",
    "foo%",
    "foo%2",
    "foo%GG",
    "foo%2G",
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
fn host_rejects_control_bytes() {
  for value in [
    "example.test\0",
    "example.test\n",
    "example.test\r",
    "example.test\r\nX-Injected: 1",
    "example.test\u{7f}",
    "\u{1b}example.test",
  ] {
    let error = Host::parse(value).expect_err("control bytes must be rejected");
    assert_eq!(
      "invalid Host header control byte",
      error.to_string(),
      "{value:?}"
    );
  }
}

#[test]
fn host_rejects_duplicate_singleton_fields() {
  assert!(Host::parse_values(["example.test", "other.test"]).is_err());
  assert!(Host::parse_values(["example.test", "example.test"]).is_err());
  assert!(Host::parse_values(["[::1]", "[::1]:80"]).is_err());
}

#[test]
fn host_enforces_value_bounds_without_panicking() {
  assert!(Host::parse("a".repeat(MAX_HOST_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_HOST_VALUE_BYTES + 1);
  assert!(Host::parse_values(["example.test", oversized_duplicate.as_str()]).is_err());
}
