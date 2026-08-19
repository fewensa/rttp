//! Bounded, policy-free `X-Content-Type-Options` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce MIME-sniffing protection.

use std::error::Error;
use std::fmt;

pub const MAX_X_CONTENT_TYPE_OPTIONS_VALUE_BYTES: usize = 64 * 1024;

/// The MIME-sniffing protection declared by `X-Content-Type-Options`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XContentTypeOptions {
  Nosniff,
}

impl XContentTypeOptions {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XContentTypeOptionsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XContentTypeOptionsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("nosniff") {
      Ok(Self::Nosniff)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Nosniff => "nosniff",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XContentTypeOptionsParseError {
  message: String,
}

impl XContentTypeOptionsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XContentTypeOptionsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XContentTypeOptionsParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, XContentTypeOptionsParseError>
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
    return Err(XContentTypeOptionsParseError::new(
      "duplicate X-Content-Type-Options header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), XContentTypeOptionsParseError> {
  if value.len() > MAX_X_CONTENT_TYPE_OPTIONS_VALUE_BYTES {
    return Err(XContentTypeOptionsParseError::new(
      "X-Content-Type-Options header value is too large",
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

fn invalid_value() -> XContentTypeOptionsParseError {
  XContentTypeOptionsParseError::new("invalid X-Content-Type-Options header value")
}
