//! Bounded, policy-free `Deprecation` response metadata parsing.
//!
//! This module validates one Structured Fields boolean or date item only.
//! Callers retain lifecycle policy, Sunset comparison, Link follow, retries,
//! and endpoint selection.

use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sfv::{BareItem, Item, Parser};

/// Maximum bytes accepted in a `Deprecation` field value.
pub const MAX_DEPRECATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Deprecation` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Deprecation {
  Boolean(bool),
  Date(SystemTime),
}

impl Deprecation {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DeprecationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DeprecationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let item = Parser::new(value)
      .parse::<Item>()
      .map_err(|_| invalid_value())?;
    if !item.params.is_empty() {
      return Err(invalid_value());
    }
    match item.bare_item {
      BareItem::Boolean(value) => Ok(Self::Boolean(value)),
      BareItem::Date(value) => {
        system_time_from_unix_seconds(i64::from(value.unix_seconds())).map(Self::Date)
      }
      _ => Err(invalid_value()),
    }
  }

  pub const fn boolean(self) -> Option<bool> {
    match self {
      Self::Boolean(value) => Some(value),
      Self::Date(_) => None,
    }
  }

  pub const fn date(self) -> Option<SystemTime> {
    match self {
      Self::Date(value) => Some(value),
      Self::Boolean(_) => None,
    }
  }

  pub fn header_value(&self) -> String {
    match *self {
      Self::Boolean(false) => "?0".to_owned(),
      Self::Boolean(true) => "?1".to_owned(),
      Self::Date(time) => format!("@{}", unix_seconds(time)),
    }
  }
}

/// An error returned when `Deprecation` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeprecationParseError {
  message: String,
}

impl DeprecationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DeprecationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DeprecationParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, DeprecationParseError>
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
    return Err(DeprecationParseError::new(
      "duplicate Deprecation header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), DeprecationParseError> {
  if value.len() > MAX_DEPRECATION_VALUE_BYTES {
    return Err(DeprecationParseError::new(
      "Deprecation header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(DeprecationParseError::new(
      "invalid Deprecation control byte",
    ));
  }
  Ok(())
}

fn system_time_from_unix_seconds(seconds: i64) -> Result<SystemTime, DeprecationParseError> {
  if seconds >= 0 {
    UNIX_EPOCH
      .checked_add(Duration::from_secs(seconds as u64))
      .ok_or_else(invalid_value)
  } else {
    UNIX_EPOCH
      .checked_sub(Duration::from_secs(seconds.unsigned_abs()))
      .ok_or_else(invalid_value)
  }
}

fn unix_seconds(time: SystemTime) -> i64 {
  match time.duration_since(UNIX_EPOCH) {
    Ok(duration) => i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
    Err(error) => i64::try_from(error.duration().as_secs())
      .map(|seconds| -seconds)
      .unwrap_or(i64::MIN),
  }
}

fn invalid_value() -> DeprecationParseError {
  DeprecationParseError::new("invalid Deprecation header value")
}
