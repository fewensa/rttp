//! Bounded, policy-free `Content-Security-Policy` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to inspect or enforce content security policy.

use std::error::Error;
use std::fmt;

pub const MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES: usize = 64 * 1024;

/// The exact policy text declared by `Content-Security-Policy`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentSecurityPolicy(String);

impl ContentSecurityPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentSecurityPolicyParseError> {
    let value = value.as_ref();
    validate_bounded_value(value)?;
    if value.is_empty() {
      return Err(invalid_value());
    }
    Ok(Self(value.to_owned()))
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentSecurityPolicyParseError>
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
      return Err(ContentSecurityPolicyParseError::new(
        "duplicate Content-Security-Policy header fields",
      ));
    }
    if value.is_empty() {
      return Err(invalid_value());
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn header_value(&self) -> &str {
    self.as_str()
  }
}

impl AsRef<str> for ContentSecurityPolicy {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSecurityPolicyParseError {
  message: String,
}

impl ContentSecurityPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentSecurityPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentSecurityPolicyParseError {}

fn validate_bounded_value(value: &str) -> Result<(), ContentSecurityPolicyParseError> {
  if value.len() > MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES {
    return Err(ContentSecurityPolicyParseError::new(
      "Content-Security-Policy header value is too large",
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

fn invalid_value() -> ContentSecurityPolicyParseError {
  ContentSecurityPolicyParseError::new("invalid Content-Security-Policy header value")
}
