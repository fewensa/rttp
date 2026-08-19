use rttp_protocol::content_language::{
  ContentLanguage, MAX_CONTENT_LANGUAGE_TAGS, MAX_CONTENT_LANGUAGE_VALUE_BYTES,
};

#[test]
fn content_language_preserves_tag_spelling_and_order() {
  let content_language =
    ContentLanguage::parse("fr-CA, es-419").expect("Content-Language should parse");

  assert_eq!(content_language.tags(), ["fr-CA", "es-419"]);
  assert_eq!(content_language.header_value(), "fr-CA, es-419");
  assert!(!content_language.is_empty());
}

#[test]
fn content_language_accepts_strict_language_tag_forms() {
  for value in [
    "en",
    "fr-CA",
    "es-419",
    "zh-cmn-Hans-CN",
    "sl-rozaj-biske-1994",
    "en-US-u-ca-gregory",
    "de-CH-x-phonebk",
    "x-private",
    "i-klingon",
  ] {
    assert!(
      ContentLanguage::parse(value).is_ok(),
      "{value:?} must parse as a valid language tag"
    );
  }
}

#[test]
fn content_language_accepts_multiple_fields_in_wire_order() {
  let content_language = ContentLanguage::parse_values(["fr-CA, es-419", "en"])
    .expect("multiple Content-Language fields should parse");

  assert_eq!(content_language.tags(), ["fr-CA", "es-419", "en"]);
  assert_eq!(content_language.header_value(), "fr-CA, es-419, en");
  assert_eq!(content_language.len(), 3);
}

#[test]
fn content_language_accepts_http_optional_whitespace_padding() {
  for value in ["\ten-US\t", " en-US "] {
    let content_language =
      ContentLanguage::parse(value).expect("OWS-padded Content-Language should parse");
    assert_eq!(content_language.tags(), ["en-US"]);
  }

  for value in [" en-US ,\tfr-CA ", "en-US,fr-CA"] {
    let content_language =
      ContentLanguage::parse(value).expect("OWS-padded Content-Language should parse");
    assert_eq!(content_language.tags(), ["en-US", "fr-CA"]);
  }
}

#[test]
fn content_language_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",en",
    "en,",
    "en,,fr",
    "en, ,fr",
    "*",
    "en US",
    "en_US",
    "en;q=1",
    "en-US-",
    "-en-US",
    "a-1",
    "en-a",
    "en-12",
    "x",
    "abcdefghi",
    "en-abcdefghi",
    "en-\u{e9}",
    "\u{0d}en",
    "en\r\nX: y",
    "en\u{7f}",
  ] {
    assert!(
      ContentLanguage::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_language_rejects_case_insensitive_duplicates_but_keeps_distinct_tags() {
  for value in ["en-US, EN-us", "fr-CA, es-419, FR-ca", "EN, en"] {
    assert!(
      ContentLanguage::parse(value).is_err(),
      "{value:?} duplicate tags must be rejected"
    );
  }

  let cross_field = ContentLanguage::parse_values(["fr-CA, es-419", "FR-ca"]);
  assert!(
    cross_field.is_err(),
    "duplicate tags across fields must be rejected"
  );

  let preserved = ContentLanguage::parse("fr-CA, es-419").expect("distinct tags should parse");
  assert_eq!(preserved.tags(), ["fr-CA", "es-419"]);
}

#[test]
fn content_language_rejects_empty_field_sets() {
  assert!(
    ContentLanguage::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn content_language_enforces_value_and_tag_bounds() {
  assert!(
    ContentLanguage::parse("x".repeat(MAX_CONTENT_LANGUAGE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_CONTENT_LANGUAGE_VALUE_BYTES);
  assert!(
    ContentLanguage::parse(&at_value_limit).is_err(),
    "values at the 64 KiB bound must still obey language tag syntax"
  );

  let at_value_limit_valid = "aa-x-".to_string() + &"private-".repeat(8191) + "pvt";
  assert_eq!(at_value_limit_valid.len(), MAX_CONTENT_LANGUAGE_VALUE_BYTES);
  assert!(
    ContentLanguage::parse(&at_value_limit_valid).is_ok(),
    "valid tags at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_CONTENT_LANGUAGE_VALUE_BYTES + 1);
  assert!(
    ContentLanguage::parse_values(["en", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_CONTENT_LANGUAGE_TAGS)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = ContentLanguage::parse(&at_limit).expect("256 tags should parse");
  assert_eq!(parsed.len(), MAX_CONTENT_LANGUAGE_TAGS);

  let too_many = (0..=MAX_CONTENT_LANGUAGE_TAGS)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    ContentLanguage::parse(&too_many).is_err(),
    "more than 256 tags must be rejected"
  );
}

#[test]
fn content_language_parse_error_implements_display_and_error() {
  use std::error::Error;

  let error = ContentLanguage::parse("en,").expect_err("trailing comma must be rejected");
  let _: &dyn Error = &error;
  assert!(!error.to_string().is_empty());
}
