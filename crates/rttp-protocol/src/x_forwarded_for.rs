//! Bounded, policy-free parsing for `X-Forwarded-For` request metadata.
//!
//! This module validates ordered `unknown`, IPv4, and IPv6 node values only.
//! It does not decide which proxy hops are trusted, derive a client address, or
//! rewrite request identity.

use std::error::Error;
use std::fmt;
use std::net::IpAddr;

/// Maximum bytes accepted in one `X-Forwarded-For` field value, in the
/// combined raw field set including `", "` separator overhead, and in the
/// combined serialized field value.
pub const MAX_X_FORWARDED_FOR_VALUE_BYTES: usize = 64 * 1024;
/// Maximum `X-Forwarded-For` node values accepted across all fields.
pub const MAX_X_FORWARDED_FOR_NODES: usize = 256;

/// Parsed, bounded `X-Forwarded-For` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedFor {
  nodes: Vec<XForwardedForNode>,
}

/// One ordered `X-Forwarded-For` node value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedForNode {
  value: String,
  kind: XForwardedForNodeKind,
}

/// The accepted node value kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XForwardedForNodeKind {
  Ip,
  Unknown,
}

/// An error returned when `X-Forwarded-For` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedForParseError {
  message: String,
}

impl XForwardedForParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XForwardedForParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XForwardedForParseError {}

impl XForwardedFor {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XForwardedForParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XForwardedForParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut nodes = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_value(value, &mut total_bytes)?;
      parse_field(value, &mut nodes)?;
    }
    if nodes.is_empty() {
      return Err(invalid_node());
    }
    let forwarded_for = Self { nodes };
    if forwarded_for.header_value().len() > MAX_X_FORWARDED_FOR_VALUE_BYTES {
      return Err(XForwardedForParseError::new(
        "X-Forwarded-For header value is too large",
      ));
    }
    Ok(forwarded_for)
  }

  pub fn nodes(&self) -> &[XForwardedForNode] {
    &self.nodes
  }

  pub fn len(&self) -> usize {
    self.nodes.len()
  }

  pub fn is_empty(&self) -> bool {
    self.nodes.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .nodes
      .iter()
      .map(|node| node.value.as_str())
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl XForwardedForNode {
  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn kind(&self) -> XForwardedForNodeKind {
    self.kind
  }

  pub fn is_unknown(&self) -> bool {
    self.kind == XForwardedForNodeKind::Unknown
  }

  pub fn is_ip(&self) -> bool {
    self.kind == XForwardedForNodeKind::Ip
  }
}

fn validate_value(value: &str, total_bytes: &mut usize) -> Result<(), XForwardedForParseError> {
  if value.len() > MAX_X_FORWARDED_FOR_VALUE_BYTES {
    return Err(XForwardedForParseError::new(
      "X-Forwarded-For header value is too large",
    ));
  }
  let separator = if *total_bytes > 0 { 2 } else { 0 };
  *total_bytes = total_bytes
    .saturating_add(separator)
    .saturating_add(value.len());
  if *total_bytes > MAX_X_FORWARDED_FOR_VALUE_BYTES {
    return Err(XForwardedForParseError::new(
      "X-Forwarded-For header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(XForwardedForParseError::new(
      "invalid X-Forwarded-For control byte",
    ));
  }
  Ok(())
}

fn parse_field(
  value: &str,
  nodes: &mut Vec<XForwardedForNode>,
) -> Result<(), XForwardedForParseError> {
  for raw_node in value.split(',') {
    if nodes.len() >= MAX_X_FORWARDED_FOR_NODES {
      return Err(XForwardedForParseError::new(
        "too many X-Forwarded-For nodes",
      ));
    }
    let node = parse_node(raw_node.trim_matches([' ', '\t']))?;
    nodes.push(node);
  }
  Ok(())
}

fn parse_node(value: &str) -> Result<XForwardedForNode, XForwardedForParseError> {
  if value.is_empty() {
    return Err(invalid_node());
  }
  if value.eq_ignore_ascii_case("unknown") {
    return Ok(XForwardedForNode {
      value: "unknown".to_string(),
      kind: XForwardedForNodeKind::Unknown,
    });
  }
  let ip_value = value
    .strip_prefix('[')
    .and_then(|rest| rest.strip_suffix(']'))
    .unwrap_or(value);
  if ip_value.parse::<IpAddr>().is_err() {
    return Err(invalid_node());
  }
  Ok(XForwardedForNode {
    value: value.to_string(),
    kind: XForwardedForNodeKind::Ip,
  })
}

fn invalid_node() -> XForwardedForParseError {
  XForwardedForParseError::new("invalid X-Forwarded-For node")
}
