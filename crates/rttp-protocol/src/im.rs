//! Bounded, policy-free `IM` response metadata parsing.
//!
//! This module validates one or more RFC 3229 `IM` field values as an ordered
//! list of instance-manipulation tokens with optional extension parameters.
//! Token, parameter, ordering, duplicate, member-count, and size rules reuse
//! the canonical instance-manipulation validator. Callers decide whether and
//! how to invert or apply instance manipulations. Unparsable input is an
//! error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::instance_manipulation::{
  parse_instance_manipulation_values, InstanceManipulationMember, InstanceManipulationParameter,
  InstanceManipulationParameterValue, MAX_MEMBERS, MAX_PARAMETERS, MAX_TOTAL_BYTES,
  MAX_VALUE_BYTES,
};

pub const MAX_IM_VALUE_BYTES: usize = MAX_VALUE_BYTES;
pub const MAX_IM_TOTAL_BYTES: usize = MAX_TOTAL_BYTES;
pub const MAX_IM_MEMBERS: usize = MAX_MEMBERS;
pub const MAX_IM_PARAMETERS: usize = MAX_PARAMETERS;

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
    let members = parse_instance_manipulation_values("IM", values)
      .map_err(|error| ImParseError::new(error.message()))?;
    Ok(Self {
      members: members.into_iter().map(ImMember::from).collect(),
    })
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

impl From<InstanceManipulationMember> for ImMember {
  fn from(member: InstanceManipulationMember) -> Self {
    Self {
      token: member.token,
      parameters: member
        .parameters
        .into_iter()
        .map(ImParameter::from)
        .collect(),
    }
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

impl From<InstanceManipulationParameter> for ImParameter {
  fn from(parameter: InstanceManipulationParameter) -> Self {
    Self {
      name: parameter.name,
      value: parameter.value.map(ImParameterValue::from),
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

impl From<InstanceManipulationParameterValue> for ImParameterValue {
  fn from(value: InstanceManipulationParameterValue) -> Self {
    Self {
      value: value.value,
      quoted: value.quoted,
    }
  }
}
