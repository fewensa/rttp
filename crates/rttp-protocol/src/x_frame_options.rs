//! Bounded, policy-free `X-Frame-Options` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce frame embedding policy.

use std::error::Error;
use std::fmt;

pub const MAX_X_FRAME_OPTIONS_VALUE_BYTES: usize = 64 * 1024;

/// The frame embedding policy declared by `X-Frame-Options`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XFrameOptions {
  Deny,
  SameOrigin,
}

impl XFrameOptions {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XFrameOptionsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XFrameOptionsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("DENY") {
      Ok(Self::Deny)
    } else if value.eq_ignore_ascii_case("SAMEORIGIN") {
      Ok(Self::SameOrigin)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Deny => "DENY",
      Self::SameOrigin => "SAMEORIGIN",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XFrameOptionsParseError {
  message: String,
}

impl XFrameOptionsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XFrameOptionsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XFrameOptionsParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, XFrameOptionsParseError>
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
    return Err(XFrameOptionsParseError::new(
      "duplicate X-Frame-Options header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), XFrameOptionsParseError> {
  if value.len() > MAX_X_FRAME_OPTIONS_VALUE_BYTES {
    return Err(XFrameOptionsParseError::new(
      "X-Frame-Options header value is too large",
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

fn invalid_value() -> XFrameOptionsParseError {
  XFrameOptionsParseError::new("invalid X-Frame-Options header value")
}
