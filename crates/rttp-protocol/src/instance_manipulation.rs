//! Shared instance-manipulation token-list validation for `A-IM` and `IM`.
//!
//! This module is the canonical authority for token, parameter, ordering,
//! duplicate, member-count, and size rules. Header-specific policy such as
//! `A-IM` q-value semantics stays in the calling modules.

use crate::http1::{is_token, is_token_byte};

pub(crate) const MAX_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_TOTAL_BYTES: usize = 64 * 1024;
pub(crate) const MAX_MEMBERS: usize = 32;
pub(crate) const MAX_PARAMETERS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceManipulationMember {
  pub token: String,
  pub parameters: Vec<InstanceManipulationParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceManipulationParameter {
  pub name: String,
  pub value: Option<InstanceManipulationParameterValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceManipulationParameterValue {
  pub value: String,
  pub quoted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstanceManipulationParseError {
  message: String,
}

impl InstanceManipulationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }

  pub(crate) fn message(&self) -> &str {
    &self.message
  }
}

pub(crate) fn parse_instance_manipulation_values<'a, I>(
  header_name: &'static str,
  values: I,
) -> Result<Vec<InstanceManipulationMember>, InstanceManipulationParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut members = Vec::new();
  let mut total_bytes = 0usize;

  for value in values {
    if value.len() > MAX_VALUE_BYTES {
      return Err(InstanceManipulationParseError::new(format!(
        "{header_name} header value is too large"
      )));
    }
    total_bytes = total_bytes.saturating_add(value.len());
    if total_bytes > MAX_TOTAL_BYTES {
      return Err(InstanceManipulationParseError::new(format!(
        "{header_name} header list is too large"
      )));
    }
    if value.bytes().any(is_invalid_control_byte) {
      return Err(InstanceManipulationParseError::new(format!(
        "invalid {header_name} control byte"
      )));
    }
    parse_field(header_name, value, &mut members)?;
  }

  if members.is_empty() {
    return Err(InstanceManipulationParseError::new(format!(
      "invalid {header_name} token"
    )));
  }
  Ok(members)
}

fn parse_field(
  header_name: &'static str,
  value: &str,
  members: &mut Vec<InstanceManipulationMember>,
) -> Result<(), InstanceManipulationParseError> {
  let mut position = 0;
  skip_ows(value, &mut position);
  if position == value.len() {
    return Err(InstanceManipulationParseError::new(format!(
      "invalid {header_name} token"
    )));
  }

  loop {
    let member = parse_member(header_name, value, &mut position)?;
    if members
      .iter()
      .any(|known: &InstanceManipulationMember| known.token.eq_ignore_ascii_case(&member.token))
    {
      return Err(InstanceManipulationParseError::new(format!(
        "duplicate {header_name} token"
      )));
    }
    if members.len() >= MAX_MEMBERS {
      return Err(InstanceManipulationParseError::new(format!(
        "too many {header_name} members"
      )));
    }
    members.push(member);
    skip_ows(value, &mut position);
    if position == value.len() {
      return Ok(());
    }
    if take_byte(value, &mut position) != Some(b',') {
      return Err(InstanceManipulationParseError::new(format!(
        "invalid {header_name} token"
      )));
    }
    skip_ows(value, &mut position);
    if position == value.len() {
      return Err(InstanceManipulationParseError::new(format!(
        "invalid {header_name} token"
      )));
    }
  }
}

fn parse_member(
  header_name: &'static str,
  value: &str,
  position: &mut usize,
) -> Result<InstanceManipulationMember, InstanceManipulationParseError> {
  let token = parse_token(header_name, value, position)?;
  let mut parameters = Vec::new();

  loop {
    skip_ows(value, position);
    if !take_if(value, position, b';') {
      break;
    }
    skip_ows(value, position);
    let name = parse_token(header_name, value, position)?;
    skip_ows(value, position);
    let parameter_value = if take_if(value, position, b'=') {
      skip_ows(value, position);
      Some(parse_parameter_value(header_name, value, position)?)
    } else {
      None
    };

    if parameters
      .iter()
      .any(|parameter: &InstanceManipulationParameter| parameter.name.eq_ignore_ascii_case(&name))
    {
      return Err(InstanceManipulationParseError::new(format!(
        "duplicate {header_name} parameter"
      )));
    }
    if parameters.len() >= MAX_PARAMETERS {
      return Err(InstanceManipulationParseError::new(format!(
        "too many {header_name} parameters"
      )));
    }
    parameters.push(InstanceManipulationParameter {
      name,
      value: parameter_value,
    });
  }

  Ok(InstanceManipulationMember { token, parameters })
}

fn parse_parameter_value(
  header_name: &'static str,
  value: &str,
  position: &mut usize,
) -> Result<InstanceManipulationParameterValue, InstanceManipulationParseError> {
  if take_if(value, position, b'"') {
    Ok(InstanceManipulationParameterValue {
      value: parse_quoted_string(header_name, value, position)?,
      quoted: true,
    })
  } else {
    Ok(InstanceManipulationParameterValue {
      value: parse_token(header_name, value, position)?,
      quoted: false,
    })
  }
}

fn parse_quoted_string(
  header_name: &'static str,
  value: &str,
  position: &mut usize,
) -> Result<String, InstanceManipulationParseError> {
  let mut parsed = String::new();
  while let Some(byte) = take_byte(value, position) {
    match byte {
      b'"' => return Ok(parsed),
      b'\\' => {
        let Some(escaped) = take_byte(value, position) else {
          return Err(InstanceManipulationParseError::new(format!(
            "invalid {header_name} quoted-string"
          )));
        };
        if !(escaped == b'\t' || (0x20..=0x7e).contains(&escaped)) {
          return Err(InstanceManipulationParseError::new(format!(
            "invalid {header_name} quoted-string"
          )));
        }
        parsed.push(escaped as char);
      }
      b'\t' | 0x20..=0x7e => parsed.push(byte as char),
      _ => {
        return Err(InstanceManipulationParseError::new(format!(
          "invalid {header_name} quoted-string"
        )))
      }
    }
  }
  Err(InstanceManipulationParseError::new(format!(
    "invalid {header_name} quoted-string"
  )))
}

fn parse_token(
  header_name: &'static str,
  value: &str,
  position: &mut usize,
) -> Result<String, InstanceManipulationParseError> {
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
    Err(InstanceManipulationParseError::new(format!(
      "invalid {header_name} token"
    )))
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
