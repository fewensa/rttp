//! Bounded, policy-free `Access-Control-Allow-Methods` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply CORS method behavior.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in each `Access-Control-Allow-Methods` field value.
pub const MAX_ACCESS_CONTROL_ALLOW_METHODS_VALUE_BYTES: usize = 64 * 1024;
/// Maximum comma-separated method members accepted across all field values.
pub const MAX_ACCESS_CONTROL_ALLOW_METHODS_METHODS: usize = 256;

/// Parsed, bounded `Access-Control-Allow-Methods` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowMethods {
  wildcard: bool,
  methods: Vec<String>,
}

impl AccessControlAllowMethods {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlAllowMethodsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlAllowMethodsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut wildcard = false;
    let mut methods = Vec::new();
    let mut method_count = 0usize;

    for value in values {
      if value.len() > MAX_ACCESS_CONTROL_ALLOW_METHODS_VALUE_BYTES {
        return Err(AccessControlAllowMethodsParseError::new(
          "Access-Control-Allow-Methods header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AccessControlAllowMethodsParseError::new(
          "invalid Access-Control-Allow-Methods control byte",
        ));
      }

      for member in value.split(',') {
        let method = member.trim_matches([' ', '\t']);
        method_count += 1;
        if method_count > MAX_ACCESS_CONTROL_ALLOW_METHODS_METHODS {
          return Err(AccessControlAllowMethodsParseError::new(
            "too many Access-Control-Allow-Methods methods",
          ));
        }
        if method == "*" {
          wildcard = true;
          continue;
        }
        if !is_http_token(method) {
          return Err(AccessControlAllowMethodsParseError::new(
            "invalid Access-Control-Allow-Methods method",
          ));
        }
        let normalized = method.to_ascii_uppercase();
        if !methods.contains(&normalized) {
          methods.push(normalized);
        }
      }
    }

    if !wildcard && methods.is_empty() {
      return Err(AccessControlAllowMethodsParseError::new(
        "invalid Access-Control-Allow-Methods method",
      ));
    }

    Ok(Self { wildcard, methods })
  }

  pub fn is_wildcard(&self) -> bool {
    self.wildcard
  }

  pub fn methods(&self) -> &[String] {
    &self.methods
  }

  pub fn len(&self) -> usize {
    self.methods.len()
  }

  pub fn is_empty(&self) -> bool {
    self.methods.is_empty()
  }

  pub fn header_value(&self) -> String {
    if self.wildcard {
      if self.methods.is_empty() {
        "*".to_string()
      } else {
        format!("*, {}", self.methods.join(", "))
      }
    } else {
      self.methods.join(", ")
    }
  }
}

/// An error returned when `Access-Control-Allow-Methods` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlAllowMethodsParseError {
  message: String,
}

impl AccessControlAllowMethodsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlAllowMethodsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlAllowMethodsParseError {}

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
