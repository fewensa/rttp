//! Bounded, policy-free `Sec-WebSocket-Version` metadata parsing.
//!
//! This module validates one or more RFC 6455 version tokens as request or
//! response metadata only. Version members follow the RFC 6455 section 4.3
//! `version` production (`DIGIT / (NZDIGIT DIGIT) / ("1" DIGIT DIGIT) /
//! ("2" DIGIT DIGIT)`): canonical decimal `0` through `299` without leading
//! zeros. Multi-member lists must appear in numeric descending order, matching
//! the common rejection response shape `13, 8, 7`.
//!
//! The parser reports declared metadata only. It does not perform a WebSocket
//! handshake, emit `Connection: Upgrade`, compute `Sec-WebSocket-Accept`,
//! negotiate versions, switch protocols, or implement WebSocket frames.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Sec-WebSocket-Version` field value, and in
/// the combined raw or canonical serialized field set.
pub const MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES: usize = 64 * 1024;

/// Maximum version tokens accepted across all combined `Sec-WebSocket-Version`
/// fields.
pub const MAX_SEC_WEBSOCKET_VERSION_MEMBERS: usize = 32;

/// Parsed, bounded `Sec-WebSocket-Version` request or response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketVersion {
  versions: Vec<String>,
}

/// An error returned when `Sec-WebSocket-Version` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketVersionParseError {
  message: String,
}

impl SecWebSocketVersion {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecWebSocketVersionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecWebSocketVersionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut versions = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_bounded_value(value)?;
      total_bytes += value.len();
      if total_bytes > MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES {
        return Err(SecWebSocketVersionParseError::new(
          "combined Sec-WebSocket-Version header values are too large",
        ));
      }
      for member in value.split(',') {
        push_version(member.trim_matches([' ', '\t']), &mut versions)?;
      }
    }
    finish_versions(versions)
  }

  /// Builds `Sec-WebSocket-Version` metadata from declared version tokens.
  pub fn from_versions<I, S>(versions: I) -> Result<Self, SecWebSocketVersionParseError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let mut parsed = Vec::new();
    let mut total_bytes = 0usize;
    for version in versions {
      if !parsed.is_empty() {
        total_bytes += 2;
      }
      let token = parse_version_token(version.as_ref())?;
      total_bytes += token.len();
      if total_bytes > MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES {
        return Err(SecWebSocketVersionParseError::new(
          "Sec-WebSocket-Version header value is too large",
        ));
      }
      push_parsed_version(token, &mut parsed)?;
    }
    finish_versions(parsed)
  }

  /// Returns the declared version tokens in canonical descending order.
  pub fn versions(&self) -> &[String] {
    &self.versions
  }

  /// Whether a canonical decimal version token is declared.
  pub fn contains(&self, version: impl AsRef<str>) -> bool {
    self.versions.iter().any(|known| known == version.as_ref())
  }

  pub fn header_value(&self) -> String {
    self.versions.join(", ")
  }
}

impl SecWebSocketVersionParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecWebSocketVersionParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecWebSocketVersionParseError {}

fn validate_bounded_value(value: &str) -> Result<(), SecWebSocketVersionParseError> {
  if value.len() > MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES {
    return Err(SecWebSocketVersionParseError::new(
      "Sec-WebSocket-Version header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| !matches!(byte, b' ' | b'\t') && !is_visible_byte(byte))
  {
    return Err(SecWebSocketVersionParseError::new(
      "invalid Sec-WebSocket-Version control byte",
    ));
  }
  Ok(())
}

fn push_version(
  token: &str,
  versions: &mut Vec<String>,
) -> Result<(), SecWebSocketVersionParseError> {
  push_parsed_version(parse_version_token(token)?, versions)
}

fn push_parsed_version(
  token: String,
  versions: &mut Vec<String>,
) -> Result<(), SecWebSocketVersionParseError> {
  if versions.len() >= MAX_SEC_WEBSOCKET_VERSION_MEMBERS {
    return Err(SecWebSocketVersionParseError::new(
      "too many Sec-WebSocket-Version versions",
    ));
  }
  if versions.iter().any(|known| known == &token) {
    return Err(SecWebSocketVersionParseError::new(
      "duplicate Sec-WebSocket-Version version",
    ));
  }
  versions.push(token);
  Ok(())
}

fn finish_versions(
  versions: Vec<String>,
) -> Result<SecWebSocketVersion, SecWebSocketVersionParseError> {
  if versions.is_empty() {
    return Err(invalid_version());
  }
  if !is_numeric_descending(&versions) {
    return Err(SecWebSocketVersionParseError::new(
      "Sec-WebSocket-Version versions are not in canonical descending order",
    ));
  }
  let parsed = SecWebSocketVersion { versions };
  if parsed.header_value().len() > MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES {
    return Err(SecWebSocketVersionParseError::new(
      "combined Sec-WebSocket-Version header values are too large",
    ));
  }
  Ok(parsed)
}

fn parse_version_token(token: &str) -> Result<String, SecWebSocketVersionParseError> {
  if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_version());
  }
  if token.len() > 1 && token.starts_with('0') {
    return Err(invalid_version());
  }
  match token.as_bytes() {
    [b'0'..=b'9'] | [b'1'..=b'9', b'0'..=b'9'] | [b'1' | b'2', b'0'..=b'9', b'0'..=b'9'] => {
      Ok(token.to_string())
    }
    _ => Err(invalid_version()),
  }
}

fn is_numeric_descending(versions: &[String]) -> bool {
  versions.windows(2).all(|pair| {
    version_number(&pair[0])
      .zip(version_number(&pair[1]))
      .is_some_and(|(left, right)| left > right)
  })
}

fn version_number(token: &str) -> Option<u16> {
  token.parse().ok()
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn invalid_version() -> SecWebSocketVersionParseError {
  SecWebSocketVersionParseError::new("invalid Sec-WebSocket-Version version")
}
