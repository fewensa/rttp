use rttp_protocol::alternates::{
  AlternateAttribute, AlternateVariant, Alternates, AlternatesParseError,
  MAX_ALTERNATES_AGGREGATE_VALUE_BYTES, MAX_ALTERNATES_ATTRIBUTES,
  MAX_ALTERNATES_ATTRIBUTE_VALUE_BYTES, MAX_ALTERNATES_URI_BYTES, MAX_ALTERNATES_VALUE_BYTES,
  MAX_ALTERNATES_VARIANTS,
};

const ILLUSTRATIVE: &str = concat!(
  r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }, "#,
  r#"{ "/resource.fr.html" 0.8 {type "text/html; charset=utf-8"} {language fr} }"#
);

#[test]
fn alternates_parses_multi_field_variants_and_standard_attributes() {
  let alternates = Alternates::parse_values([
    r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }"#,
    r#"{ "/resource.fr.html" 0.8 {type "text/html; charset=utf-8"} {language fr} }"#,
  ])
  .expect("multiple Alternates fields should parse");

  assert_eq!(2, alternates.len());
  assert!(!alternates.is_empty());
  assert_eq!("/resource.en.html", alternates.variants()[0].uri());
  assert_eq!("1.0", alternates.variants()[0].quality());
  assert_eq!(
    Some("text/html"),
    alternates.variants()[0].attribute("type")
  );
  assert_eq!(Some("en"), alternates.variants()[0].attribute("LANGUAGE"));
  assert_eq!(Some("1234"), alternates.variants()[0].attribute("length"));
  assert_eq!(
    "text/html; charset=utf-8",
    alternates.variants()[1].attribute("type").expect("type")
  );
  assert_eq!(Some("fr"), alternates.variants()[1].attribute("language"));
  assert_eq!(
    vec![
      ("type", "text/html"),
      ("language", "en"),
      ("length", "1234")
    ],
    alternates.variants()[0]
      .attributes()
      .iter()
      .map(|attribute: &AlternateAttribute| (attribute.name(), attribute.value()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn alternates_round_trips_illustrative_value_and_keeps_qvalue_text() {
  let alternates = Alternates::parse(ILLUSTRATIVE).expect("illustrative Alternates should parse");
  assert_eq!(
    concat!(
      r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }, "#,
      r#"{ "/resource.fr.html" 0.8 {type "text/html; charset=utf-8"} {language fr} }"#
    ),
    alternates.header_value()
  );
  assert_eq!(
    alternates,
    Alternates::parse(alternates.header_value()).expect("round-tripped Alternates should parse")
  );
}

#[test]
fn alternates_accepts_unquoted_parameterized_type_attribute() {
  let alternates =
    Alternates::parse(r#"{ "/resource.html" 1 {type text/html;charset=utf-8;profile=compact} }"#)
      .expect("unquoted parameterized media type should parse");

  assert_eq!(
    Some("text/html;charset=utf-8;profile=compact"),
    alternates.variants()[0].attribute("type")
  );
  assert_eq!(
    r#"{ "/resource.html" 1 {type text/html;charset=utf-8;profile=compact} }"#,
    alternates.header_value()
  );
}

#[test]
fn alternates_accepts_qvalue_grammar_and_rejects_invalid_weights() {
  for quality in [
    "0", "1", "0.", "1.", "0.5", "0.80", "0.123", "1.0", "1.00", "1.000",
  ] {
    let value = format!(r#"{{ "/resource" {quality} }}"#);
    let alternates = Alternates::parse(&value).unwrap_or_else(|_| panic!("{quality} should parse"));
    assert_eq!(quality, alternates.variants()[0].quality());
    assert_eq!(value, alternates.header_value());
  }

  for quality in ["1.001", "1.1", "0.1234", "2", "01", ".5", "1.0000", "q"] {
    let value = format!(r#"{{ "/resource" {quality} }}"#);
    assert!(
      Alternates::parse(&value).is_err(),
      "{quality} must be rejected"
    );
  }
}

#[test]
fn alternates_unescapes_quoted_attribute_values_and_reescapes_on_format() {
  let alternates = Alternates::parse(r#"{ "/ok" 1 {note "say \"hi\" and \\"} {encoding gzip} }"#)
    .expect("quoted attribute escapes should parse");

  assert_eq!(
    Some(r#"say "hi" and \"#),
    alternates.variants()[0].attribute("note")
  );
  assert_eq!(Some("gzip"), alternates.variants()[0].attribute("encoding"));
  assert_eq!(
    r#"{ "/ok" 1 {note "say \"hi\" and \\"} {encoding gzip} }"#,
    alternates.header_value()
  );
}

#[test]
fn alternates_escaped_non_ascii_quoted_pairs_do_not_panic() {
  let invalid_uri = std::panic::catch_unwind(|| Alternates::parse(r#"{ "/\é" 1 }"#))
    .expect("escaped non-ASCII quoted-pair in URI must not panic");
  assert!(invalid_uri.is_err());

  let attribute = std::panic::catch_unwind(|| Alternates::parse(r#"{ "/ok" 1 {note "\é"} }"#))
    .expect("escaped non-ASCII quoted-pair in attribute must not panic")
    .expect("escaped non-ASCII attribute value should parse");
  assert_eq!(Some("é"), attribute.variants()[0].attribute("note"));
}

#[test]
fn alternates_keeps_uris_as_raw_unresolved_text() {
  let alternates = Alternates::parse_values([
    r#"{ "HTTPS://EXAMPLE.TEST:443/A%2fB" 1 }"#,
    r#"{ "../images/logo.png?size=small#v1" 0.5 }"#,
    "{ \"#fragment\" 0 }",
  ])
  .expect("URI-references should parse");

  assert_eq!(
    "HTTPS://EXAMPLE.TEST:443/A%2fB",
    alternates.variants()[0].uri()
  );
  assert_eq!(
    "../images/logo.png?size=small#v1",
    alternates.variants()[1].uri()
  );
  assert_eq!("#fragment", alternates.variants()[2].uri());
}

#[test]
fn alternates_accepts_extension_attributes_and_lowercases_names() {
  let alternates = Alternates::parse(r#"{ "/resource" 1 {CHARSET utf-8} {features "foo bar"} }"#)
    .expect("extension attributes should parse");

  assert_eq!(Some("utf-8"), alternates.variants()[0].attribute("charset"));
  assert_eq!(
    Some("foo bar"),
    alternates.variants()[0].attribute("FEATURES")
  );
  assert_eq!("charset", alternates.variants()[0].attributes()[0].name());
}

#[test]
fn alternates_rejects_malformed_entries_and_uris() {
  for value in [
    "",
    " ",
    r#"/resource 1"#,
    r#"{ /resource 1 }"#,
    r#"{ "/resource" }"#,
    r#"{ "/resource" 1.001 }"#,
    r#"{ "unterminated 1 }"#,
    r#"{ "" 1 }"#,
    r#"{ "http://" 1 }"#,
    r#"{ "http://exa mple.test/" 1 }"#,
    r#"{ "a%zz" 1 }"#,
    r#"{ "a%2" 1 }"#,
    r#"{ "foo^bar" 1 }"#,
    r#"{ "/resource" 1 {type} }"#,
    r#"{ "/resource" 1 {length "1234"} }"#,
    r#"{ "/resource" 1 {length -1} }"#,
    r#"{ "/resource" 1 {length 18446744073709551616} }"#,
    r#"{ "/resource" 1 },"#,
    r#"{ "/a" 1 },, { "/b" 1 }"#,
    "{ \"/a\" 1 }\r\nX-Injected: 1",
  ] {
    assert!(
      Alternates::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    Alternates::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn alternates_rejects_duplicate_attributes_and_variants() {
  assert!(Alternates::parse(r#"{ "/a" 1 {type text/html} {type application/json} }"#).is_err());
  assert!(Alternates::parse(r#"{ "/a" 1 {type text/html} {TYPE application/json} }"#).is_err());
  assert!(Alternates::parse(r#"{ "/a" 1 }, { "/a" 1 }"#).is_err());
  assert!(
    Alternates::parse(r#"{ "/a" 1 {type text/html} }, { "/a" 1 {TYPE text/html} }"#).is_err()
  );

  let distinct = Alternates::parse(r#"{ "/a" 1 }, { "/a" 0.5 }, { "/b" 1 }"#)
    .expect("different URI or quality must be retained");
  assert_eq!(3, distinct.len());
}

#[test]
fn alternates_enforces_member_count_and_size_bounds() {
  assert!(Alternates::parse("a".repeat(MAX_ALTERNATES_VALUE_BYTES + 1)).is_err());

  let too_many_variants = (0..=MAX_ALTERNATES_VARIANTS)
    .map(|index| format!(r#"{{ "/v{index}" 1 }}"#))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many =
    Alternates::parse(&too_many_variants).expect_err("257 variants should be rejected");
  assert_eq!(too_many.to_string(), "too many Alternates variants");

  let at_count = (0..MAX_ALTERNATES_VARIANTS)
    .map(|index| format!(r#"{{ "/v{index}" 1 }}"#))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    Alternates::parse(&at_count).is_ok(),
    "256 variants should parse"
  );

  let too_many_attributes = format!(
    r#"{{ "/asset" 1{}}}"#,
    (0..=MAX_ALTERNATES_ATTRIBUTES)
      .map(|index| format!(" {{p{index} v}}"))
      .collect::<String>()
  );
  assert!(Alternates::parse(&too_many_attributes).is_err());

  assert!(Alternates::parse(format!(
    r#"{{ "/asset" 1 {{note {}}}}}"#,
    "a".repeat(MAX_ALTERNATES_ATTRIBUTE_VALUE_BYTES + 1)
  ))
  .is_err());

  let first = format!(r#"{{ "/{}" 1 }}"#, "a".repeat(40 * 1024));
  let second = format!(r#"{{ "/{}" 1 }}"#, "b".repeat(40 * 1024));
  let oversized = Alternates::parse_values([first.as_str(), second.as_str()])
    .expect_err("an aggregate over 64 KiB should be rejected");
  assert_eq!(
    oversized.to_string(),
    "Alternates header aggregate value is too large"
  );
  assert!(first.len() + second.len() > MAX_ALTERNATES_AGGREGATE_VALUE_BYTES);
  let _ = MAX_ALTERNATES_URI_BYTES;
}

#[test]
fn alternates_parse_error_is_public_and_displayable() {
  let error: AlternatesParseError =
    Alternates::parse("not alternates").expect_err("malformed Alternates must be rejected");
  assert!(!error.to_string().is_empty());
  assert_eq!(error.to_string(), "invalid Alternates entry");
}

#[test]
fn alternates_exposes_typed_accessors() {
  let alternates = Alternates::parse(r#"{ "/style.html" 1 {type text/html} }"#)
    .expect("valid Alternates must parse");
  let variant: &AlternateVariant = &alternates.variants()[0];

  assert_eq!("/style.html", variant.uri());
  assert_eq!("1", variant.quality());
  assert_eq!(1, variant.attributes().len());
  assert_eq!("type", variant.attributes()[0].name());
  assert_eq!("text/html", variant.attributes()[0].value());
  assert_eq!(Some("text/html"), variant.attribute("TYPE"));
}
