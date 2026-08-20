//! Bounded, policy-free `Accept-Datetime` request metadata parsing.
//!
//! This module validates one singleton `Accept-Datetime` field value only.
//! Callers retain archival selection, time negotiation, and TimeGate policy.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in an `Accept-Datetime` field value.
pub const MAX_ACCEPT_DATETIME_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Accept-Datetime` request metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AcceptDatetime(SystemTime);

/// An error returned when `Accept-Datetime` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptDatetimeParseError {
  message: String,
}

impl AcceptDatetimeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptDatetimeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptDatetimeParseError {}

impl AcceptDatetime {
  pub const fn new(datetime: SystemTime) -> Self {
    Self(datetime)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptDatetimeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptDatetimeParseError>
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

fn parse_singleton<'a, I>(values: I) -> Result<SystemTime, AcceptDatetimeParseError>
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
    return Err(AcceptDatetimeParseError::new(
      "duplicate Accept-Datetime header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  httpdate::parse_http_date(value).map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), AcceptDatetimeParseError> {
  if value.len() > MAX_ACCEPT_DATETIME_VALUE_BYTES {
    return Err(AcceptDatetimeParseError::new(
      "Accept-Datetime header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(AcceptDatetimeParseError::new(
      "invalid Accept-Datetime control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> AcceptDatetimeParseError {
  AcceptDatetimeParseError::new("invalid Accept-Datetime header value")
}
