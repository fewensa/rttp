//! Bounded, policy-free parsing for `X-Forwarded-Proto` request metadata.
//!
//! This module validates ordered scheme tokens only. It does not decide which
//! proxy hops are trusted, upgrade requests, redirect clients, or rewrite URL
//! schemes.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `X-Forwarded-Proto` field value, in the
/// combined raw field set including `", "` separator overhead, and in the
/// combined serialized field value.
pub const MAX_X_FORWARDED_PROTO_VALUE_BYTES: usize = 64 * 1024;
/// Maximum `X-Forwarded-Proto` scheme values accepted across all fields.
pub const MAX_X_FORWARDED_PROTOS: usize = 256;

/// Parsed, bounded `X-Forwarded-Proto` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedProto {
  schemes: Vec<String>,
}

/// An error returned when `X-Forwarded-Proto` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedProtoParseError {
  message: String,
}

impl XForwardedProtoParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XForwardedProtoParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XForwardedProtoParseError {}

impl XForwardedProto {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XForwardedProtoParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XForwardedProtoParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut schemes = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_value(value, &mut total_bytes)?;
      parse_field(value, &mut schemes)?;
    }
    if schemes.is_empty() {
      return Err(invalid_scheme());
    }
    let forwarded_proto = Self { schemes };
    if forwarded_proto.header_value().len() > MAX_X_FORWARDED_PROTO_VALUE_BYTES {
      return Err(XForwardedProtoParseError::new(
        "X-Forwarded-Proto header value is too large",
      ));
    }
    Ok(forwarded_proto)
  }

  pub fn schemes(&self) -> &[String] {
    &self.schemes
  }

  pub fn len(&self) -> usize {
    self.schemes.len()
  }

  pub fn is_empty(&self) -> bool {
    self.schemes.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.schemes.join(", ")
  }
}

fn validate_value(value: &str, total_bytes: &mut usize) -> Result<(), XForwardedProtoParseError> {
  if value.len() > MAX_X_FORWARDED_PROTO_VALUE_BYTES {
    return Err(XForwardedProtoParseError::new(
      "X-Forwarded-Proto header value is too large",
    ));
  }
  let separator = if *total_bytes > 0 { 2 } else { 0 };
  *total_bytes = total_bytes
    .saturating_add(separator)
    .saturating_add(value.len());
  if *total_bytes > MAX_X_FORWARDED_PROTO_VALUE_BYTES {
    return Err(XForwardedProtoParseError::new(
      "X-Forwarded-Proto header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(XForwardedProtoParseError::new(
      "invalid X-Forwarded-Proto control byte",
    ));
  }
  Ok(())
}

fn parse_field(value: &str, schemes: &mut Vec<String>) -> Result<(), XForwardedProtoParseError> {
  for raw_scheme in value.split(',') {
    if schemes.len() >= MAX_X_FORWARDED_PROTOS {
      return Err(XForwardedProtoParseError::new(
        "too many X-Forwarded-Proto values",
      ));
    }
    let scheme = raw_scheme.trim_matches([' ', '\t']);
    if !is_scheme(scheme) {
      return Err(invalid_scheme());
    }
    schemes.push(scheme.to_string());
  }
  Ok(())
}

fn is_scheme(value: &str) -> bool {
  let Some(first) = value.as_bytes().first() else {
    return false;
  };
  first.is_ascii_alphabetic()
    && value
      .bytes()
      .skip(1)
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn invalid_scheme() -> XForwardedProtoParseError {
  XForwardedProtoParseError::new("invalid X-Forwarded-Proto value")
}
