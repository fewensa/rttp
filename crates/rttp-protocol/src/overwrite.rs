//! Bounded, policy-free WebDAV `Overwrite` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to overwrite a destination resource or apply the RFC 4918 default
//! `T` when the field is absent.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Overwrite` field value.
pub const MAX_OVERWRITE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded WebDAV `Overwrite` request metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Overwrite {
  T,
  F,
}

impl Overwrite {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, OverwriteParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, OverwriteParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(self) -> &'static str {
    match self {
      Self::T => "T",
      Self::F => "F",
    }
  }
}

/// An error returned when `Overwrite` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverwriteParseError {
  message: String,
}

impl OverwriteParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for OverwriteParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for OverwriteParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<Overwrite, OverwriteParseError>
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
    return Err(OverwriteParseError::new(
      "duplicate Overwrite header fields",
    ));
  }

  match value.trim_matches([' ', '\t']) {
    "T" => Ok(Overwrite::T),
    "F" => Ok(Overwrite::F),
    _ => Err(invalid_value()),
  }
}

fn validate_bounded_value(value: &str) -> Result<(), OverwriteParseError> {
  if value.len() > MAX_OVERWRITE_VALUE_BYTES {
    return Err(OverwriteParseError::new(
      "Overwrite header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(OverwriteParseError::new("invalid Overwrite control byte"));
  }
  Ok(())
}

fn invalid_value() -> OverwriteParseError {
  OverwriteParseError::new("invalid Overwrite header value")
}
