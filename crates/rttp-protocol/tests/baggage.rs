use rttp_protocol::baggage::{
  Baggage, MAX_BAGGAGE_MEMBERS, MAX_BAGGAGE_MEMBER_BYTES, MAX_BAGGAGE_VALUE_BYTES,
};

fn member_of_len(key: &str, len: usize) -> String {
  assert!(
    key.len() + 1 < len,
    "member length must leave room for '=' and a value octet"
  );
  format!("{key}={}", "v".repeat(len - key.len() - 1))
}

#[test]
fn baggage_parses_ordered_members_properties_and_combined_fields() {
  let baggage = Baggage::parse_values(["tenant=acme;source=gateway", " , release=2026-08-19"])
    .expect("baggage should parse");

  assert_eq!(2, baggage.members().len());
  assert_eq!("tenant", baggage.members()[0].key());
  assert_eq!("acme", baggage.members()[0].value());
  assert_eq!(1, baggage.members()[0].properties().len());
  assert_eq!("source", baggage.members()[0].properties()[0].key());
  assert_eq!(
    Some("gateway"),
    baggage.members()[0].properties()[0].value()
  );
  assert_eq!("release", baggage.members()[1].key());
  assert_eq!("2026-08-19", baggage.members()[1].value());
  assert_eq!(
    "tenant=acme;source=gateway,release=2026-08-19",
    baggage.header_value()
  );

  let empty = Baggage::parse("").expect("empty baggage should parse");
  assert!(empty.members().is_empty());
  assert_eq!("", empty.header_value());

  let flag = Baggage::parse("tenant=acme;private").expect("key-only property should parse");
  assert_eq!(None, flag.members()[0].properties()[0].value());
  assert_eq!("tenant=acme;private", flag.header_value());

  let encoded = Baggage::parse("userId=alice,serverNode=DF%3A28")
    .expect("percent-encoded baggage should parse without decoding");
  assert_eq!("DF%3A28", encoded.members()[1].value());
}

#[test]
fn baggage_accepts_ows_around_members_assignments_and_properties() {
  let baggage =
    Baggage::parse("\t tenant \t= acme ; source = gateway ; private\t, \trelease\t=\t2026-08-19\t")
      .expect("OWS around tokens, '=', ';', and ',' should parse");

  assert_eq!(2, baggage.members().len());
  assert_eq!("tenant", baggage.members()[0].key());
  assert_eq!("acme", baggage.members()[0].value());
  assert_eq!(2, baggage.members()[0].properties().len());
  assert_eq!("source", baggage.members()[0].properties()[0].key());
  assert_eq!(
    Some("gateway"),
    baggage.members()[0].properties()[0].value()
  );
  assert_eq!("private", baggage.members()[0].properties()[1].key());
  assert_eq!(None, baggage.members()[0].properties()[1].value());
  assert_eq!("release", baggage.members()[1].key());
  assert_eq!("2026-08-19", baggage.members()[1].value());
  assert_eq!(
    "tenant=acme;source=gateway;private,release=2026-08-19",
    baggage.header_value()
  );

  let empty_value = Baggage::parse(" tenant =\t ").expect("OWS-only empty value should parse");
  assert_eq!("tenant", empty_value.members()[0].key());
  assert_eq!("", empty_value.members()[0].value());
  assert_eq!("tenant=", empty_value.header_value());

  let empty_property =
    Baggage::parse("tenant=acme;source=").expect("empty property values are valid baggage-octets");
  assert_eq!(
    Some(""),
    empty_property.members()[0].properties()[0].value()
  );
  assert_eq!("tenant=acme;source=", empty_property.header_value());
}

#[test]
fn baggage_keeps_percent_encoded_values_opaque() {
  let baggage =
    Baggage::parse("userId=alice%20smith;note=a%2Cb%3Bc%3D%22d%22,node=DF%3A28,raw=%zz%")
      .expect("opaque percent data should parse without decoding");

  assert_eq!("alice%20smith", baggage.members()[0].value());
  assert_eq!(
    Some("a%2Cb%3Bc%3D%22d%22"),
    baggage.members()[0].properties()[0].value()
  );
  assert_eq!("DF%3A28", baggage.members()[1].value());
  assert_eq!("%zz%", baggage.members()[2].value());
  assert_eq!(
    "userId=alice%20smith;note=a%2Cb%3Bc%3D%22d%22,node=DF%3A28,raw=%zz%",
    baggage.header_value()
  );
  assert_ne!("alice smith", baggage.members()[0].value());
  assert_ne!("DF:28", baggage.members()[1].value());
}

#[test]
fn baggage_rejects_malformed_separators_assignments_quotes_and_controls() {
  for value in [
    "tenant=acme,tenant=other",
    "Tenant=acme,Tenant=other",
    "=acme",
    "tenant acme=1",
    "tenant=acme extra",
    "tenant=acme;=",
    "tenant=acme;=gateway",
    "tenant=acme;;source=gateway",
    "tenant=acme; ;source=gateway",
    "tenant=acme;",
    "tenant=value\u{7f}",
    "tenant=value\0",
    "tenant=value\r\nX: y",
    "tenant=value\n",
    "tenant=value\u{1f}",
    "tenant=value\u{80}",
    "tenant=\"quoted\"",
    r"tenant=back\slash",
    "\"tenant\"=acme",
    "tenant==acme extra",
    "tenant",
    "tenant acme",
    "tenant=acme,=other",
    "tenant=acme extra,release=1",
    "tenant=acme;source=gate way",
    "tenant=acme;source=gateway extra",
  ] {
    assert!(
      Baggage::parse(value).is_err(),
      "baggage should reject {value:?}"
    );
  }

  assert!(
    Baggage::parse("tenant=acme extra").is_err(),
    "interior SP is not a baggage-octet"
  );
  assert!(
    Baggage::parse("tenant=acme\textra").is_err(),
    "interior HTAB is not a baggage-octet"
  );
}

#[test]
fn baggage_rejects_duplicate_keys_across_combined_fields() {
  assert!(Baggage::parse("tenant=acme,tenant=other").is_err());
  assert!(Baggage::parse_values(["tenant=acme", "tenant=other"]).is_err());
  assert!(Baggage::parse_values(["tenant=acme", " tenant = other "]).is_err());

  let mixed_case =
    Baggage::parse("Tenant=acme,tenant=other").expect("baggage keys are case-sensitive tokens");
  assert_eq!(2, mixed_case.members().len());
  assert_eq!("Tenant", mixed_case.members()[0].key());
  assert_eq!("tenant", mixed_case.members()[1].key());
}

#[test]
fn baggage_enforces_exact_member_count_and_size_limits() {
  let at_member_count = (0..MAX_BAGGAGE_MEMBERS)
    .map(|index| format!("k{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  let at_count = Baggage::parse(&at_member_count).expect("180 members should parse");
  assert_eq!(MAX_BAGGAGE_MEMBERS, at_count.members().len());

  let too_many = (0..=MAX_BAGGAGE_MEMBERS)
    .map(|index| format!("k{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Baggage::parse(too_many).is_err(),
    "181 members must be rejected"
  );

  let at_member_limit = member_of_len("k", MAX_BAGGAGE_MEMBER_BYTES);
  assert_eq!(MAX_BAGGAGE_MEMBER_BYTES, at_member_limit.len());
  let parsed_member =
    Baggage::parse(&at_member_limit).expect("a 4096-byte list-member should parse");
  assert_eq!(1, parsed_member.members().len());
  assert_eq!(at_member_limit, parsed_member.members()[0].header_value());

  let oversized_member = format!("k={}", "v".repeat(MAX_BAGGAGE_MEMBER_BYTES));
  assert!(
    oversized_member.len() > MAX_BAGGAGE_MEMBER_BYTES,
    "over-limit fixture must exceed the per-member bound"
  );
  assert!(
    Baggage::parse(oversized_member).is_err(),
    "a list-member over 4096 bytes must be rejected"
  );

  let at_combined_limit = format!(
    "{},{}",
    member_of_len("a", MAX_BAGGAGE_MEMBER_BYTES),
    member_of_len("b", MAX_BAGGAGE_VALUE_BYTES - MAX_BAGGAGE_MEMBER_BYTES - 1)
  );
  assert_eq!(MAX_BAGGAGE_VALUE_BYTES, at_combined_limit.len());
  let parsed_combined =
    Baggage::parse(&at_combined_limit).expect("an 8192-byte combined value should parse");
  assert_eq!(2, parsed_combined.members().len());
  assert_eq!(at_combined_limit, parsed_combined.header_value());

  let oversized = format!("k={}", "v".repeat(MAX_BAGGAGE_VALUE_BYTES + 1));
  assert!(Baggage::parse(oversized).is_err());

  let over_combined = format!("{at_combined_limit},c=1");
  assert!(
    over_combined.len() > MAX_BAGGAGE_VALUE_BYTES,
    "over-limit fixture must exceed the combined bound"
  );
  assert!(
    Baggage::parse(over_combined).is_err(),
    "combined values over 8192 bytes must be rejected"
  );

  let first = member_of_len("a", MAX_BAGGAGE_MEMBER_BYTES);
  let second = member_of_len("b", MAX_BAGGAGE_VALUE_BYTES - MAX_BAGGAGE_MEMBER_BYTES - 1);
  assert!(Baggage::parse_values([first.as_str(), second.as_str()]).is_ok());
  assert!(Baggage::parse_values([first.as_str(), format!("{second},c=1").as_str()]).is_err());
}

#[test]
fn baggage_formats_canonical_output_and_round_trips() {
  let parsed = Baggage::parse(
    " tenant = acme ; source = gateway ; private , userId = alice%20smith , empty = ",
  )
  .expect("OWS-padded members should parse");
  assert_eq!(
    "tenant=acme;source=gateway;private,userId=alice%20smith,empty=",
    parsed.header_value()
  );

  let reparsed = Baggage::parse(parsed.header_value()).expect("canonical output should parse");
  assert_eq!(parsed, reparsed);
  assert_eq!(parsed.header_value(), reparsed.header_value());

  let encoded =
    Baggage::parse("node=DF%3A28;note=%2C%3B%3D").expect("opaque percent data should round-trip");
  assert_eq!("node=DF%3A28;note=%2C%3B%3D", encoded.header_value());
  assert_eq!(
    encoded,
    Baggage::parse(encoded.header_value()).expect("encoded canonical form should parse")
  );

  let empty = Baggage::parse("").expect("empty baggage should parse");
  assert_eq!("", empty.header_value());
  assert_eq!(
    empty,
    Baggage::parse(empty.header_value()).expect("empty canonical form should parse")
  );
}

#[test]
fn baggage_debug_and_errors_do_not_echo_member_values() {
  let baggage = Baggage::parse("tenant=acme-secret;source=gateway").expect("baggage should parse");
  let debug = format!(
    "{baggage:?} {:?} {:?}",
    baggage.members()[0],
    baggage.members()[0].properties()[0]
  );

  assert!(!debug.contains("acme-secret"));
  assert!(!debug.contains("gateway"));
  assert!(!Baggage::parse("tenant=secret,tenant=other")
    .expect_err("duplicate baggage should fail")
    .to_string()
    .contains("secret"));
  assert!(!Baggage::parse("tenant=secret value")
    .expect_err("invalid baggage value should fail")
    .to_string()
    .contains("secret"));
}
