use rttp_protocol::proxy_status::{
  ProxyStatus, ProxyStatusBareItem, ProxyStatusIdentifier, MAX_PROXY_STATUS_MEMBERS,
  MAX_PROXY_STATUS_PARAMETERS, MAX_PROXY_STATUS_VALUE_BYTES,
};

#[test]
fn proxy_status_parses_rfc9209_examples() {
  let example_cdn = ProxyStatus::parse("ExampleCDN; error=connection_timeout")
    .expect("RFC 9209 ExampleCDN value should parse");
  assert_eq!(example_cdn.len(), 1);
  assert!(!example_cdn.is_empty());
  assert_eq!(
    example_cdn.members()[0].identifier(),
    &ProxyStatusIdentifier::Token("ExampleCDN".to_string())
  );
  assert_eq!(
    example_cdn.members()[0]
      .parameter("error")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::Token(
      "connection_timeout".to_string()
    ))
  );
  assert_eq!(
    example_cdn.header_value(),
    "ExampleCDN;error=connection_timeout"
  );

  let next_hop = ProxyStatus::parse(r#"SomeReverseProxy; next-hop="2001:db8::1:8080""#)
    .expect("quoted next-hop should parse");
  assert_eq!(
    next_hop.members()[0]
      .parameter("next-hop")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::String("2001:db8::1:8080".to_string()))
  );

  let details = ProxyStatus::parse(
    r#"SomeCDN; error=http_protocol_error; details="Invalid Content-Length header: \"foobar\"""#,
  )
  .expect("quoted details should parse");
  assert_eq!(
    details.members()[0]
      .parameter("details")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::String(
      r#"Invalid Content-Length header: "foobar""#.to_string()
    ))
  );

  let list = ProxyStatus::parse("SomeProxy1, OtherProxy2; extra-param")
    .expect("multi-member Proxy-Status should parse");
  assert_eq!(list.len(), 2);
  assert_eq!(list.members()[0].identifier().as_str(), "SomeProxy1");
  assert!(list.members()[0].identifier().is_token());
  assert_eq!(
    list.members()[1]
      .parameter("extra-param")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::Boolean(true))
  );
}

#[test]
fn proxy_status_parses_string_identifiers_and_combined_fields() {
  let status = ProxyStatus::parse_values([
    r#"FooProxy; received-status=200; next-hop=SomeCDN"#,
    r#""cdn.example.net"; extra"#,
  ])
  .expect("combined Proxy-Status fields should parse");

  assert_eq!(status.len(), 2);
  assert_eq!(
    status.members()[0]
      .parameter("received-status")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::Integer(200))
  );
  assert_eq!(
    status.members()[0]
      .parameter("next-hop")
      .map(|parameter| parameter.value()),
    Some(&ProxyStatusBareItem::Token("SomeCDN".to_string()))
  );
  assert_eq!(
    status.members()[1].identifier(),
    &ProxyStatusIdentifier::String("cdn.example.net".to_string())
  );
  assert!(status.members()[1].identifier().is_string());
  assert_eq!(
    status.header_value(),
    r#"FooProxy;received-status=200;next-hop=SomeCDN, "cdn.example.net";extra"#
  );
  assert_eq!(
    status,
    ProxyStatus::parse(status.header_value()).expect("header_value should reparse")
  );
}

#[test]
fn proxy_status_rejects_empty_malformed_inner_list_and_control_bytes() {
  for value in [
    "",
    "   ",
    ",",
    "ExampleCDN,",
    "ExampleCDN;",
    "ExampleCDN;=",
    "(ExampleCDN)",
    "(a b)",
    "123",
    "?1",
    ":YWJj:",
    "ExampleCDN;Error=timeout",
    "ExampleCDN; extra=",
  ] {
    assert!(
      ProxyStatus::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(ProxyStatus::parse_values([]).is_err());
  assert!(ProxyStatus::parse("ExampleCDN;\x01error=timeout").is_err());
  assert!(ProxyStatus::parse("ExampleCDN;\x7ferror=timeout").is_err());
}

#[test]
fn proxy_status_rejects_duplicate_parameters() {
  assert!(ProxyStatus::parse("ExampleCDN; error=timeout; error=reset").is_err());
  assert!(ProxyStatus::parse(r#"SomeCDN; details="a"; details="b""#).is_err());
}

#[test]
fn proxy_status_rejects_oversized_values_and_excessive_members() {
  assert!(ProxyStatus::parse("x".repeat(MAX_PROXY_STATUS_VALUE_BYTES + 1)).is_err());
  assert!(ProxyStatus::parse(
    (0..=MAX_PROXY_STATUS_MEMBERS)
      .map(|index| format!("Proxy{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
  assert!(ProxyStatus::parse(format!(
    "ExampleCDN{}",
    (0..=MAX_PROXY_STATUS_PARAMETERS)
      .map(|index| format!(";p{index}"))
      .collect::<String>()
  ))
  .is_err());
}
