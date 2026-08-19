//! Bounded, policy-free `Content-Location` response metadata parsing.
//!
//! This module validates one `Content-Location` URI-reference field value only.
//! Callers retain redirect, cache, representation, routing, and resolution
//! policy.

use std::error::Error;
use std::fmt;

use url::Url;

pub const MAX_CONTENT_LOCATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Content-Location` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocation {
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocationParseError {
  message: String,
}

impl ContentLocationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentLocationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentLocationParseError {}

impl ContentLocation {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentLocationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentLocationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_uri_reference(value)
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl AsRef<str> for ContentLocation {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, ContentLocationParseError>
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
    return Err(ContentLocationParseError::new(
      "duplicate Content-Location header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), ContentLocationParseError> {
  if value.len() > MAX_CONTENT_LOCATION_VALUE_BYTES {
    return Err(ContentLocationParseError::new(
      "Content-Location header value is too large",
    ));
  }
  Ok(())
}

fn parse_uri_reference(value: &str) -> Result<ContentLocation, ContentLocationParseError> {
  let value = trim_http_optional_whitespace(value);
  if value.is_empty() || !is_content_location_field_value(value) {
    return Err(invalid_value());
  }

  if Url::parse(value).is_ok() {
    return Ok(ContentLocation {
      value: value.to_string(),
    });
  }

  let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
  if !is_relative_uri_reference_field_value(value) {
    return Err(invalid_value());
  }
  Url::options()
    .base_url(Some(&base))
    .parse(value)
    .map_err(|_| invalid_value())?;

  Ok(ContentLocation {
    value: value.to_string(),
  })
}

fn invalid_value() -> ContentLocationParseError {
  ContentLocationParseError::new("invalid Content-Location value")
}

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn is_content_location_field_value(value: &str) -> bool {
  value.bytes().all(|byte| {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'<' && byte != b'>' && byte != b'\\'
  })
}

fn is_relative_uri_reference_field_value(value: &str) -> bool {
  let value = strip_network_path_authority(value);
  let mut fragment_seen = false;
  let mut query_seen = false;
  let mut bytes = value.bytes().peekable();

  while let Some(byte) = bytes.next() {
    match byte {
      b'%' => {
        let Some(first) = bytes.next() else {
          return false;
        };
        let Some(second) = bytes.next() else {
          return false;
        };
        if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
          return false;
        }
      }
      b'#' => {
        if fragment_seen {
          return false;
        }
        fragment_seen = true;
      }
      b'?' => {
        if !fragment_seen {
          query_seen = true;
        }
      }
      _ => {
        if fragment_seen {
          if !is_fragment_char(byte) {
            return false;
          }
        } else if query_seen {
          if !is_query_char(byte) {
            return false;
          }
        } else if !is_uri_path_char(byte) {
          return false;
        }
      }
    }
  }

  true
}

fn strip_network_path_authority(value: &str) -> &str {
  let Some(remainder) = value.strip_prefix("//") else {
    return value;
  };
  let Some(authority_end) = remainder.find(['/', '?', '#']) else {
    return "";
  };
  &remainder[authority_end..]
}

fn is_uri_path_char(byte: u8) -> bool {
  is_uri_pchar(byte) || byte == b'/'
}

fn is_query_char(byte: u8) -> bool {
  is_uri_pchar(byte) || matches!(byte, b'/' | b'?')
}

fn is_fragment_char(byte: u8) -> bool {
  is_query_char(byte)
}

fn is_uri_pchar(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'-'
        | b'.'
        | b'_'
        | b'~'
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
        | b':'
        | b'@'
    )
}
