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

  let padded = AcceptLanguage::parse("\t en-US \t,\t fr-CA \t;\t q \t=\t 0.8 \t")
    .expect("OWS-padded Accept-Language should parse");
  assert_eq!(padded.ranges(), ["en-US", "fr-CA"]);
  assert_eq!(padded.qualities(), [None, Some("0.8")]);
  assert_eq!(padded.header_value(), "en-US, fr-CA; q=0.8");
}

#[test]
fn accept_language_preserves_first_seen_spelling_and_accepts_q_name_case() {
  let languages = AcceptLanguage::parse("EN-us, fr-CA; Q=0.8")
    .expect("mixed-case ranges and Q parameter name should parse");

  assert_eq!(languages.ranges(), ["EN-us", "fr-CA"]);
  assert_eq!(languages.qualities(), [None, Some("0.8")]);
  assert_eq!(languages.header_value(), "EN-us, fr-CA; q=0.8");
}

#[test]
fn accept_language_accepts_eight_character_subtags() {
  let languages = AcceptLanguage::parse("abcdefgh, en-abcdefgh, aa-12345678")
    .expect("8-character primary and subtags should parse");
  assert_eq!(
    languages.ranges(),
    ["abcdefgh", "en-abcdefgh", "aa-12345678"]
  );
}

#[test]
fn accept_language_round_trips_header_value() {
  let padded = AcceptLanguage::parse("\t en-US \t,\t fr-CA \t;\t q \t=\t 0.8 \t")
    .expect("OWS-padded Accept-Language should parse");
  let padded_round_trip = AcceptLanguage::parse(padded.header_value())
    .expect("canonical padded Accept-Language should round-trip");
  assert_eq!(padded_round_trip.ranges(), padded.ranges());
  assert_eq!(padded_round_trip.qualities(), padded.qualities());
  assert_eq!(padded_round_trip.header_value(), padded.header_value());

  let mixed = AcceptLanguage::parse("en-US, fr-CA; q=0.8, de; q=1., *; q=0")
    .expect("mixed-q Accept-Language should parse");
  let mixed_round_trip = AcceptLanguage::parse(mixed.header_value())
    .expect("canonical mixed-q Accept-Language should round-trip");
  assert_eq!(mixed_round_trip.ranges(), mixed.ranges());
  assert_eq!(mixed_round_trip.qualities(), mixed.qualities());
  assert_eq!(mixed_round_trip.header_value(), mixed.header_value());
}

#[test]
fn accept_language_rejects_non_ows_whitespace_at_boundaries() {
  for whitespace in ["\r", "\n", "\u{0b}", "\u{0c}", "\u{00a0}", "\u{2003}"] {
    for value in [
      format!("{whitespace}en-US"),
      format!("en-US{whitespace}"),
      format!("en-US{whitespace},fr-CA"),
      format!("en-US,{whitespace}fr-CA"),
      format!("en{whitespace};q=0"),
      format!("en;{whitespace}q=0"),
      format!("en;q{whitespace}=0"),
      format!("en;q={whitespace}0"),
    ] {
      assert!(
        AcceptLanguage::parse(&value).is_err(),
        "{value:?} must reject non-OWS whitespace"
      );
    }
  }
}

#[test]
fn accept_language_rejects_ascii_control_bytes() {
  for value in [
    "en\u{0000}",
    "en\u{0001}-US",
    "en\r\nX: y",
    "en\u{007f}",
    "\u{0007}en",
  ] {
    let error = AcceptLanguage::parse(value).expect_err("control bytes must be rejected");
    assert_eq!(error.to_string(), "invalid Accept-Language control byte");
  }
  AcceptLanguage::parse("\ten\t,\tfr\t").expect("HTAB OWS should parse");
  assert!(
    AcceptLanguage::parse("en\tUS").is_err(),
    "HTAB inside a language range must still fail grammar validation"
  );
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
    "",
    " ",
    "en_US",
    "en..US",
    "-en",
    "en-",
    "en--US",
    "*x",
    "a1",
    "1en",
    "abcdefghi",
    "en-abcdefghi",
    "en,,fr",
    "en, ,fr",
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

  let at_value_limit = "x".repeat(MAX_ACCEPT_LANGUAGE_VALUE_BYTES);
  assert!(
    AcceptLanguage::parse(&at_value_limit).is_err(),
    "values at the 64 KiB bound must still obey language-range grammar"
  );

  let at_value_limit_valid = "aa-x-".to_string() + &"private-".repeat(8191) + "pvt";
  assert_eq!(at_value_limit_valid.len(), MAX_ACCEPT_LANGUAGE_VALUE_BYTES);
  AcceptLanguage::parse(&at_value_limit_valid)
    .expect("valid language ranges at the 64 KiB bound must parse");

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
