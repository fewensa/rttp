//! Bounded, policy-free `Access-Control-Allow-Private-Network` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply Private Network Access or CORS policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Access-Control-Allow-Private-Network` field value.
pub const MAX_ACCESS_CONTROL_ALLOW_PRIVATE_NETWORK_VALUE_BYTES: usize = 64 * 1024;

/// The private-network access signal declared by `Access-Control-Allow-Private-Network`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessControlAllowPrivateNetwork {
  True,
}

impl AccessControlAllowPrivateNetwork {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlAllowPrivateNetworkParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlAllowPrivateNetworkParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    // The Fetch CORS grammar requires the case-sensitive token %s"true".
    if value == "true" {
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

/// An error returned when `Access-Control-Allow-Private-Network` metadata is
/// malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowPrivateNetworkParseError {
  message: String,
}

impl AccessControlAllowPrivateNetworkParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlAllowPrivateNetworkParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlAllowPrivateNetworkParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, AccessControlAllowPrivateNetworkParseError>
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
    return Err(AccessControlAllowPrivateNetworkParseError::new(
      "duplicate Access-Control-Allow-Private-Network header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), AccessControlAllowPrivateNetworkParseError> {
  if value.len() > MAX_ACCESS_CONTROL_ALLOW_PRIVATE_NETWORK_VALUE_BYTES {
    return Err(AccessControlAllowPrivateNetworkParseError::new(
      "Access-Control-Allow-Private-Network header value is too large",
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

fn invalid_value() -> AccessControlAllowPrivateNetworkParseError {
  AccessControlAllowPrivateNetworkParseError::new(
    "invalid Access-Control-Allow-Private-Network header value",
  )
}
