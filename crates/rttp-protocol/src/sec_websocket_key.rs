//! Bounded, policy-free `Sec-WebSocket-Key` request metadata parsing.
//!
//! This module validates one RFC 6455 handshake nonce field as request
//! metadata only. It does not perform an HTTP upgrade, compute
//! `Sec-WebSocket-Accept`, generate a random nonce, or implement WebSocket
//! frames.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Sec-WebSocket-Key` field value.
pub const MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES: usize = 64 * 1024;

/// Decoded nonce length required by RFC 6455 section 4.1.
pub const SEC_WEBSOCKET_KEY_NONCE_LEN: usize = 16;

/// Parsed, bounded `Sec-WebSocket-Key` request metadata.
///
/// The stored value is the OWS-trimmed, RFC 4648 section 4 encoded field text
/// from the wire. The decoded nonce is redacted from typed `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct SecWebSocketKey {
  value: String,
}

/// An error returned when `Sec-WebSocket-Key` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketKeyParseError {
  message: String,
}

impl SecWebSocketKey {
  pub fn new(value: impl AsRef<str>) -> Result<Self, SecWebSocketKeyParseError> {
    Self::parse(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecWebSocketKeyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecWebSocketKeyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    Ok(Self { value })
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl fmt::Debug for SecWebSocketKey {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("SecWebSocketKey")
      .field("key", &"[REDACTED]")
      .finish()
  }
}

impl SecWebSocketKeyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecWebSocketKeyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecWebSocketKeyParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, SecWebSocketKeyParseError>
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
    return Err(SecWebSocketKeyParseError::new(
      "duplicate Sec-WebSocket-Key header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || value.contains([' ', '\t']) {
    return Err(invalid_value());
  }
  let decoded = STANDARD.decode(value).map_err(|_| invalid_value())?;
  if decoded.len() != SEC_WEBSOCKET_KEY_NONCE_LEN {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_bounded_value(value: &str) -> Result<(), SecWebSocketKeyParseError> {
  if value.len() > MAX_SEC_WEBSOCKET_KEY_VALUE_BYTES {
    return Err(SecWebSocketKeyParseError::new(
      "Sec-WebSocket-Key header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(SecWebSocketKeyParseError::new(
      "invalid Sec-WebSocket-Key control byte",
    ));
  }
  Ok(())
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_value() -> SecWebSocketKeyParseError {
  SecWebSocketKeyParseError::new("invalid Sec-WebSocket-Key header value")
}
