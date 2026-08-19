//! Bounded, policy-free `Transfer-Encoding` framing metadata parsing.
//!
//! This module validates one or more RFC 9112 `Transfer-Encoding` field values
//! as an ordered list of transfer-coding tokens. Combined fields must yield a
//! sole `chunked` coding as the last (and only) token, matching existing
//! HTTP/1 framing. Callers decide whether and how to interpret the coding.
//! Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

pub const MAX_TRANSFER_ENCODING_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_TRANSFER_ENCODING_CODINGS: usize = 256;

/// Parsed, bounded `Transfer-Encoding` framing metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEncoding {
  codings: Vec<String>,
}

impl TransferEncoding {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TransferEncodingParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TransferEncodingParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut codings: Vec<String> = Vec::new();

    for value in values {
      if value.len() > MAX_TRANSFER_ENCODING_VALUE_BYTES {
        return Err(TransferEncodingParseError::new(
          "Transfer-Encoding header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(TransferEncodingParseError::new(
          "invalid Transfer-Encoding control byte",
        ));
      }
      for member in value.split(',') {
        let coding = member.trim_matches([' ', '\t']);
        if coding.is_empty() || !is_http_token(coding) {
          return Err(TransferEncodingParseError::new(
            "invalid Transfer-Encoding coding",
          ));
        }
        if codings.len() >= MAX_TRANSFER_ENCODING_CODINGS {
          return Err(TransferEncodingParseError::new(
            "too many Transfer-Encoding codings",
          ));
        }
        codings.push(coding.to_owned());
      }
    }

    if codings.is_empty() {
      return Err(TransferEncodingParseError::new(
        "invalid Transfer-Encoding coding",
      ));
    }
    if !is_sole_chunked(&codings) {
      return Err(TransferEncodingParseError::new(
        "unsupported Transfer-Encoding",
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
pub struct TransferEncodingParseError {
  message: String,
}

impl TransferEncodingParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TransferEncodingParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TransferEncodingParseError {}

fn is_sole_chunked(codings: &[String]) -> bool {
  codings.len() == 1 && codings[0].eq_ignore_ascii_case("chunked")
}

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
