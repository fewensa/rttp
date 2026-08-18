//! Bounded, policy-free `Cross-Origin-Embedder-Policy-Report-Only` response
//! metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to use the report-only metadata. Unparsable input is an error; this
//! parser never fails open to `unsafe-none`.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Item, Parser};

pub const MAX_CROSS_ORIGIN_EMBEDDER_POLICY_REPORT_ONLY_VALUE_BYTES: usize = 64 * 1024;

/// The report-only embedder policy declared by
/// `Cross-Origin-Embedder-Policy-Report-Only`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossOriginEmbedderPolicyReportOnly {
  UnsafeNone,
  RequireCorp,
  Credentialless,
}

impl CrossOriginEmbedderPolicyReportOnly {
  pub fn parse(
    value: impl AsRef<str>,
  ) -> Result<Self, CrossOriginEmbedderPolicyReportOnlyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(
    values: I,
  ) -> Result<Self, CrossOriginEmbedderPolicyReportOnlyParseError>
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
pub struct CrossOriginEmbedderPolicyReportOnlyParseError {
  message: String,
}

impl CrossOriginEmbedderPolicyReportOnlyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CrossOriginEmbedderPolicyReportOnlyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CrossOriginEmbedderPolicyReportOnlyParseError {}

fn parse_singleton<'a, I>(
  values: I,
) -> Result<&'a str, CrossOriginEmbedderPolicyReportOnlyParseError>
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
    return Err(CrossOriginEmbedderPolicyReportOnlyParseError::new(
      "duplicate Cross-Origin-Embedder-Policy-Report-Only header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(
  value: &str,
) -> Result<(), CrossOriginEmbedderPolicyReportOnlyParseError> {
  if value.len() > MAX_CROSS_ORIGIN_EMBEDDER_POLICY_REPORT_ONLY_VALUE_BYTES {
    return Err(CrossOriginEmbedderPolicyReportOnlyParseError::new(
      "Cross-Origin-Embedder-Policy-Report-Only header value is too large",
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

fn invalid_value() -> CrossOriginEmbedderPolicyReportOnlyParseError {
  CrossOriginEmbedderPolicyReportOnlyParseError::new(
    "invalid Cross-Origin-Embedder-Policy-Report-Only header value",
  )
}
