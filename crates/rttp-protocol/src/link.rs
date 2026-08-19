//! Bounded, policy-free parsing for the HTTP `Link` response field (RFC 8288).
//!
//! This module parses one or more `Link` field values into ordered
//! [`LinkValue`] items. Each value retains its target URI-reference and its
//! ordered parameters, including unknown extension parameters alongside
//! `rel`. Targets are validated structurally as RFC 3986 URI-references and
//! stored as raw text; they are never resolved, normalized, fetched, or
//! preloaded, and fragments are allowed.
//!
//! Each field value is bounded to [`MAX_LINK_VALUE_BYTES`], the cumulative
//! value count is bounded to [`MAX_LINK_VALUES`], each value holds at most
//! [`MAX_LINK_PARAMETERS`] parameters, and each parameter value is bounded to
//! [`MAX_LINK_PARAMETER_VALUE_BYTES`]. Parameter names are matched
//! case-insensitively, stored lowercase, and must be unique within a value.
//! Quoted parameter values are unescaped; valueless parameters are preserved
//! with an empty value. A present field set that yields no value still fails
//! as invalid.
//!
//! Parsing is syntax validation only: this module does not implement preload,
//! fetch scheduling, redirects, cache policy, or route generation.
//!
//! # Examples
//!
//! ```
//! use rttp_protocol::link::LinkValues;
//!
//! let links = LinkValues::parse(
//!   "</style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload",
//! )
//! .unwrap();
//! assert_eq!(links.values()[0].target(), "/style.css");
//! assert_eq!(links.values()[0].parameter("rel"), Some("preload"));
//! assert_eq!(links.values()[1].parameter("rel"), Some("modulepreload"));
//! ```

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a single `Link` field value.
pub const MAX_LINK_VALUE_BYTES: usize = 64 * 1024;

/// Maximum cumulative `Link` values across all supplied fields.
pub const MAX_LINK_VALUES: usize = 256;

/// Maximum parameters retained on a single `Link` value.
pub const MAX_LINK_PARAMETERS: usize = 256;

/// Maximum bytes accepted in a single `Link` parameter value.
pub const MAX_LINK_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Bounded `Link` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkValues {
  values: Vec<LinkValue>,
}

/// A single parsed `Link` value with its target and ordered parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkValue {
  target: String,
  parameters: Vec<LinkParameter>,
}

/// A single parsed `Link` parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkParameter {
  name: String,
  value: String,
}

/// An error returned when `Link` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkParseError {
  message: String,
}

impl LinkValues {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, LinkParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, LinkParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parsed = Vec::new();
    for value in values {
      if value.len() > MAX_LINK_VALUE_BYTES {
        return Err(LinkParseError::new("Link header value is too large"));
      }
      for member in split_members(value, b',')? {
        if parsed.len() >= MAX_LINK_VALUES {
          return Err(LinkParseError::new("too many Link values"));
        }
        parsed.push(parse_member(&member)?);
      }
    }
    if parsed.is_empty() {
      return Err(LinkParseError::new("invalid Link value"));
    }
    Ok(Self { values: parsed })
  }

  pub fn values(&self) -> &[LinkValue] {
    &self.values
  }

  pub fn len(&self) -> usize {
    self.values.len()
  }

  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }
}

impl LinkValue {
  pub fn target(&self) -> &str {
    &self.target
  }

  pub fn parameters(&self) -> &[LinkParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(LinkParameter::value)
  }
}

impl LinkParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

impl LinkParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for LinkParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for LinkParseError {}

fn parse_member(member: &str) -> Result<LinkValue, LinkParseError> {
  let member = member.trim();
  let Some(target_and_tail) = member.strip_prefix('<') else {
    return Err(LinkParseError::new("invalid Link target"));
  };
  let Some(target_end) = target_and_tail.find('>') else {
    return Err(LinkParseError::new("invalid Link target"));
  };
  let target = &target_and_tail[..target_end];
  validate_target(target)?;

  let mut parameters = Vec::new();
  let tail = target_and_tail[target_end + 1..].trim();
  if !tail.is_empty() {
    if !tail.starts_with(';') {
      return Err(LinkParseError::new("invalid Link parameter"));
    }
    for parameter in split_members(&tail[1..], b';')? {
      if parameters.len() >= MAX_LINK_PARAMETERS {
        return Err(LinkParseError::new("too many Link parameters"));
      }
      let parameter = parse_parameter(&parameter)?;
      if parameters
        .iter()
        .any(|known: &LinkParameter| known.name.eq_ignore_ascii_case(&parameter.name))
      {
        return Err(LinkParseError::new("duplicate Link parameter"));
      }
      parameters.push(parameter);
    }
  }
  Ok(LinkValue {
    target: target.to_string(),
    parameters,
  })
}

fn parse_parameter(value: &str) -> Result<LinkParameter, LinkParseError> {
  let (name, value) = value.split_once('=').unwrap_or((value, ""));
  let name = name.trim();
  let value = value.trim();
  if !is_token(name) {
    return Err(LinkParseError::new("invalid Link parameter name"));
  }
  if value.len() > MAX_LINK_PARAMETER_VALUE_BYTES {
    return Err(LinkParseError::new("Link parameter value is too large"));
  }
  let value = if value.is_empty() {
    String::new()
  } else {
    parse_parameter_value(value)?
  };
  Ok(LinkParameter {
    name: name.to_ascii_lowercase(),
    value,
  })
}

fn parse_parameter_value(value: &str) -> Result<String, LinkParseError> {
  if let Some(value) = value.strip_prefix('"') {
    return parse_quoted_string(value)
      .map_err(|_| LinkParseError::new("invalid Link quoted-string"));
  }
  if value.contains('"') || !is_token(value) {
    return Err(LinkParseError::new("invalid Link parameter value"));
  }
  Ok(value.to_string())
}

fn parse_quoted_string(value: &str) -> Result<String, LinkParseError> {
  let mut chars = value.chars();
  let mut parsed = String::new();
  let mut closed = false;

  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(LinkParseError::new("invalid Link quoted-string"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(LinkParseError::new("invalid Link quoted-string"));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => {
        return Err(LinkParseError::new("invalid Link quoted-string"));
      }
    }
  }

  if !closed || chars.any(|ch| !ch.is_ascii_whitespace()) {
    return Err(LinkParseError::new("invalid Link quoted-string"));
  }
  Ok(parsed)
}

fn validate_target(target: &str) -> Result<(), LinkParseError> {
  if target.is_empty()
    || target
      .bytes()
      .any(|byte| byte.is_ascii_control() || matches!(byte, b'<' | b'>'))
  {
    return Err(LinkParseError::new("invalid Link target"));
  }
  let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
  Url::options()
    .base_url(Some(&base))
    .parse(target)
    .map_err(|_| LinkParseError::new("invalid Link target"))?;
  Ok(())
}

fn split_members(value: &str, delimiter: u8) -> Result<Vec<String>, LinkParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut quoted = false;
  let mut escaped = false;
  let mut in_target = false;
  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' if !in_target => quoted = !quoted,
      b'<' if !quoted => in_target = true,
      b'>' if !quoted => in_target = false,
      byte if byte == delimiter && !quoted && !in_target => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err(LinkParseError::new("invalid Link value"));
        }
        members.push(member.to_string());
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped || in_target {
    return Err(LinkParseError::new("invalid Link value"));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(LinkParseError::new("invalid Link value"));
  }
  members.push(member.to_string());
  Ok(members)
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'!' | b'#'
      | b'$'
      | b'%'
      | b'&'
      | b'\''
      | b'*'
      | b'+'
      | b'-'
      | b'.'
      | b'^'
      | b'_'
      | b'`'
      | b'|'
      | b'~'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

fn is_qdtext(ch: char) -> bool {
  matches!(ch, '\t' | ' ' | '!' | '#'..='[' | ']'..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn is_quoted_pair_char(ch: char) -> bool {
  matches!(ch, '\t' | ' '..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}
