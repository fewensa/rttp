//! Bounded, policy-free `Connection` header metadata parsing.
//!
//! This module validates one or more RFC 9110 `Connection` field values as an
//! ordered list of connection-option tokens. It is header-field syntax only
//! and is not a transport socket type. Callers decide whether and how to
//! interpret tokens such as `close` or `keep-alive`. Unparsable input is an
//! error; this parser never fails open.

use std::error::Error;
use std::fmt;

pub const MAX_CONNECTION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONNECTION_TOKENS: usize = 256;

/// Parsed, bounded `Connection` header metadata.
///
/// This type stores RFC 9110 connection-option tokens. It is not a socket or
/// keep-alive controller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Connection {
  tokens: Vec<String>,
}

impl Connection {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ConnectionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ConnectionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tokens: Vec<String> = Vec::new();

    for value in values {
      if value.len() > MAX_CONNECTION_VALUE_BYTES {
        return Err(ConnectionParseError::new(
          "Connection header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(ConnectionParseError::new("invalid Connection control byte"));
      }
      for member in value.split(',') {
        let token = member.trim_matches([' ', '\t']);
        if token.is_empty() || !is_http_token(token) {
          return Err(ConnectionParseError::new("invalid Connection token"));
        }
        if tokens.len() >= MAX_CONNECTION_TOKENS {
          return Err(ConnectionParseError::new("too many Connection tokens"));
        }
        tokens.push(token.to_owned());
      }
    }

    if tokens.is_empty() {
      return Err(ConnectionParseError::new("invalid Connection token"));
    }

    Ok(Self { tokens })
  }

  pub fn tokens(&self) -> Vec<&str> {
    self.tokens.iter().map(String::as_str).collect()
  }

  pub fn len(&self) -> usize {
    self.tokens.len()
  }

  pub fn is_empty(&self) -> bool {
    self.tokens.is_empty()
  }

  pub fn contains(&self, token: impl AsRef<str>) -> bool {
    self
      .tokens
      .iter()
      .any(|candidate| candidate.eq_ignore_ascii_case(token.as_ref()))
  }

  pub fn header_value(&self) -> String {
    self.tokens.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionParseError {
  message: String,
}

impl ConnectionParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ConnectionParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ConnectionParseError {}

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
