use rttp_protocol::entity_tag::{
  EntityTag, IfMatch, IfNoneMatch, MAX_CONDITIONAL_ENTITY_TAGS, MAX_ENTITY_TAG_VALUE_BYTES,
  MAX_IF_MATCH_VALUE_BYTES, MAX_IF_NONE_MATCH_VALUE_BYTES,
};

#[test]
fn entity_tags_parse_strong_and_weak_forms_and_serialize_canonically() {
  let strong = EntityTag::parse("\"abc\"").expect("strong entity tag");
  assert_eq!("abc", strong.opaque_tag());
  assert!(!strong.is_weak());
  assert_eq!("\"abc\"", strong.header_value());

  let weak = EntityTag::parse("W/\"abc\"").expect("weak entity tag");
  assert_eq!("abc", weak.opaque_tag());
  assert!(weak.is_weak());
  assert_eq!("W/\"abc\"", weak.header_value());
}

#[test]
fn entity_tag_constructors_validate_and_compare_strong_and_weak_forms() {
  let strong = EntityTag::strong("abc");
  let weak = EntityTag::weak("abc");
  let other = EntityTag::strong("other");

  assert_eq!("\"abc\"", strong.header_value());
  assert_eq!("W/\"abc\"", weak.header_value());
  assert!(strong.strong_matches(&EntityTag::strong("abc")));
  assert!(!strong.strong_matches(&weak));
  assert!(strong.weak_matches(&weak));
  assert!(!strong.weak_matches(&other));
}

#[test]
#[should_panic(expected = "entity tag opaque value must be valid")]
fn entity_tag_constructor_rejects_invalid_opaque_tag() {
  let _ = EntityTag::strong("bad space");
}

#[test]
fn entity_tag_constructors_enforce_serialized_header_limit() {
  let largest_strong = EntityTag::strong("a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"\"\"".len()));
  assert_eq!(
    MAX_ENTITY_TAG_VALUE_BYTES,
    largest_strong.header_value().len()
  );
  EntityTag::parse(largest_strong.header_value()).expect("largest strong tag should parse");

  let largest_weak = EntityTag::weak("a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"W/\"\"".len()));
  assert_eq!(
    MAX_ENTITY_TAG_VALUE_BYTES,
    largest_weak.header_value().len()
  );
  EntityTag::parse(largest_weak.header_value()).expect("largest weak tag should parse");

  assert!(
    std::panic::catch_unwind(|| EntityTag::strong("a".repeat(MAX_ENTITY_TAG_VALUE_BYTES))).is_err()
  );
  assert!(
    std::panic::catch_unwind(|| EntityTag::weak("a".repeat(MAX_ENTITY_TAG_VALUE_BYTES))).is_err()
  );
}

#[test]
fn conditional_entity_tag_lists_parse_values_and_serialize_canonically() {
  let if_match =
    IfMatch::parse_values([" \"one\" , W/\"two\" ", "\"three\""]).expect("If-Match list");
  assert!(!if_match.is_wildcard());
  assert_eq!(3, if_match.entity_tags().len());
  assert_eq!("\"one\", W/\"two\", \"three\"", if_match.header_value());

  let if_none_match = IfNoneMatch::parse("*").expect("If-None-Match wildcard");
  assert!(if_none_match.is_wildcard());
  assert!(if_none_match.entity_tags().is_empty());
  assert_eq!("*", if_none_match.header_value());
}

#[test]
fn if_none_match_rejects_non_ows_padding_but_accepts_ows() {
  for value in [" * ", "\t*\t"] {
    let parsed = IfNoneMatch::parse(value).expect("OWS-padded wildcard should parse");
    assert!(parsed.is_wildcard());
  }
  let parsed =
    IfNoneMatch::parse(" \"one\" , W/\"two\"\t").expect("OWS-padded entity tags should parse");
  assert_eq!(2, parsed.entity_tags().len());

  for value in [
    "\u{00a0}*",
    "*\u{2003}",
    "\u{3000}\"one\"",
    "W/\"one\"\u{00a0}",
    "\"one\"\u{2003}, \"two\"",
  ] {
    assert!(
      IfNoneMatch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn conditional_entity_tags_reject_malformed_ambiguous_and_unbounded_inputs() {
  for value in ["abc", "W/abc", "w/\"abc\"", "\"abc", "\"a b\"", "\"a\n\""] {
    assert!(
      EntityTag::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  for value in [
    "",
    ",\"one\"",
    "\"one\",",
    "\"one\",,\"two\"",
    "*, \"one\"",
    "\"one\", \"one\"",
  ] {
    assert!(IfMatch::parse(value).is_err(), "{value:?} must be rejected");
    assert!(
      IfNoneMatch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(EntityTag::parse(format!("\"{}\"", "a".repeat(MAX_ENTITY_TAG_VALUE_BYTES))).is_err());

  let too_many = std::iter::repeat_n("\"tag\"", MAX_CONDITIONAL_ENTITY_TAGS + 1)
    .collect::<Vec<_>>()
    .join(",");
  assert!(IfMatch::parse(&too_many).is_err());
  assert!(IfNoneMatch::parse(&too_many).is_err());
}

#[test]
fn entity_tag_accepts_empty_and_etagc_edge_opaque_bytes() {
  let empty = EntityTag::parse("\"\"").expect("empty opaque-tag should parse");
  assert_eq!("", empty.opaque_tag());
  assert!(!empty.is_weak());
  assert_eq!("\"\"", empty.header_value());
  assert_eq!(empty, EntityTag::strong(""));

  let weak_empty = EntityTag::parse("W/\"\"").expect("weak empty opaque-tag should parse");
  assert_eq!("", weak_empty.opaque_tag());
  assert!(weak_empty.is_weak());
  assert_eq!("W/\"\"", weak_empty.header_value());
  assert_eq!(weak_empty, EntityTag::weak(""));

  for opaque in ["!", "#", "~", "*", ",", "/", "\\", "a,b", "W/abc"] {
    let strong = EntityTag::parse(format!("\"{opaque}\"")).expect("etagc edge should parse");
    assert_eq!(opaque, strong.opaque_tag());
    assert!(!strong.is_weak());
    assert_eq!(format!("\"{opaque}\""), strong.header_value());
    assert_eq!(strong, EntityTag::strong(opaque));

    let weak = EntityTag::parse(format!("W/\"{opaque}\"")).expect("weak etagc edge should parse");
    assert_eq!(opaque, weak.opaque_tag());
    assert!(weak.is_weak());
    assert_eq!(format!("W/\"{opaque}\""), weak.header_value());
    assert_eq!(weak, EntityTag::weak(opaque));
  }
}

#[test]
fn entity_tag_accepts_obs_text_opaque_bytes_and_round_trips() {
  for opaque in ["café", "\u{0080}", "\u{00ff}", "€", "😀"] {
    let parsed =
      EntityTag::parse(format!("\"{opaque}\"")).expect("obs-text opaque-tag should parse");
    assert_eq!(opaque, parsed.opaque_tag());
    assert_eq!(format!("\"{opaque}\""), parsed.header_value());
    assert_eq!(parsed, EntityTag::strong(opaque));

    let weak = EntityTag::parse(format!("W/\"{opaque}\"")).expect("weak obs-text should parse");
    assert_eq!(opaque, weak.opaque_tag());
    assert!(weak.is_weak());
    assert_eq!(format!("W/\"{opaque}\""), weak.header_value());
    assert_eq!(weak, EntityTag::weak(opaque));

    let round_trip = EntityTag::parse(parsed.header_value()).expect("obs-text should round-trip");
    assert_eq!(round_trip, parsed);
    assert_eq!(round_trip.header_value(), parsed.header_value());
  }
}

#[test]
fn entity_tag_rejects_weak_prefix_case_ows_and_malformed_quoting() {
  for value in [
    "w/\"abc\"",
    "W /\"abc\"",
    "W\t/\"abc\"",
    "W/ \"abc\"",
    "W/\t\"abc\"",
    "WW/\"abc\"",
    "W/\"abc",
    "\"abc",
    "abc\"",
    "\"abc\"def",
    "\"a\"b\"",
    "\"a\\\"b\"",
    "\"abc\" \"def\"",
    "*",
    "W/*",
    "W/",
    "\"",
    "W/\"",
    "",
    " ",
    "\t",
  ] {
    assert!(
      EntityTag::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn entity_tag_rejects_controls_space_tab_del_and_embedded_quote() {
  for value in [
    "\"a b\"",
    "\"a\tb\"",
    "\"a\nb\"",
    "\"a\rb\"",
    "\"a\0b\"",
    "\"a\u{7f}b\"",
    "\"\u{7f}\"",
    "\"abc\"\r\nX: y",
    "\"abc\"\n",
    "\"abc\"\0",
    "W/\"a b\"",
    "W/\"a\nb\"",
  ] {
    assert!(
      EntityTag::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    std::panic::catch_unwind(|| EntityTag::strong("a\"b")).is_err(),
    "embedded DQUOTE must be rejected by the constructor"
  );
  assert!(
    std::panic::catch_unwind(|| EntityTag::weak("a\tb")).is_err(),
    "HTAB must be rejected by the constructor"
  );
  assert!(
    std::panic::catch_unwind(|| EntityTag::strong("a\u{7f}b")).is_err(),
    "DEL must be rejected by the constructor"
  );
}

#[test]
fn entity_tag_trims_ows_and_rejects_non_ows_padding() {
  for value in [" \"abc\" ", "\t\"abc\"\t", " \tW/\"abc\"\t "] {
    let parsed = EntityTag::parse(value).expect("OWS-padded entity tag should parse");
    assert_eq!("abc", parsed.opaque_tag());
    assert_eq!(
      if value.contains("W/") {
        "W/\"abc\""
      } else {
        "\"abc\""
      },
      parsed.header_value()
    );
  }

  for value in [
    "\u{00a0}\"abc\"",
    "\"abc\"\u{00a0}",
    "\u{2003}W/\"abc\"",
    "W/\"abc\"\u{3000}",
    "\u{0085}\"abc\"",
  ] {
    assert!(
      EntityTag::parse(value).is_err(),
      "{value:?} must reject non-OWS padding"
    );
  }
}

#[test]
fn entity_tag_enforces_exact_serialized_and_wire_limits() {
  let exact_strong = format!(
    "\"{}\"",
    "a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"\"\"".len())
  );
  assert_eq!(MAX_ENTITY_TAG_VALUE_BYTES, exact_strong.len());
  let parsed_strong = EntityTag::parse(&exact_strong).expect("exact strong bound should parse");
  assert_eq!(
    MAX_ENTITY_TAG_VALUE_BYTES,
    parsed_strong.header_value().len()
  );

  let exact_weak = format!(
    "W/\"{}\"",
    "a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"W/\"\"".len())
  );
  assert_eq!(MAX_ENTITY_TAG_VALUE_BYTES, exact_weak.len());
  let parsed_weak = EntityTag::parse(&exact_weak).expect("exact weak bound should parse");
  assert!(parsed_weak.is_weak());
  assert_eq!(MAX_ENTITY_TAG_VALUE_BYTES, parsed_weak.header_value().len());

  let padding = " ".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"\"x\"".len());
  let exact_padded = format!("{padding}\"x\"");
  assert_eq!(MAX_ENTITY_TAG_VALUE_BYTES, exact_padded.len());
  let parsed_padded =
    EntityTag::parse(&exact_padded).expect("OWS-padded exact wire bound should parse");
  assert_eq!("x", parsed_padded.opaque_tag());
  assert_eq!("\"x\"", parsed_padded.header_value());

  assert!(EntityTag::parse(format!(
    "\"{}\"",
    "a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"\"\"".len() + 1)
  ))
  .is_err());
  assert!(EntityTag::parse(format!(
    "W/\"{}\"",
    "a".repeat(MAX_ENTITY_TAG_VALUE_BYTES - b"W/\"\"".len() + 1)
  ))
  .is_err());
  assert!(EntityTag::parse(format!(" {exact_padded}")).is_err());
}

#[test]
fn entity_tag_equality_distinguishes_weak_from_strong_and_compares_opaque_tags() {
  let strong = EntityTag::strong("abc");
  let weak = EntityTag::weak("abc");
  let other_strong = EntityTag::strong("xyz");
  let other_weak = EntityTag::weak("xyz");
  let parsed_strong = EntityTag::parse("\"abc\"").expect("strong tag");
  let parsed_weak = EntityTag::parse("W/\"abc\"").expect("weak tag");

  assert_eq!(strong, parsed_strong);
  assert_eq!(weak, parsed_weak);
  assert_ne!(strong, weak);
  assert_ne!(parsed_strong, parsed_weak);
  assert_ne!(strong, other_strong);
  assert_ne!(weak, other_weak);

  assert!(strong.strong_matches(&parsed_strong));
  assert!(!strong.strong_matches(&weak));
  assert!(!weak.strong_matches(&parsed_weak));
  assert!(!strong.strong_matches(&other_strong));

  assert!(strong.weak_matches(&weak));
  assert!(weak.weak_matches(&strong));
  assert!(weak.weak_matches(&parsed_weak));
  assert!(!strong.weak_matches(&other_strong));
  assert!(!weak.weak_matches(&other_weak));
}

#[test]
fn entity_tag_header_value_round_trips_canonical_forms() {
  for value in [
    "\"abc\"",
    "W/\"abc\"",
    "\"\"",
    "W/\"\"",
    "\"!#~\"",
    "\"a,b\"",
    "W/\"a,b\"",
    " \t\"abc\"\t ",
    "\"café\"",
    "W/\"\u{0080}\"",
  ] {
    let parsed = EntityTag::parse(value).expect("entity tag should parse");
    let header_value = parsed.header_value();
    let round_trip = EntityTag::parse(&header_value).expect("canonical header_value should parse");
    assert_eq!(round_trip, parsed);
    assert_eq!(round_trip.header_value(), header_value);
    assert!(!header_value.contains(' '));
    if parsed.is_weak() {
      assert!(header_value.starts_with("W/\""));
    } else {
      assert!(header_value.starts_with('"'));
      assert!(!header_value.starts_with("W/"));
    }
  }
}

#[test]
fn conditional_entity_tags_keep_quoted_commas_and_reject_malformed_delimiters() {
  let quoted_comma =
    IfMatch::parse("\"a,b\", W/\"c,d\"").expect("comma inside quotes should parse");
  assert_eq!(2, quoted_comma.entity_tags().len());
  assert_eq!("a,b", quoted_comma.entity_tags()[0].opaque_tag());
  assert_eq!("c,d", quoted_comma.entity_tags()[1].opaque_tag());
  assert!(quoted_comma.entity_tags()[1].is_weak());
  assert_eq!("\"a,b\", W/\"c,d\"", quoted_comma.header_value());

  for value in [
    "\"one\" \"two\"",
    "\"one\"; \"two\"",
    "\"one\",, \"two\"",
    " ,\"one\"",
    "\"one\", ",
    "\"one\" , , \"two\"",
    "\"one\", W/ \"two\"",
    "\"one\"\r\n, \"two\"",
    "\"one\",\0\"two\"",
    "w/\"one\"",
    "W /\"one\"",
    "W/\t\"one\"",
    "\"one\",\t,",
    "\"one\"\u{00a0}, \"two\"",
    "\"a\nb\", \"two\"",
    "\"a\u{7f}\", \"two\"",
  ] {
    assert!(IfMatch::parse(value).is_err(), "{value:?} must be rejected");
    assert!(
      IfNoneMatch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(IfMatch::parse_values([] as [&str; 0]).is_err());
  assert!(IfNoneMatch::parse_values([] as [&str; 0]).is_err());
  assert!(IfMatch::parse_values(["\"one\"", ""]).is_err());
  assert!(IfNoneMatch::parse_values(["   "]).is_err());
}

#[test]
fn conditional_entity_tags_reject_wildcard_mixing_and_duplicate_wildcards() {
  for value in [
    "*, \"one\"",
    "\"one\", *",
    "*,*",
    "* , *",
    "*,",
    ",*",
    "* \"one\"",
    "\"one\" *",
    "W/\"one\", *",
  ] {
    assert!(IfMatch::parse(value).is_err(), "{value:?} must be rejected");
    assert!(
      IfNoneMatch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(IfMatch::parse_values(["*", "*"]).is_err());
  assert!(IfNoneMatch::parse_values(["*", "\"one\""]).is_err());
  assert!(IfMatch::parse_values(["\"one\"", "*"]).is_err());
  assert!(IfNoneMatch::parse_values(["* ", "*"]).is_err());

  let wildcard = IfMatch::parse(" * ").expect("OWS-padded If-Match wildcard should parse");
  assert!(wildcard.is_wildcard());
  assert!(wildcard.entity_tags().is_empty());
  assert_eq!("*", wildcard.header_value());
}

#[test]
fn conditional_entity_tags_allow_strong_and_weak_duplicates_of_distinct_validators() {
  let parsed = IfNoneMatch::parse("\"one\", W/\"one\", \"two\"")
    .expect("strong and weak tags with the same opaque-tag are distinct");
  assert_eq!(3, parsed.entity_tags().len());
  assert!(!parsed.entity_tags()[0].is_weak());
  assert!(parsed.entity_tags()[1].is_weak());
  assert_eq!(
    parsed.entity_tags()[0].opaque_tag(),
    parsed.entity_tags()[1].opaque_tag()
  );
  assert_eq!("\"one\", W/\"one\", \"two\"", parsed.header_value());

  assert!(IfMatch::parse("\"one\", \"one\"").is_err());
  assert!(IfNoneMatch::parse("W/\"one\", W/\"one\"").is_err());
  assert!(IfMatch::parse_values(["\"one\"", "\"one\""]).is_err());
  assert!(IfNoneMatch::parse_values([" W/\"one\" ", "W/\"one\""]).is_err());
}

#[test]
fn conditional_entity_tags_enforce_exact_count_and_field_byte_limits() {
  let at_count = (0..MAX_CONDITIONAL_ENTITY_TAGS)
    .map(|index| format!("\"t{index}\""))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = IfMatch::parse(&at_count).expect("exactly 256 unique tags should parse");
  assert_eq!(MAX_CONDITIONAL_ENTITY_TAGS, parsed.entity_tags().len());
  IfNoneMatch::parse(&at_count).expect("exactly 256 unique If-None-Match tags should parse");

  let over_count = (0..=MAX_CONDITIONAL_ENTITY_TAGS)
    .map(|index| format!("\"t{index}\""))
    .collect::<Vec<_>>()
    .join(",");
  assert!(IfMatch::parse(&over_count).is_err());
  assert!(IfNoneMatch::parse(&over_count).is_err());

  let exact_if_match = format!(
    "\"{}\"",
    "a".repeat(MAX_IF_MATCH_VALUE_BYTES - b"\"\"".len())
  );
  assert_eq!(MAX_IF_MATCH_VALUE_BYTES, exact_if_match.len());
  IfMatch::parse(&exact_if_match).expect("exact If-Match byte bound should parse");
  assert!(IfMatch::parse(format!("\"{}\"", "a".repeat(MAX_IF_MATCH_VALUE_BYTES - 1))).is_err());

  let exact_if_none_match = format!(
    "\"{}\"",
    "a".repeat(MAX_IF_NONE_MATCH_VALUE_BYTES - b"\"\"".len())
  );
  assert_eq!(MAX_IF_NONE_MATCH_VALUE_BYTES, exact_if_none_match.len());
  IfNoneMatch::parse(&exact_if_none_match).expect("exact If-None-Match byte bound should parse");
  assert!(IfNoneMatch::parse(format!(
    "\"{}\"",
    "a".repeat(MAX_IF_NONE_MATCH_VALUE_BYTES - 1)
  ))
  .is_err());

  let oversized = "x".repeat(MAX_IF_MATCH_VALUE_BYTES + 1);
  assert!(IfMatch::parse_values(["\"ok\"", oversized.as_str()]).is_err());
  assert!(IfNoneMatch::parse_values([oversized.as_str(), "\"ok\""]).is_err());

  let first_half = (0..MAX_CONDITIONAL_ENTITY_TAGS / 2)
    .map(|index| format!("\"t{index}\""))
    .collect::<Vec<_>>()
    .join(",");
  let second_half = (MAX_CONDITIONAL_ENTITY_TAGS / 2..MAX_CONDITIONAL_ENTITY_TAGS)
    .map(|index| format!("\"t{index}\""))
    .collect::<Vec<_>>()
    .join(",");
  let split_at_count = IfMatch::parse_values([first_half.as_str(), second_half.as_str()])
    .expect("exactly 256 unique tags split across fields should parse");
  assert_eq!(
    MAX_CONDITIONAL_ENTITY_TAGS,
    split_at_count.entity_tags().len()
  );

  let extra = format!("\"t{MAX_CONDITIONAL_ENTITY_TAGS}\"");
  assert!(IfMatch::parse_values([at_count.as_str(), extra.as_str()]).is_err());
  assert!(IfNoneMatch::parse_values([at_count.as_str(), extra.as_str()]).is_err());
}

#[test]
fn conditional_entity_tags_round_trip_canonical_header_values() {
  for value in [
    "*",
    " * ",
    "\"one\"",
    "W/\"two\"",
    "\"one\",W/\"two\"",
    " \"one\" , W/\"two\" , \"a,b\" ",
    "W/\"café\", \"\u{0080}\"",
  ] {
    let parsed = IfMatch::parse(value).expect("If-Match should parse");
    let header_value = parsed.header_value();
    let round_trip = IfMatch::parse(&header_value).expect("canonical If-Match should parse");
    assert_eq!(round_trip, parsed);
    assert_eq!(round_trip.header_value(), header_value);

    let none_match = IfNoneMatch::parse(value).expect("If-None-Match should parse");
    let none_header = none_match.header_value();
    let none_round_trip =
      IfNoneMatch::parse(&none_header).expect("canonical If-None-Match should parse");
    assert_eq!(none_round_trip, none_match);
    assert_eq!(none_round_trip.header_value(), none_header);
    assert_eq!(none_header, header_value);
  }
}
