//! Bounded, policy-free `Max-Forwards` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to apply hop-limit or TRACE/OPTIONS diagnostic behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Max-Forwards` field value.
pub const MAX_FORWARDS_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Max-Forwards` request metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MaxForwards(u32);

impl MaxForwards {
  pub const fn new(value: u32) -> Self {
    Self(value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, MaxForwardsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, MaxForwardsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values).map(Self)
  }

  pub const fn value(self) -> u32 {
    self.0
  }

  pub fn header_value(self) -> String {
    self.0.to_string()
  }
}

/// An error returned when `Max-Forwards` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaxForwardsParseError {
  message: String,
}

impl MaxForwardsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for MaxForwardsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for MaxForwardsParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<u32, MaxForwardsParseError>
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
    return Err(MaxForwardsParseError::new(
      "duplicate Max-Forwards header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_value());
  }
  value.parse().map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), MaxForwardsParseError> {
  if value.len() > MAX_FORWARDS_VALUE_BYTES {
    return Err(MaxForwardsParseError::new(
      "Max-Forwards header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(MaxForwardsParseError::new(
      "invalid Max-Forwards control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> MaxForwardsParseError {
  MaxForwardsParseError::new("invalid Max-Forwards header value")
}
