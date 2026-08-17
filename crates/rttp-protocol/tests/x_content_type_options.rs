use rttp_protocol::x_content_type_options::{
  XContentTypeOptions, MAX_X_CONTENT_TYPE_OPTIONS_VALUE_BYTES,
};

#[test]
fn x_content_type_options_parses_nosniff_case_insensitively() {
  assert_eq!(
    XContentTypeOptions::Nosniff,
    XContentTypeOptions::parse("nosniff").expect("nosniff should parse")
  );
  assert_eq!(
    XContentTypeOptions::Nosniff,
    XContentTypeOptions::parse("NoSniff").expect("NoSniff should parse")
  );
  assert_eq!(
    XContentTypeOptions::Nosniff,
    XContentTypeOptions::parse("NOSNIFF").expect("NOSNIFF should parse")
  );
  assert_eq!("nosniff", XContentTypeOptions::Nosniff.header_value());
}

#[test]
fn x_content_type_options_accepts_http_optional_whitespace_padding() {
  for value in ["\tnosniff\t", " \tnosniff\t ", "nosniff\t", "\tnosniff"] {
    assert_eq!(
      XContentTypeOptions::Nosniff,
      XContentTypeOptions::parse(value).expect("OWS-padded nosniff should parse")
    );
  }
}

#[test]
fn x_content_type_options_rejects_empty_duplicate_malformed_and_ambiguous_values() {
  for value in [
    "",
    "   ",
    "same-origin",
    "nosniff, nosniff",
    "nosniff; foo",
    "\"nosniff\"",
    "nosniff\r\nX: y",
    "nosniff\u{7f}",
  ] {
    assert!(
      XContentTypeOptions::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    XContentTypeOptions::parse_values(["nosniff", "nosniff"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    XContentTypeOptions::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    XContentTypeOptions::parse("a".repeat(MAX_X_CONTENT_TYPE_OPTIONS_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn x_content_type_options_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_X_CONTENT_TYPE_OPTIONS_VALUE_BYTES + 1);

  assert!(
    XContentTypeOptions::parse_values(["nosniff", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
