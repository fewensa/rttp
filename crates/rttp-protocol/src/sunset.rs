//! Bounded, policy-free RFC 8594 `Sunset` response metadata parsing.
//!
//! This module validates one singleton `Sunset` HTTP-date field value only.
//! Callers decide whether and how to apply deprecation, lifecycle, or
//! comparison policy.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in a `Sunset` field value.
pub const MAX_SUNSET_VALUE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SunsetParseError {
  message: String,
}

impl SunsetParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SunsetParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SunsetParseError {}

/// Parses RFC 8594 `Sunset` response metadata as an HTTP-date.
pub fn parse_sunset(value: impl AsRef<str>) -> Result<SystemTime, SunsetParseError> {
  parse_sunset_values([value.as_ref()]).map(|value| value.expect("single value is present"))
}

/// Parses an optional single RFC 8594 `Sunset` response field.
pub fn parse_sunset_values<'a, I>(values: I) -> Result<Option<SystemTime>, SunsetParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  validate_bounded_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value)?;
  }
  if has_duplicate {
    return Err(SunsetParseError::new("duplicate Sunset header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  httpdate::parse_http_date(value)
    .map(Some)
    .map_err(|_| invalid_value())
}

/// Formats a `Sunset` HTTP-date as canonical IMF-fixdate.
pub fn format_sunset(datetime: SystemTime) -> String {
  httpdate::fmt_http_date(datetime)
}

fn validate_bounded_value(value: &str) -> Result<(), SunsetParseError> {
  if value.len() > MAX_SUNSET_VALUE_BYTES {
    return Err(SunsetParseError::new("Sunset header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(SunsetParseError::new("invalid Sunset control byte"));
  }
  Ok(())
}

fn invalid_value() -> SunsetParseError {
  SunsetParseError::new("invalid Sunset HTTP-date")
}
