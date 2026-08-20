use rttp_protocol::a_im::{
  AIm, MAX_A_IM_MEMBERS, MAX_A_IM_PARAMETERS, MAX_A_IM_TOTAL_BYTES, MAX_A_IM_VALUE_BYTES,
};

#[test]
fn a_im_parses_ordered_tokens_q_values_and_parameters() {
  let a_im = AIm::parse("diffe, gzip;q=0.3;profile=compact").expect("A-IM should parse");

  assert_eq!(2, a_im.len());
  assert!(!a_im.is_empty());
  assert_eq!("diffe", a_im.members()[0].token());
  assert_eq!(1000, a_im.members()[0].quality());
  assert!(a_im.members()[0].parameters().is_empty());
  assert_eq!("gzip", a_im.members()[1].token());
  assert_eq!(300, a_im.members()[1].quality());
  assert_eq!("q", a_im.members()[1].parameters()[0].name());
  assert_eq!(Some("0.3"), a_im.members()[1].parameters()[0].value());
  assert_eq!("profile", a_im.members()[1].parameters()[1].name());
  assert_eq!(Some("compact"), a_im.members()[1].parameters()[1].value());
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3;profile=compact");
}

#[test]
fn a_im_round_trips_explicit_q_text() {
  let a_im = AIm::parse("gzip;q=1, br;q=1.000, identity;q=0.80")
    .expect("explicit A-IM q-values should parse");

  assert_eq!(1000, a_im.members()[0].quality());
  assert_eq!(1000, a_im.members()[1].quality());
  assert_eq!(800, a_im.members()[2].quality());
  assert_eq!(a_im.header_value(), "gzip;q=1, br;q=1.000, identity;q=0.80");
  let round_trip = AIm::parse(a_im.header_value()).expect("formatted A-IM should parse");
  assert_eq!(round_trip.header_value(), a_im.header_value());
}

#[test]
fn a_im_accepts_multiple_fields_in_wire_order() {
  let a_im = AIm::parse_values(["diffe, gzip;q=0.3", "identity; q=0"])
    .expect("multiple A-IM fields should parse");

  assert_eq!(3, a_im.len());
  assert_eq!("diffe", a_im.members()[0].token());
  assert_eq!("gzip", a_im.members()[1].token());
  assert_eq!(300, a_im.members()[1].quality());
  assert_eq!("identity", a_im.members()[2].token());
  assert_eq!(0, a_im.members()[2].quality());
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3, identity;q=0");
}

#[test]
fn a_im_preserves_quoted_parameter_values() {
  let a_im = AIm::parse(r#"gzip;profile="compact, delta";note=x"#)
    .expect("quoted A-IM parameters should parse");

  assert_eq!(
    Some("compact, delta"),
    a_im.members()[0].parameters()[0].value()
  );
  assert_eq!(Some("x"), a_im.members()[0].parameters()[1].value());
  assert_eq!(
    a_im.header_value(),
    r#"gzip;profile="compact, delta";note=x"#
  );
}

#[test]
fn a_im_accepts_http_optional_whitespace_padding() {
  for value in ["\tdiffe\t", " diffe "] {
    let a_im = AIm::parse(value).expect("OWS-padded A-IM should parse");
    assert_eq!(a_im.members()[0].token(), "diffe");
    assert_eq!(a_im.header_value(), "diffe");
  }

  let a_im = AIm::parse(" diffe ,\tgzip; q=0.3 ; profile = compact ")
    .expect("OWS-padded A-IM members should parse");
  assert_eq!(a_im.members()[0].token(), "diffe");
  assert_eq!(a_im.members()[1].token(), "gzip");
  assert_eq!(300, a_im.members()[1].quality());
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3;profile=compact");
}

#[test]
fn a_im_builds_default_quality_lists() {
  let a_im = AIm::from_members(["diffe", "gzip", "identity"]).expect("A-IM should build");

  assert_eq!(a_im.len(), 3);
  assert_eq!(a_im.members()[0].token(), "diffe");
  assert_eq!(a_im.members()[0].quality(), 1000);
  assert_eq!(a_im.header_value(), "diffe, gzip, identity");
}

#[test]
fn a_im_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    ",diffe",
    "diffe,",
    "diffe,,gzip",
    "bad token",
    "gzip;q=1.1",
    "gzip;q=1.0000",
    "gzip;q=-0",
    "gzip;q=",
    "gzip;q",
    r#"gzip;q="0.3""#,
    "gzip;profile=",
    "gzip;=",
    "gzip: diffe",
    "\u{0d}diffe",
    "diffe\r\nX: y",
    "diffe\u{7f}",
  ] {
    assert!(AIm::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn a_im_rejects_duplicates_case_insensitively() {
  assert!(
    AIm::parse("diffe, DIFFE").is_err(),
    "duplicate tokens in one field must be rejected"
  );
  assert!(
    AIm::parse_values(["gzip", "GZIP;q=0.5"]).is_err(),
    "duplicate tokens across fields must be rejected"
  );
  assert!(
    AIm::parse("gzip;profile=a;PROFILE=b").is_err(),
    "duplicate parameter names must be rejected"
  );
}

#[test]
fn a_im_rejects_empty_field_sets() {
  assert!(
    AIm::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn a_im_enforces_value_member_parameter_and_total_bounds() {
  assert!(
    AIm::parse("x".repeat(MAX_A_IM_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_A_IM_VALUE_BYTES);
  assert!(
    AIm::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_A_IM_VALUE_BYTES + 1);
  assert!(
    AIm::parse_values(["gzip", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let first = "a".repeat(MAX_A_IM_TOTAL_BYTES / 2 + 1);
  let second = "b".repeat(MAX_A_IM_TOTAL_BYTES / 2 + 1);
  assert!(
    first.len() + second.len() > MAX_A_IM_TOTAL_BYTES,
    "combined fields should exceed the total bound"
  );
  assert!(
    AIm::parse_values([first.as_str(), second.as_str()]).is_err(),
    "combined oversized fields must be rejected"
  );

  let at_limit = (0..MAX_A_IM_MEMBERS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = AIm::parse(&at_limit).expect("32 members should parse");
  assert_eq!(parsed.len(), MAX_A_IM_MEMBERS);

  let too_many = (0..=MAX_A_IM_MEMBERS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    AIm::parse(&too_many).is_err(),
    "more than 32 members must be rejected"
  );

  let at_parameter_limit = format!(
    "gzip;{}",
    (0..MAX_A_IM_PARAMETERS)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(";")
  );
  let parsed = AIm::parse(&at_parameter_limit).expect("16 parameters should parse");
  assert_eq!(parsed.members()[0].parameters().len(), MAX_A_IM_PARAMETERS);

  let too_many_parameters = format!(
    "gzip;{}",
    (0..=MAX_A_IM_PARAMETERS)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(";")
  );
  assert!(
    AIm::parse(&too_many_parameters).is_err(),
    "more than 16 parameters must be rejected"
  );
}
