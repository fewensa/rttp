//! Bounded, policy-free `Access-Control-Allow-Headers` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply CORS header behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in each `Access-Control-Allow-Headers` field value.
pub const MAX_ACCESS_CONTROL_ALLOW_HEADERS_VALUE_BYTES: usize = 64 * 1024;
/// Maximum comma-separated field-name members accepted across all field values.
pub const MAX_ACCESS_CONTROL_ALLOW_HEADERS_FIELD_NAMES: usize = 256;

/// Parsed, bounded `Access-Control-Allow-Headers` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowHeaders {
  wildcard: bool,
  field_names: Vec<String>,
}

impl AccessControlAllowHeaders {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlAllowHeadersParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlAllowHeadersParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut wildcard = false;
    let mut field_names = Vec::new();
    let mut field_count = 0usize;

    for value in values {
      if value.len() > MAX_ACCESS_CONTROL_ALLOW_HEADERS_VALUE_BYTES {
        return Err(AccessControlAllowHeadersParseError::new(
          "Access-Control-Allow-Headers header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AccessControlAllowHeadersParseError::new(
          "invalid Access-Control-Allow-Headers control byte",
        ));
      }

      for member in value.split(',') {
        let field_name = member.trim_matches([' ', '\t']);
        field_count += 1;
        if field_count > MAX_ACCESS_CONTROL_ALLOW_HEADERS_FIELD_NAMES {
          return Err(AccessControlAllowHeadersParseError::new(
            "too many Access-Control-Allow-Headers field names",
          ));
        }
        if field_name == "*" {
          if wildcard || !field_names.is_empty() {
            return Err(AccessControlAllowHeadersParseError::new(
              "invalid Access-Control-Allow-Headers field name",
            ));
          }
          wildcard = true;
          continue;
        }
        if wildcard || !is_http_token(field_name) {
          return Err(AccessControlAllowHeadersParseError::new(
            "invalid Access-Control-Allow-Headers field name",
          ));
        }
        let normalized = field_name.to_ascii_lowercase();
        if field_names.contains(&normalized) {
          return Err(AccessControlAllowHeadersParseError::new(
            "duplicate Access-Control-Allow-Headers field name",
          ));
        }
        field_names.push(normalized);
      }
    }

    if !wildcard && field_names.is_empty() {
      return Err(AccessControlAllowHeadersParseError::new(
        "invalid Access-Control-Allow-Headers field name",
      ));
    }

    Ok(Self {
      wildcard,
      field_names,
    })
  }

  pub fn is_wildcard(&self) -> bool {
    self.wildcard
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
    if self.wildcard {
      "*".to_string()
    } else {
      self.field_names.join(", ")
    }
  }
}

/// An error returned when `Access-Control-Allow-Headers` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowHeadersParseError {
  message: String,
}

impl AccessControlAllowHeadersParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlAllowHeadersParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlAllowHeadersParseError {}

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
