use rttp_protocol::content_encoding::{
  ContentEncoding, MAX_CONTENT_ENCODING_CODINGS, MAX_CONTENT_ENCODING_VALUE_BYTES,
};

#[test]
fn content_encoding_preserves_coding_spelling_and_order() {
  let content_encoding = ContentEncoding::parse("gzip, br").expect("Content-Encoding should parse");

  assert_eq!(content_encoding.codings(), ["gzip", "br"]);
  assert_eq!(content_encoding.header_value(), "gzip, br");
}

#[test]
fn content_encoding_accepts_multiple_fields_in_wire_order() {
  let content_encoding = ContentEncoding::parse_values(["gzip, br", "zstd"])
    .expect("multiple Content-Encoding fields should parse");

  assert_eq!(content_encoding.codings(), ["gzip", "br", "zstd"]);
  assert_eq!(content_encoding.header_value(), "gzip, br, zstd");
  assert_eq!(content_encoding.len(), 3);
}

#[test]
fn content_encoding_accepts_http_optional_whitespace_padding() {
  for value in ["\tgzip\t", " gzip "] {
    let content_encoding =
      ContentEncoding::parse(value).expect("OWS-padded Content-Encoding should parse");
    assert_eq!(content_encoding.codings(), ["gzip"]);
  }

  for value in [" gzip ,\tbr ", "gzip,br"] {
    let content_encoding =
      ContentEncoding::parse(value).expect("OWS-padded Content-Encoding should parse");
    assert_eq!(content_encoding.codings(), ["gzip", "br"]);
  }
}

#[test]
fn content_encoding_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",gzip",
    "gzip,",
    "gzip,,br",
    "g zip",
    "gzip; q=1",
    "gzip: br",
    "\u{0d}gzip",
    "gzip\r\nX: y",
    "gzip\u{7f}",
  ] {
    assert!(
      ContentEncoding::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_encoding_rejects_case_insensitive_duplicates() {
  assert!(
    ContentEncoding::parse("gzip, GZIP").is_err(),
    "in-field case-insensitive duplicates must be rejected"
  );
  assert!(
    ContentEncoding::parse("br, gzip, br").is_err(),
    "in-field duplicates must be rejected"
  );
  assert!(
    ContentEncoding::parse_values(["gzip, br", "GZIP"]).is_err(),
    "cross-field case-insensitive duplicates must be rejected"
  );
  assert!(
    ContentEncoding::parse_values(["br", "gzip, BR"]).is_err(),
    "cross-field duplicates must be rejected"
  );
}

#[test]
fn content_encoding_rejects_empty_field_sets() {
  assert!(
    ContentEncoding::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn content_encoding_enforces_value_and_coding_bounds() {
  assert!(
    ContentEncoding::parse("x".repeat(MAX_CONTENT_ENCODING_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_CONTENT_ENCODING_VALUE_BYTES);
  assert!(
    ContentEncoding::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_CONTENT_ENCODING_VALUE_BYTES + 1);
  assert!(
    ContentEncoding::parse_values(["gzip", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_CONTENT_ENCODING_CODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = ContentEncoding::parse(&at_limit).expect("256 codings should parse");
  assert_eq!(parsed.len(), MAX_CONTENT_ENCODING_CODINGS);

  let too_many = (0..=MAX_CONTENT_ENCODING_CODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    ContentEncoding::parse(&too_many).is_err(),
    "more than 256 codings must be rejected"
  );
}
