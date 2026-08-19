use rttp_protocol::range::{ByteRangeSpec, ContentRange, Range, MAX_RANGE_COUNT};

#[test]
fn range_parses_byte_and_suffix_specs_from_one_field_value() {
  let range = Range::parse("bytes=0-499, 500-, -200").expect("valid range");

  assert_eq!(
    &[
      ByteRangeSpec::FromTo {
        start: 0,
        end: Some(499)
      },
      ByteRangeSpec::FromTo {
        start: 500,
        end: None,
      },
      ByteRangeSpec::Suffix { length: 200 },
    ],
    range.ranges()
  );
  assert_eq!("bytes=0-499, 500-, -200", range.header_value());
}

#[test]
fn range_rejects_repeated_field_values() {
  assert!(Range::parse_values(["bytes=0-1", "bytes=2-3"]).is_err());
}

#[test]
fn content_range_parses_satisfied_unknown_and_unsatisfied_forms() {
  let satisfied = ContentRange::parse("bytes 0-499/1234").expect("satisfied content range");
  assert_eq!(
    ContentRange::Bytes {
      start: 0,
      end: 499,
      complete_length: Some(1_234),
    },
    satisfied
  );
  assert_eq!(
    ContentRange::Bytes {
      start: 500,
      end: 999,
      complete_length: None,
    },
    ContentRange::parse("bytes 500-999/*").expect("unknown complete length")
  );
  assert_eq!(
    ContentRange::Unsatisfied {
      complete_length: 1_234,
    },
    ContentRange::parse("bytes */1234").expect("unsatisfied content range")
  );
  assert_eq!("bytes", satisfied.unit());
  assert_eq!(Some(0), satisfied.start());
  assert_eq!(Some(499), satisfied.end());
  assert_eq!(Some(1_234), satisfied.complete_length());
  assert!(!satisfied.is_unsatisfied());
  assert_eq!("bytes 0-499/1234", satisfied.header_value());
}

#[test]
fn content_range_rejects_repeated_field_values() {
  assert!(ContentRange::parse_values(["bytes 0-1/4", "bytes 2-3/4"]).is_err());
}

#[test]
fn range_and_content_range_reject_invalid_syntax_controls_and_overflow() {
  for value in [
    "items=0-1",
    "bytes=1-0",
    "bytes=0--1",
    "bytes=18446744073709551616-1",
    "bytes=0-1\r\nInjected: yes",
  ] {
    assert!(Range::parse(value).is_err(), "{value:?} must be rejected");
  }

  for value in [
    "items 0-1/2",
    "bytes 2-1/2",
    "bytes 0-2/2",
    "bytes */*",
    "bytes */18446744073709551616",
    "bytes 0-1/2\n",
  ] {
    assert!(
      ContentRange::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn range_enforces_the_member_limit() {
  let values = (0..=MAX_RANGE_COUNT)
    .map(|index| format!("{index}-{index}"))
    .collect::<Vec<_>>()
    .join(",");

  assert!(Range::parse(format!("bytes={values}")).is_err());
}
