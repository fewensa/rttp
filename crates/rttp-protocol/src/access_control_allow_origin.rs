//! Bounded, policy-free `Access-Control-Allow-Origin` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply CORS policy.

use std::error::Error;
use std::fmt;

use crate::origin::Origin;

/// Maximum bytes accepted in an `Access-Control-Allow-Origin` field value.
pub const MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Access-Control-Allow-Origin` response metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AccessControlAllowOrigin {
  Wildcard,
  Origin(Origin),
}

impl AccessControlAllowOrigin {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlAllowOriginParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlAllowOriginParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value == "*" {
      return Ok(Self::Wildcard);
    }
    Origin::parse(value)
      .map(Self::Origin)
      .map_err(|error| AccessControlAllowOriginParseError::new(error.to_string()))
  }

  pub const fn is_wildcard(&self) -> bool {
    matches!(self, Self::Wildcard)
  }

  pub fn origin(&self) -> Option<&Origin> {
    match self {
      Self::Wildcard => None,
      Self::Origin(origin) => Some(origin),
    }
  }

  pub fn header_value(&self) -> String {
    match self {
      Self::Wildcard => "*".to_string(),
      Self::Origin(origin) => origin.header_value(),
    }
  }
}

/// An error returned when `Access-Control-Allow-Origin` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowOriginParseError {
  message: String,
}

impl AccessControlAllowOriginParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlAllowOriginParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlAllowOriginParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, AccessControlAllowOriginParseError>
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
    return Err(AccessControlAllowOriginParseError::new(
      "duplicate Access-Control-Allow-Origin header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || value.contains(',') {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_value(value: &str) -> Result<(), AccessControlAllowOriginParseError> {
  if value.len() > MAX_ACCESS_CONTROL_ALLOW_ORIGIN_VALUE_BYTES {
    return Err(AccessControlAllowOriginParseError::new(
      "Access-Control-Allow-Origin header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(AccessControlAllowOriginParseError::new(
      "invalid Access-Control-Allow-Origin header control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> AccessControlAllowOriginParseError {
  AccessControlAllowOriginParseError::new("invalid Access-Control-Allow-Origin header value")
}
