//! Bounded, policy-free `Content-DPR` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to observe device-pixel-ratio metadata. It does not rescale images,
//! send request DPR, apply Client Hints policy, retry, or change transport.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Content-DPR` field value.
pub const MAX_CONTENT_DPR_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Content-DPR` response metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentDpr {
  value: String,
}

impl ContentDpr {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentDprParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentDprParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    parse_ratio(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn ratio(&self) -> f64 {
    parse_ratio(&self.value).expect("Content-DPR values are validated at construction")
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

/// An error returned when `Content-DPR` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDprParseError {
  message: String,
}

impl ContentDprParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentDprParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentDprParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, ContentDprParseError>
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
    return Err(ContentDprParseError::new(
      "duplicate Content-DPR header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), ContentDprParseError> {
  if value.len() > MAX_CONTENT_DPR_VALUE_BYTES {
    return Err(ContentDprParseError::new(
      "Content-DPR header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ContentDprParseError::new(
      "invalid Content-DPR control byte",
    ));
  }
  Ok(())
}

fn parse_ratio(value: &str) -> Result<f64, ContentDprParseError> {
  if !matches_content_dpr_grammar(value) {
    return Err(invalid_value());
  }
  let ratio: f64 = value.parse().map_err(|_| invalid_value())?;
  if !ratio.is_finite() || ratio <= 0.0 {
    return Err(invalid_value());
  }
  Ok(ratio)
}

fn matches_content_dpr_grammar(value: &str) -> bool {
  let bytes = value.as_bytes();
  if bytes.is_empty() || !bytes[0].is_ascii_digit() {
    return false;
  }

  let mut index = 0;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  if index == bytes.len() {
    return true;
  }
  if bytes[index] != b'.' {
    return false;
  }
  index += 1;
  let fraction_start = index;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  index > fraction_start && index == bytes.len()
}

fn invalid_value() -> ContentDprParseError {
  ContentDprParseError::new("invalid Content-DPR header value")
}
