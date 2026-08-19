use rttp_protocol::cdn_loop::{
  CdnLoop, MAX_CDN_LOOP_MEMBERS, MAX_CDN_LOOP_PARAMETERS, MAX_CDN_LOOP_VALUE_BYTES,
};

#[test]
fn cdn_loop_parses_rfc_8586_examples_and_combined_fields() {
  let cdn_loop = CdnLoop::parse_values([
    r#"foo123.foocdn.example, barcdn.example; trace="abcdef""#,
    r#"AnotherCDN; abc=123; def="456""#,
  ])
  .expect("RFC 8586 CDN-Loop examples should parse");

  assert_eq!(3, cdn_loop.len());
  assert_eq!("foo123.foocdn.example", cdn_loop.members()[0].identifier());
  assert!(cdn_loop.members()[0].parameters().is_empty());
  assert_eq!("barcdn.example", cdn_loop.members()[1].identifier());
  assert_eq!(Some("abcdef"), cdn_loop.members()[1].parameter("trace"));
  assert_eq!("AnotherCDN", cdn_loop.members()[2].identifier());
  assert_eq!(Some("123"), cdn_loop.members()[2].parameter("abc"));
  assert_eq!(Some("456"), cdn_loop.members()[2].parameter("def"));
  assert_eq!(
    "foo123.foocdn.example, barcdn.example; trace=abcdef, AnotherCDN; abc=123; def=456",
    cdn_loop.header_value()
  );
}

#[test]
fn cdn_loop_parses_host_port_ipv6_and_token_pseudonyms() {
  let cdn_loop = CdnLoop::parse("cdn.example:443, [2001:db8::1]:8443, foo^bar, AnotherCDN")
    .expect("host:port and pseudonym identifiers should parse");

  assert_eq!(4, cdn_loop.len());
  assert_eq!("cdn.example:443", cdn_loop.members()[0].identifier());
  assert_eq!("[2001:db8::1]:8443", cdn_loop.members()[1].identifier());
  assert_eq!("foo^bar", cdn_loop.members()[2].identifier());
  assert_eq!("AnotherCDN", cdn_loop.members()[3].identifier());
  assert_eq!(
    "cdn.example:443, [2001:db8::1]:8443, foo^bar, AnotherCDN",
    cdn_loop.header_value()
  );
}

#[test]
fn cdn_loop_preserves_identifier_spelling_and_parameter_order() {
  let cdn_loop = CdnLoop::parse("Edge.CDN; Alpha=1; beta=\"two words\"; Gamma=three")
    .expect("mixed-case identifiers and parameters should parse");

  assert_eq!("Edge.CDN", cdn_loop.members()[0].identifier());
  assert_eq!(3, cdn_loop.members()[0].parameters().len());
  assert_eq!("alpha", cdn_loop.members()[0].parameters()[0].name());
  assert_eq!("1", cdn_loop.members()[0].parameters()[0].value());
  assert_eq!("beta", cdn_loop.members()[0].parameters()[1].name());
  assert_eq!("two words", cdn_loop.members()[0].parameters()[1].value());
  assert_eq!("gamma", cdn_loop.members()[0].parameters()[2].name());
  assert_eq!(Some("three"), cdn_loop.members()[0].parameter("GAMMA"));
  assert_eq!(
    "Edge.CDN; alpha=1; beta=\"two words\"; gamma=three",
    cdn_loop.header_value()
  );
}

#[test]
fn cdn_loop_accepts_repeated_identifiers_and_repeated_fields() {
  let cdn_loop =
    CdnLoop::parse_values(["edge.example; hop=1", "edge.example, other.example; hop=2"])
      .expect("repeated CDN identifiers are valid loop metadata");

  assert_eq!(3, cdn_loop.len());
  assert_eq!("edge.example", cdn_loop.members()[0].identifier());
  assert_eq!("edge.example", cdn_loop.members()[1].identifier());
  assert_eq!("other.example", cdn_loop.members()[2].identifier());
  assert_eq!(
    "edge.example; hop=1, edge.example, other.example; hop=2",
    cdn_loop.header_value()
  );
}

#[test]
fn cdn_loop_rejects_malformed_identifiers_parameters_and_members() {
  for value in [
    "",
    " ",
    "not valid",
    "foo@bar",
    "foo/bar",
    "foo?bar",
    "foo:bar:baz",
    "foo;",
    "foo; trace",
    "foo; =1",
    "foo; trace=",
    "foo; trace=\"unterminated",
    "cdn,",
    ",cdn",
    "cdn,,other",
    "cdn; trace=1 extra",
    "cdn\r\nX-Injected: 1",
    "cdn\u{0}",
    "cdn\u{7f}",
    "foo\u{80}bar",
  ] {
    assert!(
      CdnLoop::parse(value).is_err(),
      "CDN-Loop should reject {value:?}"
    );
  }

  for (index, values) in [vec!["", "cdn"], vec!["cdn", ""], vec!["", ""]]
    .into_iter()
    .enumerate()
  {
    assert!(
      CdnLoop::parse_values(values).is_err(),
      "CDN-Loop should reject empty combined fields at index {index}"
    );
  }
}

#[test]
fn cdn_loop_rejects_duplicate_parameters_on_one_member() {
  for value in [
    "cdn; trace=1; TRACE=2",
    "cdn; a=1; a=2",
    "cdn; a=1; A=\"two\"",
  ] {
    assert!(
      CdnLoop::parse(value).is_err(),
      "CDN-Loop should reject duplicate parameters in {value:?}"
    );
  }
}

#[test]
fn cdn_loop_enforces_member_parameter_and_size_bounds() {
  let too_many = (0..=MAX_CDN_LOOP_MEMBERS)
    .map(|index| format!("cdn{index}.example"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(CdnLoop::parse(too_many).is_err());

  let too_many_parameters = (0..=MAX_CDN_LOOP_PARAMETERS)
    .map(|index| format!("p{index}=1"))
    .collect::<Vec<_>>()
    .join("; ");
  assert!(CdnLoop::parse(format!("cdn; {too_many_parameters}")).is_err());

  let oversized_field = format!("cdn; trace=\"{}\"", "a".repeat(MAX_CDN_LOOP_VALUE_BYTES));
  assert!(CdnLoop::parse(oversized_field).is_err());

  let first = "a".repeat(MAX_CDN_LOOP_VALUE_BYTES - 4);
  let combined = CdnLoop::parse_values([first.as_str(), "b".repeat(10).as_str()]);
  assert!(
    combined.is_err(),
    "combined CDN-Loop fields over the value bound should be rejected"
  );

  let padded = format!("cdn{}", " ".repeat(MAX_CDN_LOOP_VALUE_BYTES - 3));
  assert_eq!(padded.len(), MAX_CDN_LOOP_VALUE_BYTES);
  assert!(
    CdnLoop::parse(padded.as_str()).is_ok(),
    "one OWS-padded field at the value bound should parse"
  );
  assert!(
    CdnLoop::parse_values([padded.as_str(), padded.as_str()]).is_err(),
    "repeated OWS-padded CDN-Loop fields over the raw aggregate bound should be rejected"
  );
}
