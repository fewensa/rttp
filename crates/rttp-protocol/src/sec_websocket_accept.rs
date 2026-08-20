//! Bounded, policy-free `Sec-WebSocket-Accept` response metadata parsing.
//!
//! This module validates one RFC 6455 handshake response field and owns the
//! deterministic `base64(SHA-1(Sec-WebSocket-Key || GUID))` transform. It does
//! not perform an HTTP upgrade, generate a random nonce, or implement
//! WebSocket frames.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha1::{Digest, Sha1};
use std::error::Error;
use std::fmt;

use crate::sec_websocket_key::SecWebSocketKey;

/// Maximum bytes accepted in a `Sec-WebSocket-Accept` field value.
pub const MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES: usize = 64 * 1024;

/// RFC 6455 section 1.3 GUID appended to a validated `Sec-WebSocket-Key`.
pub const SEC_WEBSOCKET_ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// SHA-1 output length required by the RFC 6455 handshake transform.
pub const SEC_WEBSOCKET_ACCEPT_SHA1_LEN: usize = 20;

/// Parsed, bounded `Sec-WebSocket-Accept` response metadata.
///
/// The stored value is the OWS-trimmed base64 field text from the wire. Typed
/// `Debug` redacts it because it proves knowledge of the request nonce.
#[derive(Clone, Eq, PartialEq)]
pub struct SecWebSocketAccept {
  value: String,
}

/// An error returned when `Sec-WebSocket-Accept` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketAcceptParseError {
  message: String,
}

impl SecWebSocketAccept {
  pub fn new(value: impl AsRef<str>) -> Result<Self, SecWebSocketAcceptParseError> {
    Self::parse(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecWebSocketAcceptParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecWebSocketAcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    Ok(Self { value })
  }

  pub fn derive_from_key(key: &SecWebSocketKey) -> Self {
    let mut sha1 = Sha1::new();
    sha1.update(key.as_str().as_bytes());
    sha1.update(SEC_WEBSOCKET_ACCEPT_GUID.as_bytes());
    let digest = sha1.finalize();
    Self {
      value: STANDARD.encode(digest),
    }
  }

  pub fn verify_key(&self, key: &SecWebSocketKey) -> bool {
    self == &Self::derive_from_key(key)
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl fmt::Debug for SecWebSocketAccept {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("SecWebSocketAccept")
      .field("accept", &"[REDACTED]")
      .finish()
  }
}

impl SecWebSocketAcceptParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecWebSocketAcceptParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecWebSocketAcceptParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, SecWebSocketAcceptParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_bounded_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value)?;
  }
  if has_duplicate {
    return Err(SecWebSocketAcceptParseError::new(
      "duplicate Sec-WebSocket-Accept header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || value.contains([' ', '\t']) {
    return Err(invalid_value());
  }
  let decoded = STANDARD.decode(value).map_err(|_| invalid_value())?;
  if decoded.len() != SEC_WEBSOCKET_ACCEPT_SHA1_LEN {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_bounded_value(value: &str) -> Result<(), SecWebSocketAcceptParseError> {
  if value.len() > MAX_SEC_WEBSOCKET_ACCEPT_VALUE_BYTES {
    return Err(SecWebSocketAcceptParseError::new(
      "Sec-WebSocket-Accept header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(SecWebSocketAcceptParseError::new(
      "invalid Sec-WebSocket-Accept control byte",
    ));
  }
  Ok(())
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_value() -> SecWebSocketAcceptParseError {
  SecWebSocketAcceptParseError::new("invalid Sec-WebSocket-Accept header value")
}
