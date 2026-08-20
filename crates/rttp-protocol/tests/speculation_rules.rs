use rttp_protocol::speculation_rules::{SpeculationRules, MAX_SPECULATION_RULES_VALUE_BYTES};

#[test]
fn speculation_rules_preserves_opaque_value() {
  let value = r#"https://example.test/rules.json"#;
  let rules = SpeculationRules::parse(value).expect("Speculation-Rules should parse");

  assert_eq!(rules.as_str(), value);
  assert_eq!(rules.header_value(), value);
}

#[test]
fn speculation_rules_rejects_duplicate_singleton_fields() {
  let error = SpeculationRules::parse_values([
    "https://example.test/one.json",
    "https://example.test/two.json",
  ])
  .expect_err("duplicate Speculation-Rules fields should fail closed");

  assert_eq!(
    error.to_string(),
    "duplicate Speculation-Rules header fields"
  );
}

#[test]
fn speculation_rules_rejects_control_byte_injection() {
  let error = SpeculationRules::parse("https://example.test/rules.json\r\nInjected: yes")
    .expect_err("control bytes should be rejected");

  assert_eq!(
    error.to_string(),
    "Speculation-Rules header value contains an invalid control byte"
  );
}

#[test]
fn speculation_rules_accepts_explicit_size_bound() {
  let at_limit = "a".repeat(MAX_SPECULATION_RULES_VALUE_BYTES);
  assert!(SpeculationRules::parse(&at_limit).is_ok());

  let oversized = "a".repeat(MAX_SPECULATION_RULES_VALUE_BYTES + 1);
  let error = SpeculationRules::parse(&oversized).expect_err("oversized value should fail");
  assert_eq!(
    error.to_string(),
    "Speculation-Rules header value is too large"
  );
}

#[test]
fn speculation_rules_debug_and_errors_do_not_dump_value() {
  let secret = "https://example.test/private-rules.json";
  let rules = SpeculationRules::parse(secret).expect("Speculation-Rules should parse");
  let debug = format!("{rules:?}");

  assert!(debug.contains("value_bytes"));
  assert!(!debug.contains(secret));

  let error =
    SpeculationRules::parse(format!("{secret}\n")).expect_err("injected value should be rejected");
  let error_text = format!("{error:?} {error}");
  assert!(!error_text.contains(secret));
}
