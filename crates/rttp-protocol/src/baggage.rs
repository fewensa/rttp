//! Bounded W3C Baggage request metadata parsing.
//!
//! This module validates `baggage` syntax only. It does not interpret
//! application keys or values, store request context, or propagate baggage
//! automatically.

use std::error::Error;
use std::fmt;

use crate::http1::is_tchar;

/// Maximum bytes accepted in a combined `baggage` field value.
pub const MAX_BAGGAGE_VALUE_BYTES: usize = 8192;
/// Maximum `baggage` list-members accepted.
pub const MAX_BAGGAGE_MEMBERS: usize = 180;
/// Maximum bytes accepted in one `baggage` list-member.
pub const MAX_BAGGAGE_MEMBER_BYTES: usize = 4096;

/// Parsed W3C `baggage` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct Baggage {
  members: Vec<BaggageMember>,
}

/// One ordered W3C `baggage` list-member.
#[derive(Clone, Eq, PartialEq)]
pub struct BaggageMember {
  key: String,
  value: String,
  properties: Vec<BaggageProperty>,
}

/// One optional property attached to a `baggage` list-member.
#[derive(Clone, Eq, PartialEq)]
pub struct BaggageProperty {
  key: String,
  value: Option<String>,
}

/// An error returned when `baggage` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaggageParseError {
  message: String,
}

impl Baggage {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, BaggageParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, BaggageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut combined = String::new();
    for value in values {
      if combined.is_empty() {
        combined.push_str(value);
      } else {
        combined.push(',');
        combined.push_str(value);
      }
      if combined.len() > MAX_BAGGAGE_VALUE_BYTES {
        return Err(BaggageParseError::new("baggage header value is too large"));
      }
    }
    parse_baggage_value(&combined)
  }

  pub fn members(&self) -> &[BaggageMember] {
    &self.members
  }

  pub fn header_value(&self) -> String {
    self
      .members
      .iter()
      .map(BaggageMember::header_value)
      .collect::<Vec<_>>()
      .join(",")
  }
}

impl fmt::Debug for Baggage {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Baggage")
      .field("member_count", &self.members.len())
      .finish()
  }
}

impl BaggageMember {
  pub fn key(&self) -> &str {
    &self.key
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn properties(&self) -> &[BaggageProperty] {
    &self.properties
  }

  pub fn header_value(&self) -> String {
    let mut value = format!("{}={}", self.key, self.value);
    for property in &self.properties {
      value.push(';');
      value.push_str(&property.header_value());
    }
    value
  }
}

impl fmt::Debug for BaggageMember {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("BaggageMember")
      .field("key", &self.key)
      .field("value", &"[REDACTED]")
      .field("properties", &self.properties)
      .finish()
  }
}

impl BaggageProperty {
  pub fn key(&self) -> &str {
    &self.key
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  pub fn header_value(&self) -> String {
    match &self.value {
      Some(value) => format!("{}={}", self.key, value),
      None => self.key.clone(),
    }
  }
}

impl fmt::Debug for BaggageProperty {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("BaggageProperty")
      .field("key", &self.key)
      .field("value", &self.value.as_ref().map(|_| "[REDACTED]"))
      .finish()
  }
}

impl BaggageParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for BaggageParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for BaggageParseError {}

fn parse_baggage_value(value: &str) -> Result<Baggage, BaggageParseError> {
  if value.len() > MAX_BAGGAGE_VALUE_BYTES {
    return Err(BaggageParseError::new("baggage header value is too large"));
  }

  let mut members = Vec::new();
  for raw_member in value.split(',') {
    let member = raw_member.trim_matches([' ', '\t']);
    if member.is_empty() {
      continue;
    }
    if members.len() >= MAX_BAGGAGE_MEMBERS {
      return Err(BaggageParseError::new("too many baggage members"));
    }
    if member.len() > MAX_BAGGAGE_MEMBER_BYTES {
      return Err(BaggageParseError::new("baggage member is too large"));
    }

    let parsed = parse_baggage_member(member)?;
    if members
      .iter()
      .any(|known: &BaggageMember| known.key == parsed.key)
    {
      return Err(BaggageParseError::new("duplicate baggage key"));
    }
    members.push(parsed);
  }

  Ok(Baggage { members })
}

fn parse_baggage_member(member: &str) -> Result<BaggageMember, BaggageParseError> {
  let Some((key, rest)) = split_token_assignment(member) else {
    return Err(BaggageParseError::new("invalid baggage member"));
  };
  if !is_valid_baggage_key(key) {
    return Err(BaggageParseError::new("invalid baggage key"));
  }

  let (value, properties) = match rest.split_once(';') {
    Some((value, properties)) => (trim_ows(value), parse_baggage_properties(properties)?),
    None => (trim_ows(rest), Vec::new()),
  };
  if !is_valid_baggage_value(value) {
    return Err(BaggageParseError::new("invalid baggage value"));
  }

  Ok(BaggageMember {
    key: key.to_string(),
    value: value.to_string(),
    properties,
  })
}

fn parse_baggage_properties(value: &str) -> Result<Vec<BaggageProperty>, BaggageParseError> {
  let mut properties = Vec::new();
  for raw_property in value.split(';') {
    let property = raw_property.trim_matches([' ', '\t']);
    if property.is_empty() {
      return Err(BaggageParseError::new("invalid baggage property"));
    }

    if let Some((key, rest)) = split_token_assignment(property) {
      if !is_valid_baggage_key(key) {
        return Err(BaggageParseError::new("invalid baggage property key"));
      }
      let value = trim_ows(rest);
      if !is_valid_baggage_value(value) {
        return Err(BaggageParseError::new("invalid baggage property value"));
      }
      properties.push(BaggageProperty {
        key: key.to_string(),
        value: Some(value.to_string()),
      });
      continue;
    }

    if !is_valid_baggage_key(property) {
      return Err(BaggageParseError::new("invalid baggage property key"));
    }
    properties.push(BaggageProperty {
      key: property.to_string(),
      value: None,
    });
  }
  Ok(properties)
}

fn split_token_assignment(value: &str) -> Option<(&str, &str)> {
  let key_end = value
    .bytes()
    .position(|byte| !is_tchar(byte))
    .unwrap_or(value.len());
  if key_end == 0 {
    return None;
  }
  let key = &value[..key_end];
  let rest = trim_ows(&value[key_end..]);
  rest.strip_prefix('=').map(|after_eq| (key, after_eq))
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}

fn is_valid_baggage_key(key: &str) -> bool {
  !key.is_empty() && key.bytes().all(is_tchar)
}

fn is_valid_baggage_value(value: &str) -> bool {
  value.bytes().all(is_baggage_octet)
}

fn is_baggage_octet(byte: u8) -> bool {
  matches!(
    byte,
    0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e
  )
}
