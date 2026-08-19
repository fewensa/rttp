//! Bounded, policy-free `Save-Data` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to adapt content for a reduced-data preference.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Save-Data` field value.
pub const MAX_SAVE_DATA_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Save-Data` request metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SaveData;

impl SaveData {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SaveDataParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SaveDataParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_singleton(values)
  }

  pub fn header_value(&self) -> &'static str {
    "on"
  }
}

/// An error returned when `Save-Data` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveDataParseError {
  message: String,
}

impl SaveDataParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SaveDataParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SaveDataParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<SaveData, SaveDataParseError>
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
    return Err(SaveDataParseError::new("duplicate Save-Data header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value != "on" {
    return Err(invalid_value());
  }

  Ok(SaveData)
}

fn validate_value(value: &str) -> Result<(), SaveDataParseError> {
  if value.len() > MAX_SAVE_DATA_VALUE_BYTES {
    return Err(SaveDataParseError::new(
      "Save-Data header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(SaveDataParseError::new("invalid Save-Data control byte"));
  }
  Ok(())
}

fn invalid_value() -> SaveDataParseError {
  SaveDataParseError::new("invalid Save-Data header value")
}
