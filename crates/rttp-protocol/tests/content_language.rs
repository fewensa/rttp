use rttp_protocol::content_language::{
  ContentLanguage, MAX_CONTENT_LANGUAGE_TAGS, MAX_CONTENT_LANGUAGE_VALUE_BYTES,
};

#[test]
fn content_language_preserves_tag_spelling_and_order() {
  let content_language =
    ContentLanguage::parse("en, fr-CA, x-private").expect("Content-Language should parse");

  assert_eq!(content_language.tags(), ["en", "fr-CA", "x-private"]);
  assert_eq!(content_language.header_value(), "en, fr-CA, x-private");
  assert_eq!(content_language.len(), 3);
}

#[test]
fn content_language_accepts_multiple_fields_in_wire_order() {
  let content_language = ContentLanguage::parse_values(["en-US, fr", "zh-Hant-TW, es-419"])
    .expect("multiple Content-Language fields should parse");

  assert_eq!(
    content_language.tags(),
    ["en-US", "fr", "zh-Hant-TW", "es-419"]
  );
  assert_eq!(
    content_language.header_value(),
    "en-US, fr, zh-Hant-TW, es-419"
  );
}

#[test]
fn content_language_accepts_http_optional_whitespace_padding() {
  for value in ["\ten\t", " en "] {
    let content_language =
      ContentLanguage::parse(value).expect("OWS-padded Content-Language should parse");
    assert_eq!(content_language.tags(), ["en"]);
  }

  for value in [" en ,\tfr-CA ", "en,fr-CA"] {
    let content_language =
      ContentLanguage::parse(value).expect("OWS-padded Content-Language should parse");
    assert_eq!(content_language.tags(), ["en", "fr-CA"]);
  }
}

#[test]
fn content_language_builds_values_from_language_tags() {
  let content_language = ContentLanguage::from_languages(["en", "fr-CA", "x-private"])
    .expect("Content-Language should build");

  assert_eq!(content_language.tags(), ["en", "fr-CA", "x-private"]);
  assert_eq!(content_language.header_value(), "en, fr-CA, x-private");

  assert!(
    ContentLanguage::from_languages(["en", "en"]).is_err(),
    "duplicate tags must be rejected"
  );
  assert!(
    ContentLanguage::from_languages(["bad tag"]).is_err(),
    "invalid tags must be rejected"
  );
  assert!(
    ContentLanguage::from_languages(
      (0..=MAX_CONTENT_LANGUAGE_TAGS).map(|index| format!("x-{index}"))
    )
    .is_err(),
    "too many tags must be rejected"
  );
}

#[test]
fn content_language_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",en",
    "en,",
    "en,,fr",
    "en us",
    "en_US",
    "en; q=1",
    "-en",
    "en-",
    "en--US",
    "123en",
    "en123456789",
    "en-123456789",
    "en-\u{7f}",
  ] {
    assert!(
      ContentLanguage::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_language_rejects_case_insensitive_duplicates() {
  let in_field = ContentLanguage::parse("en, EN").expect_err("repeated tags must be rejected");
  assert_eq!(in_field.to_string(), "duplicate Content-Language tag");

  assert!(
    ContentLanguage::parse_values(["en, fr", "EN"]).is_err(),
    "repeated tags across fields must be rejected"
  );
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
