//! Bounded, policy-free WebDAV `Depth` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to traverse resources or enforce method-specific WebDAV policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Depth` field value.
pub const MAX_DEPTH_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded WebDAV `Depth` request metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Depth {
  Zero,
  One,
  Infinity,
}

impl Depth {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DepthParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DepthParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(self) -> &'static str {
    match self {
      Self::Zero => "0",
      Self::One => "1",
      Self::Infinity => "infinity",
    }
  }
}

/// An error returned when `Depth` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepthParseError {
  message: String,
}

impl DepthParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DepthParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DepthParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<Depth, DepthParseError>
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
    return Err(DepthParseError::new("duplicate Depth header fields"));
  }

  match value.trim_matches([' ', '\t']) {
    "0" => Ok(Depth::Zero),
    "1" => Ok(Depth::One),
    value if value.eq_ignore_ascii_case("infinity") => Ok(Depth::Infinity),
    _ => Err(invalid_value()),
  }
}

fn validate_bounded_value(value: &str) -> Result<(), DepthParseError> {
  if value.len() > MAX_DEPTH_VALUE_BYTES {
    return Err(DepthParseError::new("Depth header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(DepthParseError::new("invalid Depth control byte"));
  }
  Ok(())
}

fn invalid_value() -> DepthParseError {
  DepthParseError::new("invalid Depth header value")
}
