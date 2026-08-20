//! Bounded, policy-free `Sec-WebSocket-Extensions` metadata parsing.
//!
//! This module validates RFC 6455 extension-list syntax as request or
//! response metadata only. Request offers are an ordered list of extension
//! tokens with ordered parameters. Response selections use the same grammar
//! but must select exactly one extension member. Duplicate extension tokens
//! and duplicate parameter names within one extension are rejected.
//!
//! The parser reports declared metadata only. It does not activate
//! compression, negotiate extensions, emit `Connection: Upgrade`, switch
//! protocols, or implement WebSocket frames.

use crate::http1::{is_qdtext, is_quoted_pair_char, is_token};
use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Sec-WebSocket-Extensions` field value, and
/// in the combined raw or canonical serialized field set.
pub const MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES: usize = 64 * 1024;

/// Maximum extension members accepted across all combined
/// `Sec-WebSocket-Extensions` fields.
pub const MAX_SEC_WEBSOCKET_EXTENSIONS_MEMBERS: usize = 32;

/// Maximum parameters accepted on one extension member.
pub const MAX_SEC_WEBSOCKET_EXTENSION_PARAMETERS: usize = 32;

/// Parsed, bounded `Sec-WebSocket-Extensions` request or response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketExtensions {
  extensions: Vec<SecWebSocketExtension>,
}

/// One `Sec-WebSocket-Extensions` extension member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketExtension {
  token: String,
  parameters: Vec<SecWebSocketExtensionParameter>,
}

/// One ordered extension parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketExtensionParameter {
  name: String,
  value: Option<SecWebSocketExtensionParameterValue>,
}

/// An extension parameter value, preserving whether it was quoted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecWebSocketExtensionParameterValue {
  Token(String),
  Quoted(String),
}

/// An error returned when `Sec-WebSocket-Extensions` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecWebSocketExtensionsParseError {
  message: String,
}

impl SecWebSocketExtensions {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecWebSocketExtensionsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecWebSocketExtensionsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut extensions = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_bounded_value(value)?;
      total_bytes += value.len();
      if total_bytes > MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES {
        return Err(SecWebSocketExtensionsParseError::new(
          "combined Sec-WebSocket-Extensions header values are too large",
        ));
      }
      for member in split_quoted(value, b',')? {
        push_extension(
          parse_extension(member.trim_matches([' ', '\t']))?,
          &mut extensions,
        )?;
      }
    }
    finish_extensions(extensions)
  }

  /// Parses a `Sec-WebSocket-Extensions` response selection from one raw
  /// field, requiring exactly one extension across the combined members.
  pub fn parse_selection(value: impl AsRef<str>) -> Result<Self, SecWebSocketExtensionsParseError> {
    Self::parse_selection_values([value.as_ref()])
  }

  /// Parses a `Sec-WebSocket-Extensions` response selection from raw fields,
  /// requiring exactly one extension across all combined members.
  pub fn parse_selection_values<'a, I>(values: I) -> Result<Self, SecWebSocketExtensionsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    finish_selection(Self::parse_values(values)?.extensions)
  }

  /// Returns the declared extension members in wire or preference order.
  pub fn extensions(&self) -> &[SecWebSocketExtension] {
    &self.extensions
  }

  /// Whether an extension token is declared, matched case-sensitively.
  pub fn contains(&self, token: impl AsRef<str>) -> bool {
    self
      .extensions
      .iter()
      .any(|extension| extension.token == token.as_ref())
  }

  /// The singleton selected extension, when this is selection metadata.
  pub fn selected(&self) -> Option<&SecWebSocketExtension> {
    match self.extensions.as_slice() {
      [extension] => Some(extension),
      _ => None,
    }
  }

  pub fn header_value(&self) -> String {
    self
      .extensions
      .iter()
      .map(SecWebSocketExtension::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SecWebSocketExtension {
  pub fn token(&self) -> &str {
    &self.token
  }

  pub fn parameters(&self) -> &[SecWebSocketExtensionParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&SecWebSocketExtensionParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  pub fn header_value(&self) -> String {
    let mut value = self.token.clone();
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(&parameter.header_value());
    }
    value
  }
}

impl SecWebSocketExtensionParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&SecWebSocketExtensionParameterValue> {
    self.value.as_ref()
  }

  pub fn header_value(&self) -> String {
    match &self.value {
      None => self.name.clone(),
      Some(SecWebSocketExtensionParameterValue::Token(value)) => {
        format!("{}={}", self.name, value)
      }
      Some(SecWebSocketExtensionParameterValue::Quoted(value)) => {
        format!("{}=\"{}\"", self.name, quote_value(value))
      }
    }
  }
}

impl SecWebSocketExtensionParameterValue {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Token(value) | Self::Quoted(value) => value,
    }
  }

  pub fn is_quoted(&self) -> bool {
    matches!(self, Self::Quoted(_))
  }
}

impl SecWebSocketExtensionsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecWebSocketExtensionsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecWebSocketExtensionsParseError {}

fn validate_bounded_value(value: &str) -> Result<(), SecWebSocketExtensionsParseError> {
  if value.len() > MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES {
    return Err(SecWebSocketExtensionsParseError::new(
      "Sec-WebSocket-Extensions header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte > 0x7e || (byte < 0x20 && byte != b'\t'))
  {
    return Err(SecWebSocketExtensionsParseError::new(
      "invalid Sec-WebSocket-Extensions byte",
    ));
  }
  Ok(())
}

fn split_quoted(value: &str, separator: u8) -> Result<Vec<&str>, SecWebSocketExtensionsParseError> {
  let mut parts = Vec::new();
  let mut start = 0usize;
  let mut quoted = false;
  let mut escaped = false;
  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' => quoted = !quoted,
      byte if byte == separator && !quoted => {
        parts.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped {
    return Err(invalid_extensions());
  }
  parts.push(&value[start..]);
  Ok(parts)
}

fn parse_extension(value: &str) -> Result<SecWebSocketExtension, SecWebSocketExtensionsParseError> {
  let mut parts = split_quoted(value, b';')?.into_iter();
  let token = parts
    .next()
    .ok_or_else(invalid_extensions)?
    .trim_matches([' ', '\t']);
  if !is_token(token) {
    return Err(invalid_extension_token());
  }
  let mut parameters = Vec::new();
  for parameter in parts {
    push_parameter(
      parse_parameter(parameter.trim_matches([' ', '\t']))?,
      &mut parameters,
    )?;
  }
  Ok(SecWebSocketExtension {
    token: token.to_string(),
    parameters,
  })
}

fn parse_parameter(
  value: &str,
) -> Result<SecWebSocketExtensionParameter, SecWebSocketExtensionsParseError> {
  let mut parts = split_quoted(value, b'=')?;
  if parts.len() > 2 {
    return Err(invalid_parameter());
  }
  let name = parts.remove(0).trim_matches([' ', '\t']);
  if !is_token(name) {
    return Err(invalid_parameter());
  }
  let value = match parts.pop() {
    None => None,
    Some(raw) => {
      let raw = raw.trim_matches([' ', '\t']);
      if raw.starts_with('"') || raw.ends_with('"') {
        Some(SecWebSocketExtensionParameterValue::Quoted(
          parse_quoted_value(raw)?,
        ))
      } else {
        if !is_token(raw) {
          return Err(invalid_parameter());
        }
        Some(SecWebSocketExtensionParameterValue::Token(raw.to_string()))
      }
    }
  };
  Ok(SecWebSocketExtensionParameter {
    name: name.to_string(),
    value,
  })
}

fn parse_quoted_value(value: &str) -> Result<String, SecWebSocketExtensionsParseError> {
  let bytes = value.as_bytes();
  if bytes.len() < 2 || bytes.first() != Some(&b'"') || bytes.last() != Some(&b'"') {
    return Err(invalid_parameter());
  }
  let mut parsed = String::new();
  let mut index = 1usize;
  while index + 1 < bytes.len() {
    match bytes[index] {
      b'\\' => {
        index += 1;
        if index + 1 >= bytes.len() || !is_quoted_pair_char(bytes[index]) || bytes[index] > 0x7e {
          return Err(invalid_parameter());
        }
        parsed.push(char::from(bytes[index]));
      }
      b'"' => return Err(invalid_parameter()),
      byte => {
        if !is_qdtext(byte) || byte > 0x7e {
          return Err(invalid_parameter());
        }
        parsed.push(char::from(byte));
      }
    }
    index += 1;
  }
  Ok(parsed)
}

fn push_extension(
  extension: SecWebSocketExtension,
  extensions: &mut Vec<SecWebSocketExtension>,
) -> Result<(), SecWebSocketExtensionsParseError> {
  if extensions.len() >= MAX_SEC_WEBSOCKET_EXTENSIONS_MEMBERS {
    return Err(SecWebSocketExtensionsParseError::new(
      "too many Sec-WebSocket-Extensions extensions",
    ));
  }
  if extensions
    .iter()
    .any(|known| known.token == extension.token)
  {
    return Err(SecWebSocketExtensionsParseError::new(
      "duplicate Sec-WebSocket-Extensions extension",
    ));
  }
  extensions.push(extension);
  Ok(())
}

fn push_parameter(
  parameter: SecWebSocketExtensionParameter,
  parameters: &mut Vec<SecWebSocketExtensionParameter>,
) -> Result<(), SecWebSocketExtensionsParseError> {
  if parameters.len() >= MAX_SEC_WEBSOCKET_EXTENSION_PARAMETERS {
    return Err(SecWebSocketExtensionsParseError::new(
      "too many Sec-WebSocket-Extensions parameters",
    ));
  }
  if parameters.iter().any(|known| known.name == parameter.name) {
    return Err(SecWebSocketExtensionsParseError::new(
      "duplicate Sec-WebSocket-Extensions parameter",
    ));
  }
  parameters.push(parameter);
  Ok(())
}

fn finish_extensions(
  extensions: Vec<SecWebSocketExtension>,
) -> Result<SecWebSocketExtensions, SecWebSocketExtensionsParseError> {
  if extensions.is_empty() {
    return Err(invalid_extensions());
  }
  let parsed = SecWebSocketExtensions { extensions };
  if parsed.header_value().len() > MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES {
    return Err(SecWebSocketExtensionsParseError::new(
      "combined Sec-WebSocket-Extensions header values are too large",
    ));
  }
  Ok(parsed)
}

fn finish_selection(
  extensions: Vec<SecWebSocketExtension>,
) -> Result<SecWebSocketExtensions, SecWebSocketExtensionsParseError> {
  if extensions.len() != 1 {
    return Err(SecWebSocketExtensionsParseError::new(
      "Sec-WebSocket-Extensions selection must be exactly one extension",
    ));
  }
  finish_extensions(extensions)
}

fn quote_value(value: &str) -> String {
  let mut quoted = String::new();
  for byte in value.bytes() {
    if matches!(byte, b'"' | b'\\') {
      quoted.push('\\');
    }
    quoted.push(char::from(byte));
  }
  quoted
}

fn invalid_extensions() -> SecWebSocketExtensionsParseError {
  SecWebSocketExtensionsParseError::new("invalid Sec-WebSocket-Extensions extensions")
}

fn invalid_extension_token() -> SecWebSocketExtensionsParseError {
  SecWebSocketExtensionsParseError::new("invalid Sec-WebSocket-Extensions extension token")
}

fn invalid_parameter() -> SecWebSocketExtensionsParseError {
  SecWebSocketExtensionsParseError::new("invalid Sec-WebSocket-Extensions parameter")
}
