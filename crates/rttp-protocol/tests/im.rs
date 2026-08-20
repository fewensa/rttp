use rttp_protocol::im::{
  Im, MAX_IM_MEMBERS, MAX_IM_PARAMETERS, MAX_IM_TOTAL_BYTES, MAX_IM_VALUE_BYTES,
};

#[test]
fn im_parses_ordered_tokens_and_parameters() {
  let im = Im::parse("diffe, gzip;profile=compact").expect("IM should parse");

  assert_eq!(2, im.len());
  assert!(!im.is_empty());
  assert_eq!("diffe", im.members()[0].token());
  assert!(im.members()[0].parameters().is_empty());
  assert_eq!("gzip", im.members()[1].token());
  assert_eq!("profile", im.members()[1].parameters()[0].name());
  assert_eq!(Some("compact"), im.members()[1].parameters()[0].value());
  assert_eq!(im.header_value(), "diffe, gzip;profile=compact");
}

#[test]
fn im_accepts_multiple_fields_in_wire_order() {
  let im = Im::parse_values(["diffe, gzip;profile=compact", "identity"])
    .expect("multiple IM fields should parse");

  assert_eq!(3, im.len());
  assert_eq!("diffe", im.members()[0].token());
  assert_eq!("gzip", im.members()[1].token());
  assert_eq!("identity", im.members()[2].token());
  assert_eq!(im.header_value(), "diffe, gzip;profile=compact, identity");
}

#[test]
fn im_preserves_quoted_parameter_values() {
  let im = Im::parse(r#"gzip;profile="compact, delta";note=x"#)
    .expect("quoted IM parameters should parse");

  assert_eq!(
    Some("compact, delta"),
    im.members()[0].parameters()[0].value()
  );
  assert_eq!(Some("x"), im.members()[0].parameters()[1].value());
  assert_eq!(im.header_value(), r#"gzip;profile="compact, delta";note=x"#);
}

#[test]
fn im_accepts_http_optional_whitespace_padding() {
  for value in ["\tdiffe\t", " diffe "] {
    let im = Im::parse(value).expect("OWS-padded IM should parse");
    assert_eq!(im.members()[0].token(), "diffe");
    assert_eq!(im.header_value(), "diffe");
  }

  let im =
    Im::parse(" diffe ,\tgzip; profile = compact ").expect("OWS-padded IM members should parse");
  assert_eq!(im.members()[0].token(), "diffe");
  assert_eq!(im.members()[1].token(), "gzip");
  assert_eq!(im.header_value(), "diffe, gzip;profile=compact");
}

#[test]
fn im_builds_lists_from_members() {
  let im =
    Im::from_members(["diffe", "gzip;profile=compact", "identity"]).expect("IM should build");

  assert_eq!(im.len(), 3);
  assert_eq!(im.members()[0].token(), "diffe");
  assert_eq!(im.members()[1].token(), "gzip");
  assert_eq!(Some("compact"), im.members()[1].parameters()[0].value());
  assert_eq!(im.header_value(), "diffe, gzip;profile=compact, identity");
}

#[test]
fn im_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    ",diffe",
    "diffe,",
    "diffe,,gzip",
    "bad token",
    "gzip;q=0.3",
    "gzip;q",
    "gzip;q=",
    r#"gzip;q="0.3""#,
    "gzip;Q=1",
    "gzip;profile=",
    "gzip;=",
    "gzip: diffe",
    "\u{0d}diffe",
    "diffe\r\nX: y",
    "diffe\u{7f}",
  ] {
    assert!(Im::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn im_rejects_duplicates_case_insensitively() {
  assert!(
    Im::parse("diffe, DIFFE").is_err(),
    "duplicate tokens in one field must be rejected"
  );
  assert!(
    Im::parse_values(["gzip", "GZIP;profile=compact"]).is_err(),
    "duplicate tokens across fields must be rejected"
  );
  assert!(
    Im::parse("gzip;profile=a;PROFILE=b").is_err(),
    "duplicate parameter names must be rejected"
  );
}

#[test]
fn im_rejects_empty_field_sets() {
  assert!(
    Im::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn im_enforces_value_member_parameter_and_total_bounds() {
  assert!(
    Im::parse("x".repeat(MAX_IM_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_IM_VALUE_BYTES);
  assert!(
    Im::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_IM_VALUE_BYTES + 1);
  assert!(
    Im::parse_values(["gzip", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let first = "a".repeat(MAX_IM_TOTAL_BYTES / 2 + 1);
  let second = "b".repeat(MAX_IM_TOTAL_BYTES / 2 + 1);
  assert!(
    first.len() + second.len() > MAX_IM_TOTAL_BYTES,
    "combined fields should exceed the total bound"
  );
  assert!(
    Im::parse_values([first.as_str(), second.as_str()]).is_err(),
    "combined oversized fields must be rejected"
  );

  let at_limit = (0..MAX_IM_MEMBERS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Im::parse(&at_limit).expect("32 members should parse");
  assert_eq!(parsed.len(), MAX_IM_MEMBERS);

  let too_many = (0..=MAX_IM_MEMBERS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Im::parse(&too_many).is_err(),
    "more than 32 members must be rejected"
  );

  let at_parameter_limit = format!(
    "gzip;{}",
    (0..MAX_IM_PARAMETERS)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(";")
  );
  let parsed = Im::parse(&at_parameter_limit).expect("16 parameters should parse");
  assert_eq!(parsed.members()[0].parameters().len(), MAX_IM_PARAMETERS);

  let too_many_parameters = format!(
    "gzip;{}",
    (0..=MAX_IM_PARAMETERS)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(";")
  );
  assert!(
    Im::parse(&too_many_parameters).is_err(),
    "more than 16 parameters must be rejected"
  );
}
