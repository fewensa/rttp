//! Bounded, policy-free `Origin-Agent-Cluster` response metadata parsing.
//!
//! This module validates the Structured Fields boolean response value only.
//! Callers retain browser isolation and trust policy.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Item, Parser};

/// Maximum bytes accepted in an `Origin-Agent-Cluster` field value.
pub const MAX_ORIGIN_AGENT_CLUSTER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Origin-Agent-Cluster` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OriginAgentCluster {
  Boolean(bool),
}

impl OriginAgentCluster {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, OriginAgentClusterParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, OriginAgentClusterParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let item = Parser::new(value)
      .parse::<Item>()
      .map_err(|_| invalid_value())?;
    if !item.params.is_empty() {
      return Err(invalid_value());
    }
    match item.bare_item {
      BareItem::Boolean(value) => Ok(Self::Boolean(value)),
      _ => Err(invalid_value()),
    }
  }

  pub const fn boolean(self) -> bool {
    match self {
      Self::Boolean(value) => value,
    }
  }

  pub const fn value(self) -> bool {
    self.boolean()
  }

  pub const fn is_enabled(self) -> bool {
    self.boolean()
  }

  pub const fn header_value(self) -> &'static str {
    match self {
      Self::Boolean(false) => "?0",
      Self::Boolean(true) => "?1",
    }
  }
}

/// An error returned when `Origin-Agent-Cluster` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginAgentClusterParseError {
  message: String,
}

impl OriginAgentClusterParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for OriginAgentClusterParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for OriginAgentClusterParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, OriginAgentClusterParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_bounded_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value)?;
  }
  if has_duplicate {
    return Err(OriginAgentClusterParseError::new(
      "duplicate Origin-Agent-Cluster header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), OriginAgentClusterParseError> {
  if value.len() > MAX_ORIGIN_AGENT_CLUSTER_VALUE_BYTES {
    return Err(OriginAgentClusterParseError::new(
      "Origin-Agent-Cluster header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(OriginAgentClusterParseError::new(
      "invalid Origin-Agent-Cluster control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> OriginAgentClusterParseError {
  OriginAgentClusterParseError::new("invalid Origin-Agent-Cluster header value")
}
