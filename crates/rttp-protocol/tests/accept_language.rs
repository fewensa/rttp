use rttp_protocol::accept_language::{
  AcceptLanguage, MAX_ACCEPT_LANGUAGE_RANGES, MAX_ACCEPT_LANGUAGE_VALUE_BYTES,
};

#[test]
fn accept_language_parses_ordered_ranges_with_qualities() {
  let languages = AcceptLanguage::parse("en-US, fr-CA; q=0.8, de; q=1., *")
    .expect("valid Accept-Language should parse");

  assert_eq!(languages.ranges(), ["en-US", "fr-CA", "de", "*"]);
  assert_eq!(languages.qualities(), [None, Some("0.8"), Some("1."), None]);
  assert_eq!(languages.header_value(), "en-US, fr-CA; q=0.8, de; q=1., *");
}

#[test]
fn accept_language_combines_field_values_in_wire_order() {
  let languages = AcceptLanguage::parse_values(["en-US, fr-CA; q=0.8", "*;q=0"])
    .expect("multiple Accept-Language fields should parse");

  assert_eq!(languages.ranges(), ["en-US", "fr-CA", "*"]);
  assert_eq!(languages.qualities(), [None, Some("0.8"), Some("0")]);
  assert_eq!(languages.header_value(), "en-US, fr-CA; q=0.8, *; q=0");
}

#[test]
fn accept_language_accepts_wildcard_and_whitespace_padding() {
  let wildcard = AcceptLanguage::parse("*; q=0").expect("wildcard with q=0 should parse");
  assert_eq!(wildcard.ranges(), ["*"]);
  assert_eq!(wildcard.qualities(), [Some("0")]);
  assert_eq!(wildcard.header_value(), "*; q=0");

  let padded = AcceptLanguage::parse(" en-US , fr-CA ; q = 0.8 ")
    .expect("OWS-padded Accept-Language should parse");
  assert_eq!(padded.ranges(), ["en-US", "fr-CA"]);
  assert_eq!(padded.qualities(), [None, Some("0.8")]);
  assert_eq!(padded.header_value(), "en-US, fr-CA; q=0.8");
}

#[test]
fn accept_language_accepts_boundary_q_values() {
  for value in [
    "en; q=0",
    "en; q=1",
    "en; q=0.000",
    "en; q=1.000",
    "en; q=0.001",
  ] {
    let languages =
      AcceptLanguage::parse(value).unwrap_or_else(|_| panic!("{value:?} should parse"));
    assert_eq!(
      languages.qualities(),
      [Some(value.split_once('=').unwrap().1.trim())]
    );
  }
}

#[test]
fn accept_language_rejects_malformed_q_values() {
  for value in [
    "en; q=1.001",
    "en; q=0.1234",
    "en; q=",
    "en; q=x",
    "en; q=.5",
    "en; q=2",
    "en; q=1.0.0",
    "en; q=1e1",
    "en; level=1",
    "en; q=1; q=2",
  ] {
    assert!(
      AcceptLanguage::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_language_rejects_invalid_ranges_and_wildcards() {
  for value in [
    "", " ", "en_US", "en..US", "-en", "en-", "en--US", "*x", "a1", "1en",
  ] {
    assert!(
      AcceptLanguage::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_language_rejects_case_insensitive_duplicates() {
  for value in ["en, EN", "en, en-US, en", "*, *"] {
    assert!(
      AcceptLanguage::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  assert!(
    AcceptLanguage::parse_values(["en", "EN"]).is_err(),
    "duplicates across fields must be rejected"
  );
  assert!(
    AcceptLanguage::parse_values(["en, fr", "fr-CA", "EN"]).is_err(),
    "case-insensitive duplicates across fields must be rejected"
  );
}

#[test]
fn accept_language_enforces_value_and_count_bounds() {
  let oversized = "x".repeat(MAX_ACCEPT_LANGUAGE_VALUE_BYTES + 1);
  assert!(AcceptLanguage::parse(&oversized).is_err());
  assert!(
    AcceptLanguage::parse_values(["en", oversized.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let mut ranges = (0..=MAX_ACCEPT_LANGUAGE_RANGES)
    .map(|index| {
      format!(
        "{}{}",
        char::from(b'a' + (index / 26) as u8),
        char::from(b'a' + (index % 26) as u8)
      )
    })
    .collect::<Vec<_>>();
  assert!(
    AcceptLanguage::from_ranges(&ranges).is_err(),
    "more than 32 ranges must be rejected"
  );
  ranges.pop();
  assert_eq!(ranges.len(), MAX_ACCEPT_LANGUAGE_RANGES);
  AcceptLanguage::from_ranges(&ranges).expect("exactly 32 ranges should parse");
}

#[test]
fn accept_language_rejects_empty_input() {
  assert!(AcceptLanguage::parse("").is_err());
  assert!(AcceptLanguage::parse(" ").is_err());
  assert!(AcceptLanguage::parse_values(std::iter::empty()).is_err());
  assert!(AcceptLanguage::from_ranges(std::iter::empty::<&str>()).is_err());
}

#[test]
fn accept_language_rejects_trailing_items_without_ranges() {
  assert!(
    AcceptLanguage::parse("en,").is_err(),
    "empty member must be rejected"
  );
  assert!(
    AcceptLanguage::parse(", en").is_err(),
    "empty leading member must be rejected"
  );
}
