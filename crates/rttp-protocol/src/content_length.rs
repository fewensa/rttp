//! Bounded, policy-free HTTP `Content-Length` framing metadata parsing.
//!
//! This module validates field-value syntax and the framing requirement that
//! every `Content-Length` member has the same decimal value. Callers decide
//! whether and how to apply that framing metadata.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Content-Length` field value.
pub const MAX_CONTENT_LENGTH_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across supplied fields.
pub const MAX_CONTENT_LENGTH_TOTAL_BYTES: usize = 64 * 1024;

/// Read-only HTTP `Content-Length` framing metadata.
///
/// This value carries validated message framing state. It does not decide body
/// framing policy.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HttpContentLength {
  len: usize,
}

impl HttpContentLength {
  pub fn new(len: usize) -> Self {
    Self { len }
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpContentLengthParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpContentLengthParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parsed = None;
    let mut total_bytes = 0usize;
    let mut seen_field = false;

    for value in values {
      seen_field = true;
      validate_field(value)?;
      total_bytes = total_bytes.checked_add(value.len()).ok_or_else(|| {
        HttpContentLengthParseError::new("Content-Length header list is too large")
      })?;
      if total_bytes > MAX_CONTENT_LENGTH_TOTAL_BYTES {
        return Err(HttpContentLengthParseError::new(
          "Content-Length header list is too large",
        ));
      }

      for member in value.split(',') {
        let member = member.trim_matches([' ', '\t']);
        if member.is_empty() || !member.bytes().all(|byte| byte.is_ascii_digit()) {
          return Err(HttpContentLengthParseError::new(
            "invalid Content-Length header value",
          ));
        }
        let number = parse_decimal(member)?;
        if let Some(expected) = parsed {
          if expected != number {
            return Err(HttpContentLengthParseError::new(
              "mismatched Content-Length values",
            ));
          }
        } else {
          parsed = Some(number);
        }
      }
    }

    if !seen_field {
      return Err(HttpContentLengthParseError::new(
        "invalid Content-Length header value",
      ));
    }

    parsed
      .map(Self::new)
      .ok_or_else(|| HttpContentLengthParseError::new("invalid Content-Length header value"))
  }

  pub fn len(&self) -> usize {
    self.len
  }

  pub fn is_zero(&self) -> bool {
    self.len == 0
  }

  pub fn is_empty(&self) -> bool {
    self.is_zero()
  }

  pub fn header_value(&self) -> String {
    self.len.to_string()
  }
}

/// An error returned when `Content-Length` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpContentLengthParseError {
  message: String,
}

impl HttpContentLengthParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpContentLengthParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpContentLengthParseError {}

fn validate_field(value: &str) -> Result<(), HttpContentLengthParseError> {
  if value.len() > MAX_CONTENT_LENGTH_VALUE_BYTES {
    return Err(HttpContentLengthParseError::new(
      "Content-Length header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(HttpContentLengthParseError::new(
      "invalid Content-Length control byte",
    ));
  }
  Ok(())
}

fn parse_decimal(value: &str) -> Result<usize, HttpContentLengthParseError> {
  value
    .bytes()
    .try_fold(0usize, |number, digit| {
      number
        .checked_mul(10)
        .and_then(|number| number.checked_add(usize::from(digit - b'0')))
    })
    .ok_or_else(|| HttpContentLengthParseError::new("Content-Length value overflows usize"))
}
