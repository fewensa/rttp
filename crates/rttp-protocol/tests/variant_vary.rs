use rttp_protocol::variant_vary::{
  VariantVary, MAX_VARIANT_VARY_FIELD_NAMES, MAX_VARIANT_VARY_TOTAL_BYTES,
  MAX_VARIANT_VARY_VALUE_BYTES,
};

#[test]
fn variant_vary_parses_wildcard() {
  let variant_vary = VariantVary::parse("*").expect("valid wildcard Variant-Vary");

  assert!(variant_vary.is_any());
  assert_eq!(Vec::<&str>::new(), variant_vary.field_names());
  assert_eq!(0, variant_vary.len());
  assert!(variant_vary.is_empty());
  assert_eq!("*", variant_vary.header_value());
}

#[test]
fn variant_vary_normalizes_singleton_field_names_case_insensitively() {
  let variant_vary = VariantVary::parse("Accept-Language").expect("valid Variant-Vary field name");

  assert!(!variant_vary.is_any());
  assert_eq!(vec!["accept-language"], variant_vary.field_names());
  assert_eq!(1, variant_vary.len());
  assert!(!variant_vary.is_empty());
  assert_eq!("accept-language", variant_vary.header_value());
}

#[test]
fn variant_vary_parses_comma_lists_and_optional_whitespace() {
  let variant_vary =
    VariantVary::parse(" Accept-Language , Sec-CH-DPR ").expect("valid Variant-Vary field names");

  assert_eq!(
    vec!["accept-language", "sec-ch-dpr"],
    variant_vary.field_names()
  );
  assert_eq!("accept-language, sec-ch-dpr", variant_vary.header_value());
}

#[test]
fn variant_vary_parses_repeated_header_values_preserving_order() {
  let variant_vary = VariantVary::parse_values(["Accept-Language", "Sec-CH-DPR", "User-Agent"])
    .expect("valid Variant-Vary field names");

  assert_eq!(
    vec!["accept-language", "sec-ch-dpr", "user-agent"],
    variant_vary.field_names()
  );
  assert_eq!(
    "accept-language, sec-ch-dpr, user-agent",
    variant_vary.header_value()
  );
}

#[test]
fn variant_vary_rejects_empty_members() {
  for value in [
    "",
    " ",
    ",",
    "Accept-Language,",
    ",Accept-Language",
    "Accept-Language,,User-Agent",
  ] {
    assert!(
      VariantVary::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn variant_vary_rejects_control_bytes_except_horizontal_tab() {
  assert!(VariantVary::parse("\rAccept-Language").is_err());
  assert!(VariantVary::parse("Accept-Language\n").is_err());
  assert!(VariantVary::parse("Accept-Language\0").is_err());
  assert_eq!(
    vec!["accept-language"],
    VariantVary::parse("\tAccept-Language\t")
      .expect("tab OWS is valid")
      .field_names()
  );
}

#[test]
fn variant_vary_rejects_non_token_members() {
  assert!(VariantVary::parse("Accept Language").is_err());
  assert!(VariantVary::parse("Accept-Language;q=1").is_err());
  assert!(VariantVary::parse("Accept-Language=").is_err());
}

#[test]
fn variant_vary_rejects_wildcard_mixed_with_field_names() {
  assert!(VariantVary::parse("*, Accept-Language").is_err());
  assert!(VariantVary::parse("Accept-Language, *").is_err());
  assert!(VariantVary::parse_values(["*", "Accept-Language"]).is_err());
  assert!(VariantVary::parse_values(["Accept-Language", "*"]).is_err());
}

#[test]
fn variant_vary_rejects_duplicate_field_names_case_insensitively() {
  assert!(VariantVary::parse("Accept-Language, accept-language").is_err());
  assert!(VariantVary::parse("Accept-Language, ACCEPT-LANGUAGE").is_err());
  assert!(VariantVary::parse_values(["Accept-Language", "accept-language"]).is_err());
}

#[test]
fn variant_vary_rejects_duplicate_wildcards() {
  assert!(VariantVary::parse("*, *").is_err());
  assert!(VariantVary::parse_values(["*", "*"]).is_err());
}

#[test]
fn variant_vary_rejects_field_name_list_overflow() {
  let too_many = (0..=MAX_VARIANT_VARY_FIELD_NAMES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");

  assert!(VariantVary::parse(too_many).is_err());
}

#[test]
fn variant_vary_rejects_oversized_field_values() {
  let oversized = "a".repeat(MAX_VARIANT_VARY_VALUE_BYTES + 1);
  assert!(VariantVary::parse(oversized).is_err());
}

#[test]
fn variant_vary_rejects_oversized_canonical_values() {
  let first = "a".repeat(MAX_VARIANT_VARY_TOTAL_BYTES / 2 + 1);
  let second = "b".repeat(MAX_VARIANT_VARY_TOTAL_BYTES / 2 + 1);
  assert!(first.len() <= MAX_VARIANT_VARY_VALUE_BYTES);
  assert!(second.len() <= MAX_VARIANT_VARY_VALUE_BYTES);
  assert!(first.len() + second.len() + 2 > MAX_VARIANT_VARY_TOTAL_BYTES);
  assert!(VariantVary::parse_values([first.as_str(), second.as_str()]).is_err());
}

#[test]
fn variant_vary_contains_field_name_is_case_insensitive_and_rejects_invalid_tokens() {
  let variant_vary =
    VariantVary::parse("Accept-Language, Sec-CH-DPR").expect("valid Variant-Vary field names");

  assert!(variant_vary.contains_field_name("ACCEPT-LANGUAGE"));
  assert!(variant_vary.contains_field_name("sec-ch-dpr"));
  assert!(!variant_vary.contains_field_name("user-agent"));
  assert!(!variant_vary.contains_field_name("Accept Language"));
  assert!(!variant_vary.contains_field_name(""));

  let wildcard = VariantVary::parse("*").expect("valid wildcard Variant-Vary");
  assert!(!wildcard.contains_field_name("accept-language"));
}
