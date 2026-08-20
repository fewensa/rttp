//! Bounded, policy-free `IM` response metadata parsing.
//!
//! This module validates one or more RFC 3229 `IM` field values as an ordered
//! list of instance-manipulation tokens with optional extension parameters.
//! Callers decide whether and how to invert or apply instance manipulations.
//! Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::http1::{is_token, is_token_byte};

pub const MAX_IM_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_IM_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_IM_MEMBERS: usize = 32;
pub const MAX_IM_PARAMETERS: usize = 16;

/// Parsed, bounded `IM` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Im {
  members: Vec<ImMember>,
}

/// One instance-manipulation token from a parsed `IM` list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImMember {
  token: String,
  parameters: Vec<ImParameter>,
}

/// One parameter from a parsed `IM` member, retained in wire order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImParameter {
  name: String,
  value: Option<ImParameterValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImParameterValue {
  value: String,
  quoted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImParseError {
  message: String,
}

impl ImParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ImParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ImParseError {}

impl Im {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ImParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ImParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      if value.len() > MAX_IM_VALUE_BYTES {
        return Err(ImParseError::new("IM header value is too large"));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_IM_TOTAL_BYTES {
        return Err(ImParseError::new("IM header list is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(ImParseError::new("invalid IM control byte"));
      }
      parse_field(value, &mut members)?;
    }

    if members.is_empty() {
      return Err(ImParseError::new("invalid IM token"));
    }
    Ok(Self { members })
  }

  pub fn from_members<I, M>(members: I) -> Result<Self, ImParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let mut value = String::new();

    for (index, member) in members.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(member.as_ref());
      if value.len() > MAX_IM_VALUE_BYTES {
        return Err(ImParseError::new("IM header value is too large"));
      }
    }

    Self::parse(value)
  }

  pub fn members(&self) -> &[ImMember] {
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
      .map(ImMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ImMember {
  pub fn token(&self) -> &str {
    &self.token
  }

  pub fn parameters(&self) -> &[ImParameter] {
    &self.parameters
  }

  fn header_value(&self) -> String {
    let mut value = self.token.clone();
    for parameter in &self.parameters {
      value.push(';');
      value.push_str(&parameter.header_value());
    }
    value
  }
}

impl ImParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_ref().map(|value| value.value.as_str())
  }

  fn header_value(&self) -> String {
    match &self.value {
      Some(parameter_value) => {
        format!("{}={}", self.name, parameter_value.header_value())
      }
      None => self.name.clone(),
    }
  }
}

impl ImParameterValue {
  fn header_value(&self) -> String {
    if self.quoted {
      format!(
        "\"{}\"",
        self.value.replace('\\', "\\\\").replace('"', "\\\"")
      )
    } else {
      self.value.clone()
    }
  }
}

fn parse_field(value: &str, members: &mut Vec<ImMember>) -> Result<(), ImParseError> {
  let mut position = 0;
  skip_ows(value, &mut position);
  if position == value.len() {
    return Err(ImParseError::new("invalid IM token"));
  }

  loop {
    let member = parse_member(value, &mut position)?;
    if members
      .iter()
      .any(|known: &ImMember| known.token.eq_ignore_ascii_case(&member.token))
    {
      return Err(ImParseError::new("duplicate IM token"));
    }
    if members.len() >= MAX_IM_MEMBERS {
      return Err(ImParseError::new("too many IM members"));
    }
    members.push(member);
    skip_ows(value, &mut position);
    if position == value.len() {
      return Ok(());
    }
    if take_byte(value, &mut position) != Some(b',') {
      return Err(ImParseError::new("invalid IM token"));
    }
    skip_ows(value, &mut position);
    if position == value.len() {
      return Err(ImParseError::new("invalid IM token"));
    }
  }
}

fn parse_member(value: &str, position: &mut usize) -> Result<ImMember, ImParseError> {
  let token = parse_token(value, position)?;
  let mut parameters = Vec::new();

  loop {
    skip_ows(value, position);
    if !take_if(value, position, b';') {
      break;
    }
    skip_ows(value, position);
    let name = parse_token(value, position)?;
    if name.eq_ignore_ascii_case("q") {
      return Err(ImParseError::new("invalid IM q-parameter"));
    }
    skip_ows(value, position);
    let parameter_value = if take_if(value, position, b'=') {
      skip_ows(value, position);
      Some(parse_parameter_value(value, position)?)
    } else {
      None
    };

    if parameters
      .iter()
      .any(|parameter: &ImParameter| parameter.name.eq_ignore_ascii_case(&name))
    {
      return Err(ImParseError::new("duplicate IM parameter"));
    }
    if parameters.len() >= MAX_IM_PARAMETERS {
      return Err(ImParseError::new("too many IM parameters"));
    }
    parameters.push(ImParameter {
      name,
      value: parameter_value,
    });
  }

  Ok(ImMember { token, parameters })
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<ImParameterValue, ImParseError> {
  if take_if(value, position, b'"') {
    Ok(ImParameterValue {
      value: parse_quoted_string(value, position)?,
      quoted: true,
    })
  } else {
    Ok(ImParameterValue {
      value: parse_token(value, position)?,
      quoted: false,
    })
  }
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, ImParseError> {
  let mut parsed = String::new();
  while let Some(byte) = take_byte(value, position) {
    match byte {
      b'"' => return Ok(parsed),
      b'\\' => {
        let Some(escaped) = take_byte(value, position) else {
          return Err(ImParseError::new("invalid IM quoted-string"));
        };
        if !(escaped == b'\t' || (0x20..=0x7e).contains(&escaped)) {
          return Err(ImParseError::new("invalid IM quoted-string"));
        }
        parsed.push(escaped as char);
      }
      b'\t' | 0x20..=0x7e => parsed.push(byte as char),
      _ => return Err(ImParseError::new("invalid IM quoted-string")),
    }
  }
  Err(ImParseError::new("invalid IM quoted-string"))
}

fn parse_token(value: &str, position: &mut usize) -> Result<String, ImParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  let token = &value[start..*position];
  if is_token(token) {
    Ok(token.to_string())
  } else {
    Err(ImParseError::new("invalid IM token"))
  }
}

fn skip_ows(value: &str, position: &mut usize) {
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn take_if(value: &str, position: &mut usize, expected: u8) -> bool {
  if value.as_bytes().get(*position) == Some(&expected) {
    *position += 1;
    true
  } else {
    false
  }
}

fn take_byte(value: &str, position: &mut usize) -> Option<u8> {
  let byte = *value.as_bytes().get(*position)?;
  *position += 1;
  Some(byte)
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
