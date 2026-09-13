//! Bounded, policy-free `X-Download-Options` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce download handling.

use std::error::Error;
use std::fmt;

pub const MAX_X_DOWNLOAD_OPTIONS_VALUE_BYTES: usize = 64 * 1024;

/// The download handling declared by `X-Download-Options`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XDownloadOptions {
  Noopen,
}

impl XDownloadOptions {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XDownloadOptionsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XDownloadOptionsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("noopen") {
      Ok(Self::Noopen)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Noopen => "noopen",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDownloadOptionsParseError {
  message: String,
}

impl XDownloadOptionsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XDownloadOptionsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XDownloadOptionsParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, XDownloadOptionsParseError>
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
    return Err(XDownloadOptionsParseError::new(
      "duplicate X-Download-Options header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), XDownloadOptionsParseError> {
  if value.len() > MAX_X_DOWNLOAD_OPTIONS_VALUE_BYTES {
    return Err(XDownloadOptionsParseError::new(
      "X-Download-Options header value is too large",
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

fn invalid_value() -> XDownloadOptionsParseError {
  XDownloadOptionsParseError::new("invalid X-Download-Options header value")
}
