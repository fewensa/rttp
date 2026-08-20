use rttp_protocol::schedule_tag::{
  ScheduleTag, ScheduleTagParseError, MAX_SCHEDULE_TAG_VALUE_BYTES,
};

#[test]
fn parses_valid_strong_schedule_tag_validators() {
  let validator = ScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse");

  assert_eq!("sched-17", validator.opaque_tag());
  assert!(!validator.is_weak());
  assert_eq!("\"sched-17\"", validator.entity_tag().header_value());
  assert_eq!("\"sched-17\"", validator.header_value());
  assert_eq!(
    validator,
    ScheduleTag::parse(validator.header_value()).expect("canonical value should reparse")
  );
}

#[test]
fn parses_valid_weak_schedule_tag_validators() {
  let validator = ScheduleTag::parse("W/\"sched-17\"").expect("weak Schedule-Tag should parse");

  assert_eq!("sched-17", validator.opaque_tag());
  assert!(validator.is_weak());
  assert_eq!("W/\"sched-17\"", validator.entity_tag().header_value());
  assert_eq!("W/\"sched-17\"", validator.header_value());
}

#[test]
fn trims_outer_optional_whitespace_and_emits_canonical_tag() {
  let validator = ScheduleTag::parse(" \t\"sched-17\"\t ").expect("Schedule-Tag should parse");

  assert_eq!("sched-17", validator.opaque_tag());
  assert_eq!("\"sched-17\"", validator.header_value());
}

#[test]
fn accepts_an_empty_opaque_tag() {
  let validator = ScheduleTag::parse("\"\"").expect("empty entity tag should parse");

  assert_eq!("", validator.opaque_tag());
  assert_eq!("\"\"", validator.header_value());
}

#[test]
fn rejects_malformed_schedule_tag_validators() {
  for value in [
    "",
    " \t ",
    "sched-17",
    "\"unterminated",
    "W/\"unterminated",
    "\"sched-17\" trailing",
    "\"one\", \"two\"",
    "*",
    "W/*",
    "\"a\"b\"",
  ] {
    assert!(
      ScheduleTag::parse(value).is_err(),
      "Schedule-Tag should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_schedule_tag_fields() {
  let error: ScheduleTagParseError = ScheduleTag::parse_values(["\"sched-16\"", "\"sched-17\""])
    .expect_err("duplicate Schedule-Tag fields should be rejected");

  assert_eq!("duplicate Schedule-Tag header fields", error.to_string());
}

#[test]
fn rejects_duplicate_fields_even_when_later_fields_are_oversized() {
  let oversized = "a".repeat(MAX_SCHEDULE_TAG_VALUE_BYTES + 1);
  assert!(ScheduleTag::parse_values(["\"sched-17\"", &oversized]).is_err());
}

#[test]
fn rejects_oversized_schedule_tag_values() {
  let oversized = format!("\"{}\"", "a".repeat(MAX_SCHEDULE_TAG_VALUE_BYTES - 1));
  assert!(ScheduleTag::parse(oversized).is_err());
}

#[test]
fn accepts_a_value_at_exactly_the_size_bound() {
  let at_bound = format!("\"{}\"", "a".repeat(MAX_SCHEDULE_TAG_VALUE_BYTES - 2));
  assert_eq!(at_bound.len(), MAX_SCHEDULE_TAG_VALUE_BYTES);

  let validator = ScheduleTag::parse(at_bound).expect("bound-sized value should parse");
  assert_eq!(validator.header_value().len(), MAX_SCHEDULE_TAG_VALUE_BYTES);
}

#[test]
fn rejects_schedule_tag_control_byte_injection() {
  assert!(ScheduleTag::parse("\"sched-17\"\r\nX: y").is_err());
  assert!(ScheduleTag::parse("\"sched-17\"\n").is_err());
  assert!(ScheduleTag::parse("\"sched-17\"\0").is_err());
  assert!(ScheduleTag::parse("\"sched-17\"\u{7f}").is_err());
  assert!(ScheduleTag::parse("\"sched\t-17\"").is_err());
}

#[test]
fn rejects_missing_schedule_tag_values() {
  assert!(ScheduleTag::parse_values([] as [&str; 0]).is_err());
}
