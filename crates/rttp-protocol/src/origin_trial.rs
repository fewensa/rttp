//! Bounded, policy-free parsing for HTTP `Origin-Trial` response metadata.
//!
//! This module preserves multiple opaque trial tokens in wire order. It does
//! not validate token signatures, expiration, origin applicability, feature
//! activation, browser behavior, or trial policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Origin-Trial` field value after OWS trim.
pub const MAX_ORIGIN_TRIAL_VALUE_BYTES: usize = 8 * 1024;

/// Maximum number of `Origin-Trial` tokens accepted in one collection.
pub const MAX_ORIGIN_TRIAL_TOKENS: usize = 64;

/// Maximum combined token bytes accepted across one `Origin-Trial` collection.
pub const MAX_ORIGIN_TRIAL_TOTAL_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Origin-Trial` response metadata.
///
/// Tokens are stored in wire order after OWS trim. Duplicate token strings are
/// preserved. Debug output reports only the type name and token count.
#[derive(Clone, Eq, PartialEq)]
pub struct OriginTrials {
  tokens: Vec<String>,
}

/// An error returned when `Origin-Trial` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginTrialParseError {
  message: String,
}

impl OriginTrials {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, OriginTrialParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, OriginTrialParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tokens = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      let trimmed = value.trim_matches([' ', '\t']);
      if trimmed.is_empty() || trimmed.bytes().any(is_disallowed_origin_trial_byte) {
        return Err(invalid_value());
      }
      if trimmed.len() > MAX_ORIGIN_TRIAL_VALUE_BYTES {
        return Err(values_too_large());
      }
      if tokens.len() >= MAX_ORIGIN_TRIAL_TOKENS {
        return Err(too_many_values());
      }
      total_bytes = total_bytes
        .checked_add(trimmed.len())
        .filter(|total| *total <= MAX_ORIGIN_TRIAL_TOTAL_BYTES)
        .ok_or_else(values_too_large)?;
      tokens.push(trimmed.to_string());
    }

    if tokens.is_empty() {
      return Err(invalid_value());
    }

    Ok(Self { tokens })
  }

  pub fn tokens(&self) -> &[String] {
    &self.tokens
  }

  pub fn header_values(&self) -> &[String] {
    &self.tokens
  }

  pub fn len(&self) -> usize {
    self.tokens.len()
  }

  pub fn is_empty(&self) -> bool {
    self.tokens.is_empty()
  }
}

impl fmt::Debug for OriginTrials {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("OriginTrials")
      .field("token_count", &self.tokens.len())
      .finish()
  }
}

impl OriginTrialParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for OriginTrialParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for OriginTrialParseError {}

fn is_disallowed_origin_trial_byte(byte: u8) -> bool {
  byte < 0x20 || byte == 0x7f || byte >= 0x80
}

fn invalid_value() -> OriginTrialParseError {
  OriginTrialParseError::new("invalid Origin-Trial header value")
}

fn too_many_values() -> OriginTrialParseError {
  OriginTrialParseError::new("too many Origin-Trial header values")
}

fn values_too_large() -> OriginTrialParseError {
  OriginTrialParseError::new("Origin-Trial header values are too large")
}
