//! Bounded, policy-free WebDAV `Timeout` request metadata parsing.
//!
//! This module validates one or more RFC 4918 `Timeout` field values as an
//! ordered list of timeout alternatives. Callers decide whether to create
//! locks, refresh locks, or select an application timeout.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Timeout` field value.
pub const MAX_TIMEOUT_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across all supplied fields.
pub const MAX_TIMEOUT_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum timeout alternatives accepted across the combined list.
pub const MAX_TIMEOUT_MEMBERS: usize = 32;

/// One parsed WebDAV `Timeout` alternative.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TimeoutType {
  Infinite,
  Second(u64),
}

impl TimeoutType {
  pub fn header_value(self) -> String {
    match self {
      Self::Infinite => "infinite".to_owned(),
      Self::Second(seconds) => format!("second-{seconds}"),
    }
  }
}

/// Parsed, bounded WebDAV `Timeout` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timeout {
  members: Vec<TimeoutType>,
}

impl Timeout {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TimeoutParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TimeoutParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      if value.len() > MAX_TIMEOUT_VALUE_BYTES {
        return Err(TimeoutParseError::new("Timeout header value is too large"));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_TIMEOUT_TOTAL_BYTES {
        return Err(TimeoutParseError::new("Timeout header list is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(TimeoutParseError::new("invalid Timeout control byte"));
      }
      for member in value.split(',') {
        let member = parse_timeout_member(member)?;
        if members.contains(&member) {
          return Err(TimeoutParseError::new("duplicate Timeout member"));
        }
        if members.len() >= MAX_TIMEOUT_MEMBERS {
          return Err(TimeoutParseError::new("too many Timeout members"));
        }
        members.push(member);
      }
    }

    if members.is_empty() {
      return Err(TimeoutParseError::new("invalid Timeout member"));
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[TimeoutType] {
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
      .map(|member| member.header_value())
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// An error returned when `Timeout` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeoutParseError {
  message: String,
}

impl TimeoutParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TimeoutParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TimeoutParseError {}

fn parse_timeout_member(member: &str) -> Result<TimeoutType, TimeoutParseError> {
  let member = member.trim_matches([' ', '\t']);
  if member.eq_ignore_ascii_case("infinite") {
    return Ok(TimeoutType::Infinite);
  }
  let Some(seconds) = member.strip_prefix_ignore_ascii_case("second-") else {
    return Err(TimeoutParseError::new("invalid Timeout member"));
  };
  if seconds.is_empty() || !seconds.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(TimeoutParseError::new("invalid Timeout seconds"));
  }
  let seconds = seconds
    .parse::<u64>()
    .map_err(|_| TimeoutParseError::new("Timeout seconds overflow"))?;
  Ok(TimeoutType::Second(seconds))
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

trait StripPrefixIgnoreAsciiCase {
  fn strip_prefix_ignore_ascii_case(&self, prefix: &str) -> Option<&str>;
}

impl StripPrefixIgnoreAsciiCase for str {
  fn strip_prefix_ignore_ascii_case(&self, prefix: &str) -> Option<&str> {
    let candidate = self.get(..prefix.len())?;
    candidate
      .eq_ignore_ascii_case(prefix)
      .then(|| &self[prefix.len()..])
  }
}
