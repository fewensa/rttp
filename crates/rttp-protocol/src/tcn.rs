//! Bounded, policy-free RFC 2295 `TCN` response metadata parsing.
//!
//! This module validates the `TCN` response field as a singleton, ordered list
//! of transparent-content-negotiation result tokens. It exposes metadata only:
//! callers decide whether and how to apply negotiation, variant, or cache
//! policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `TCN` field value.
pub const MAX_TCN_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across supplied fields.
pub const MAX_TCN_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum result tokens accepted in the `TCN` field.
pub const MAX_TCN_MEMBERS: usize = 32;

/// One parsed RFC 2295 `TCN` result token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TcnDirective {
  /// The `list` response type.
  List,
  /// The `choice` response type.
  Choice,
  /// The `adhoc` response type.
  Adhoc,
  /// The `re-choose` negotiation directive.
  ReChoose,
  /// The `keep` negotiation directive.
  Keep,
}

impl TcnDirective {
  fn header_value(&self) -> &'static str {
    match self {
      Self::List => "list",
      Self::Choice => "choice",
      Self::Adhoc => "adhoc",
      Self::ReChoose => "re-choose",
      Self::Keep => "keep",
    }
  }
}

/// Parsed, bounded RFC 2295 `TCN` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tcn {
  members: Vec<TcnDirective>,
}

impl Tcn {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TcnParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TcnParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;
    let mut seen_field = false;

    for value in values {
      if seen_field {
        return Err(TcnParseError::new("duplicate TCN header field"));
      }
      seen_field = true;
      if value.len() > MAX_TCN_VALUE_BYTES {
        return Err(TcnParseError::new("TCN header value is too large"));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_TCN_TOTAL_BYTES {
        return Err(TcnParseError::new("TCN header list is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(TcnParseError::new("invalid TCN control byte"));
      }
      for member in value.split(',') {
        let member = parse_tcn_directive(member)?;
        if members.contains(&member) {
          return Err(TcnParseError::new("duplicate TCN directive"));
        }
        if members.len() >= MAX_TCN_MEMBERS {
          return Err(TcnParseError::new("too many TCN members"));
        }
        members.push(member);
      }
    }

    if members.is_empty() {
      return Err(TcnParseError::new("invalid TCN directive"));
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[TcnDirective] {
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
      .map(TcnDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// An error returned when `TCN` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TcnParseError {
  message: String,
}

impl TcnParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TcnParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TcnParseError {}

fn parse_tcn_directive(member: &str) -> Result<TcnDirective, TcnParseError> {
  match member
    .trim_matches([' ', '\t'])
    .to_ascii_lowercase()
    .as_str()
  {
    "list" => Ok(TcnDirective::List),
    "choice" => Ok(TcnDirective::Choice),
    "adhoc" => Ok(TcnDirective::Adhoc),
    "re-choose" => Ok(TcnDirective::ReChoose),
    "keep" => Ok(TcnDirective::Keep),
    _ => Err(TcnParseError::new("invalid TCN directive")),
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
