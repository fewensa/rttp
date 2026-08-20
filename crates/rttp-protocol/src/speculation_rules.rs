//! Bounded, opaque `Speculation-Rules` response metadata.
//!
//! This module preserves one `Speculation-Rules` field value as metadata only.
//! It does not fetch, parse, validate, or execute speculation rule resources.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Speculation-Rules` field value.
pub const MAX_SPECULATION_RULES_VALUE_BYTES: usize = 64 * 1024;

/// Bounded, opaque `Speculation-Rules` response metadata.
///
/// The field value is preserved exactly after validation. Debug output reports
/// only the byte length and never includes the value.
#[derive(Clone, Eq, PartialEq)]
pub struct SpeculationRules {
  value: String,
}

/// An error returned when `Speculation-Rules` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeculationRulesParseError {
  message: String,
}

impl SpeculationRules {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SpeculationRulesParseError> {
    validate_value(value.as_ref())?;
    Ok(Self {
      value: value.as_ref().to_string(),
    })
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SpeculationRulesParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Err(invalid_value());
    };
    validate_value(value)?;
    let mut has_duplicate = false;
    for value in values {
      has_duplicate = true;
      validate_value(value)?;
    }
    if has_duplicate {
      return Err(SpeculationRulesParseError::new(
        "duplicate Speculation-Rules header fields",
      ));
    }
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> &str {
    &self.value
  }
}

impl fmt::Debug for SpeculationRules {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("SpeculationRules")
      .field("value_bytes", &self.value.len())
      .finish()
  }
}

impl SpeculationRulesParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SpeculationRulesParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SpeculationRulesParseError {}

fn validate_value(value: &str) -> Result<(), SpeculationRulesParseError> {
  if value.is_empty() {
    return Err(invalid_value());
  }
  if value.len() > MAX_SPECULATION_RULES_VALUE_BYTES {
    return Err(SpeculationRulesParseError::new(
      "Speculation-Rules header value is too large",
    ));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(SpeculationRulesParseError::new(
      "Speculation-Rules header value contains an invalid control byte",
    ));
  }
  Ok(())
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn invalid_value() -> SpeculationRulesParseError {
  SpeculationRulesParseError::new("invalid Speculation-Rules header value")
}
