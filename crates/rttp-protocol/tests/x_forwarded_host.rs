use rttp_protocol::x_forwarded_host::{
  XForwardedHost, MAX_X_FORWARDED_HOSTS, MAX_X_FORWARDED_HOST_VALUE_BYTES,
};

#[test]
fn x_forwarded_host_parses_ordered_authorities_and_combined_fields() {
  let forwarded_host =
    XForwardedHost::parse_values(["example.test:443, [2001:db8::1]:8443", "internal.test"])
      .expect("X-Forwarded-Host authorities should parse");

  assert_eq!(3, forwarded_host.len());
  assert_eq!("example.test", forwarded_host.hosts()[0].host());
  assert_eq!(Some("443"), forwarded_host.hosts()[0].port());
  assert_eq!("[2001:db8::1]", forwarded_host.hosts()[1].host());
  assert_eq!(Some("8443"), forwarded_host.hosts()[1].port());
  assert_eq!("internal.test", forwarded_host.hosts()[2].host());
  assert_eq!(
    "example.test:443, [2001:db8::1]:8443, internal.test",
    forwarded_host.header_value()
  );
}

#[test]
fn x_forwarded_host_rejects_malformed_injected_and_empty_authorities() {
  for value in [
    "",
    " ",
    "example.test,",
    ", example.test",
    "example.test,, internal.test",
    "https://example.test",
    "user@example.test",
    "example.test/path",
    "[2001:db8::1",
    "example.test:port",
    "example.test\r\nX-Injected: 1",
    "example.test\u{0}",
    "example.test\u{7f}",
  ] {
    assert!(
      XForwardedHost::parse(value).is_err(),
      "X-Forwarded-Host should reject {value:?}"
    );
  }
}

#[test]
fn x_forwarded_host_enforces_member_and_size_bounds() {
  let too_many = (0..=MAX_X_FORWARDED_HOSTS)
    .map(|index| format!("h{index}.example"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(XForwardedHost::parse(too_many).is_err());

  let padded = format!(
    "example.test{}",
    " ".repeat(MAX_X_FORWARDED_HOST_VALUE_BYTES - "example.test".len())
  );
  assert_eq!(MAX_X_FORWARDED_HOST_VALUE_BYTES, padded.len());
  assert!(XForwardedHost::parse(padded.as_str()).is_ok());
  assert!(XForwardedHost::parse_values([padded.as_str(), "internal.test"]).is_err());
}
