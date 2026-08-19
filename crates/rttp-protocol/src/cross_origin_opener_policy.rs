//! Bounded, policy-free `Cross-Origin-Opener-Policy` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce opener policy. Unparsable input is an error; this
//! parser never fails open to `unsafe-none`.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Item, Parser};

pub const MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES: usize = 64 * 1024;

/// The opener policy declared by `Cross-Origin-Opener-Policy`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossOriginOpenerPolicy {
  UnsafeNone,
  SameOriginAllowPopups,
  SameOrigin,
  NoopenerAllowPopups,
}

impl CrossOriginOpenerPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CrossOriginOpenerPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CrossOriginOpenerPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let item = Parser::new(value)
      .parse::<Item>()
      .map_err(|_| invalid_value())?;
    let BareItem::Token(token) = item.bare_item else {
      return Err(invalid_value());
    };
    Self::from_directive_token(token.as_str()).ok_or_else(invalid_value)
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::UnsafeNone => "unsafe-none",
      Self::SameOriginAllowPopups => "same-origin-allow-popups",
      Self::SameOrigin => "same-origin",
      Self::NoopenerAllowPopups => "noopener-allow-popups",
    }
  }

  pub(crate) fn from_directive_token(token: &str) -> Option<Self> {
    match token {
      "unsafe-none" => Some(Self::UnsafeNone),
      "same-origin-allow-popups" => Some(Self::SameOriginAllowPopups),
      "same-origin" => Some(Self::SameOrigin),
      "noopener-allow-popups" => Some(Self::NoopenerAllowPopups),
      _ => None,
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginOpenerPolicyParseError {
  message: String,
}

impl CrossOriginOpenerPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CrossOriginOpenerPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CrossOriginOpenerPolicyParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, CrossOriginOpenerPolicyParseError>
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
    return Err(CrossOriginOpenerPolicyParseError::new(
      "duplicate Cross-Origin-Opener-Policy header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), CrossOriginOpenerPolicyParseError> {
  if value.len() > MAX_CROSS_ORIGIN_OPENER_POLICY_VALUE_BYTES {
    return Err(CrossOriginOpenerPolicyParseError::new(
      "Cross-Origin-Opener-Policy header value is too large",
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

fn invalid_value() -> CrossOriginOpenerPolicyParseError {
  CrossOriginOpenerPolicyParseError::new("invalid Cross-Origin-Opener-Policy header value")
}
