use rttp_protocol::sec_websocket_accept::{
  SecWebSocketAccept, MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES, SEC_WEBSOCKET_ACCEPT_GUID,
  SEC_WEBSOCKET_ACCEPT_SHA1_LEN,
};
use rttp_protocol::sec_websocket_key::SecWebSocketKey;

const RFC_6455_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const RFC_6455_ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

/// Independent key-to-accept vectors: `(Sec-WebSocket-Key, Sec-WebSocket-Accept)`.
const INDEPENDENT_KEY_ACCEPT_VECTORS: &[(&str, &str)] = &[
  ("AAAAAAAAAAAAAAAAAAAAAA==", "ICX+Yqv66kxgM0FcWaLWlFLwTAI="),
  ("+/z9/v8AAQIDBAUGBwgJCg==", "7jS4YVQIrAyNrUk1Okz37nIH9dU="),
  ("AQIDBAUGBwgJCgsMDQ4PEA==", "C/0nmHhBztSRGR1CwL6Tf4ZjwpY="),
];

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
fn sec_websocket_accept_derives_independent_key_to_accept_vectors() {
  assert_eq!(
    SEC_WEBSOCKET_ACCEPT_GUID,
    "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
  );
  assert_eq!(SEC_WEBSOCKET_ACCEPT_SHA1_LEN, 20);

  for &(key_value, accept_value) in INDEPENDENT_KEY_ACCEPT_VECTORS {
    let key = SecWebSocketKey::parse(key_value).expect("independent key should parse");
    let derived = SecWebSocketAccept::derive_from_key(&key);
    assert_eq!(
      derived.as_str(),
      accept_value,
      "derive mismatch for {key_value}"
    );
    assert_eq!(derived.header_value(), accept_value);
    assert!(derived.verify_key(&key));

    let parsed = SecWebSocketAccept::parse(accept_value).expect("independent accept should parse");
    assert_eq!(parsed, derived);
    assert!(parsed.verify_key(&key));
  }
}

#[test]
fn sec_websocket_accept_round_trips_derive_header_value_parse_and_verify_key() {
  for &(key_value, accept_value) in
    std::iter::once(&(RFC_6455_KEY, RFC_6455_ACCEPT)).chain(INDEPENDENT_KEY_ACCEPT_VECTORS.iter())
  {
    let key = SecWebSocketKey::parse(key_value).expect("key should parse");
    let derived = SecWebSocketAccept::derive_from_key(&key);
    let header = derived.header_value();
    assert_eq!(header, accept_value);

    let reparsed = SecWebSocketAccept::parse(&header).expect("derived header_value must reparse");
    assert_eq!(reparsed.as_str(), accept_value);
    assert_eq!(reparsed.header_value(), accept_value);
    assert!(reparsed.verify_key(&key));
    assert_eq!(reparsed, derived);
  }
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
    "\t",
    "the accept value",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo= =",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo =",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOp=", // non-canonical trailing base64 bits
    "s3pPLMBiTxaQ9kYGzzhZRbK-xOo=", // URL-safe alphabet
    "_3pPLMBiTxaQ9kYGzzhZRbK+xOo=", // non-alphabet character
    "AAAAAAAAAAAAAAAAAAAAAA==",     // 16-byte payload
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", // 24-byte payload / missing padding
    "AAAA",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nX-Injected: 1",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\rX: y",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\nX: y",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\0value",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\u{1}value",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\u{7f}value",
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\u{80}",
  ] {
    let error = SecWebSocketAccept::parse(value).expect_err("value should be rejected");
    let message = error.to_string();
    assert!(
      message.contains("Sec-WebSocket-Accept"),
      "{value:?} => {message}"
    );
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

  let exact = format!(
    "{}{}",
    " ".repeat(MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES - RFC_6455_ACCEPT.len()),
    RFC_6455_ACCEPT
  );
  let parsed =
    SecWebSocketAccept::parse(&exact).expect("OWS-padded value at the bound should parse");
  assert_eq!(parsed.as_str(), RFC_6455_ACCEPT);

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
  assert!(
    SecWebSocketAccept::parse_values([oversized_duplicate.as_str(), RFC_6455_ACCEPT]).is_err()
  );
}
