//! Bounded, policy-free `DNT` request metadata parsing.
//!
//! This module validates the W3C Tracking Preference Expression request field
//! value only. Callers decide whether and how to honor the declared tracking
//! preference.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `DNT` field value.
pub const MAX_DNT_VALUE_BYTES: usize = 64 * 1024;

/// The tracking preference declared by `DNT`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Dnt {
  AllowTracking,
  DoNotTrack,
}

impl Dnt {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DntParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DntParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    match value {
      "0" => Ok(Self::AllowTracking),
      "1" => Ok(Self::DoNotTrack),
      _ => Err(invalid_value()),
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::AllowTracking => "0",
      Self::DoNotTrack => "1",
    }
  }
}

/// An error returned when `DNT` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DntParseError {
  message: String,
}

impl DntParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DntParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DntParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, DntParseError>
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
    return Err(DntParseError::new("duplicate DNT header fields"));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), DntParseError> {
  if value.len() > MAX_DNT_VALUE_BYTES {
    return Err(DntParseError::new("DNT header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn invalid_value() -> DntParseError {
  DntParseError::new("invalid DNT header value")
}
