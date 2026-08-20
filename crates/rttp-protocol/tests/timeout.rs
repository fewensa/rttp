use rttp_protocol::timeout::{
  Timeout, TimeoutType, MAX_TIMEOUT_MEMBERS, MAX_TIMEOUT_TOTAL_BYTES, MAX_TIMEOUT_VALUE_BYTES,
};

#[test]
fn parses_valid_timeout_values_in_wire_order() {
  let timeout = Timeout::parse("Second-60, Infinite, second-120").expect("Timeout should parse");

  assert_eq!(
    &[
      TimeoutType::Second(60),
      TimeoutType::Infinite,
      TimeoutType::Second(120)
    ],
    timeout.members()
  );
  assert_eq!("second-60, infinite, second-120", timeout.header_value());
}

#[test]
fn parses_multiple_timeout_field_values_as_one_ordered_list() {
  let timeout =
    Timeout::parse_values(["Second-30", "Infinite", "Second-60"]).expect("Timeout should parse");

  assert_eq!(
    &[
      TimeoutType::Second(30),
      TimeoutType::Infinite,
      TimeoutType::Second(60)
    ],
    timeout.members()
  );
  assert_eq!("second-30, infinite, second-60", timeout.header_value());
}

#[test]
fn formats_canonical_timeout_members() {
  assert_eq!("infinite", TimeoutType::Infinite.header_value());
  assert_eq!("second-0", TimeoutType::Second(0).header_value());
  assert_eq!(
    "second-18446744073709551615",
    TimeoutType::Second(u64::MAX).header_value()
  );
}

#[test]
fn rejects_malformed_timeout_values() {
  for value in [
    "",
    ",",
    "Second-",
    "Second--1",
    "Second-1.0",
    "Second=1",
    "Seconds-1",
    "Finite",
    "Infinite;foo=bar",
    "Second-1,",
    ",Second-1",
    "Second-1,,Infinite",
  ] {
    assert!(
      Timeout::parse(value).is_err(),
      "Timeout should reject {value:?}"
    );
  }
}

#[test]
fn rejects_timeout_seconds_overflow() {
  assert!(Timeout::parse("Second-18446744073709551616").is_err());
}

#[test]
fn rejects_duplicate_timeout_members() {
  assert!(Timeout::parse("Second-60, second-60").is_err());
  assert!(Timeout::parse("Infinite, infinite").is_err());
  assert!(Timeout::parse_values(["Second-60", "second-60"]).is_err());
}

#[test]
fn rejects_too_many_timeout_members() {
  let value = (0..=MAX_TIMEOUT_MEMBERS)
    .map(|seconds| format!("Second-{seconds}"))
    .collect::<Vec<_>>()
    .join(", ");

  assert!(Timeout::parse(value).is_err());
}

#[test]
fn rejects_oversized_timeout_values() {
  let oversized = format!("Second-{}", "1".repeat(MAX_TIMEOUT_VALUE_BYTES));
  assert!(Timeout::parse(oversized).is_err());
}

#[test]
fn rejects_oversized_timeout_aggregate_values() {
  let first = format!("{}Second-1", " ".repeat(MAX_TIMEOUT_TOTAL_BYTES / 2));
  let second = format!("{}Infinite", " ".repeat(MAX_TIMEOUT_TOTAL_BYTES / 2));

  assert!(Timeout::parse_values([first.as_str(), second.as_str()]).is_err());
}

#[test]
fn rejects_timeout_control_bytes_except_horizontal_tab() {
  assert!(Timeout::parse("Second-1\r").is_err());
  assert!(Timeout::parse("Second-1\n").is_err());
  assert!(Timeout::parse("Second-1\0").is_err());
  assert_eq!(
    &[TimeoutType::Second(1)],
    Timeout::parse("\tSecond-1\t")
      .expect("tab OWS is valid")
      .members()
  );
}
