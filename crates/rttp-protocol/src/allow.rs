//! Bounded, policy-free `Allow` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply method handling behavior.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

/// Maximum bytes accepted in each `Allow` field value.
pub const MAX_ALLOW_VALUE_BYTES: usize = 64 * 1024;
/// Maximum comma-separated method members accepted across all field values.
pub const MAX_ALLOW_METHODS: usize = 256;

/// Parsed, bounded `Allow` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allow {
  methods: Vec<String>,
}

impl Allow {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AllowParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AllowParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut methods = Vec::new();

    for value in values {
      validate_field_value(value)?;

      for member in value.split(',') {
        let method = member.trim_matches([' ', '\t']);
        if method.is_empty() || !is_token(method) {
          return Err(AllowParseError::new("invalid Allow method"));
        }
        if methods.iter().any(|known| known == method) {
          return Err(AllowParseError::new("duplicate Allow method"));
        }
        if methods.len() >= MAX_ALLOW_METHODS {
          return Err(AllowParseError::new("too many Allow methods"));
        }
        methods.push(method.to_string());
      }
    }

    if methods.is_empty() {
      return Err(AllowParseError::new("invalid Allow method"));
    }

    Ok(Self { methods })
  }

  pub fn from_methods<I, M>(methods: I) -> Result<Self, AllowParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let mut value = String::new();

    for (index, method) in methods.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(method.as_ref());
      if value.len() > MAX_ALLOW_VALUE_BYTES {
        return Err(AllowParseError::new("Allow header value is too large"));
      }
    }

    Self::parse(value)
  }

  pub fn methods(&self) -> Vec<&str> {
    self.methods.iter().map(String::as_str).collect()
  }

  pub fn contains_method(&self, method: impl AsRef<str>) -> bool {
    self
      .methods
      .iter()
      .any(|candidate| candidate == method.as_ref())
  }

  pub fn header_value(&self) -> String {
    self.methods.join(", ")
  }
}

/// An error returned when `Allow` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowParseError {
  message: String,
}

impl AllowParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AllowParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AllowParseError {}

fn validate_field_value(value: &str) -> Result<(), AllowParseError> {
  if value.len() > MAX_ALLOW_VALUE_BYTES {
    return Err(AllowParseError::new("Allow header value is too large"));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(AllowParseError::new("invalid Allow control byte"));
  }
  Ok(())
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
