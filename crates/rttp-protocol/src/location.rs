//! Bounded, policy-free parsing for the HTTP `Location` response header.
//!
//! This module validates one RFC 9110 URI reference (`absolute-URI` /
//! `partial-URI`). Surrounding SP and HTAB are trimmed as optional whitespace.
//! A successful parse stores that trimmed text and does not resolve relative
//! references or apply redirect policy.

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a `Location` field value.
pub const MAX_LOCATION_VALUE_BYTES: usize = 64 * 1024;

/// A parsed HTTP `Location` field value.
///
/// The stored text is the OWS-trimmed URI reference from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Location(String);

/// An error returned when `Location` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocationParseError {
  message: String,
}

impl Location {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, LocationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, LocationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    Ok(Self(value))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn header_value(&self) -> String {
    self.0.clone()
  }
}

impl LocationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for LocationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for LocationParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, LocationParseError>
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
    return Err(LocationParseError::new("duplicate Location header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  if !is_uri_reference_text(value) {
    return Err(invalid_value());
  }
  if !is_structural_uri_reference(value) {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_value(value: &str) -> Result<(), LocationParseError> {
  if value.len() > MAX_LOCATION_VALUE_BYTES {
    return Err(LocationParseError::new(
      "Location header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(LocationParseError::new(
      "invalid Location header control byte",
    ));
  }
  Ok(())
}

fn is_uri_reference_text(value: &str) -> bool {
  let bytes = value.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    if byte == b'%' {
      if index + 2 >= bytes.len()
        || !bytes[index + 1].is_ascii_hexdigit()
        || !bytes[index + 2].is_ascii_hexdigit()
      {
        return false;
      }
      index += 3;
      continue;
    }
    if !is_uri_byte(byte) {
      return false;
    }
    index += 1;
  }
  true
}

fn is_uri_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'-'
        | b'.'
        | b'_'
        | b'~'
        | b':'
        | b'/'
        | b'?'
        | b'#'
        | b'['
        | b']'
        | b'@'
        | b'!'
        | b'$'
        | b'&'
        | b'\''
        | b'('
        | b')'
        | b'*'
        | b'+'
        | b','
        | b';'
        | b'='
    )
}

fn is_structural_uri_reference(value: &str) -> bool {
  if Url::parse(value).is_ok() {
    return true;
  }
  let Ok(base) = Url::parse("https://rttp.invalid/") else {
    return false;
  };
  Url::options().base_url(Some(&base)).parse(value).is_ok()
}

fn invalid_value() -> LocationParseError {
  LocationParseError::new("invalid Location header value")
}
