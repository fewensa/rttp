use rttp_protocol::timing_allow_origin::{
  TimingAllowOrigin, MAX_TIMING_ALLOW_ORIGIN_ORIGINS, MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES,
};

#[test]
fn timing_allow_origin_parses_wildcard_and_origin_lists_across_fields() {
  let wildcard = TimingAllowOrigin::parse("*").expect("wildcard should parse");
  assert!(wildcard.is_wildcard());
  assert!(wildcard.origins().is_empty());
  assert_eq!("*", wildcard.header_value());

  let origins = TimingAllowOrigin::parse_values([
    "https://example.test, https://api.example.test",
    "https://static.example.test",
  ])
  .expect("origin list should parse");
  assert!(!origins.is_wildcard());
  assert_eq!(
    origins.origins(),
    [
      "https://example.test",
      "https://api.example.test",
      "https://static.example.test",
    ]
  );
  assert_eq!(
    origins.header_value(),
    "https://example.test, https://api.example.test, https://static.example.test"
  );
}

#[test]
fn timing_allow_origin_rejects_malformed_duplicate_and_mixed_values() {
  for value in [
    "",
    "https://example.test,",
    ",https://example.test",
    "https://example.test,,https://api.example.test",
    "https://example.test/path",
    "https://example.test\u{7f}",
    "https://example.test\n",
    "*, https://example.test",
    "https://example.test, https://example.test",
  ] {
    assert!(
      TimingAllowOrigin::parse(value).is_err(),
      "should reject {value:?}"
    );
  }
  assert!(TimingAllowOrigin::parse_values(["*", "*"]).is_err());
}

#[test]
fn timing_allow_origin_enforces_value_and_origin_bounds() {
  assert!(TimingAllowOrigin::parse("x".repeat(MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES + 1)).is_err());

  let too_many = (0..=MAX_TIMING_ALLOW_ORIGIN_ORIGINS)
    .map(|index| format!("https://{index}.example.test"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(TimingAllowOrigin::parse(too_many).is_err());
}
