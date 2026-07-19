//! Bounded, policy-free `Access-Control-Expose-Headers` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply CORS exposure behavior.

use std::error::Error;
use std::fmt;

pub const MAX_ACCESS_CONTROL_EXPOSE_HEADERS_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCESS_CONTROL_EXPOSE_HEADERS_FIELD_NAMES: usize = 256;

/// Parsed, bounded `Access-Control-Expose-Headers` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlExposeHeaders {
  wildcard: bool,
  field_names: Vec<String>,
}

impl AccessControlExposeHeaders {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AccessControlExposeHeadersParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AccessControlExposeHeadersParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut wildcard = false;
    let mut field_names = Vec::new();

    for value in values {
      if value.len() > MAX_ACCESS_CONTROL_EXPOSE_HEADERS_VALUE_BYTES {
        return Err(AccessControlExposeHeadersParseError::new(
          "Access-Control-Expose-Headers header value is too large",
        ));
      }

      for field_name in value.split(',') {
        let field_name = field_name.trim();
        if field_name.is_empty() {
          return Err(AccessControlExposeHeadersParseError::new(
            "invalid Access-Control-Expose-Headers field name",
          ));
        }
        if field_name == "*" {
          if wildcard || !field_names.is_empty() {
            return Err(AccessControlExposeHeadersParseError::new(
              "Access-Control-Expose-Headers wildcard cannot be combined with field names",
            ));
          }
          wildcard = true;
          continue;
        }
        if wildcard {
          return Err(AccessControlExposeHeadersParseError::new(
            "Access-Control-Expose-Headers wildcard cannot be combined with field names",
          ));
        }
        if !is_http_token(field_name) {
          return Err(AccessControlExposeHeadersParseError::new(
            "invalid Access-Control-Expose-Headers field name",
          ));
        }
        if field_names.len() >= MAX_ACCESS_CONTROL_EXPOSE_HEADERS_FIELD_NAMES {
          return Err(AccessControlExposeHeadersParseError::new(
            "too many Access-Control-Expose-Headers field names",
          ));
        }

        let normalized = field_name.to_ascii_lowercase();
        if field_names.contains(&normalized) {
          return Err(AccessControlExposeHeadersParseError::new(
            "duplicate Access-Control-Expose-Headers field name",
          ));
        }
        field_names.push(normalized);
      }
    }

    if wildcard {
      return Ok(Self {
        wildcard,
        field_names,
      });
    }
    if field_names.is_empty() {
      return Err(AccessControlExposeHeadersParseError::new(
        "invalid Access-Control-Expose-Headers field name",
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessControlExposeHeadersParseError {
  message: String,
}

impl AccessControlExposeHeadersParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AccessControlExposeHeadersParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AccessControlExposeHeadersParseError {}

fn is_http_token(value: &str) -> bool {
  !value.is_empty()
    && value.bytes().all(|byte| {
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
    })
}
