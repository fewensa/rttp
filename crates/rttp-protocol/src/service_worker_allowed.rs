//! Bounded, policy-free `Service-Worker-Allowed` response metadata parsing.
//!
//! This module validates one origin-relative or absolute path field value only.
//! Callers retain service-worker registration, scope resolution, routing, and
//! application policy.

use std::error::Error;
use std::fmt;

pub const MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Service-Worker-Allowed` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceWorkerAllowed {
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceWorkerAllowedParseError {
  message: String,
}

impl ServiceWorkerAllowedParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ServiceWorkerAllowedParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ServiceWorkerAllowedParseError {}

impl ServiceWorkerAllowed {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ServiceWorkerAllowedParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ServiceWorkerAllowedParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_path_value(value)
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl AsRef<str> for ServiceWorkerAllowed {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl fmt::Display for ServiceWorkerAllowed {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(self.as_str())
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, ServiceWorkerAllowedParseError>
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
    return Err(ServiceWorkerAllowedParseError::new(
      "duplicate Service-Worker-Allowed header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), ServiceWorkerAllowedParseError> {
  if value.len() > MAX_SERVICE_WORKER_ALLOWED_VALUE_BYTES {
    return Err(ServiceWorkerAllowedParseError::new(
      "Service-Worker-Allowed header value is too large",
    ));
  }
  Ok(())
}

fn parse_path_value(value: &str) -> Result<ServiceWorkerAllowed, ServiceWorkerAllowedParseError> {
  let value = trim_http_optional_whitespace(value);
  if !is_service_worker_allowed_path_text(value) {
    return Err(invalid_value());
  }

  Ok(ServiceWorkerAllowed {
    value: value.to_string(),
  })
}

fn invalid_value() -> ServiceWorkerAllowedParseError {
  ServiceWorkerAllowedParseError::new("invalid Service-Worker-Allowed value")
}

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn is_service_worker_allowed_path_text(value: &str) -> bool {
  if value.is_empty() || !is_safe_field_value(value) || !has_valid_percent_escapes(value) {
    return false;
  }
  if value.starts_with("//") || has_scheme_prefix(value) {
    return false;
  }

  let path_end = value.find(['?', '#']).unwrap_or(value.len());
  if path_end == 0 {
    return false;
  }

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
        } else if !is_path_char(byte) {
          return false;
        }
      }
    }
  }

  true
}

fn is_safe_field_value(value: &str) -> bool {
  value.bytes().all(|byte| {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'<' && byte != b'>' && byte != b'\\'
  })
}

fn has_scheme_prefix(value: &str) -> bool {
  let first_segment_end = value.find(['/', '?', '#']).unwrap_or(value.len());
  value[..first_segment_end].contains(':')
}

fn has_valid_percent_escapes(value: &str) -> bool {
  let bytes = value.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] != b'%' {
      index += 1;
      continue;
    }
    if index + 2 >= bytes.len()
      || !bytes[index + 1].is_ascii_hexdigit()
      || !bytes[index + 2].is_ascii_hexdigit()
    {
      return false;
    }
    index += 3;
  }
  true
}

fn is_path_char(byte: u8) -> bool {
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
