//! Bounded, policy-free `Sec-WebSocket-Protocol` metadata parsing.
//!
//! This module validates RFC 6455 protocol tokens as request or response
//! metadata only. Request offers are an ordered `1#token` list in client
//! preference order; a successful handshake selection is a singleton token.
//! Members follow the RFC 6455 section 11.3.4 `token` production and compare
//! case-sensitively, so `chat` and `Chat` are distinct.
//!
//! The parser reports declared metadata only. It does not perform a WebSocket
//! handshake, emit `Connection: Upgrade`, choose an application subprotocol,
//! or implement WebSocket frames.

use crate::http1::is_token;
use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Sec-WebSocket-Protocol` field value, and in
/// the combined raw or canonical serialized field set.
pub const MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES: usize = 64 * 1024;

/// Maximum protocol tokens accepted across all combined
/// `Sec-WebSocket-Protocol` fields.
pub const MAX_SEC_WEBSOCKET_PROTOCOL_MEMBERS: usize = 32;

/// Parsed, bounded `Sec-WebSocket-Protocol` request or response metadata.
///
/// Offer metadata stores one or more RFC 6455 `token` members in wire or
/// preference order. Selection metadata stores exactly one token; a one-token
/// offer equals a selection of that token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketProtocol {
  protocols: Vec<String>,
}

/// An error returned when `Sec-WebSocket-Protocol` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketProtocolParseError {
  message: String,
}

impl SecWebSocketProtocol {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecWebSocketProtocolParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecWebSocketProtocolParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut protocols = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_bounded_value(value)?;
      total_bytes += value.len();
      if total_bytes > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES {
        return Err(SecWebSocketProtocolParseError::new(
          "combined Sec-WebSocket-Protocol header values are too large",
        ));
      }
      for member in value.split(',') {
        push_protocol(member.trim_matches([' ', '\t']), &mut protocols)?;
      }
    }
    finish_protocols(protocols)
  }

  /// Builds `Sec-WebSocket-Protocol` offer metadata from declared protocol
  /// tokens in preference order.
  pub fn from_protocols<I, S>(protocols: I) -> Result<Self, SecWebSocketProtocolParseError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let mut parsed = Vec::new();
    let mut total_bytes = 0usize;
    for protocol in protocols {
      if !parsed.is_empty() {
        total_bytes += 2;
      }
      let token = parse_protocol_token(protocol.as_ref())?;
      total_bytes += token.len();
      if total_bytes > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES {
        return Err(SecWebSocketProtocolParseError::new(
          "Sec-WebSocket-Protocol header value is too large",
        ));
      }
      push_parsed_protocol(token, &mut parsed)?;
    }
    finish_protocols(parsed)
  }

  /// Builds `Sec-WebSocket-Protocol` selection metadata from exactly one
  /// protocol token. The whole value is one token; no comma splitting is
  /// applied.
  pub fn from_selection(token: impl AsRef<str>) -> Result<Self, SecWebSocketProtocolParseError> {
    let token = token.as_ref();
    if token.len() > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES {
      return Err(SecWebSocketProtocolParseError::new(
        "Sec-WebSocket-Protocol header value is too large",
      ));
    }
    let token = parse_protocol_token(token)?;
    finish_selection(vec![token])
  }

  /// Parses a `Sec-WebSocket-Protocol` selection from one raw field,
  /// requiring exactly one token across the combined members.
  pub fn parse_selection(value: impl AsRef<str>) -> Result<Self, SecWebSocketProtocolParseError> {
    Self::parse_selection_values([value.as_ref()])
  }

  /// Parses a `Sec-WebSocket-Protocol` selection from raw fields, requiring
  /// exactly one token across all combined members.
  pub fn parse_selection_values<'a, I>(values: I) -> Result<Self, SecWebSocketProtocolParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    finish_selection(Self::parse_values(values)?.protocols)
  }

  /// Returns the declared protocol tokens in wire or preference order.
  pub fn protocols(&self) -> &[String] {
    &self.protocols
  }

  /// Whether a protocol token is declared, matched case-sensitively.
  pub fn contains(&self, protocol: impl AsRef<str>) -> bool {
    self
      .protocols
      .iter()
      .any(|known| known == protocol.as_ref())
  }

  /// The singleton selected token, when this is selection metadata.
  pub fn selected(&self) -> Option<&str> {
    match self.protocols.as_slice() {
      [token] => Some(token),
      _ => None,
    }
  }

  pub fn header_value(&self) -> String {
    self.protocols.join(", ")
  }
}

impl SecWebSocketProtocolParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecWebSocketProtocolParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecWebSocketProtocolParseError {}

fn validate_bounded_value(value: &str) -> Result<(), SecWebSocketProtocolParseError> {
  if value.len() > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES {
    return Err(SecWebSocketProtocolParseError::new(
      "Sec-WebSocket-Protocol header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(SecWebSocketProtocolParseError::new(
      "invalid Sec-WebSocket-Protocol control byte",
    ));
  }
  Ok(())
}

fn push_protocol(
  token: &str,
  protocols: &mut Vec<String>,
) -> Result<(), SecWebSocketProtocolParseError> {
  push_parsed_protocol(parse_protocol_token(token)?, protocols)
}

fn push_parsed_protocol(
  token: String,
  protocols: &mut Vec<String>,
) -> Result<(), SecWebSocketProtocolParseError> {
  if protocols.len() >= MAX_SEC_WEBSOCKET_PROTOCOL_MEMBERS {
    return Err(SecWebSocketProtocolParseError::new(
      "too many Sec-WebSocket-Protocol protocols",
    ));
  }
  if protocols.iter().any(|known| known == &token) {
    return Err(SecWebSocketProtocolParseError::new(
      "duplicate Sec-WebSocket-Protocol protocol",
    ));
  }
  protocols.push(token);
  Ok(())
}

fn finish_protocols(
  protocols: Vec<String>,
) -> Result<SecWebSocketProtocol, SecWebSocketProtocolParseError> {
  if protocols.is_empty() {
    return Err(invalid_protocol());
  }
  let parsed = SecWebSocketProtocol { protocols };
  if parsed.header_value().len() > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES {
    return Err(SecWebSocketProtocolParseError::new(
      "combined Sec-WebSocket-Protocol header values are too large",
    ));
  }
  Ok(parsed)
}

fn finish_selection(
  protocols: Vec<String>,
) -> Result<SecWebSocketProtocol, SecWebSocketProtocolParseError> {
  if protocols.len() != 1 {
    return Err(SecWebSocketProtocolParseError::new(
      "Sec-WebSocket-Protocol selection must be exactly one token",
    ));
  }
  finish_protocols(protocols)
}

fn parse_protocol_token(token: &str) -> Result<String, SecWebSocketProtocolParseError> {
  if !is_token(token) {
    return Err(invalid_protocol());
  }
  Ok(token.to_string())
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_protocol() -> SecWebSocketProtocolParseError {
  SecWebSocketProtocolParseError::new("invalid Sec-WebSocket-Protocol protocol")
}
