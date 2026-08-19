//! Bounded, policy-free `Access-Control-Allow-Credentials` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply CORS credentials policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Access-Control-Allow-Credentials` field value.
pub const MAX_ACCESS_CONTROL_ALLOW_CREDENTIALS_VALUE_BYTES: usize = 64 * 1024;

/// The credentials mode declared by `Access-Control-Allow-Credentials`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessControlAllowCredentials {
  True,
}

impl AccessControlAllowCredentials {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlAllowCredentialsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlAllowCredentialsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("true") {
      Ok(Self::True)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::True => "true",
    }
  }
}

/// An error returned when `Access-Control-Allow-Credentials` metadata is
/// malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowCredentialsParseError {
  message: String,
}

impl AccessControlAllowCredentialsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlAllowCredentialsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlAllowCredentialsParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, AccessControlAllowCredentialsParseError>
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
    return Err(AccessControlAllowCredentialsParseError::new(
      "duplicate Access-Control-Allow-Credentials header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), AccessControlAllowCredentialsParseError> {
  if value.len() > MAX_ACCESS_CONTROL_ALLOW_CREDENTIALS_VALUE_BYTES {
    return Err(AccessControlAllowCredentialsParseError::new(
      "Access-Control-Allow-Credentials header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn invalid_value() -> AccessControlAllowCredentialsParseError {
  AccessControlAllowCredentialsParseError::new(
    "invalid Access-Control-Allow-Credentials header value",
  )
}
