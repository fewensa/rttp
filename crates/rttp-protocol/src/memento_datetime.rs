//! Bounded, policy-free `Memento-Datetime` response metadata parsing.
//!
//! This module validates one singleton `Memento-Datetime` field value only.
//! Callers retain archival selection, time negotiation, and TimeGate policy.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in a `Memento-Datetime` field value.
pub const MAX_MEMENTO_DATETIME_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Memento-Datetime` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MementoDatetime(SystemTime);

/// An error returned when `Memento-Datetime` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MementoDatetimeParseError {
  message: String,
}

impl MementoDatetimeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for MementoDatetimeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for MementoDatetimeParseError {}

impl MementoDatetime {
  pub const fn new(datetime: SystemTime) -> Self {
    Self(datetime)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, MementoDatetimeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, MementoDatetimeParseError>
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

fn parse_singleton<'a, I>(values: I) -> Result<SystemTime, MementoDatetimeParseError>
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
    return Err(MementoDatetimeParseError::new(
      "duplicate Memento-Datetime header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  httpdate::parse_http_date(value).map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), MementoDatetimeParseError> {
  if value.len() > MAX_MEMENTO_DATETIME_VALUE_BYTES {
    return Err(MementoDatetimeParseError::new(
      "Memento-Datetime header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(MementoDatetimeParseError::new(
      "invalid Memento-Datetime control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> MementoDatetimeParseError {
  MementoDatetimeParseError::new("invalid Memento-Datetime header value")
}
