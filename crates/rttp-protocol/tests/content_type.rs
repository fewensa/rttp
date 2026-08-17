use rttp_protocol::content_type::{
  ContentType, MAX_CONTENT_TYPE_PARAMETERS, MAX_CONTENT_TYPE_VALUE_BYTES,
};

#[test]
fn content_type_parses_media_type_and_preserves_spelling() {
  let content_type = ContentType::parse(r#"Text/HTML; Charset="utf-8"; boundary=abc-123"#)
    .expect("Content-Type should parse");

  assert_eq!(content_type.type_(), "Text");
  assert_eq!(content_type.subtype(), "HTML");
  assert_eq!(content_type.media_type().type_(), "Text");
  assert_eq!(content_type.parameters()[0].name(), "Charset");
  assert_eq!(content_type.parameter("charset"), Some("utf-8"));
  assert_eq!(content_type.parameter("boundary"), Some("abc-123"));
  assert_eq!(
    content_type.header_value(),
    "Text/HTML; Charset=utf-8; boundary=abc-123"
  );
}

#[test]
fn content_type_preserves_quoted_commas_and_optional_whitespace() {
  let content_type =
    ContentType::parse("\tapplication/json; profile=\"https://example.test/profile, v1\"\t")
      .expect("quoted comma and OWS should parse");

  assert_eq!(
    content_type.parameters()[0].value(),
    "https://example.test/profile, v1"
  );
  assert_eq!(
    content_type.header_value(),
    "application/json; profile=\"https://example.test/profile, v1\""
  );

  let padded = ContentType::parse(" text/plain ; charset = utf-8 ")
    .expect("OWS around media-type separators should parse");
  assert_eq!(padded.header_value(), "text/plain; charset=utf-8");
}

#[test]
fn content_type_rejects_invalid_syntax() {
  for value in [
    "",
    "text",
    "/json",
    "text/",
    "text/pl ain",
    "text/plain;",
    "text/plain; charset",
    "text/plain; char set=utf-8",
    "text/plain; charset=utf 8",
    "text/plain; charset=\"utf-8",
    "application/json,, text/plain",
    ",application/json",
    "application/json,",
    "text/plain\r\nX: y",
    "text/plain\u{7f}",
  ] {
    assert!(
      ContentType::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_type_rejects_duplicate_fields_extra_media_types_and_empty_sets() {
  assert!(
    ContentType::parse_values(["text/plain", "text/html"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    ContentType::parse("text/plain, text/html").is_err(),
    "comma-separated extra media types must be rejected"
  );
  assert!(
    ContentType::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn content_type_enforces_value_and_parameter_bounds() {
  assert!(
    ContentType::parse("x".repeat(MAX_CONTENT_TYPE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_CONTENT_TYPE_VALUE_BYTES + 1);
  assert!(
    ContentType::parse_values(["text/plain", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = format!(
    "text/plain{}",
    (0..MAX_CONTENT_TYPE_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let parsed = ContentType::parse(&at_limit).expect("256 parameters should parse");
  assert_eq!(parsed.parameters().len(), MAX_CONTENT_TYPE_PARAMETERS);

  let too_many = format!(
    "text/plain{}",
    (0..=MAX_CONTENT_TYPE_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(
    ContentType::parse(&too_many).is_err(),
    "more than 256 parameters must be rejected"
  );
}

#[test]
fn content_type_rejects_duplicate_parameter_names() {
  assert!(
    ContentType::parse("text/plain; charset=utf-8; CHARSET=iso-8859-1").is_err(),
    "case-insensitive duplicate parameter names must be rejected"
  );
}
