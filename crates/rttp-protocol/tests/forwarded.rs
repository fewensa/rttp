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
    "for=[2001:db8:cafe::17]",
    "host=example.test:8443",
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

#[test]
fn forwarded_accepts_http_ows_and_serializes_canonically() {
  for value in [
    " for=192.0.2.43;proto=https ",
    "\tfor=192.0.2.43;proto=https\t",
    "for = 192.0.2.43 ; proto = https",
    "for\t=\t192.0.2.43\t;\tproto\t=\thttps",
    " \tfor = 192.0.2.43\t;\tproto = https\t ",
  ] {
    let forwarded = Forwarded::parse(value)
      .unwrap_or_else(|error| panic!("{value:?} must accept HTTP OWS padding: {error}"));
    assert_eq!(1, forwarded.len(), "{value:?}");
    assert_eq!(Some("192.0.2.43"), forwarded.elements()[0].for_value());
    assert_eq!(Some("https"), forwarded.elements()[0].proto());
    assert_eq!("for=192.0.2.43; proto=https", forwarded.header_value());
  }

  for value in [
    " for=192.0.2.43 , for=198.51.100.17 ",
    "for=192.0.2.43,\tfor=198.51.100.17",
    "\tfor = 192.0.2.43\t,\tfor = 198.51.100.17\t",
  ] {
    let forwarded = Forwarded::parse(value)
      .unwrap_or_else(|error| panic!("{value:?} must accept HTTP OWS list padding: {error}"));
    assert_eq!(2, forwarded.len(), "{value:?}");
    assert_eq!(Some("192.0.2.43"), forwarded.elements()[0].for_value());
    assert_eq!(Some("198.51.100.17"), forwarded.elements()[1].for_value());
    assert_eq!(
      "for=192.0.2.43, for=198.51.100.17",
      forwarded.header_value()
    );
  }
}

#[test]
fn forwarded_rejects_non_ows_unicode_and_control_padding() {
  for value in [
    "\u{00a0}for=192.0.2.43",
    "for=192.0.2.43\u{00a0}",
    "for\u{00a0}=192.0.2.43",
    "for=\u{00a0}192.0.2.43",
    "for=192.0.2.43\u{00a0};proto=https",
    "for=192.0.2.43;\u{00a0}proto=https",
    "for=192.0.2.43\u{00a0},for=198.51.100.17",
    "for=192.0.2.43,\u{00a0}for=198.51.100.17",
    "\u{2003}for=192.0.2.43",
    "for=192.0.2.43\u{3000}",
    "for=192.0.2.43\u{0085}",
    "\u{000b}for=192.0.2.43",
    "for=192.0.2.43\u{000c}",
    "for=\u{000b}192.0.2.43",
    "for=192.0.2.43\u{000b},for=198.51.100.17",
    "for=192.0.2.43;\u{000c}proto=https",
    "for=\r192.0.2.43",
    "for=192.0.2.43\n;proto=https",
  ] {
    assert!(
      Forwarded::parse(value).is_err(),
      "Forwarded should reject non-OWS padding in {value:?}"
    );
  }
}

#[test]
fn forwarded_aggregates_elements_across_repeated_fields() {
  let forwarded = Forwarded::parse_values([
    "for=192.0.2.43;proto=https",
    r#"for="[2001:db8:cafe::17]:4711";by=_hidden"#,
  ])
  .expect("repeated Forwarded fields should parse as one list");

  assert_eq!(2, forwarded.len());
  assert_eq!(Some("192.0.2.43"), forwarded.elements()[0].for_value());
  assert_eq!(Some("https"), forwarded.elements()[0].proto());
  assert_eq!(
    Some("[2001:db8:cafe::17]:4711"),
    forwarded.elements()[1].for_value()
  );
  assert_eq!(Some("_hidden"), forwarded.elements()[1].by());
  assert_eq!(
    r#"for=192.0.2.43; proto=https, for="[2001:db8:cafe::17]:4711"; by=_hidden"#,
    forwarded.header_value()
  );

  let at_limit = (0..MAX_FORWARDED_ELEMENTS)
    .map(|index| format!("for={index}"))
    .collect::<Vec<_>>();
  let mid = MAX_FORWARDED_ELEMENTS / 2;
  let first = at_limit[..mid].join(", ");
  let second = at_limit[mid..].join(", ");
  let parsed = Forwarded::parse_values([first.as_str(), second.as_str()])
    .expect("element bound should apply across repeated fields");
  assert_eq!(MAX_FORWARDED_ELEMENTS, parsed.len());

  let too_many = (0..=MAX_FORWARDED_ELEMENTS)
    .map(|index| format!("for={index}"))
    .collect::<Vec<_>>();
  let too_many_mid = MAX_FORWARDED_ELEMENTS / 2;
  let too_many_first = too_many[..=too_many_mid].join(", ");
  let too_many_second = too_many[too_many_mid + 1..].join(", ");
  assert!(Forwarded::parse_values([too_many_first.as_str(), too_many_second.as_str()]).is_err());
}

#[test]
fn forwarded_parse_serialize_round_trips_quoted_nodes_and_obfuscated_ids() {
  for value in [
    r#"for="[2001:db8:cafe::17]:4711";by=_hidden;host="example.test:8443";proto=https"#,
    r#"For="[2001:db8:cafe::17]:4711""#,
    r#"for="_gazonk";by=unknown"#,
    r#"for="quoted\\value\"""#,
    "for=192.0.2.43;proto=https;by=203.0.113.43, for=198.51.100.17;proto=http",
    " FOR = 192.0.2.43 ; PROTO = https ",
  ] {
    let parsed =
      Forwarded::parse(value).unwrap_or_else(|error| panic!("{value:?} should parse: {error}"));
    let serialized = parsed.header_value();
    let round_trip = Forwarded::parse(&serialized)
      .unwrap_or_else(|error| panic!("serialized {serialized:?} should parse: {error}"));
    assert_eq!(parsed, round_trip, "{value:?}");
    assert_eq!(serialized, round_trip.header_value(), "{value:?}");
  }
}
