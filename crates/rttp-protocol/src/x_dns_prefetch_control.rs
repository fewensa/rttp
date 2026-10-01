//! Bounded, policy-free `X-DNS-Prefetch-Control` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply DNS-prefetch behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `X-DNS-Prefetch-Control` field value.
pub const MAX_X_DNS_PREFETCH_CONTROL_VALUE_BYTES: usize = 64 * 1024;

/// The DNS-prefetch behavior declared by `X-DNS-Prefetch-Control`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum XDnsPrefetchControl {
  On,
  Off,
}

impl XDnsPrefetchControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XDnsPrefetchControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XDnsPrefetchControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value.eq_ignore_ascii_case("on") {
      Ok(Self::On)
    } else if value.eq_ignore_ascii_case("off") {
      Ok(Self::Off)
    } else {
      Err(invalid_value())
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::On => "on",
      Self::Off => "off",
    }
  }
}

/// An error returned when `X-DNS-Prefetch-Control` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XDnsPrefetchControlParseError {
  message: String,
}

impl XDnsPrefetchControlParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XDnsPrefetchControlParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XDnsPrefetchControlParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, XDnsPrefetchControlParseError>
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
    return Err(XDnsPrefetchControlParseError::new(
      "duplicate X-DNS-Prefetch-Control header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), XDnsPrefetchControlParseError> {
  if value.len() > MAX_X_DNS_PREFETCH_CONTROL_VALUE_BYTES {
    return Err(XDnsPrefetchControlParseError::new(
      "X-DNS-Prefetch-Control header value is too large",
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

fn invalid_value() -> XDnsPrefetchControlParseError {
  XDnsPrefetchControlParseError::new("invalid X-DNS-Prefetch-Control header value")
}
