//! Bounded, policy-free `Content-Encoding` response metadata parsing.
//!
//! This module validates one or more RFC 9110 `Content-Encoding` field values
//! as an ordered list of content-coding tokens. Callers decide whether and how
//! to decode or negotiate content codings. Unparsable input is an error; this
//! parser never fails open.

use std::error::Error;
use std::fmt;

pub const MAX_CONTENT_ENCODING_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONTENT_ENCODING_CODINGS: usize = 256;

/// Parsed, bounded `Content-Encoding` representation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEncoding {
  codings: Vec<String>,
}

impl ContentEncoding {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentEncodingParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentEncodingParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut codings: Vec<String> = Vec::new();

    for value in values {
      if value.len() > MAX_CONTENT_ENCODING_VALUE_BYTES {
        return Err(ContentEncodingParseError::new(
          "Content-Encoding header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(ContentEncodingParseError::new(
          "invalid Content-Encoding control byte",
        ));
      }
      for member in value.split(',') {
        let coding = member.trim_matches([' ', '\t']);
        if coding.is_empty() || !is_http_token(coding) {
          return Err(ContentEncodingParseError::new(
            "invalid Content-Encoding coding",
          ));
        }
        if codings.len() >= MAX_CONTENT_ENCODING_CODINGS {
          return Err(ContentEncodingParseError::new(
            "too many Content-Encoding codings",
          ));
        }
        codings.push(coding.to_owned());
      }
    }

    if codings.is_empty() {
      return Err(ContentEncodingParseError::new(
        "invalid Content-Encoding coding",
      ));
    }

    Ok(Self { codings })
  }

  pub fn codings(&self) -> Vec<&str> {
    self.codings.iter().map(String::as_str).collect()
  }

  pub fn len(&self) -> usize {
    self.codings.len()
  }

  pub fn is_empty(&self) -> bool {
    self.codings.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.codings.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEncodingParseError {
  message: String,
}

impl ContentEncodingParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentEncodingParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentEncodingParseError {}

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
