use rttp_protocol::x_forwarded_proto::{
  XForwardedProto, MAX_X_FORWARDED_PROTOS, MAX_X_FORWARDED_PROTO_VALUE_BYTES,
};

#[test]
fn x_forwarded_proto_parses_ordered_scheme_tokens_and_combined_fields() {
  let forwarded_proto = XForwardedProto::parse_values(["https, http", "web+demo-1.2"])
    .expect("X-Forwarded-Proto schemes should parse");

  assert_eq!(3, forwarded_proto.len());
  assert_eq!(["https", "http", "web+demo-1.2"], forwarded_proto.schemes());
  assert_eq!("https, http, web+demo-1.2", forwarded_proto.header_value());
}

#[test]
fn x_forwarded_proto_rejects_malformed_injected_and_empty_schemes() {
  for value in [
    "",
    " ",
    "https,",
    ", https",
    "https,, http",
    "1https",
    "ht/tps",
    "https://",
    "ht tps",
    "https\r\nX-Injected: 1",
    "https\u{0}",
    "https\u{7f}",
  ] {
    assert!(
      XForwardedProto::parse(value).is_err(),
      "X-Forwarded-Proto should reject {value:?}"
    );
  }
}

#[test]
fn x_forwarded_proto_enforces_member_and_size_bounds() {
  let too_many = (0..=MAX_X_FORWARDED_PROTOS)
    .map(|index| format!("s{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(XForwardedProto::parse(too_many).is_err());

  let padded = format!(
    "https{}",
    " ".repeat(MAX_X_FORWARDED_PROTO_VALUE_BYTES - "https".len())
  );
  assert_eq!(MAX_X_FORWARDED_PROTO_VALUE_BYTES, padded.len());
  assert!(XForwardedProto::parse(padded.as_str()).is_ok());
  assert!(XForwardedProto::parse_values([padded.as_str(), "http"]).is_err());
}
