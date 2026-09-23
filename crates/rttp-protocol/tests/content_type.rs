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
fn content_type_builds_media_types_and_parameters() {
  let content_type = ContentType::new("Application", "JSON")
    .expect("media type should build")
    .with_parameter("Charset", "UTF-8")
    .expect("parameter should build")
    .with_parameter("profile", "https://example.test/a;b")
    .expect("quoted parameter should build");

  assert_eq!(content_type.type_(), "application");
  assert_eq!(content_type.subtype(), "json");
  assert_eq!(content_type.parameter("charset"), Some("UTF-8"));
  assert_eq!(
    content_type.parameter("profile"),
    Some("https://example.test/a;b")
  );
  assert_eq!(
    content_type.header_value(),
    r#"application/json; charset=UTF-8; profile="https://example.test/a;b""#
  );
}

#[test]
fn content_type_builder_accepts_http_ows_around_type_subtype_and_parameter_names() {
  for (
    type_name,
    subtype,
    parameter_name,
    expected_type,
    expected_subtype,
    expected_parameter,
    expected_header,
  ) in [
    (
      " application ",
      " json ",
      " charset ",
      "application",
      "json",
      "charset",
      "application/json; charset=UTF-8",
    ),
    (
      "\ttext\t",
      "\tplain\t",
      "\tboundary\t",
      "text",
      "plain",
      "boundary",
      "text/plain; boundary=UTF-8",
    ),
    (
      " \tApplication\t ",
      " \tJSON\t ",
      " \tCharset\t ",
      "application",
      "json",
      "charset",
      "application/json; charset=UTF-8",
    ),
  ] {
    let content_type = ContentType::new(type_name, subtype)
      .expect("OWS-padded media type should build")
      .with_parameter(parameter_name, "UTF-8")
      .expect("OWS-padded parameter name should build");

    assert_eq!(content_type.type_(), expected_type);
    assert_eq!(content_type.subtype(), expected_subtype);
    assert_eq!(content_type.parameter(expected_parameter), Some("UTF-8"));
    assert_eq!(content_type.header_value(), expected_header);
  }
}

#[test]
fn content_type_builder_rejects_non_ows_padding_around_type_subtype_and_parameter_names() {
  for whitespace in ["\u{000b}", "\u{000c}", "\r", "\n", "\u{00a0}", "\u{2003}"] {
    let type_name = format!("{whitespace}application{whitespace}");
    assert!(
      ContentType::new(&type_name, "json").is_err(),
      "non-OWS type padding must be rejected: {type_name:?}"
    );

    let subtype = format!("{whitespace}json{whitespace}");
    assert!(
      ContentType::new("application", &subtype).is_err(),
      "non-OWS subtype padding must be rejected: {subtype:?}"
    );

    let parameter_name = format!("{whitespace}charset{whitespace}");
    assert!(
      ContentType::new("application", "json")
        .expect("media type should build")
        .with_parameter(&parameter_name, "utf-8")
        .is_err(),
      "non-OWS parameter name padding must be rejected: {parameter_name:?}"
    );
  }
}

#[test]
fn content_type_builder_preserves_normalized_media_type_and_opaque_parameter_values() {
  let content_type = ContentType::new(" Application ", " JSON ")
    .expect("media type should build")
    .with_parameter(" Charset ", "UTF-8")
    .expect("parameter should build")
    .with_parameter("profile", "https://example.test/a;b")
    .expect("quoted parameter should build");

  assert_eq!(content_type.type_(), "application");
  assert_eq!(content_type.subtype(), "json");
  assert_eq!(content_type.parameter("charset"), Some("UTF-8"));
  assert_eq!(
    content_type.parameter("profile"),
    Some("https://example.test/a;b")
  );
  assert_eq!(
    content_type.header_value(),
    r#"application/json; charset=UTF-8; profile="https://example.test/a;b""#
  );
}

#[test]
fn content_type_builder_rejects_invalid_media_types_and_parameters() {
  assert!(
    ContentType::new("bad type", "plain").is_err(),
    "invalid type tokens must be rejected"
  );
  assert!(
    ContentType::new("text", "pl ain").is_err(),
    "invalid subtype tokens must be rejected"
  );
  assert!(
    ContentType::new("", "plain").is_err(),
    "empty tokens must be rejected"
  );

  let content_type = ContentType::new("text", "plain").expect("media type should build");
  assert!(
    content_type
      .clone()
      .with_parameter("bad name", "value")
      .is_err(),
    "invalid parameter names must be rejected"
  );
  assert!(
    content_type.clone().with_parameter("charset", "").is_err(),
    "empty parameter values must be rejected"
  );
  assert!(
    content_type
      .clone()
      .with_parameter("charset", "caf\u{e9}")
      .is_err(),
    "non-ASCII parameter values must be rejected"
  );
  assert!(
    content_type
      .clone()
      .with_parameter("charset", "bad\r\nX-Evil: yes")
      .is_err(),
    "control bytes in parameter values must be rejected"
  );
  assert!(
    content_type
      .clone()
      .with_parameter("charset", "utf-8")
      .expect("parameter should build")
      .with_parameter("CHARSET", "us-ascii")
      .is_err(),
    "case-insensitive duplicate parameters must be rejected"
  );

  let at_limit = (0..MAX_CONTENT_TYPE_PARAMETERS).fold(content_type, |content_type, index| {
    content_type
      .with_parameter(format!("p{index}"), "v")
      .expect("parameter should build")
  });
  assert!(
    at_limit.with_parameter("overflow", "v").is_err(),
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
