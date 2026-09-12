//! Bounded, policy-free `Early-Data` request metadata parsing.
//!
//! This module validates the RFC 8470 request field value only. Callers
//! decide whether and how to handle replay or 0-RTT policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Early-Data` field value.
pub const MAX_EARLY_DATA_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded RFC 8470 `Early-Data` request metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct EarlyData;

impl EarlyData {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, EarlyDataParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, EarlyDataParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(&self) -> &'static str {
    "1"
  }
}

/// An error returned when `Early-Data` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EarlyDataParseError {
  message: String,
}

impl EarlyDataParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for EarlyDataParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for EarlyDataParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<EarlyData, EarlyDataParseError>
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
    return Err(EarlyDataParseError::new(
      "duplicate Early-Data header fields",
    ));
  }

  if value.trim_matches([' ', '\t']) != "1" {
    return Err(invalid_value());
  }

  Ok(EarlyData)
}

fn validate_value(value: &str) -> Result<(), EarlyDataParseError> {
  if value.len() > MAX_EARLY_DATA_VALUE_BYTES {
    return Err(EarlyDataParseError::new(
      "Early-Data header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(EarlyDataParseError::new("invalid Early-Data control byte"));
  }
  Ok(())
}

fn invalid_value() -> EarlyDataParseError {
  EarlyDataParseError::new("invalid Early-Data header value")
}
