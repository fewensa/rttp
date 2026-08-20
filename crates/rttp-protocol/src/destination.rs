//! Bounded, policy-free WebDAV `Destination` request metadata parsing.
//!
//! This module validates one RFC 3986 `absolute-URI` field value only.
//! Surrounding SP and HTAB are trimmed as optional whitespace. A successful
//! parse stores that trimmed text and does not resolve the destination against
//! a request target, normalize URI components, authorize access, or copy or
//! move application resources.

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a `Destination` field value.
pub const MAX_DESTINATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded WebDAV `Destination` request metadata.
///
/// The stored text is the OWS-trimmed absolute URI from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Destination(String);

/// An error returned when `Destination` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestinationParseError {
  message: String,
}

impl Destination {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DestinationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DestinationParseError>
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

impl AsRef<str> for Destination {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl DestinationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DestinationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DestinationParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, DestinationParseError>
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
    return Err(DestinationParseError::new(
      "duplicate Destination header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  if !is_absolute_uri_text(value) {
    return Err(invalid_value());
  }
  if Url::parse(value).is_err() {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_value(value: &str) -> Result<(), DestinationParseError> {
  if value.len() > MAX_DESTINATION_VALUE_BYTES {
    return Err(DestinationParseError::new(
      "Destination header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(DestinationParseError::new(
      "invalid Destination header control byte",
    ));
  }
  Ok(())
}

fn is_absolute_uri_text(value: &str) -> bool {
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

fn invalid_value() -> DestinationParseError {
  DestinationParseError::new("invalid Destination header value")
}
