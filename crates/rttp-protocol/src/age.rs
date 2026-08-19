//! Bounded, policy-free `Age` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply freshness or cache behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Age` field value.
pub const MAX_AGE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Age` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Age(u64);

impl Age {
  pub const fn new(seconds: u64) -> Self {
    Self(seconds)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, AgeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AgeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values).map(Self)
  }

  pub const fn seconds(self) -> u64 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

/// An error returned when `Age` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgeParseError {
  message: String,
}

impl AgeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AgeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AgeParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<u64, AgeParseError>
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
    return Err(AgeParseError::new("duplicate Age header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_value());
  }
  value.parse().map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), AgeParseError> {
  if value.len() > MAX_AGE_VALUE_BYTES {
    return Err(AgeParseError::new("Age header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(AgeParseError::new("invalid Age control byte"));
  }
  Ok(())
}

fn invalid_value() -> AgeParseError {
  AgeParseError::new("invalid Age header value")
}
