//! Bounded, policy-free `Access-Control-Request-Method` request metadata parsing.
//!
//! This module validates the request field value only. Callers decide whether
//! and how to apply CORS preflight behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in an `Access-Control-Request-Method` field value.
pub const MAX_ACCESS_CONTROL_REQUEST_METHOD_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Access-Control-Request-Method` request metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AccessControlRequestMethod(String);

impl AccessControlRequestMethod {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlRequestMethodParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlRequestMethodParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let method = parse_singleton(values)?;
    Ok(Self(method))
  }

  pub fn method(&self) -> &str {
    &self.0
  }

  pub fn header_value(&self) -> String {
    self.0.clone()
  }
}

/// An error returned when `Access-Control-Request-Method` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlRequestMethodParseError {
  message: String,
}

impl AccessControlRequestMethodParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlRequestMethodParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlRequestMethodParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, AccessControlRequestMethodParseError>
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
    return Err(AccessControlRequestMethodParseError::new(
      "duplicate Access-Control-Request-Method header fields",
    ));
  }

  let method = value.trim_matches([' ', '\t']);
  if method.is_empty() || method.contains(',') {
    return Err(invalid_value());
  }
  if method == "*" {
    return Err(AccessControlRequestMethodParseError::new(
      "invalid Access-Control-Request-Method method",
    ));
  }
  if !is_http_token(method) {
    return Err(AccessControlRequestMethodParseError::new(
      "invalid Access-Control-Request-Method method",
    ));
  }
  Ok(method.to_ascii_uppercase())
}

fn validate_value(value: &str) -> Result<(), AccessControlRequestMethodParseError> {
  if value.len() > MAX_ACCESS_CONTROL_REQUEST_METHOD_VALUE_BYTES {
    return Err(AccessControlRequestMethodParseError::new(
      "Access-Control-Request-Method header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(AccessControlRequestMethodParseError::new(
      "invalid Access-Control-Request-Method control byte",
    ));
  }
  Ok(())
}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_http_token_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!'
        | b'#'
        | b'$'
        | b'%'
        | b'&'
        | b'\''
        | b'*'
        | b'+'
        | b'-'
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~'
    )
}

fn invalid_value() -> AccessControlRequestMethodParseError {
  AccessControlRequestMethodParseError::new("invalid Access-Control-Request-Method header value")
}
