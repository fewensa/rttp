//! Bounded, policy-free RFC 2295 `Negotiate` request metadata parsing.
//!
//! This module validates one or more `Negotiate` field values as an ordered
//! list of negotiate-directives. Callers decide whether to select a variant,
//! run transparent content negotiation, or apply TCN policy.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

/// Maximum bytes accepted in one `Negotiate` field value.
pub const MAX_NEGOTIATE_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across all supplied fields.
pub const MAX_NEGOTIATE_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum negotiate-directives accepted across the combined list.
pub const MAX_NEGOTIATE_MEMBERS: usize = 32;

/// One parsed RFC 2295 `Negotiate` directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NegotiateDirective {
  /// The `trans` flag: only the result of transparent content negotiation is
  /// acceptable.
  Trans,
  /// The `vlist` flag: only the variant list is acceptable.
  Vlist,
  /// The `guess-small` flag: only the smallest variant is acceptable.
  GuessSmall,
  /// The `*` wildcard: any representation is acceptable.
  Any,
  /// A remote variant selection algorithm version `major.minor`.
  RvsaVersion { major: u64, minor: u64 },
  /// An extension directive `token` or `token=token`.
  Extension { name: String, value: Option<String> },
}

impl NegotiateDirective {
  fn header_value(&self) -> String {
    match self {
      Self::Trans => "trans".to_owned(),
      Self::Vlist => "vlist".to_owned(),
      Self::GuessSmall => "guess-small".to_owned(),
      Self::Any => "*".to_owned(),
      Self::RvsaVersion { major, minor } => format!("{major}.{minor}"),
      Self::Extension { name, value: None } => name.clone(),
      Self::Extension {
        name,
        value: Some(value),
      } => format!("{name}={value}"),
    }
  }
}

/// Parsed, bounded RFC 2295 `Negotiate` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Negotiate {
  members: Vec<NegotiateDirective>,
}

impl Negotiate {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, NegotiateParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, NegotiateParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      if value.len() > MAX_NEGOTIATE_VALUE_BYTES {
        return Err(NegotiateParseError::new(
          "Negotiate header value is too large",
        ));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_NEGOTIATE_TOTAL_BYTES {
        return Err(NegotiateParseError::new(
          "Negotiate header list is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(NegotiateParseError::new("invalid Negotiate control byte"));
      }
      for member in value.split(',') {
        let member = parse_negotiate_directive(member)?;
        if members.iter().any(|known| is_duplicate(known, &member)) {
          return Err(NegotiateParseError::new("duplicate Negotiate directive"));
        }
        if members.len() >= MAX_NEGOTIATE_MEMBERS {
          return Err(NegotiateParseError::new("too many Negotiate members"));
        }
        members.push(member);
      }
    }

    if members.is_empty() {
      return Err(NegotiateParseError::new("invalid Negotiate member"));
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[NegotiateDirective] {
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
      .map(NegotiateDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// An error returned when `Negotiate` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NegotiateParseError {
  message: String,
}

impl NegotiateParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for NegotiateParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for NegotiateParseError {}

fn parse_negotiate_directive(member: &str) -> Result<NegotiateDirective, NegotiateParseError> {
  let member = member.trim_matches([' ', '\t']);
  if member.eq_ignore_ascii_case("trans") {
    return Ok(NegotiateDirective::Trans);
  }
  if member.eq_ignore_ascii_case("vlist") {
    return Ok(NegotiateDirective::Vlist);
  }
  if member.eq_ignore_ascii_case("guess-small") {
    return Ok(NegotiateDirective::GuessSmall);
  }
  if member == "*" {
    return Ok(NegotiateDirective::Any);
  }
  if let Some((name, value)) = member.split_once('=') {
    return parse_extension(name, value);
  }
  if is_rvsa_version(member) {
    return parse_rvsa_version(member);
  }
  if !is_token(member) {
    return Err(NegotiateParseError::new("invalid Negotiate directive"));
  }
  Ok(NegotiateDirective::Extension {
    name: member.to_string(),
    value: None,
  })
}

fn parse_extension(name: &str, value: &str) -> Result<NegotiateDirective, NegotiateParseError> {
  let name = name.trim_matches([' ', '\t']);
  let value = value.trim_matches([' ', '\t']);
  if is_known_flag(name) || is_rvsa_version(name) || !is_token(name) || !is_token(value) {
    return Err(NegotiateParseError::new("invalid Negotiate directive"));
  }
  Ok(NegotiateDirective::Extension {
    name: name.to_string(),
    value: Some(value.to_string()),
  })
}

fn is_known_flag(name: &str) -> bool {
  name.eq_ignore_ascii_case("trans")
    || name.eq_ignore_ascii_case("vlist")
    || name.eq_ignore_ascii_case("guess-small")
    || name == "*"
}

fn is_rvsa_version(member: &str) -> bool {
  let Some((major, minor)) = member.split_once('.') else {
    return false;
  };
  !major.is_empty()
    && !minor.is_empty()
    && !minor.contains('.')
    && major.bytes().all(|byte| byte.is_ascii_digit())
    && minor.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_rvsa_version(member: &str) -> Result<NegotiateDirective, NegotiateParseError> {
  let (major, minor) = member
    .split_once('.')
    .expect("rvsa-version shape was validated");
  let major = major
    .parse::<u64>()
    .map_err(|_| NegotiateParseError::new("Negotiate version overflow"))?;
  let minor = minor
    .parse::<u64>()
    .map_err(|_| NegotiateParseError::new("Negotiate version overflow"))?;
  Ok(NegotiateDirective::RvsaVersion { major, minor })
}

fn is_duplicate(known: &NegotiateDirective, candidate: &NegotiateDirective) -> bool {
  if let NegotiateDirective::Extension { name, .. } = candidate {
    matches!(known, NegotiateDirective::Extension { name: known_name, .. }
      if known_name.eq_ignore_ascii_case(name))
  } else {
    known == candidate
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
