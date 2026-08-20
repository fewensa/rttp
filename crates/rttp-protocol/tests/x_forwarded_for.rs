use rttp_protocol::x_forwarded_for::{
  XForwardedFor, XForwardedForNodeKind, MAX_X_FORWARDED_FOR_NODES, MAX_X_FORWARDED_FOR_VALUE_BYTES,
};

#[test]
fn x_forwarded_for_parses_ordered_ip_and_unknown_nodes() {
  let forwarded_for = XForwardedFor::parse_values(["192.0.2.60, unknown", "[2001:db8:cafe::17]"])
    .expect("X-Forwarded-For nodes should parse");

  assert_eq!(3, forwarded_for.len());
  assert_eq!("192.0.2.60", forwarded_for.nodes()[0].value());
  assert!(forwarded_for.nodes()[0].is_ip());
  assert_eq!("unknown", forwarded_for.nodes()[1].value());
  assert!(forwarded_for.nodes()[1].is_unknown());
  assert_eq!(XForwardedForNodeKind::Ip, forwarded_for.nodes()[2].kind());
  assert_eq!("[2001:db8:cafe::17]", forwarded_for.nodes()[2].value());
  assert_eq!(
    "192.0.2.60, unknown, [2001:db8:cafe::17]",
    forwarded_for.header_value()
  );
}

#[test]
fn x_forwarded_for_rejects_malformed_injected_and_empty_nodes() {
  for value in [
    "",
    " ",
    "192.0.2.60,",
    ", 192.0.2.60",
    "192.0.2.60,, 198.51.100.17",
    "client.example",
    "_hidden",
    "192.0.2.999",
    "[2001:db8::1",
    "192.0.2.60\r\nX-Injected: 1",
    "unknown\u{0}",
    "unknown\u{7f}",
  ] {
    assert!(
      XForwardedFor::parse(value).is_err(),
      "X-Forwarded-For should reject {value:?}"
    );
  }
}

#[test]
fn x_forwarded_for_enforces_node_and_size_bounds() {
  let too_many = (0..=MAX_X_FORWARDED_FOR_NODES)
    .map(|index| format!("192.0.2.{}", index % 255))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(XForwardedFor::parse(too_many).is_err());

  let padded = format!(
    "unknown{}",
    " ".repeat(MAX_X_FORWARDED_FOR_VALUE_BYTES - "unknown".len())
  );
  assert_eq!(MAX_X_FORWARDED_FOR_VALUE_BYTES, padded.len());
  assert!(XForwardedFor::parse(padded.as_str()).is_ok());
  assert!(XForwardedFor::parse_values([padded.as_str(), "unknown"]).is_err());
}
