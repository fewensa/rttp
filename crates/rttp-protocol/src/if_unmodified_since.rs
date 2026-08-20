//! Bounded, policy-free `If-Unmodified-Since` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to apply conditional request evaluation.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in an `If-Unmodified-Since` field value.
pub const MAX_IF_UNMODIFIED_SINCE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `If-Unmodified-Since` request metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IfUnmodifiedSince(SystemTime);

impl IfUnmodifiedSince {
  pub const fn new(datetime: SystemTime) -> Self {
    Self(datetime)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, IfUnmodifiedSinceParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, IfUnmodifiedSinceParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values).map(Self)
  }

  pub const fn datetime(self) -> SystemTime {
    self.0
  }

  pub fn header_value(self) -> String {
    httpdate::fmt_http_date(self.0)
  }
}

/// An error returned when `If-Unmodified-Since` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfUnmodifiedSinceParseError {
  message: String,
}

impl IfUnmodifiedSinceParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for IfUnmodifiedSinceParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for IfUnmodifiedSinceParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<SystemTime, IfUnmodifiedSinceParseError>
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
    return Err(IfUnmodifiedSinceParseError::new(
      "duplicate If-Unmodified-Since header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  httpdate::parse_http_date(value).map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), IfUnmodifiedSinceParseError> {
  if value.len() > MAX_IF_UNMODIFIED_SINCE_VALUE_BYTES {
    return Err(IfUnmodifiedSinceParseError::new(
      "If-Unmodified-Since header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(IfUnmodifiedSinceParseError::new(
      "invalid If-Unmodified-Since control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> IfUnmodifiedSinceParseError {
  IfUnmodifiedSinceParseError::new("invalid If-Unmodified-Since header value")
}
