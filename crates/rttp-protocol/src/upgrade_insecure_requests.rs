//! Bounded, policy-free `Upgrade-Insecure-Requests` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to treat the preference; this crate does not rewrite URLs, redirect
//! requests, or enforce Content-Security-Policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Upgrade-Insecure-Requests` field value.
pub const MAX_UPGRADE_INSECURE_REQUESTS_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Upgrade-Insecure-Requests` request metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct UpgradeInsecureRequests;

impl UpgradeInsecureRequests {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, UpgradeInsecureRequestsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, UpgradeInsecureRequestsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(&self) -> &'static str {
    "1"
  }
}

/// An error returned when `Upgrade-Insecure-Requests` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeInsecureRequestsParseError {
  message: String,
}

impl UpgradeInsecureRequestsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for UpgradeInsecureRequestsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for UpgradeInsecureRequestsParseError {}

fn parse_singleton<'a, I>(
  values: I,
) -> Result<UpgradeInsecureRequests, UpgradeInsecureRequestsParseError>
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
    return Err(UpgradeInsecureRequestsParseError::new(
      "duplicate Upgrade-Insecure-Requests header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value != "1" {
    return Err(invalid_value());
  }

  Ok(UpgradeInsecureRequests)
}

fn validate_value(value: &str) -> Result<(), UpgradeInsecureRequestsParseError> {
  if value.len() > MAX_UPGRADE_INSECURE_REQUESTS_VALUE_BYTES {
    return Err(UpgradeInsecureRequestsParseError::new(
      "Upgrade-Insecure-Requests header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(UpgradeInsecureRequestsParseError::new(
      "invalid Upgrade-Insecure-Requests control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> UpgradeInsecureRequestsParseError {
  UpgradeInsecureRequestsParseError::new("invalid Upgrade-Insecure-Requests header value")
}
