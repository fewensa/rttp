//! Bounded, policy-free `Sec-GPC` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to handle the declared global privacy control signal.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Sec-GPC` field value.
pub const MAX_SEC_GPC_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Sec-GPC` request metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SecGpc;

impl SecGpc {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecGpcParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecGpcParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(&self) -> &'static str {
    "1"
  }
}

/// An error returned when `Sec-GPC` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecGpcParseError {
  message: String,
}

impl SecGpcParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecGpcParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecGpcParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<SecGpc, SecGpcParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_value(value)?;
  }
  if has_duplicate {
    return Err(SecGpcParseError::new("duplicate Sec-GPC header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value != "1" {
    return Err(invalid_value());
  }

  Ok(SecGpc)
}

fn validate_value(value: &str) -> Result<(), SecGpcParseError> {
  if value.len() > MAX_SEC_GPC_VALUE_BYTES {
    return Err(SecGpcParseError::new("Sec-GPC header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(SecGpcParseError::new("invalid Sec-GPC control byte"));
  }
  Ok(())
}

fn invalid_value() -> SecGpcParseError {
  SecGpcParseError::new("invalid Sec-GPC header value")
}
