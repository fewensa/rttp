use rttp_protocol::baggage::{
  Baggage, MAX_BAGGAGE_MEMBERS, MAX_BAGGAGE_MEMBER_BYTES, MAX_BAGGAGE_VALUE_BYTES,
};

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

  let flag = Baggage::parse("tenant=acme;private").expect("key-only property should parse");
  assert_eq!(None, flag.members()[0].properties()[0].value());
  assert_eq!("tenant=acme;private", flag.header_value());

  let encoded = Baggage::parse("userId=alice,serverNode=DF%3A28")
    .expect("percent-encoded baggage should parse without decoding");
  assert_eq!("DF%3A28", encoded.members()[1].value());
}

#[test]
fn baggage_rejects_duplicates_invalid_members_count_and_size_bounds() {
  for value in [
    "tenant=acme,tenant=other",
    "=acme",
    "tenant acme=1",
    "tenant=acme extra",
    "tenant=acme;=",
    "tenant=value\u{7f}",
    "tenant=\"quoted\"",
    "tenant=acme;;source=gateway",
  ] {
    assert!(
      Baggage::parse(value).is_err(),
      "baggage should reject {value:?}"
    );
  }

  let too_many = (0..=MAX_BAGGAGE_MEMBERS)
    .map(|index| format!("k{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(Baggage::parse(too_many).is_err());

  let oversized = format!("k={}", "v".repeat(MAX_BAGGAGE_VALUE_BYTES + 1));
  assert!(Baggage::parse(oversized).is_err());

  let oversized_member = format!("k={}", "v".repeat(MAX_BAGGAGE_MEMBER_BYTES));
  assert!(Baggage::parse(oversized_member).is_err());
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
