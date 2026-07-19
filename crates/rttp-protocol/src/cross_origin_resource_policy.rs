//! Bounded, policy-free `Cross-Origin-Resource-Policy` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce resource isolation policy.

use std::error::Error;
use std::fmt;

pub const MAX_CROSS_ORIGIN_RESOURCE_POLICY_VALUE_BYTES: usize = 64 * 1024;

/// The resource-sharing policy declared by `Cross-Origin-Resource-Policy`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossOriginResourcePolicy {
  SameOrigin,
  SameSite,
  CrossOrigin,
}

impl CrossOriginResourcePolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CrossOriginResourcePolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CrossOriginResourcePolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("same-origin") {
      Ok(Self::SameOrigin)
    } else if value.eq_ignore_ascii_case("same-site") {
      Ok(Self::SameSite)
    } else if value.eq_ignore_ascii_case("cross-origin") {
      Ok(Self::CrossOrigin)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::SameOrigin => "same-origin",
      Self::SameSite => "same-site",
      Self::CrossOrigin => "cross-origin",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginResourcePolicyParseError {
  message: String,
}

impl CrossOriginResourcePolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CrossOriginResourcePolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CrossOriginResourcePolicyParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, CrossOriginResourcePolicyParseError>
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
    return Err(CrossOriginResourcePolicyParseError::new(
      "duplicate Cross-Origin-Resource-Policy header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), CrossOriginResourcePolicyParseError> {
  if value.len() > MAX_CROSS_ORIGIN_RESOURCE_POLICY_VALUE_BYTES {
    return Err(CrossOriginResourcePolicyParseError::new(
      "Cross-Origin-Resource-Policy header value is too large",
    ));
  }
  if value.bytes().any(|byte| byte.is_ascii_control()) {
    return Err(invalid_value());
  }
  Ok(())
}

fn invalid_value() -> CrossOriginResourcePolicyParseError {
  CrossOriginResourcePolicyParseError::new("invalid Cross-Origin-Resource-Policy header value")
}
