//! Bounded RFC 8586 `CDN-Loop` request metadata parsing.
//!
//! This module validates `CDN-Loop` syntax and bounds only. It does not detect
//! loops, reject requests, insert a local CDN identifier, or forward the field
//! automatically.

use std::error::Error;
use std::fmt;

use crate::host::Host;
use crate::http1::{is_token, is_token_byte};

/// Maximum bytes accepted in one `CDN-Loop` field value, in the combined raw
/// field set including `", "` separator overhead, and in the combined
/// serialized field value.
pub const MAX_CDN_LOOP_VALUE_BYTES: usize = 64 * 1024;
/// Maximum `CDN-Loop` list-members accepted across all fields.
pub const MAX_CDN_LOOP_MEMBERS: usize = 256;
/// Maximum HTTP parameters accepted on one `CDN-Loop` member.
pub const MAX_CDN_LOOP_PARAMETERS: usize = 32;

/// Parsed, bounded RFC 8586 `CDN-Loop` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdnLoop {
  members: Vec<CdnLoopMember>,
}

/// One ordered RFC 8586 `cdn-info` list-member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdnLoopMember {
  identifier: String,
  parameters: Vec<CdnLoopParameter>,
}

/// One HTTP `parameter` attached to a `CDN-Loop` member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdnLoopParameter {
  name: String,
  value: String,
}

/// An error returned when `CDN-Loop` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdnLoopParseError {
  message: String,
}

impl CdnLoopParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CdnLoopParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CdnLoopParseError {}

impl CdnLoop {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CdnLoopParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CdnLoopParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_CDN_LOOP_VALUE_BYTES {
        return Err(CdnLoopParseError::new("CDN-Loop header value is too large"));
      }
      let separator = if total_bytes > 0 { 2 } else { 0 };
      total_bytes = total_bytes
        .saturating_add(separator)
        .saturating_add(value.len());
      if total_bytes > MAX_CDN_LOOP_VALUE_BYTES {
        return Err(CdnLoopParseError::new("CDN-Loop header value is too large"));
      }
      if value
        .bytes()
        .any(|byte| byte.is_ascii_control() && byte != b'\t')
      {
        return Err(CdnLoopParseError::new("invalid CDN-Loop control byte"));
      }
      parse_field(value, &mut members)?;
    }
    if members.is_empty() {
      return Err(CdnLoopParseError::new("invalid CDN-Loop member"));
    }
    let cdn_loop = CdnLoop { members };
    if cdn_loop.header_value().len() > MAX_CDN_LOOP_VALUE_BYTES {
      return Err(CdnLoopParseError::new("CDN-Loop header value is too large"));
    }
    Ok(cdn_loop)
  }

  pub fn members(&self) -> &[CdnLoopMember] {
    &self.members
  }

  pub fn len(&self) -> usize {
    self.members.len()
  }

  pub fn is_empty(&self) -> bool {
    self.members.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .members
      .iter()
      .map(CdnLoopMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl CdnLoopMember {
  /// Returns the opaque CDN identifier, preserving the accepted wire spelling.
  pub fn identifier(&self) -> &str {
    &self.identifier
  }

  pub fn parameters(&self) -> &[CdnLoopParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|parameter| parameter.value.as_str())
  }

  fn header_value(&self) -> String {
    let mut value = self.identifier.clone();
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(&parameter.header_value());
    }
    value
  }
}

impl CdnLoopParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  fn header_value(&self) -> String {
    if is_token(&self.value) {
      format!("{}={}", self.name, self.value)
    } else {
      format!(
        "{}=\"{}\"",
        self.name,
        self.value.replace('\\', "\\\\").replace('"', "\\\"")
      )
    }
  }
}

fn parse_field(value: &str, members: &mut Vec<CdnLoopMember>) -> Result<(), CdnLoopParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(CdnLoopParseError::new("invalid CDN-Loop member"));
  }

  loop {
    if members.len() >= MAX_CDN_LOOP_MEMBERS {
      return Err(CdnLoopParseError::new("too many CDN-Loop members"));
    }
    let member = parse_member(value, &mut position)?;
    members.push(member);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(CdnLoopParseError::new("invalid CDN-Loop member"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(CdnLoopParseError::new("invalid CDN-Loop member"));
    }
  }
}

fn parse_member(value: &str, position: &mut usize) -> Result<CdnLoopMember, CdnLoopParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  while bytes
    .get(*position)
    .is_some_and(|byte| !matches!(*byte, b';' | b','))
  {
    *position += 1;
  }
  let identifier = value[start..*position].trim_matches([' ', '\t']);
  if !is_valid_identifier(identifier) {
    return Err(CdnLoopParseError::new("invalid CDN-Loop identifier"));
  }

  let mut parameters = Vec::new();
  skip_ows(bytes, position);
  loop {
    match bytes.get(*position) {
      Some(b';') => {
        *position += 1;
        skip_ows(bytes, position);
        if *position == value.len() {
          return Err(CdnLoopParseError::new("invalid CDN-Loop parameter"));
        }
        let parameter = parse_parameter(value, position)?;
        if parameters
          .iter()
          .any(|known: &CdnLoopParameter| known.name.eq_ignore_ascii_case(&parameter.name))
        {
          return Err(CdnLoopParseError::new("duplicate CDN-Loop parameter"));
        }
        if parameters.len() >= MAX_CDN_LOOP_PARAMETERS {
          return Err(CdnLoopParseError::new("too many CDN-Loop parameters"));
        }
        parameters.push(parameter);
        skip_ows(bytes, position);
      }
      Some(b',') | None => break,
      _ => return Err(CdnLoopParseError::new("invalid CDN-Loop parameter")),
    }
  }
  Ok(CdnLoopMember {
    identifier: identifier.to_string(),
    parameters,
  })
}

fn parse_parameter(
  value: &str,
  position: &mut usize,
) -> Result<CdnLoopParameter, CdnLoopParseError> {
  let name = parse_token(value, position, "invalid CDN-Loop parameter name")?.to_ascii_lowercase();
  skip_ows(value.as_bytes(), position);
  if value.as_bytes().get(*position) != Some(&b'=') {
    return Err(CdnLoopParseError::new("invalid CDN-Loop parameter"));
  }
  *position += 1;
  skip_ows(value.as_bytes(), position);
  let parameter_value = parse_value(value, position)?;
  Ok(CdnLoopParameter {
    name,
    value: parameter_value,
  })
}

fn parse_value(value: &str, position: &mut usize) -> Result<String, CdnLoopParseError> {
  if value.as_bytes().get(*position) != Some(&b'"') {
    return Ok(parse_token(value, position, "invalid CDN-Loop parameter value")?.to_string());
  }

  *position += 1;
  let mut parsed = String::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => return Ok(parsed),
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(CdnLoopParseError::new("invalid CDN-Loop quoted-string"));
        };
        if !(escaped == b'\t' || (0x20..=0x7e).contains(&escaped)) {
          return Err(CdnLoopParseError::new("invalid CDN-Loop quoted-string"));
        }
        *position += 1;
        parsed.push(escaped as char);
      }
      b'\t' | 0x20..=0x7e => parsed.push(byte as char),
      _ => return Err(CdnLoopParseError::new("invalid CDN-Loop quoted-string")),
    }
  }
  Err(CdnLoopParseError::new("invalid CDN-Loop quoted-string"))
}

fn is_valid_identifier(identifier: &str) -> bool {
  Host::parse(identifier).is_ok() || is_token(identifier)
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, CdnLoopParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(CdnLoopParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}
