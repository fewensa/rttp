use rttp_protocol::sec_websocket_key::{
  SecWebSocketKey, MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES, SEC_WEBSOCKET_KEY_NONCE_LEN,
};

const RFC_6455_EXAMPLE: &str = "dGhlIHNhbXBsZSBub25jZQ==";

#[test]
fn sec_websocket_key_accepts_rfc_6455_non_ces_and_normalizes_ows() {
  for (value, expected) in [
    (RFC_6455_EXAMPLE, RFC_6455_EXAMPLE),
    ("AAAAAAAAAAAAAAAAAAAAAA==", "AAAAAAAAAAAAAAAAAAAAAA=="),
    (" \t+/z9/v8AAQIDBAUGBwgJCg==\t ", "+/z9/v8AAQIDBAUGBwgJCg=="),
  ] {
    let key = SecWebSocketKey::parse(value).expect("nonce should parse");
    assert_eq!(key.as_str(), expected);
    assert_eq!(key.header_value(), expected);
  }

  let constructed = SecWebSocketKey::new(RFC_6455_EXAMPLE).expect("new should behave like parse");
  assert_eq!(constructed.as_str(), RFC_6455_EXAMPLE);
  assert_eq!(constructed.header_value(), RFC_6455_EXAMPLE);
}

#[test]
fn sec_websocket_key_rejects_non_base64_and_malformed_values() {
  for value in [
    "",
    " ",
    "\t",
    "the sample nonce",
    "dGhlIHNhbXBsZSBub25jZQ= =",
    "dGhlIHNhbXBsZSBub25jZQ=extra",
    "dGhlIHNhbXBsZSBub25jZQ",
    "_z9/v8AAQIDBAUGBwgJCg==",
  ] {
    assert!(
      SecWebSocketKey::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_key_rejects_decoded_nonces_that_are_not_sixteen_bytes() {
  for value in [
    "AAAAAAAAAAAAAAAAAAAA",             // 15 bytes
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", // 24 bytes
    "AAAAAA==",                         // 4 bytes
  ] {
    let error = SecWebSocketKey::parse(value).expect_err("non-16-byte nonce must be rejected");
    let message = error.to_string();
    assert!(message.contains("Sec-WebSocket-Key"));
    assert!(!message.contains(value));
  }
  assert_eq!(SEC_WEBSOCKET_KEY_NONCE_LEN, 16);
}

#[test]
fn sec_websocket_key_rejects_injected_obs_text_and_control_bytes() {
  for value in [
    "dGhlIHNhbXBsZSBub25jZQ==\r\nX-Injected: 1",
    "dGhlIHNhbXBsZSBub25jZQ==\rX: y",
    "dGhlIHNhbXBsZSBub25jZQ==\nX: y",
    "dGhlIHNhbXBsZSBub25jZQ==\0value",
    "dGhlIHNhbXBsZSBub25jZQ==\u{1}value",
    "dGhlIHNhbXBsZSBub25jZQ==\u{7f}value",
    "dGhlIHNhbXBsZSBub25jZQ==\u{80}value",
  ] {
    assert!(
      SecWebSocketKey::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_key_rejects_duplicate_fields() {
  assert!(SecWebSocketKey::parse_values([RFC_6455_EXAMPLE, RFC_6455_EXAMPLE]).is_err());
  assert!(SecWebSocketKey::parse_values([]).is_err());
}

#[test]
fn sec_websocket_key_enforces_value_bounds() {
  let padded = "A".repeat((MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES / 3) * 4);
  assert!(
    SecWebSocketKey::parse(padded).is_err(),
    "a value at the 64 KiB bound must still decode to 16 bytes"
  );
  let oversized = "A".repeat(MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES + 1);
  assert!(
    SecWebSocketKey::parse(oversized).is_err(),
    "a value over the 64 KiB bound should be rejected"
  );
}

#[test]
fn sec_websocket_key_checks_duplicate_values_against_its_bound() {
  let oversized = "A".repeat(MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES + 1);

  assert!(
    SecWebSocketKey::parse_values([RFC_6455_EXAMPLE, oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
  assert!(
    SecWebSocketKey::parse_values([oversized.as_str(), RFC_6455_EXAMPLE]).is_err(),
    "an oversized first field must not bypass validation"
  );
}

#[test]
fn sec_websocket_key_debug_and_errors_redact_the_nonce() {
  let key = SecWebSocketKey::parse(RFC_6455_EXAMPLE).expect("nonce should parse");
  let debug = format!("{key:?}");
  assert!(debug.contains("SecWebSocketKey"));
  assert!(debug.contains("[REDACTED]"));
  assert!(!debug.contains(RFC_6455_EXAMPLE));
  assert!(!debug.contains("the sample nonce"));

  let error = SecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==\r\nX-Injected: 1")
    .expect_err("injected nonce should be rejected");
  let message = error.to_string();
  assert!(message.contains("Sec-WebSocket-Key"));
  assert!(!message.contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert!(!message.contains("X-Injected"));

  let duplicate = SecWebSocketKey::parse_values([RFC_6455_EXAMPLE, RFC_6455_EXAMPLE])
    .expect_err("duplicate fields should be rejected");
  let duplicate_message = duplicate.to_string();
  assert!(duplicate_message.contains("duplicate"));
  assert!(!duplicate_message.contains(RFC_6455_EXAMPLE));

  let oversized = SecWebSocketKey::parse("A".repeat(MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES + 1))
    .expect_err("oversized nonce should be rejected");
  let oversized_message = oversized.to_string();
  assert!(oversized_message.contains("too large"));
  assert!(oversized_message.contains("Sec-WebSocket-Key"));
}
