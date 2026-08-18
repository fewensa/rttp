//! Bounded, policy-free `Content-Location` response metadata parsing.
//!
//! This module validates a singleton `Content-Location` field value as an
//! absolute URI or relative reference. It preserves the unresolved reference
//! string and never performs redirect, cache selection, representation
//! replacement, retry, route, or status-policy behavior.

use std::error::Error;
use std::fmt;

use url::Url;

pub const MAX_CONTENT_LOCATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Content-Location` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocation {
  value: String,
}

impl ContentLocation {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentLocationParseError> {
    let value = value.as_ref();
    if value.len() > MAX_CONTENT_LOCATION_VALUE_BYTES {
      return Err(ContentLocationParseError::new(
        "Content-Location header value is too large",
      ));
    }

    let value = trim_http_optional_whitespace(value);
    if value.is_empty() {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }
    if !is_content_location_field_value(value) {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }

    if Url::parse(value).is_ok() {
      return Ok(Self {
        value: value.to_string(),
      });
    }

    let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
    if !is_relative_uri_reference_field_value(value) {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }
    Url::options()
      .base_url(Some(&base))
      .parse(value)
      .map_err(|_| ContentLocationParseError::new("Invalid Content-Location value"))?;

    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Option<Self>, ContentLocationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(ContentLocationParseError::new(
        "Duplicate Content-Location header values",
      ));
    }
    Self::parse(value).map(Some)
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> &str {
    &self.value
  }
}

impl AsRef<str> for ContentLocation {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
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

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn is_content_location_field_value(value: &str) -> bool {
  value.bytes().all(|byte| {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'<' && byte != b'>' && byte != b'\\'
  })
}

fn is_relative_uri_reference_field_value(value: &str) -> bool {
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
