//! Bounded, policy-free `A-IM` request metadata parsing.
//!
//! This module validates one or more `A-IM` field values as an ordered list of
//! instance-manipulation tokens with optional quality weights and extension
//! parameters. Callers decide whether and how to select or apply delta
//! encodings. Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::http1::{is_token, is_token_byte};

pub const MAX_A_IM_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_A_IM_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_A_IM_MEMBERS: usize = 32;
pub const MAX_A_IM_PARAMETERS: usize = 16;

/// Parsed, bounded `A-IM` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AIm {
  members: Vec<AImMember>,
}

/// One instance-manipulation token from a parsed `A-IM` list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AImMember {
  token: String,
  quality: u16,
  parameters: Vec<AImParameter>,
}

/// One parameter from a parsed `A-IM` member, including an optional `q`
/// parameter retained in wire order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AImParameter {
  name: String,
  value: Option<AImParameterValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AImParameterValue {
  value: String,
  quoted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AImParseError {
  message: String,
}

impl AImParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AImParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AImParseError {}

impl AIm {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AImParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AImParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      if value.len() > MAX_A_IM_VALUE_BYTES {
        return Err(AImParseError::new("A-IM header value is too large"));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_A_IM_TOTAL_BYTES {
        return Err(AImParseError::new("A-IM header list is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AImParseError::new("invalid A-IM control byte"));
      }
      parse_field(value, &mut members)?;
    }

    if members.is_empty() {
      return Err(AImParseError::new("invalid A-IM token"));
    }
    Ok(Self { members })
  }

  pub fn from_members<I, M>(members: I) -> Result<Self, AImParseError>
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
      if value.len() > MAX_A_IM_VALUE_BYTES {
        return Err(AImParseError::new("A-IM header value is too large"));
      }
    }

    Self::parse(value)
  }

  pub fn members(&self) -> &[AImMember] {
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
      .map(AImMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AImMember {
  pub fn token(&self) -> &str {
    &self.token
  }

  /// Returns the q-value as thousandths, where `1000` is the default quality
  /// of `1` and `0` means not acceptable.
  pub fn quality(&self) -> u16 {
    self.quality
  }

  pub fn parameters(&self) -> &[AImParameter] {
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

impl AImParameter {
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

impl AImParameterValue {
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

fn parse_field(value: &str, members: &mut Vec<AImMember>) -> Result<(), AImParseError> {
  let mut position = 0;
  skip_ows(value, &mut position);
  if position == value.len() {
    return Err(AImParseError::new("invalid A-IM token"));
  }

  loop {
    let member = parse_member(value, &mut position)?;
    if members
      .iter()
      .any(|known: &AImMember| known.token.eq_ignore_ascii_case(&member.token))
    {
      return Err(AImParseError::new("duplicate A-IM token"));
    }
    if members.len() >= MAX_A_IM_MEMBERS {
      return Err(AImParseError::new("too many A-IM members"));
    }
    members.push(member);
    skip_ows(value, &mut position);
    if position == value.len() {
      return Ok(());
    }
    if take_byte(value, &mut position) != Some(b',') {
      return Err(AImParseError::new("invalid A-IM token"));
    }
    skip_ows(value, &mut position);
    if position == value.len() {
      return Err(AImParseError::new("invalid A-IM token"));
    }
  }
}

fn parse_member(value: &str, position: &mut usize) -> Result<AImMember, AImParseError> {
  let token = parse_token(value, position)?;
  let mut quality = 1000;
  let mut parameters = Vec::new();

  loop {
    skip_ows(value, position);
    if !take_if(value, position, b';') {
      break;
    }
    skip_ows(value, position);
    let name = parse_token(value, position)?;
    skip_ows(value, position);
    let parameter_value = if take_if(value, position, b'=') {
      skip_ows(value, position);
      Some(parse_parameter_value(value, position, &name)?)
    } else if name.eq_ignore_ascii_case("q") {
      return Err(AImParseError::new("invalid A-IM q-value"));
    } else {
      None
    };

    if name.eq_ignore_ascii_case("q") {
      let Some(parameter_value) = &parameter_value else {
        return Err(AImParseError::new("invalid A-IM q-value"));
      };
      if parameter_value.quoted {
        return Err(AImParseError::new("invalid A-IM q-value"));
      }
      quality = parse_qvalue(&parameter_value.value)?;
    }

    if parameters
      .iter()
      .any(|parameter: &AImParameter| parameter.name.eq_ignore_ascii_case(&name))
    {
      return Err(AImParseError::new("duplicate A-IM parameter"));
    }
    if parameters.len() >= MAX_A_IM_PARAMETERS {
      return Err(AImParseError::new("too many A-IM parameters"));
    }
    parameters.push(AImParameter {
      name,
      value: parameter_value,
    });
  }

  Ok(AImMember {
    token,
    quality,
    parameters,
  })
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
  name: &str,
) -> Result<AImParameterValue, AImParseError> {
  if name.eq_ignore_ascii_case("q") {
    let qvalue = parse_token(value, position)?;
    return Ok(AImParameterValue {
      value: qvalue,
      quoted: false,
    });
  }
  if take_if(value, position, b'"') {
    Ok(AImParameterValue {
      value: parse_quoted_string(value, position)?,
      quoted: true,
    })
  } else {
    Ok(AImParameterValue {
      value: parse_token(value, position)?,
      quoted: false,
    })
  }
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, AImParseError> {
  let mut parsed = String::new();
  while let Some(byte) = take_byte(value, position) {
    match byte {
      b'"' => return Ok(parsed),
      b'\\' => {
        let Some(escaped) = take_byte(value, position) else {
          return Err(AImParseError::new("invalid A-IM quoted-string"));
        };
        if !(escaped == b'\t' || (0x20..=0x7e).contains(&escaped)) {
          return Err(AImParseError::new("invalid A-IM quoted-string"));
        }
        parsed.push(escaped as char);
      }
      b'\t' | 0x20..=0x7e => parsed.push(byte as char),
      _ => return Err(AImParseError::new("invalid A-IM quoted-string")),
    }
  }
  Err(AImParseError::new("invalid A-IM quoted-string"))
}

fn parse_token(value: &str, position: &mut usize) -> Result<String, AImParseError> {
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
    Err(AImParseError::new("invalid A-IM token"))
  }
}

fn parse_qvalue(qvalue: &str) -> Result<u16, AImParseError> {
  let Some((whole, fraction)) = qvalue.split_once('.') else {
    return match qvalue {
      "0" => Ok(0),
      "1" => Ok(1000),
      _ => Err(AImParseError::new("invalid A-IM q-value")),
    };
  };
  if !matches!(whole, "0" | "1")
    || fraction.len() > 3
    || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    || (whole == "1" && !fraction.bytes().all(|byte| byte == b'0'))
  {
    return Err(AImParseError::new("invalid A-IM q-value"));
  }
  let fractional = if fraction.is_empty() {
    0
  } else {
    fraction
      .parse::<u16>()
      .map_err(|_| AImParseError::new("invalid A-IM q-value"))?
  };
  Ok(if whole == "1" {
    1000
  } else {
    fractional * 10_u16.pow(3 - fraction.len() as u32)
  })
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
