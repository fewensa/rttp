//! Bounded, policy-free `Access-Control-Request-Private-Network` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to apply Private Network Access or CORS preflight behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Access-Control-Request-Private-Network` field value.
pub const MAX_ACCESS_CONTROL_REQUEST_PRIVATE_NETWORK_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Access-Control-Request-Private-Network` request metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct AccessControlRequestPrivateNetwork;

impl AccessControlRequestPrivateNetwork {
  pub fn parse(
    value: impl AsRef<str>,
  ) -> Result<Self, AccessControlRequestPrivateNetworkParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(
    values: I,
  ) -> Result<Self, AccessControlRequestPrivateNetworkParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(&self) -> &'static str {
    "true"
  }
}

/// An error returned when `Access-Control-Request-Private-Network` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlRequestPrivateNetworkParseError {
  message: String,
}

impl AccessControlRequestPrivateNetworkParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlRequestPrivateNetworkParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlRequestPrivateNetworkParseError {}

fn parse_singleton<'a, I>(
  values: I,
) -> Result<AccessControlRequestPrivateNetwork, AccessControlRequestPrivateNetworkParseError>
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
    return Err(AccessControlRequestPrivateNetworkParseError::new(
      "duplicate Access-Control-Request-Private-Network header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value != "true" {
    return Err(invalid_value());
  }

  Ok(AccessControlRequestPrivateNetwork)
}

fn validate_value(value: &str) -> Result<(), AccessControlRequestPrivateNetworkParseError> {
  if value.len() > MAX_ACCESS_CONTROL_REQUEST_PRIVATE_NETWORK_VALUE_BYTES {
    return Err(AccessControlRequestPrivateNetworkParseError::new(
      "Access-Control-Request-Private-Network header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(AccessControlRequestPrivateNetworkParseError::new(
      "invalid Access-Control-Request-Private-Network control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> AccessControlRequestPrivateNetworkParseError {
  AccessControlRequestPrivateNetworkParseError::new(
    "invalid Access-Control-Request-Private-Network header value",
  )
}
