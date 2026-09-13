use rttp_protocol::forwarded::{
  Forwarded, MAX_FORWARDED_ELEMENTS, MAX_FORWARDED_PARAMETERS, MAX_FORWARDED_VALUE_BYTES,
};

#[test]
fn forwarded_parses_rfc_examples_and_exposes_parameters() {
  let forwarded = Forwarded::parse(
    "for=192.0.2.43;proto=https;by=203.0.113.43, \
     for=198.51.100.17;proto=http",
  )
  .expect("RFC 7239 examples should parse");

  assert_eq!(2, forwarded.len());
  assert_eq!(Some("192.0.2.43"), forwarded.elements()[0].for_value());
  assert_eq!(Some("https"), forwarded.elements()[0].proto());
  assert_eq!(Some("203.0.113.43"), forwarded.elements()[0].by());
  assert_eq!(Some("198.51.100.17"), forwarded.elements()[1].for_value());
  assert_eq!(Some("http"), forwarded.elements()[1].proto());
  assert_eq!(
    "for=192.0.2.43; proto=https; by=203.0.113.43, \
     for=198.51.100.17; proto=http",
    forwarded.header_value()
  );
}

#[test]
fn forwarded_parses_quoted_escaping_ipv6_and_obfuscated_identifiers() {
  let forwarded = Forwarded::parse(
    r#"for="[2001:db8:cafe::17]:4711";by=_hidden;host="example.test:8443";proto=https"#,
  )
  .expect("RFC 7239 node forms should parse");
  assert_eq!(
    Some("[2001:db8:cafe::17]:4711"),
    forwarded.elements()[0].for_value()
  );
  assert_eq!(Some("_hidden"), forwarded.elements()[0].by());
  assert_eq!(Some("example.test:8443"), forwarded.elements()[0].host());
  assert_eq!(Some("https"), forwarded.elements()[0].proto());

  let escaped =
    Forwarded::parse(r#"for="quoted\\value\"""#).expect("quoted-pair escapes should be unescaped");
  assert_eq!(Some(r#"quoted\value""#), escaped.elements()[0].for_value());
  assert_eq!(r#"for="quoted\\value\"""#, escaped.header_value());
}

#[test]
fn forwarded_rejects_duplicates_malformed_syntax_and_controls() {
  for value in [
    "for=192.0.2.1;FOR=198.51.100.1",
    "for=192.0.2.1,",
    ", for=192.0.2.1",
    "for=192.0.2.1,,for=198.51.100.1",
    "for=\"unterminated",
    "for=\"bad\\",
    "for=192.0.2.1;proto",
    "for=192.0.2.1;proto=https,",
    "for=192.0.2.1\r\nX-Injected: 1",
    "for=\"bad\u{0}\"",
    "for=\"bad\u{1f}\"",
    "for=\"bad\u{7f}\"",
  ] {
    assert!(
      Forwarded::parse(value).is_err(),
      "Forwarded should reject {value:?}"
    );
  }
}

#[test]
fn forwarded_enforces_element_parameter_and_value_bounds() {
  let too_many_elements = (0..=MAX_FORWARDED_ELEMENTS)
    .map(|index| format!("for={index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Forwarded::parse(too_many_elements).is_err());

  let too_many_parameters = (0..=MAX_FORWARDED_PARAMETERS)
    .map(|index| format!("p{index}=value"))
    .collect::<Vec<_>>()
    .join(";");
  assert!(Forwarded::parse(too_many_parameters).is_err());

  let oversized = format!("for={}", "a".repeat(MAX_FORWARDED_VALUE_BYTES));
  assert!(Forwarded::parse(oversized).is_err());
}

#[test]
fn forwarded_checks_aggregate_and_serialized_size_bounds() {
  let first = format!("for={}", "a".repeat(MAX_FORWARDED_VALUE_BYTES - 4));
  assert_eq!(MAX_FORWARDED_VALUE_BYTES, first.len());
  assert!(Forwarded::parse_values([first.as_str(), "for=b"]).is_err());

  let at_limit = format!("for={}", "a".repeat(MAX_FORWARDED_VALUE_BYTES - 4));
  assert_eq!(MAX_FORWARDED_VALUE_BYTES, at_limit.len());
  let parsed = Forwarded::parse(at_limit.as_str()).expect("serialized limit should be accepted");
  assert_eq!(MAX_FORWARDED_VALUE_BYTES, parsed.header_value().len());
}
