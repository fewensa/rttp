use rttp_protocol::entity_tag::{
  EntityTag, IfMatch, IfNoneMatch, MAX_CONDITIONAL_ENTITY_TAGS, MAX_ENTITY_TAG_VALUE_BYTES,
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
