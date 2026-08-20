//! Bounded HTTP `Via` request and response metadata parsing.
//!
//! This module validates `Via` list syntax and bounds only. It does not append
//! or remove hops, infer trusted proxies, rewrite request identity, or change
//! HTTP/1.1 or HTTP/2 proxy policy.

use std::error::Error;
use std::fmt;

use crate::host::Host;
use crate::http1::{is_token, is_token_byte};

/// Maximum bytes accepted in one `Via` field value, in the combined raw
/// field set including `", "` separator overhead, and in the combined
/// serialized field value.
pub const MAX_VIA_VALUE_BYTES: usize = 64 * 1024;
/// Maximum `Via` list-members accepted across all fields.
pub const MAX_VIA_MEMBERS: usize = 256;

/// Parsed, bounded HTTP `Via` hop-chain metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Via {
  members: Vec<ViaMember>,
}

/// One ordered HTTP `Via` received-protocol / received-by hop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViaMember {
  protocol_name: Option<String>,
  protocol_version: String,
  received_by: String,
  comment: Option<String>,
}

/// An error returned when `Via` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViaParseError {
  message: String,
}

impl ViaParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ViaParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ViaParseError {}

impl Via {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ViaParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ViaParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_VIA_VALUE_BYTES {
        return Err(ViaParseError::new("Via header value is too large"));
      }
      let separator = if total_bytes > 0 { 2 } else { 0 };
      total_bytes = total_bytes
        .saturating_add(separator)
        .saturating_add(value.len());
      if total_bytes > MAX_VIA_VALUE_BYTES {
        return Err(ViaParseError::new("Via header value is too large"));
      }
      if value
        .bytes()
        .any(|byte| byte.is_ascii_control() && byte != b'\t')
      {
        return Err(ViaParseError::new("invalid Via control byte"));
      }
      parse_field(value, &mut members)?;
    }
    if members.is_empty() {
      return Err(ViaParseError::new("invalid Via member"));
    }
    let via = Via { members };
    if via.header_value().len() > MAX_VIA_VALUE_BYTES {
      return Err(ViaParseError::new("Via header value is too large"));
    }
    Ok(via)
  }

  pub fn members(&self) -> &[ViaMember] {
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
      .map(ViaMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ViaMember {
  pub fn protocol_name(&self) -> Option<&str> {
    self.protocol_name.as_deref()
  }

  pub fn protocol_version(&self) -> &str {
    &self.protocol_version
  }

  pub fn received_by(&self) -> &str {
    &self.received_by
  }

  pub fn comment(&self) -> Option<&str> {
    self.comment.as_deref()
  }

  fn header_value(&self) -> String {
    let protocol = match &self.protocol_name {
      Some(name) => format!("{name}/{}", self.protocol_version),
      None => self.protocol_version.clone(),
    };
    match &self.comment {
      Some(comment) => format!("{protocol} {} ({comment})", self.received_by),
      None => format!("{protocol} {}", self.received_by),
    }
  }
}

fn parse_field(value: &str, members: &mut Vec<ViaMember>) -> Result<(), ViaParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ViaParseError::new("invalid Via member"));
  }

  loop {
    if members.len() >= MAX_VIA_MEMBERS {
      return Err(ViaParseError::new("too many Via members"));
    }
    let member = parse_member(value, &mut position)?;
    members.push(member);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(ViaParseError::new("invalid Via member"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ViaParseError::new("invalid Via member"));
    }
  }
}

fn parse_member(value: &str, position: &mut usize) -> Result<ViaMember, ViaParseError> {
  let (protocol_name, protocol_version) = parse_received_protocol(value, position)?;
  if !skip_required_ows(value.as_bytes(), position) {
    return Err(ViaParseError::new("invalid Via received-protocol"));
  }
  let received_by = parse_received_by(value, position)?;
  let comment = parse_optional_comment(value, position)?;
  Ok(ViaMember {
    protocol_name,
    protocol_version,
    received_by,
    comment,
  })
}

fn parse_received_protocol(
  value: &str,
  position: &mut usize,
) -> Result<(Option<String>, String), ViaParseError> {
  let first = parse_token(value, position, "invalid Via received-protocol")?;
  if value.as_bytes().get(*position) == Some(&b'/') {
    *position += 1;
    let version = parse_token(value, position, "invalid Via received-protocol")?;
    Ok((Some(first.to_string()), version.to_string()))
  } else {
    Ok((None, first.to_string()))
  }
}

fn parse_received_by(value: &str, position: &mut usize) -> Result<String, ViaParseError> {
  let start = *position;
  if value.as_bytes().get(*position) == Some(&b'[') {
    *position += 1;
    while value
      .as_bytes()
      .get(*position)
      .is_some_and(|byte| *byte != b']')
    {
      *position += 1;
    }
    if value.as_bytes().get(*position) != Some(&b']') {
      return Err(ViaParseError::new("invalid Via received-by"));
    }
    *position += 1;
    parse_optional_port(value, position)?;
  } else {
    parse_token(value, position, "invalid Via received-by")?;
    parse_optional_port(value, position)?;
  }
  let received_by = &value[start..*position];
  if !is_valid_received_by(received_by) {
    return Err(ViaParseError::new("invalid Via received-by"));
  }
  Ok(received_by.to_string())
}

fn parse_optional_port(value: &str, position: &mut usize) -> Result<(), ViaParseError> {
  if value.as_bytes().get(*position) != Some(&b':') {
    return Ok(());
  }
  *position += 1;
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| byte.is_ascii_digit())
  {
    *position += 1;
  }
  if start == *position {
    return Err(ViaParseError::new("invalid Via received-by"));
  }
  Ok(())
}

fn parse_optional_comment(
  value: &str,
  position: &mut usize,
) -> Result<Option<String>, ViaParseError> {
  let skipped = skip_ows(value.as_bytes(), position);
  if value.as_bytes().get(*position) != Some(&b'(') {
    return Ok(None);
  }
  if skipped == 0 {
    return Err(ViaParseError::new("invalid Via comment"));
  }
  Ok(Some(parse_comment(value, position)?))
}

fn parse_comment(value: &str, position: &mut usize) -> Result<String, ViaParseError> {
  if value.as_bytes().get(*position) != Some(&b'(') {
    return Err(ViaParseError::new("invalid Via comment"));
  }
  *position += 1;
  let inner_start = *position;
  parse_comment_body(value, position)?;
  Ok(value[inner_start..*position - 1].to_string())
}

fn parse_comment_body(value: &str, position: &mut usize) -> Result<(), ViaParseError> {
  let bytes = value.as_bytes();
  while let Some(&byte) = bytes.get(*position) {
    match byte {
      b')' => {
        *position += 1;
        return Ok(());
      }
      b'(' => {
        *position += 1;
        parse_comment_body(value, position)?;
      }
      b'\\' => {
        *position += 1;
        let Some(&escaped) = bytes.get(*position) else {
          return Err(ViaParseError::new("invalid Via comment"));
        };
        if !(escaped == b'\t'
          || escaped == b' '
          || (0x21..=0x7e).contains(&escaped)
          || escaped >= 0x80)
        {
          return Err(ViaParseError::new("invalid Via comment"));
        }
        *position += 1;
      }
      b'\t' | b' ' | 0x21..=0x27 | 0x2A..=0x5B | 0x5D..=0x7E => *position += 1,
      byte if byte >= 0x80 => *position += 1,
      _ => return Err(ViaParseError::new("invalid Via comment")),
    }
  }
  Err(ViaParseError::new("invalid Via comment"))
}

fn is_valid_received_by(received_by: &str) -> bool {
  if received_by.starts_with('[') {
    return Host::parse(received_by).is_ok();
  }
  if is_token(received_by) {
    return true;
  }
  received_by
    .rsplit_once(':')
    .is_some_and(|(name, port)| is_token(name) && is_port(port))
}

fn is_port(port: &str) -> bool {
  !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, ViaParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(ViaParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_required_ows(bytes: &[u8], position: &mut usize) -> bool {
  skip_ows(bytes, position) > 0
}

fn skip_ows(bytes: &[u8], position: &mut usize) -> usize {
  let start = *position;
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
  *position - start
}
