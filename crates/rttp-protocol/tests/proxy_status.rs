use rttp_protocol::proxy_status::{
  ProxyStatus, ProxyStatusBareItem, ProxyStatusIdentifier, MAX_PROXY_STATUS_MEMBERS,
  MAX_PROXY_STATUS_PARAMETERS, MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES,
  MAX_PROXY_STATUS_VALUE_BYTES,
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
fn proxy_status_parses_all_supported_parameter_bare_items() {
  let status = ProxyStatus::parse(
    r#"Proxy; flag; disabled=?0; integer=-42; decimal=1.230; date=@1700000000; bytes=:YWJj:; text="hello"; token=value; display=%"hello%20world%21""#,
  )
  .expect("all RFC 9209 bare item forms should parse");

  assert_eq!(
    status.members()[0].parameter("flag").unwrap().value(),
    &ProxyStatusBareItem::Boolean(true)
  );
  assert_eq!(
    status.members()[0].parameter("disabled").unwrap().value(),
    &ProxyStatusBareItem::Boolean(false)
  );
  assert_eq!(
    status.members()[0].parameter("integer").unwrap().value(),
    &ProxyStatusBareItem::Integer(-42)
  );
  assert_eq!(
    status.members()[0].parameter("decimal").unwrap().value(),
    &ProxyStatusBareItem::Decimal("1.23".to_string())
  );
  assert_eq!(
    status.members()[0].parameter("date").unwrap().value(),
    &ProxyStatusBareItem::Date(1_700_000_000)
  );
  assert_eq!(
    status.members()[0].parameter("bytes").unwrap().value(),
    &ProxyStatusBareItem::ByteSequence(b"abc".to_vec())
  );
  assert_eq!(
    status.members()[0].parameter("text").unwrap().value(),
    &ProxyStatusBareItem::String("hello".to_string())
  );
  assert_eq!(
    status.members()[0].parameter("token").unwrap().value(),
    &ProxyStatusBareItem::Token("value".to_string())
  );
  assert_eq!(
    status.members()[0].parameter("display").unwrap().value(),
    &ProxyStatusBareItem::DisplayString("hello world!".to_string())
  );
  assert_eq!(
    status.header_value(),
    r#"Proxy;flag;disabled=?0;integer=-42;decimal=1.23;date=@1700000000;bytes=:YWJj:;text="hello";token=value;display=%"hello world!""#
  );
  assert_eq!(status, ProxyStatus::parse(status.header_value()).unwrap());
}

#[test]
fn proxy_status_rejects_duplicate_parameters_and_malformed_bare_items() {
  assert!(ProxyStatus::parse("ExampleCDN; error=timeout; error=reset").is_err());
  assert!(ProxyStatus::parse(r#"SomeCDN; details="a"; details="b""#).is_err());

  for value in [
    "Proxy; bytes=:not-base64?:",
    "Proxy; bytes=:YWJj",
    "Proxy; date=@1.0",
    "Proxy; date=@",
    "Proxy; display=%\"bad%gg\"",
    "Proxy; display=%\"unterminated",
  ] {
    assert!(
      ProxyStatus::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn proxy_status_rejects_combined_member_and_parameter_value_limits() {
  let fields = (0..MAX_PROXY_STATUS_MEMBERS)
    .map(|index| format!("Proxy{index}"))
    .collect::<Vec<_>>();
  assert_eq!(
    ProxyStatus::parse_values(fields.iter().map(String::as_str))
      .unwrap()
      .len(),
    MAX_PROXY_STATUS_MEMBERS
  );
  let too_many_fields = (0..=MAX_PROXY_STATUS_MEMBERS)
    .map(|index| format!("Proxy{index}"))
    .collect::<Vec<_>>();
  assert!(ProxyStatus::parse_values(too_many_fields.iter().map(String::as_str)).is_err());

  // Equal constants mean any single-field payload with a parameter value above the
  // parameter bound also exceeds the field bound, so public parse rejects at the
  // field-size check. Direct MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES coverage lives
  // in the crate unit tests that call parse_field below that gate.
  assert_eq!(
    MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES, MAX_PROXY_STATUS_VALUE_BYTES,
    "parameter-value bound equals field bound; field size is the effective public limit"
  );
  let oversized_parameter = format!(
    "Proxy;value={}",
    "a".repeat(MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    oversized_parameter.len() > MAX_PROXY_STATUS_VALUE_BYTES,
    "token parameter payloads above the equal bound exceed the field size first"
  );
  assert_eq!(
    ProxyStatus::parse(&oversized_parameter)
      .expect_err("field-oversized token parameter payloads must be rejected")
      .to_string(),
    "Proxy-Status header value is too large"
  );
  let oversized_quoted_parameter = format!(
    "Proxy;value=\"{}\"",
    "a".repeat(MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    oversized_quoted_parameter.len() > MAX_PROXY_STATUS_VALUE_BYTES,
    "quoted parameter payloads above the equal bound exceed the field size first"
  );
  assert_eq!(
    ProxyStatus::parse(&oversized_quoted_parameter)
      .expect_err("field-oversized quoted parameter payloads must be rejected")
      .to_string(),
    "Proxy-Status header value is too large"
  );
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
