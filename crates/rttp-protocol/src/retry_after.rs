//! Bounded, policy-free `Retry-After` response metadata parsing.
//!
//! This module validates one singleton `Retry-After` field value only. Callers
//! decide whether and how to apply retry, sleep, backoff, scheduler, or status
//! policy.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in a `Retry-After` field value.
pub const MAX_RETRY_AFTER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Retry-After` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RetryAfter {
  DeltaSeconds(u64),
  HttpDate(SystemTime),
}

/// An error returned when `Retry-After` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetryAfterParseError {
  message: String,
}

impl RetryAfterParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for RetryAfterParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for RetryAfterParseError {}

impl RetryAfter {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, RetryAfterParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, RetryAfterParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub const fn delta_seconds(self) -> Option<u64> {
    match self {
      Self::DeltaSeconds(delta_seconds) => Some(delta_seconds),
      Self::HttpDate(_) => None,
    }
  }

  pub const fn http_date(self) -> Option<SystemTime> {
    match self {
      Self::DeltaSeconds(_) => None,
      Self::HttpDate(http_date) => Some(http_date),
    }
  }

  pub fn header_value(self) -> String {
    match self {
      Self::DeltaSeconds(delta_seconds) => delta_seconds.to_string(),
      Self::HttpDate(http_date) => httpdate::fmt_http_date(http_date),
    }
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<RetryAfter, RetryAfterParseError>
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
    return Err(RetryAfterParseError::new(
      "duplicate Retry-After header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  if value.bytes().all(|byte| byte.is_ascii_digit()) {
    return value
      .parse::<u64>()
      .map(RetryAfter::DeltaSeconds)
      .map_err(|_| invalid_value());
  }
  httpdate::parse_http_date(value)
    .map(RetryAfter::HttpDate)
    .map_err(|_| invalid_value())
}

fn validate_bounded_value(value: &str) -> Result<(), RetryAfterParseError> {
  if value.len() > MAX_RETRY_AFTER_VALUE_BYTES {
    return Err(RetryAfterParseError::new(
      "Retry-After header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(RetryAfterParseError::new(
      "invalid Retry-After control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> RetryAfterParseError {
  RetryAfterParseError::new("invalid Retry-After header value")
}
