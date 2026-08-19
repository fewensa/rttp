use rttp_protocol::idempotency_key::{IdempotencyKey, MAX_IDEMPOTENCY_KEY_VALUE_BYTES};

#[test]
fn idempotency_key_preserves_opaque_visible_keys_and_normalizes_ows() {
  for (value, expected) in [
    ("charge-2026-08-19-9f3c", "charge-2026-08-19-9f3c"),
    (
      "urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00",
      "urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00",
    ),
    ("A", "A"),
    ("\"quoted\"", "\"quoted\""),
    (" \tcharge-2026-08-19-9f3c\t ", "charge-2026-08-19-9f3c"),
  ] {
    let key = IdempotencyKey::parse(value).expect("visible key should parse");
    assert_eq!(key.as_str(), expected);
    assert_eq!(key.header_value(), expected);
  }

  let constructed =
    IdempotencyKey::new("charge-2026-08-19-9f3c").expect("new should behave like parse");
  assert_eq!(constructed.as_str(), "charge-2026-08-19-9f3c");
  assert_eq!(constructed.header_value(), "charge-2026-08-19-9f3c");
}

#[test]
fn idempotency_key_rejects_empty_invisible_injected_and_obs_text_values() {
  for value in [
    "",
    " ",
    "\t",
    "key with space",
    "key\r\nX-Injected: 1",
    "key\rX: y",
    "key\nX: y",
    "key\0value",
    "key\u{1}value",
    "key\u{7f}value",
    "key\u{80}value",
  ] {
    assert!(
      IdempotencyKey::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn idempotency_key_rejects_duplicate_fields() {
  assert!(IdempotencyKey::parse_values(["charge-1", "charge-2"]).is_err());
  assert!(IdempotencyKey::parse_values([]).is_err());
}

#[test]
fn idempotency_key_enforces_value_bounds() {
  assert!(
    IdempotencyKey::parse("x".repeat(MAX_IDEMPOTENCY_KEY_VALUE_BYTES)).is_ok(),
    "a value at the 64 KiB bound should parse"
  );
  assert!(
    IdempotencyKey::parse("x".repeat(MAX_IDEMPOTENCY_KEY_VALUE_BYTES + 1)).is_err(),
    "a value over the 64 KiB bound should be rejected"
  );
}

#[test]
fn idempotency_key_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_IDEMPOTENCY_KEY_VALUE_BYTES + 1);

  assert!(
    IdempotencyKey::parse_values(["charge-1", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
  assert!(
    IdempotencyKey::parse_values([oversized.as_str(), "charge-1"]).is_err(),
    "an oversized first field must not bypass validation"
  );
}

#[test]
fn idempotency_key_debug_and_errors_redact_the_key() {
  let key = IdempotencyKey::parse("charge-2026-08-19-9f3c").expect("key should parse");
  let debug = format!("{key:?}");
  assert!(debug.contains("IdempotencyKey"));
  assert!(debug.contains("[REDACTED]"));
  assert!(!debug.contains("charge-2026-08-19-9f3c"));

  let error = IdempotencyKey::parse("charge-2026-08-19-9f3c\r\nX-Injected: 1")
    .expect_err("injected key should be rejected");
  let message = error.to_string();
  assert!(message.contains("Idempotency-Key"));
  assert!(!message.contains("charge-2026-08-19-9f3c"));
  assert!(!message.contains("X-Injected"));

  let duplicate = IdempotencyKey::parse_values(["secret-a", "secret-b"])
    .expect_err("duplicate fields should be rejected");
  let duplicate_message = duplicate.to_string();
  assert!(duplicate_message.contains("duplicate"));
  assert!(!duplicate_message.contains("secret-a"));
  assert!(!duplicate_message.contains("secret-b"));

  let oversized = IdempotencyKey::parse("x".repeat(MAX_IDEMPOTENCY_KEY_VALUE_BYTES + 1))
    .expect_err("oversized key should be rejected");
  let oversized_message = oversized.to_string();
  assert!(oversized_message.contains("too large"));
  assert!(oversized_message.contains("Idempotency-Key"));
}
