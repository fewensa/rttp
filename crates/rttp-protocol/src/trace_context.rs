//! Bounded W3C Trace Context request metadata parsing.
//!
//! This module validates `traceparent` and `tracestate` syntax only. It does
//! not create identifiers, decide sampling, select a tracing backend, or
//! propagate context automatically.

use std::error::Error;
use std::fmt;

/// Exact bytes in a version 00 `traceparent` field value.
pub const TRACEPARENT_VALUE_BYTES: usize = 55;
/// Maximum bytes accepted in a combined `tracestate` field value.
pub const MAX_TRACESTATE_VALUE_BYTES: usize = 512;
/// Maximum `tracestate` list-members accepted.
pub const MAX_TRACESTATE_MEMBERS: usize = 32;
/// Maximum bytes accepted in one `tracestate` key.
pub const MAX_TRACESTATE_KEY_BYTES: usize = 256;
/// Maximum bytes accepted in one `tracestate` value.
pub const MAX_TRACESTATE_MEMBER_VALUE_BYTES: usize = 256;

/// Parsed W3C `traceparent` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct TraceParent {
  version: String,
  trace_id: String,
  parent_id: String,
  flags: String,
}

/// Parsed W3C `tracestate` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct TraceState {
  members: Vec<TraceStateMember>,
}

/// One ordered W3C `tracestate` list-member.
#[derive(Clone, Eq, PartialEq)]
pub struct TraceStateMember {
  key: String,
  value: String,
}

/// An error returned when `traceparent` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceParentParseError {
  message: String,
}

/// An error returned when `tracestate` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceStateParseError {
  message: String,
}

impl TraceParent {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TraceParentParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TraceParentParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let value = values.next().ok_or_else(invalid_traceparent)?;
    if values.next().is_some() {
      return Err(TraceParentParseError::new(
        "duplicate traceparent header fields",
      ));
    }
    parse_traceparent_value(value)
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn trace_id(&self) -> &str {
    &self.trace_id
  }

  pub fn parent_id(&self) -> &str {
    &self.parent_id
  }

  pub fn flags(&self) -> &str {
    &self.flags
  }

  pub fn sampled(&self) -> bool {
    u8::from_str_radix(&self.flags, 16)
      .map(|flags| flags & 0x01 == 0x01)
      .unwrap_or(false)
  }

  pub fn header_value(&self) -> String {
    format!(
      "{}-{}-{}-{}",
      self.version, self.trace_id, self.parent_id, self.flags
    )
  }
}

impl fmt::Debug for TraceParent {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("TraceParent")
      .field("version", &self.version)
      .field("trace_id", &"[REDACTED]")
      .field("parent_id", &"[REDACTED]")
      .field("flags", &self.flags)
      .finish()
  }
}

impl TraceState {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TraceStateParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TraceStateParseError>
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
      if combined.len() > MAX_TRACESTATE_VALUE_BYTES {
        return Err(TraceStateParseError::new(
          "tracestate header value is too large",
        ));
      }
    }
    parse_tracestate_value(&combined)
  }

  pub fn members(&self) -> &[TraceStateMember] {
    &self.members
  }

  pub fn header_value(&self) -> String {
    self
      .members
      .iter()
      .map(TraceStateMember::header_value)
      .collect::<Vec<_>>()
      .join(",")
  }
}

impl fmt::Debug for TraceState {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("TraceState")
      .field("member_count", &self.members.len())
      .finish()
  }
}

impl TraceStateMember {
  pub fn key(&self) -> &str {
    &self.key
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> String {
    format!("{}={}", self.key, self.value)
  }
}

impl fmt::Debug for TraceStateMember {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("TraceStateMember")
      .field("key", &self.key)
      .field("value", &"[REDACTED]")
      .finish()
  }
}

impl TraceParentParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TraceParentParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TraceParentParseError {}

impl TraceStateParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TraceStateParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TraceStateParseError {}

fn parse_traceparent_value(value: &str) -> Result<TraceParent, TraceParentParseError> {
  if value.len() != TRACEPARENT_VALUE_BYTES {
    return Err(invalid_traceparent());
  }
  if value.as_bytes().get(2) != Some(&b'-')
    || value.as_bytes().get(35) != Some(&b'-')
    || value.as_bytes().get(52) != Some(&b'-')
  {
    return Err(invalid_traceparent());
  }

  let version = &value[0..2];
  let trace_id = &value[3..35];
  let parent_id = &value[36..52];
  let flags = &value[53..55];
  if version == "ff" || version != "00" {
    return Err(TraceParentParseError::new(
      "unsupported traceparent version",
    ));
  }
  if !is_lower_hex(version)
    || !is_lower_hex(trace_id)
    || !is_lower_hex(parent_id)
    || !is_lower_hex(flags)
  {
    return Err(invalid_traceparent());
  }
  if all_zero_hex(trace_id) {
    return Err(TraceParentParseError::new(
      "traceparent trace-id must not be all zero",
    ));
  }
  if all_zero_hex(parent_id) {
    return Err(TraceParentParseError::new(
      "traceparent parent-id must not be all zero",
    ));
  }

  Ok(TraceParent {
    version: version.to_string(),
    trace_id: trace_id.to_string(),
    parent_id: parent_id.to_string(),
    flags: flags.to_string(),
  })
}

fn parse_tracestate_value(value: &str) -> Result<TraceState, TraceStateParseError> {
  if value.len() > MAX_TRACESTATE_VALUE_BYTES {
    return Err(TraceStateParseError::new(
      "tracestate header value is too large",
    ));
  }

  let mut members = Vec::new();
  for raw_member in value.split(',') {
    let member = raw_member.trim_matches([' ', '\t']);
    if member.is_empty() {
      continue;
    }
    if members.len() >= MAX_TRACESTATE_MEMBERS {
      return Err(TraceStateParseError::new("too many tracestate members"));
    }

    let (key, value) = member
      .split_once('=')
      .ok_or_else(|| TraceStateParseError::new("invalid tracestate member"))?;
    if !is_valid_tracestate_key(key) {
      return Err(TraceStateParseError::new("invalid tracestate key"));
    }
    if members
      .iter()
      .any(|known: &TraceStateMember| known.key == key)
    {
      return Err(TraceStateParseError::new("duplicate tracestate key"));
    }
    if !is_valid_tracestate_member_value(value) {
      return Err(TraceStateParseError::new("invalid tracestate value"));
    }

    members.push(TraceStateMember {
      key: key.to_string(),
      value: value.to_string(),
    });
  }

  Ok(TraceState { members })
}

fn is_valid_tracestate_key(key: &str) -> bool {
  if key.is_empty() || key.len() > MAX_TRACESTATE_KEY_BYTES {
    return false;
  }
  let mut parts = key.split('@');
  let first = parts.next().unwrap_or_default();
  let second = parts.next();
  if parts.next().is_some() {
    return false;
  }
  if let Some(second) = second {
    is_tracestate_tenant_id(first) && is_tracestate_system_id(second)
  } else {
    is_tracestate_simple_key(first)
  }
}

fn is_tracestate_simple_key(key: &str) -> bool {
  let Some(first) = key.as_bytes().first().copied() else {
    return false;
  };
  is_lower_alpha(first) && key.bytes().all(is_tracestate_key_char)
}

fn is_tracestate_tenant_id(tenant_id: &str) -> bool {
  let Some(first) = tenant_id.as_bytes().first().copied() else {
    return false;
  };
  matches!(first, b'a'..=b'z' | b'0'..=b'9')
    && tenant_id.len() <= 241
    && tenant_id.bytes().all(is_tracestate_key_char)
}

fn is_tracestate_system_id(system_id: &str) -> bool {
  let Some(first) = system_id.as_bytes().first().copied() else {
    return false;
  };
  is_lower_alpha(first) && system_id.len() <= 14 && system_id.bytes().all(is_tracestate_key_char)
}

fn is_lower_alpha(byte: u8) -> bool {
  byte.is_ascii_lowercase()
}

fn is_tracestate_key_char(byte: u8) -> bool {
  matches!(
    byte,
    b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'*' | b'/'
  )
}

fn is_valid_tracestate_member_value(value: &str) -> bool {
  let Some(last) = value.as_bytes().last().copied() else {
    return false;
  };
  value.len() <= MAX_TRACESTATE_MEMBER_VALUE_BYTES
    && is_tracestate_nonblank_value_char(last)
    && value.bytes().all(is_tracestate_value_char)
}

fn is_tracestate_value_char(byte: u8) -> bool {
  byte == b' ' || is_tracestate_nonblank_value_char(byte)
}

fn is_tracestate_nonblank_value_char(byte: u8) -> bool {
  matches!(byte, 0x21..=0x2b | 0x2d..=0x3c | 0x3e..=0x7e)
}

fn is_lower_hex(value: &str) -> bool {
  value
    .bytes()
    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn all_zero_hex(value: &str) -> bool {
  value.bytes().all(|byte| byte == b'0')
}

fn invalid_traceparent() -> TraceParentParseError {
  TraceParentParseError::new("invalid traceparent header value")
}
