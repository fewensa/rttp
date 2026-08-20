use rttp_protocol::sec_websocket_accept::{
  SecWebSocketAccept, MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES, SEC_WEBSOCKET_ACCEPT_GUID,
  SEC_WEBSOCKET_ACCEPT_SHA1_LEN,
};
use rttp_protocol::sec_websocket_key::SecWebSocketKey;

const RFC_6455_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const RFC_6455_ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

#[test]
fn sec_websocket_accept_derives_rfc_6455_vector_from_validated_key() {
  let key = SecWebSocketKey::parse(RFC_6455_KEY).expect("Sec-WebSocket-Key should parse");
  let accept = SecWebSocketAccept::derive_from_key(&key);

  assert_eq!(
    SEC_WEBSOCKET_ACCEPT_GUID,
    "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
  );
  assert_eq!(SEC_WEBSOCKET_ACCEPT_SHA1_LEN, 20);
  assert_eq!(accept.as_str(), RFC_6455_ACCEPT);
  assert_eq!(accept.header_value(), RFC_6455_ACCEPT);
  assert!(accept.verify_key(&key));
}

#[test]
fn sec_websocket_accept_accepts_singleton_sha1_base64_and_normalizes_ows() {
  for value in [
    RFC_6455_ACCEPT,
    " AAAAAAAAAAAAAAAAAAAAAAAAAAA=\t",
    "\t+/z9/v8AAQIDBAUGBwgJCgsMDQ4=\t ",
  ] {
    let accept = SecWebSocketAccept::parse(value).expect("accept value should parse");
    assert_eq!(accept.as_str(), value.trim_matches([' ', '\t']));
    assert_eq!(accept.header_value(), value.trim_matches([' ', '\t']));
  }

  let constructed = SecWebSocketAccept::new(RFC_6455_ACCEPT).expect("new should parse");
  assert_eq!(constructed.as_str(), RFC_6455_ACCEPT);
}

#[test]
fn sec_websocket_accept_rejects_malformed_wrong_length_or_duplicate_values() {
  for value in [
    "",
    " ",
    "the accept value",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo= =",
    "AAAAAAAAAAAAAAAAAAAAAA==",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nX-Injected: 1",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\u{80}",
  ] {
    let error = SecWebSocketAccept::parse(value).expect_err("value should be rejected");
    let message = error.to_string();
    assert!(message.contains("Sec-WebSocket-Accept"));
    assert!(value.trim_matches([' ', '\t']).is_empty() || !message.contains(value));
  }

  assert!(SecWebSocketAccept::parse_values([RFC_6455_ACCEPT, RFC_6455_ACCEPT]).is_err());
  assert!(SecWebSocketAccept::parse_values([]).is_err());
}

#[test]
fn sec_websocket_accept_enforces_value_bounds_and_redacts_debug_errors() {
  let accept = SecWebSocketAccept::parse(RFC_6455_ACCEPT).expect("accept should parse");
  let debug = format!("{accept:?}");
  assert!(debug.contains("SecWebSocketAccept"));
  assert!(debug.contains("[REDACTED]"));
  assert!(!debug.contains(RFC_6455_ACCEPT));

  let oversized = "A".repeat(MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES + 1);
  let error = SecWebSocketAccept::parse(oversized).expect_err("oversized value should fail");
  let message = error.to_string();
  assert!(message.contains("Sec-WebSocket-Accept"));
  assert!(message.contains("too large"));
  assert!(!message.contains(RFC_6455_ACCEPT));

  let oversized_duplicate = "A".repeat(MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES + 1);
  assert!(
    SecWebSocketAccept::parse_values([RFC_6455_ACCEPT, oversized_duplicate.as_str()]).is_err()
  );
}
