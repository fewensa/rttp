//! Bounded, policy-free `Idempotency-Key` request metadata parsing.
//!
//! This module validates one opaque, visible-value request field. It does not
//! retry requests, store keys, compare keys across requests, or apply
//! application idempotency policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Idempotency-Key` field value.
pub const MAX_IDEMPOTENCY_KEY_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Idempotency-Key` request metadata.
///
/// The stored key is the OWS-trimmed, visible-value field text from the wire.
#[derive(Clone, Eq, PartialEq)]
pub struct IdempotencyKey {
  value: String,
}

/// An error returned when `Idempotency-Key` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyKeyParseError {
  message: String,
}

impl IdempotencyKey {
  pub fn new(value: impl AsRef<str>) -> Result<Self, IdempotencyKeyParseError> {
    Self::parse(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, IdempotencyKeyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, IdempotencyKeyParseError>
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

impl fmt::Debug for IdempotencyKey {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("IdempotencyKey")
      .field("key", &"[REDACTED]")
      .finish()
  }
}

impl IdempotencyKeyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for IdempotencyKeyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for IdempotencyKeyParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, IdempotencyKeyParseError>
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
    return Err(IdempotencyKeyParseError::new(
      "duplicate Idempotency-Key header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(is_visible_byte) {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_bounded_value(value: &str) -> Result<(), IdempotencyKeyParseError> {
  if value.len() > MAX_IDEMPOTENCY_KEY_VALUE_BYTES {
    return Err(IdempotencyKeyParseError::new(
      "Idempotency-Key header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(IdempotencyKeyParseError::new(
      "invalid Idempotency-Key control byte",
    ));
  }
  Ok(())
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_value() -> IdempotencyKeyParseError {
  IdempotencyKeyParseError::new("invalid Idempotency-Key header value")
}
