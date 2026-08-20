use rttp_protocol::delta_base::{DeltaBase, MAX_DELTA_BASE_VALUE_BYTES};
use rttp_protocol::entity_tag::EntityTag;

#[test]
fn delta_base_parses_strong_and_weak_entity_tags() {
  let strong = DeltaBase::parse("\"asset-v7\"").expect("strong Delta-Base should parse");
  assert_eq!(EntityTag::strong("asset-v7"), *strong.entity_tag());
  assert_eq!("\"asset-v7\"", strong.header_value());

  let weak = DeltaBase::parse(" W/\"asset-v7\" ").expect("weak Delta-Base should parse");
  assert_eq!(EntityTag::weak("asset-v7"), *weak.entity_tag());
  assert_eq!("W/\"asset-v7\"", weak.header_value());
  assert_eq!(EntityTag::weak("asset-v7"), weak.into_entity_tag());
}

#[test]
fn delta_base_rejects_missing_malformed_duplicate_and_list_values() {
  assert!(DeltaBase::parse_values(std::iter::empty()).is_err());
  assert!(DeltaBase::parse_values(["\"one\"", "\"two\""]).is_err());

  for value in [
    "",
    "*",
    "asset-v7",
    "W/asset-v7",
    "\"bad space\"",
    "\"one\", \"two\"",
    "\"one\", \"one\"",
    "\"asset-v7\"\r\nX-Injected: 1",
  ] {
    assert!(
      DeltaBase::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn delta_base_enforces_64k_value_bound() {
  let at_bound = format!(
    "\"{}\"",
    "a".repeat(MAX_DELTA_BASE_VALUE_BYTES - b"\"\"".len())
  );
  assert_eq!(MAX_DELTA_BASE_VALUE_BYTES, at_bound.len());
  DeltaBase::parse(&at_bound).expect("at-bound Delta-Base should parse");

  let oversized = format!("\"{}\"", "a".repeat(MAX_DELTA_BASE_VALUE_BYTES));
  assert!(
    DeltaBase::parse(&oversized).is_err(),
    "oversized Delta-Base must be rejected"
  );
}
