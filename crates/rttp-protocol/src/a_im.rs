//! Bounded, policy-free `A-IM` request metadata parsing.
//!
//! This module validates one or more `A-IM` field values as an ordered list of
//! instance-manipulation tokens with optional quality weights and extension
//! parameters. Token, parameter, ordering, duplicate, member-count, and size
//! rules reuse the canonical instance-manipulation validator. Callers decide
//! whether and how to select or apply delta encodings. Unparsable input is an
//! error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::instance_manipulation::{
  parse_instance_manipulation_values, InstanceManipulationMember, InstanceManipulationParameter,
  InstanceManipulationParameterValue, MAX_MEMBERS, MAX_PARAMETERS, MAX_TOTAL_BYTES,
  MAX_VALUE_BYTES,
};

pub const MAX_A_IM_VALUE_BYTES: usize = MAX_VALUE_BYTES;
pub const MAX_A_IM_TOTAL_BYTES: usize = MAX_TOTAL_BYTES;
pub const MAX_A_IM_MEMBERS: usize = MAX_MEMBERS;
pub const MAX_A_IM_PARAMETERS: usize = MAX_PARAMETERS;

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
    let members = parse_instance_manipulation_values("A-IM", values)
      .map_err(|error| AImParseError::new(error.message()))?;
    Ok(Self {
      members: members
        .into_iter()
        .map(AImMember::try_from)
        .collect::<Result<_, _>>()?,
    })
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

impl TryFrom<InstanceManipulationMember> for AImMember {
  type Error = AImParseError;

  fn try_from(member: InstanceManipulationMember) -> Result<Self, Self::Error> {
    let mut quality = 1000;
    let mut parameters = Vec::with_capacity(member.parameters.len());

    for parameter in member.parameters {
      let parameter = AImParameter::from(parameter);
      if parameter.name.eq_ignore_ascii_case("q") {
        let Some(parameter_value) = &parameter.value else {
          return Err(AImParseError::new("invalid A-IM q-value"));
        };
        if parameter_value.quoted {
          return Err(AImParseError::new("invalid A-IM q-value"));
        }
        quality = parse_qvalue(&parameter_value.value)?;
      }
      parameters.push(parameter);
    }

    Ok(Self {
      token: member.token,
      quality,
      parameters,
    })
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

impl From<InstanceManipulationParameter> for AImParameter {
  fn from(parameter: InstanceManipulationParameter) -> Self {
    Self {
      name: parameter.name,
      value: parameter.value.map(AImParameterValue::from),
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

impl From<InstanceManipulationParameterValue> for AImParameterValue {
  fn from(value: InstanceManipulationParameterValue) -> Self {
    Self {
      value: value.value,
      quoted: value.quoted,
    }
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
