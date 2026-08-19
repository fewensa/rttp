use rttp_protocol::accept_charset::{
  AcceptCharset, MAX_ACCEPT_CHARSETS, MAX_ACCEPT_CHARSET_VALUE_BYTES,
};

#[test]
fn accept_charset_parses_ranges_and_quality_values() {
  let charsets =
    AcceptCharset::parse("utf-8, iso-8859-1;q=0.5, *;q=0").expect("Accept-Charset should parse");

  assert_eq!(3, charsets.len());
  assert_eq!("utf-8", charsets.charsets()[0].charset());
  assert_eq!(1000, charsets.charsets()[0].quality());
  assert!(!charsets.charsets()[0].is_wildcard());
  assert_eq!("iso-8859-1", charsets.charsets()[1].charset());
  assert_eq!(500, charsets.charsets()[1].quality());
  assert_eq!("*", charsets.charsets()[2].charset());
  assert_eq!(0, charsets.charsets()[2].quality());
  assert!(charsets.charsets()[2].is_wildcard());
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5, *;q=0");
}

#[test]
fn accept_charset_round_trips_client_locked_header_value() {
  let charsets = AcceptCharset::parse("utf-8, iso-8859-1;q=0.5, *;q=0")
    .expect("client-locked Accept-Charset should parse");

  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5, *;q=0");
  let round_trip =
    AcceptCharset::parse(charsets.header_value()).expect("formatted Accept-Charset should parse");
  assert_eq!(round_trip.header_value(), charsets.header_value());
}

#[test]
fn accept_charset_accepts_multiple_fields_in_wire_order() {
  let charsets = AcceptCharset::parse_values(["utf-8, iso-8859-1;q=0.5", "*; q=0"])
    .expect("multiple Accept-Charset fields should parse");

  assert_eq!(3, charsets.len());
  assert_eq!("utf-8", charsets.charsets()[0].charset());
  assert_eq!(1000, charsets.charsets()[0].quality());
  assert_eq!("iso-8859-1", charsets.charsets()[1].charset());
  assert_eq!(500, charsets.charsets()[1].quality());
  assert_eq!("*", charsets.charsets()[2].charset());
  assert_eq!(0, charsets.charsets()[2].quality());
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5, *;q=0");
}

#[test]
fn accept_charset_preserves_explicit_q_text_including_one() {
  let charsets = AcceptCharset::parse("utf-8;q=1, iso-8859-1;q=1.000, *;q=0.80")
    .expect("explicit q-values should parse");

  assert_eq!(1000, charsets.charsets()[0].quality());
  assert_eq!(1000, charsets.charsets()[1].quality());
  assert_eq!(800, charsets.charsets()[2].quality());
  assert_eq!(
    charsets.header_value(),
    "utf-8;q=1, iso-8859-1;q=1.000, *;q=0.80"
  );
}

#[test]
fn accept_charset_accepts_wildcard_with_quality() {
  let charsets =
    AcceptCharset::parse("utf-8, *;q=0").expect("wildcard Accept-Charset should parse");

  assert_eq!("utf-8", charsets.charsets()[0].charset());
  assert!(!charsets.charsets()[0].is_wildcard());
  assert_eq!("*", charsets.charsets()[1].charset());
  assert!(charsets.charsets()[1].is_wildcard());
  assert_eq!(0, charsets.charsets()[1].quality());
  assert_eq!(charsets.header_value(), "utf-8, *;q=0");
}

#[test]
fn accept_charset_accepts_http_optional_whitespace_padding() {
  for value in ["\tutf-8\t", " utf-8 "] {
    let charsets = AcceptCharset::parse(value).expect("OWS-padded Accept-Charset should parse");
    assert_eq!(charsets.charsets()[0].charset(), "utf-8");
    assert_eq!(charsets.header_value(), "utf-8");
  }

  let charsets = AcceptCharset::parse(" utf-8 ,\tiso-8859-1; q=0.5 ")
    .expect("OWS-padded Accept-Charset members should parse");
  assert_eq!(charsets.charsets()[0].charset(), "utf-8");
  assert_eq!(charsets.charsets()[1].charset(), "iso-8859-1");
  assert_eq!(500, charsets.charsets()[1].quality());
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5");
}

#[test]
fn accept_charset_builds_default_quality_lists() {
  let charsets = AcceptCharset::from_charsets(["utf-8", "iso-8859-1", "*"])
    .expect("Accept-Charset should build");

  assert_eq!(charsets.len(), 3);
  assert_eq!(charsets.charsets()[0].charset(), "utf-8");
  assert_eq!(charsets.charsets()[0].quality(), 1000);
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1, *");
}

#[test]
fn accept_charset_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    ",utf-8",
    "utf-8,",
    "utf-8,,iso-8859-1",
    "utf 8",
    "utf-8;q=1.1",
    "utf-8;q=1.001",
    "utf-8;q=0.1234",
    "utf-8;q=-0",
    "utf-8;level=1",
    "utf-8;q=0.5;level=1",
    "utf-8: iso-8859-1",
    "\u{0d}utf-8",
    "utf-8\r\nX: y",
    "utf-8\u{7f}",
  ] {
    assert!(
      AcceptCharset::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_charset_rejects_duplicates_case_insensitively() {
  assert!(
    AcceptCharset::parse("utf-8, UTF-8").is_err(),
    "duplicate charsets in one field must be rejected"
  );
  assert!(
    AcceptCharset::parse_values(["utf-8", "UTF-8;q=0.5"]).is_err(),
    "duplicate charsets across fields must be rejected"
  );
  assert!(
    AcceptCharset::parse("*, *;q=0").is_err(),
    "duplicate wildcards must be rejected"
  );
}

#[test]
fn accept_charset_rejects_empty_field_sets() {
  assert!(
    AcceptCharset::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn accept_charset_enforces_value_and_member_bounds() {
  assert!(
    AcceptCharset::parse("x".repeat(MAX_ACCEPT_CHARSET_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_ACCEPT_CHARSET_VALUE_BYTES);
  assert!(
    AcceptCharset::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_ACCEPT_CHARSET_VALUE_BYTES + 1);
  assert!(
    AcceptCharset::parse_values(["utf-8", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_ACCEPT_CHARSETS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = AcceptCharset::parse(&at_limit).expect("32 charsets should parse");
  assert_eq!(parsed.len(), MAX_ACCEPT_CHARSETS);

  let too_many = (0..=MAX_ACCEPT_CHARSETS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    AcceptCharset::parse(&too_many).is_err(),
    "more than 32 charsets must be rejected"
  );
}
