//! Bounded, policy-free `Access-Control-Request-Headers` request metadata parsing.
//!
//! This module validates one request field value only. Callers decide whether
//! and how to apply CORS preflight policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in each `Access-Control-Request-Headers` field value.
pub const MAX_ACCESS_CONTROL_REQUEST_HEADERS_VALUE_BYTES: usize = 64 * 1024;
/// Maximum comma-separated field-name members accepted across all field values.
pub const MAX_ACCESS_CONTROL_REQUEST_HEADERS_FIELD_NAMES: usize = 256;

/// Parsed, bounded `Access-Control-Request-Headers` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlRequestHeaders {
  field_names: Vec<String>,
}

impl AccessControlRequestHeaders {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlRequestHeadersParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlRequestHeadersParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut field_names = Vec::new();
    let mut field_count = 0usize;

    for value in values {
      if value.len() > MAX_ACCESS_CONTROL_REQUEST_HEADERS_VALUE_BYTES {
        return Err(AccessControlRequestHeadersParseError::new(
          "Access-Control-Request-Headers header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AccessControlRequestHeadersParseError::new(
          "invalid Access-Control-Request-Headers control byte",
        ));
      }

      for member in value.split(',') {
        let field_name = member.trim_matches([' ', '\t']);
        field_count += 1;
        if field_count > MAX_ACCESS_CONTROL_REQUEST_HEADERS_FIELD_NAMES {
          return Err(AccessControlRequestHeadersParseError::new(
            "too many Access-Control-Request-Headers field names",
          ));
        }
        if !is_http_token(field_name) {
          return Err(AccessControlRequestHeadersParseError::new(
            "invalid Access-Control-Request-Headers field name",
          ));
        }
        let normalized = field_name.to_ascii_lowercase();
        if field_names.contains(&normalized) {
          return Err(AccessControlRequestHeadersParseError::new(
            "duplicate Access-Control-Request-Headers field name",
          ));
        }
        field_names.push(normalized);
      }
    }

    if field_names.is_empty() {
      return Err(AccessControlRequestHeadersParseError::new(
        "invalid Access-Control-Request-Headers field name",
      ));
    }

    Ok(Self { field_names })
  }

  pub fn field_names(&self) -> &[String] {
    &self.field_names
  }

  pub fn len(&self) -> usize {
    self.field_names.len()
  }

  pub fn is_empty(&self) -> bool {
    self.field_names.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.field_names.join(", ")
  }
}

/// An error returned when `Access-Control-Request-Headers` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlRequestHeadersParseError {
  message: String,
}

impl AccessControlRequestHeadersParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlRequestHeadersParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlRequestHeadersParseError {}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
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
