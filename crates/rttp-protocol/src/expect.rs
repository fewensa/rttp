//! Bounded, policy-free `Expect` request metadata parsing and formatting.
//!
//! This module validates one or more HTTP `Expect` field values as an ordered
//! list of expectation tokens. The standardized `100-continue` expectation is
//! exposed separately from unsupported extension names. Callers decide whether
//! to wait for an interim response, send `100 Continue`, or reject unsupported
//! extensions. Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

/// Maximum bytes accepted in one `Expect` field value.
pub const MAX_EXPECT_VALUE_BYTES: usize = 64 * 1024;
/// Maximum combined expectation count across all supplied fields.
pub const MAX_EXPECTATIONS: usize = 32;

/// Parsed, bounded `Expect` request metadata.
///
/// The standardized `100-continue` expectation is exposed separately from
/// unsupported extension expectations so callers can make their own policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expect {
  expects_continue: bool,
  unsupported: Vec<String>,
}

impl Expect {
  /// Construct the standardized `100-continue` singleton.
  pub fn expect_continue() -> Self {
    Self {
      expects_continue: true,
      unsupported: Vec::new(),
    }
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, ExpectParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ExpectParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut expects_continue = false;
    let mut unsupported = Vec::new();
    let mut seen = Vec::<String>::new();

    for value in values {
      if value.len() > MAX_EXPECT_VALUE_BYTES {
        return Err(ExpectParseError::new("Expect header value is too large"));
      }
      for member in value.split(',') {
        let expectation = member.trim();
        let name = expectation
          .split(['=', ';'])
          .next()
          .unwrap_or_default()
          .trim();
        if !is_token(name) {
          return Err(ExpectParseError::new("invalid Expect expectation"));
        }
        if seen.iter().any(|known| known.eq_ignore_ascii_case(name)) {
          return Err(ExpectParseError::new("duplicate Expect expectation"));
        }
        if seen.len() >= MAX_EXPECTATIONS {
          return Err(ExpectParseError::new("too many Expect expectations"));
        }
        seen.push(name.to_string());
        if name.eq_ignore_ascii_case("100-continue") {
          expects_continue = true;
        } else {
          unsupported.push(name.to_string());
        }
      }
    }

    if seen.is_empty() {
      return Err(ExpectParseError::new("invalid Expect expectation"));
    }
    Ok(Self {
      expects_continue,
      unsupported,
    })
  }

  pub fn expects_continue(&self) -> bool {
    self.expects_continue
  }

  pub fn unsupported(&self) -> &[String] {
    &self.unsupported
  }

  pub fn header_value(&self) -> String {
    let mut parts = Vec::with_capacity(self.unsupported.len() + usize::from(self.expects_continue));
    if self.expects_continue {
      parts.push("100-continue");
    }
    parts.extend(self.unsupported.iter().map(String::as_str));
    parts.join(", ")
  }
}

/// An error returned when `Expect` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectParseError {
  message: String,
}

impl ExpectParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ExpectParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ExpectParseError {}
