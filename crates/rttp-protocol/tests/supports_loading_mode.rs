use rttp_protocol::supports_loading_mode::{
  SupportsLoadingMode, MAX_SUPPORTS_LOADING_MODE_TOKENS, MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES,
};

#[test]
fn supports_loading_mode_parses_known_tokens_and_reformats() {
  let modes = SupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
    .expect("known loading-mode tokens should parse");

  assert_eq!(modes.tokens(), ["fenced-frame", "credentialed-prerender"]);
  assert!(modes.contains_fenced_frame());
  assert!(modes.contains_credentialed_prerender());
  assert!(!modes.contains_prerender_cross_origin_frames());
  assert!(modes.contains("CREDENTIALED-PRERENDER"));
  assert!(!modes.contains("unknown-mode"));
  assert_eq!(modes.header_value(), "fenced-frame, credentialed-prerender");
}

#[test]
fn supports_loading_mode_parses_all_defined_tokens() {
  let modes = SupportsLoadingMode::parse(
    "fenced-frame, credentialed-prerender, prerender-cross-origin-frames",
  )
  .expect("all defined loading-mode tokens should parse");

  assert_eq!(
    modes.tokens(),
    [
      "fenced-frame",
      "credentialed-prerender",
      "prerender-cross-origin-frames"
    ]
  );
  assert!(modes.contains_fenced_frame());
  assert!(modes.contains_credentialed_prerender());
  assert!(modes.contains_prerender_cross_origin_frames());
  assert_eq!(
    modes.header_value(),
    "fenced-frame, credentialed-prerender, prerender-cross-origin-frames"
  );
}

#[test]
fn supports_loading_mode_retains_unknown_well_formed_tokens() {
  let modes = SupportsLoadingMode::parse("uncredentialed-prerender, vendor-mode")
    .expect("unknown well-formed tokens should be retained");

  assert_eq!(modes.tokens(), ["uncredentialed-prerender", "vendor-mode"]);
  assert!(modes.contains("uncredentialed-prerender"));
  assert!(!modes.contains_fenced_frame());
  assert_eq!(
    modes.header_value(),
    "uncredentialed-prerender, vendor-mode"
  );
}

#[test]
fn supports_loading_mode_combines_fields_in_wire_order() {
  let modes = SupportsLoadingMode::parse_values(["fenced-frame", "credentialed-prerender"])
    .expect("combined Supports-Loading-Mode fields should parse");

  assert_eq!(modes.tokens(), ["fenced-frame", "credentialed-prerender"]);
  assert_eq!(modes.header_value(), "fenced-frame, credentialed-prerender");
}

#[test]
fn supports_loading_mode_from_tokens_validates_and_deduplicates() {
  let modes = SupportsLoadingMode::from_tokens(["fenced-frame", "credentialed-prerender"])
    .expect("declared tokens should parse");

  assert_eq!(modes.tokens(), ["fenced-frame", "credentialed-prerender"]);
  assert_eq!(modes.header_value(), "fenced-frame, credentialed-prerender");
  assert!(
    SupportsLoadingMode::from_tokens(["fenced-frame", "Fenced-Frame"]).is_err(),
    "case-insensitive duplicates must be rejected"
  );
  assert!(
    SupportsLoadingMode::from_tokens(["?not-a-token"]).is_err(),
    "non-token members must be rejected"
  );
}

#[test]
fn supports_loading_mode_accepts_structured_tokens_with_colon_and_slash() {
  let parsed = SupportsLoadingMode::parse("vendor/mode, vendor:mode")
    .expect("Structured Fields tokens may contain ':' and '/'");
  assert_eq!(parsed.tokens(), ["vendor/mode", "vendor:mode"]);
  assert_eq!(parsed.header_value(), "vendor/mode, vendor:mode");

  let declared = SupportsLoadingMode::from_tokens(["vendor/mode", "vendor:mode"])
    .expect("from_tokens must accept the same Structured Fields token grammar");
  assert_eq!(declared.tokens(), ["vendor/mode", "vendor:mode"]);
  assert_eq!(declared.header_value(), "vendor/mode, vendor:mode");
  assert_eq!(
    SupportsLoadingMode::parse(declared.header_value()).expect("canonical header must round-trip"),
    declared
  );
}

#[test]
fn supports_loading_mode_retains_first_seen_spelling() {
  let modes =
    SupportsLoadingMode::parse("Fenced-Frame").expect("mixed-case well-formed token should parse");

  assert_eq!(modes.tokens(), ["Fenced-Frame"]);
  assert!(!modes.contains_fenced_frame());
  assert!(modes.contains("fenced-frame"));
  assert_eq!(modes.header_value(), "Fenced-Frame");
}

#[test]
fn supports_loading_mode_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    "fenced-frame credentialed-prerender",
    "fenced-frame,,credentialed-prerender",
    "fenced-frame,",
    ",fenced-frame",
    "?1",
    "5",
    "1.5",
    ":YWJj:",
    "@123",
    "\"fenced-frame\"",
    "(fenced-frame)",
    "fenced-frame;foo=bar",
    "fenced-frame;foo",
    "fenced-frame\tcredentialed-prerender",
  ] {
    assert!(
      SupportsLoadingMode::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn supports_loading_mode_rejects_duplicates_and_empty_field_sets() {
  assert!(
    SupportsLoadingMode::parse("fenced-frame, fenced-frame").is_err(),
    "exact duplicates must be rejected"
  );
  assert!(
    SupportsLoadingMode::parse("fenced-frame, Fenced-Frame").is_err(),
    "case-insensitive duplicates must be rejected"
  );
  assert!(
    SupportsLoadingMode::parse_values(["fenced-frame", "FENCED-FRAME"]).is_err(),
    "duplicates across fields must be rejected"
  );
  assert!(
    SupportsLoadingMode::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    SupportsLoadingMode::from_tokens(std::iter::empty::<&str>()).is_err(),
    "empty token sets must be rejected"
  );
}

#[test]
fn supports_loading_mode_enforces_value_and_token_bounds() {
  assert!(
    SupportsLoadingMode::parse("x".repeat(MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = "x".repeat(MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES + 1);
  assert!(
    SupportsLoadingMode::parse_values(["fenced-frame", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let half = "fenced-frame".repeat(5000);
  let other_half = "fenced-frame".repeat(5000);
  assert!(
    SupportsLoadingMode::parse_values([half.as_str(), other_half.as_str()]).is_err(),
    "combined values over 64 KiB must be rejected"
  );

  let at_limit = (0..MAX_SUPPORTS_LOADING_MODE_TOKENS)
    .map(|index| format!("mode{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed = SupportsLoadingMode::parse(&at_limit).expect("256 tokens should parse");
  assert_eq!(parsed.tokens().len(), MAX_SUPPORTS_LOADING_MODE_TOKENS);

  let too_many = (0..=MAX_SUPPORTS_LOADING_MODE_TOKENS)
    .map(|index| format!("mode{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    SupportsLoadingMode::parse(&too_many).is_err(),
    "more than 256 tokens must be rejected"
  );
  assert!(
    SupportsLoadingMode::from_tokens(
      (0..=MAX_SUPPORTS_LOADING_MODE_TOKENS).map(|index| format!("mode{index}"))
    )
    .is_err(),
    "from_tokens must reject more than 256 tokens"
  );

  let first = format!("a{}", "x".repeat(32_766));
  let second = format!("b{}", "x".repeat(32_766));
  assert_eq!(
    first.len() + second.len() + 2,
    MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES
  );
  let at_serialized_limit = SupportsLoadingMode::from_tokens([&first, &second])
    .expect("canonical serialized value at 64 KiB should be accepted");
  assert_eq!(
    at_serialized_limit.header_value().len(),
    MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES
  );

  let over_first = format!("a{}", "x".repeat(32_767));
  let over_second = format!("b{}", "x".repeat(32_767));
  assert!(over_first.len() + over_second.len() <= MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES);
  assert!(over_first.len() + over_second.len() + 2 > MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES);
  assert!(
    SupportsLoadingMode::from_tokens([&over_first, &over_second]).is_err(),
    "from_tokens must reject lists whose comma-space serialization exceeds 64 KiB"
  );
}
