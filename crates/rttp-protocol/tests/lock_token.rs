use rttp_protocol::lock_token::{LockToken, MAX_LOCK_TOKEN_VALUE_BYTES};

const OPAQUE_LOCK_TOKEN: &str = "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>";
const HTTP_LOCK_TOKEN: &str = "<http://example.test/locks/1>";

#[test]
fn lock_token_preserves_coded_urls_and_normalizes_ows() {
  for (value, expected) in [
    (OPAQUE_LOCK_TOKEN, OPAQUE_LOCK_TOKEN),
    (HTTP_LOCK_TOKEN, HTTP_LOCK_TOKEN),
    (
      "<urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00>",
      "<urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00>",
    ),
    (
      " \t<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\t ",
      OPAQUE_LOCK_TOKEN,
    ),
  ] {
    let token = LockToken::parse(value).expect("lock token should parse");
    assert_eq!(token.as_str(), expected);
    assert_eq!(token.header_value(), expected);
  }

  let constructed = LockToken::new(OPAQUE_LOCK_TOKEN).expect("new should behave like parse");
  assert_eq!(constructed.as_str(), OPAQUE_LOCK_TOKEN);
  assert_eq!(constructed.header_value(), OPAQUE_LOCK_TOKEN);
}

#[test]
fn lock_token_rejects_empty_missing_brackets_lists_and_relative_uris() {
  for value in [
    "",
    " ",
    "\t",
    "opaquelocktoken:550e8400-e29b-41d4-a716-446655440000",
    "<>",
    "< >",
    "<relative>",
    "</locks/1>",
    "<locks/1>",
    "<<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>>",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>>",
    "<<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>, <http://example.test/locks/2>",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000> extra",
    "prefix <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000",
    "opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    "<http://example.test/locks/1 extra>",
  ] {
    assert!(
      LockToken::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn lock_token_rejects_injected_obs_text_and_control_bytes() {
  for value in [
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\nX-Injected: 1",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\rX: y",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\nX: y",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\0value",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\u{1}value",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\u{7f}value",
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\u{80}value",
  ] {
    assert!(
      LockToken::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn lock_token_rejects_duplicate_fields() {
  assert!(LockToken::parse_values([OPAQUE_LOCK_TOKEN, HTTP_LOCK_TOKEN]).is_err());
  assert!(LockToken::parse_values([]).is_err());
}

#[test]
fn lock_token_enforces_value_bounds() {
  let prefix = "<http://example.test/";
  let suffix = ">";
  let at_bound = format!(
    "{prefix}{}{suffix}",
    "a".repeat(MAX_LOCK_TOKEN_VALUE_BYTES - prefix.len() - suffix.len())
  );
  assert!(
    LockToken::parse(&at_bound).is_ok(),
    "a coded URL at the 64 KiB bound should parse"
  );

  let oversized = "x".repeat(MAX_LOCK_TOKEN_VALUE_BYTES + 1);
  assert!(
    LockToken::parse(oversized).is_err(),
    "a value over the 64 KiB bound should be rejected"
  );
}

#[test]
fn lock_token_checks_duplicate_values_against_its_bound() {
  let oversized = "x".repeat(MAX_LOCK_TOKEN_VALUE_BYTES + 1);

  assert!(
    LockToken::parse_values([OPAQUE_LOCK_TOKEN, oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
  assert!(
    LockToken::parse_values([oversized.as_str(), OPAQUE_LOCK_TOKEN]).is_err(),
    "an oversized first field must not bypass validation"
  );
}

#[test]
fn lock_token_debug_and_errors_redact_the_token() {
  let token = LockToken::parse(OPAQUE_LOCK_TOKEN).expect("token should parse");
  let debug = format!("{token:?}");
  assert!(debug.contains("LockToken"));
  assert!(debug.contains("[REDACTED]"));
  assert!(!debug.contains(OPAQUE_LOCK_TOKEN));
  assert!(!debug.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!debug.contains("opaquelocktoken"));

  let error =
    LockToken::parse("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\nX-Injected: 1")
      .expect_err("injected token should be rejected");
  let message = error.to_string();
  assert!(message.contains("Lock-Token"));
  assert!(!message.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!message.contains("opaquelocktoken"));
  assert!(!message.contains("X-Injected"));

  let duplicate = LockToken::parse_values([OPAQUE_LOCK_TOKEN, HTTP_LOCK_TOKEN])
    .expect_err("duplicate fields should be rejected");
  let duplicate_message = duplicate.to_string();
  assert!(duplicate_message.contains("duplicate"));
  assert!(!duplicate_message.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!duplicate_message.contains("example.test"));

  let oversized = LockToken::parse("x".repeat(MAX_LOCK_TOKEN_VALUE_BYTES + 1))
    .expect_err("oversized token should be rejected");
  let oversized_message = oversized.to_string();
  assert!(oversized_message.contains("too large"));
  assert!(oversized_message.contains("Lock-Token"));
}
