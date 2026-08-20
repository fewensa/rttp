//! Bounded, policy-free WebDAV `Lock-Token` metadata parsing.
//!
//! This module validates one angle-bracketed state token URI as request or
//! response metadata only. It does not create, refresh, release, persist,
//! compare ownership of, or enforce WebDAV locks.

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a `Lock-Token` field value.
pub const MAX_LOCK_TOKEN_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded WebDAV `Lock-Token` metadata.
///
/// The stored value is the OWS-trimmed field text from the wire, including the
/// surrounding `<` and `>` of the coded URL. The token is redacted from typed
/// `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct LockToken {
  value: String,
}

/// An error returned when `Lock-Token` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockTokenParseError {
  message: String,
}

impl LockToken {
  pub fn new(value: impl AsRef<str>) -> Result<Self, LockTokenParseError> {
    Self::parse(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, LockTokenParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, LockTokenParseError>
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

impl fmt::Debug for LockToken {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("LockToken")
      .field("token", &"[REDACTED]")
      .finish()
  }
}

impl LockTokenParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for LockTokenParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for LockTokenParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, LockTokenParseError>
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
    return Err(LockTokenParseError::new(
      "duplicate Lock-Token header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  parse_coded_url(value)
}

fn parse_coded_url(value: &str) -> Result<String, LockTokenParseError> {
  if value.len() < 2 || !value.starts_with('<') || !value.ends_with('>') {
    return Err(invalid_value());
  }
  let uri = &value[1..value.len() - 1];
  if uri.is_empty() || uri.contains(['<', '>', ' ', '\t']) || !uri.bytes().all(is_visible_byte) {
    return Err(invalid_value());
  }
  Url::parse(uri).map_err(|_| invalid_value())?;
  Ok(value.to_string())
}

fn validate_bounded_value(value: &str) -> Result<(), LockTokenParseError> {
  if value.len() > MAX_LOCK_TOKEN_VALUE_BYTES {
    return Err(LockTokenParseError::new(
      "Lock-Token header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(LockTokenParseError::new("invalid Lock-Token control byte"));
  }
  Ok(())
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_value() -> LockTokenParseError {
  LockTokenParseError::new("invalid Lock-Token header value")
}
