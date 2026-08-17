//! Bounded, policy-free `Cross-Origin-Embedder-Policy` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to enforce embedder policy. Unparsable input is an error; this
//! parser never fails open to `unsafe-none`.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Item, Parser};

pub const MAX_CROSS_ORIGIN_EMBEDDER_POLICY_VALUE_BYTES: usize = 64 * 1024;

/// The embedder policy declared by `Cross-Origin-Embedder-Policy`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossOriginEmbedderPolicy {
  UnsafeNone,
  RequireCorp,
  Credentialless,
}

impl CrossOriginEmbedderPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CrossOriginEmbedderPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CrossOriginEmbedderPolicyParseError>
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
    match token.as_str() {
      "unsafe-none" => Ok(Self::UnsafeNone),
      "require-corp" => Ok(Self::RequireCorp),
      "credentialless" => Ok(Self::Credentialless),
      _ => Err(invalid_value()),
    }
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::UnsafeNone => "unsafe-none",
      Self::RequireCorp => "require-corp",
      Self::Credentialless => "credentialless",
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginEmbedderPolicyParseError {
  message: String,
}

impl CrossOriginEmbedderPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CrossOriginEmbedderPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CrossOriginEmbedderPolicyParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, CrossOriginEmbedderPolicyParseError>
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
    return Err(CrossOriginEmbedderPolicyParseError::new(
      "duplicate Cross-Origin-Embedder-Policy header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), CrossOriginEmbedderPolicyParseError> {
  if value.len() > MAX_CROSS_ORIGIN_EMBEDDER_POLICY_VALUE_BYTES {
    return Err(CrossOriginEmbedderPolicyParseError::new(
      "Cross-Origin-Embedder-Policy header value is too large",
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

fn invalid_value() -> CrossOriginEmbedderPolicyParseError {
  CrossOriginEmbedderPolicyParseError::new("invalid Cross-Origin-Embedder-Policy header value")
}
