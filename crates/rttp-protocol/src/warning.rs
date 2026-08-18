//! Bounded, policy-free RFC 7234 `Warning` response metadata parsing.
//!
//! This module validates `warning-value` list syntax only. Any well-formed
//! 3-digit warn-code is accepted; registered codes such as `110` are ordinary
//! opaque metadata. Callers own cache freshness, stale-response handling,
//! retry, redirect, and response-acceptance policy.
//!
//! ```
//! use rttp_protocol::warning::Warning;
//!
//! let warning = Warning::parse(
//!   r#"110 - "Response is Stale", 299 example.com:80 "Deprecated API" "Wed, 21 Oct 2015 07:28:00 GMT""#,
//! )
//! .expect("valid Warning");
//! assert_eq!(warning.items()[0].code(), 110);
//! assert_eq!(warning.items()[0].agent(), "-");
//! assert_eq!(warning.items()[0].text(), "Response is Stale");
//! assert_eq!(warning.items()[1].code(), 299);
//! assert_eq!(warning.items()[1].agent(), "example.com:80");
//! assert_eq!(warning.items()[1].text(), "Deprecated API");
//! assert!(warning.items()[1].date().is_some());
//! ```

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

use crate::http1::{is_qdtext, is_quoted_pair_char};

/// Maximum bytes accepted in a `Warning` field value.
pub const MAX_WARNING_VALUE_BYTES: usize = 64 * 1024;
/// Maximum warning-value members accepted across the combined field set.
pub const MAX_WARNING_ITEMS: usize = 256;
/// Maximum bytes accepted in a single unescaped warn-agent.
pub const MAX_WARNING_AGENT_BYTES: usize = 64 * 1024;
/// Maximum bytes accepted in a single unescaped warn-text.
pub const MAX_WARNING_TEXT_BYTES: usize = 64 * 1024;

/// RFC 7234 warn-code for a stale response.
pub const CODE_RESPONSE_IS_STALE: u16 = 110;
/// RFC 7234 warn-code for a failed revalidation.
pub const CODE_REVALIDATION_FAILED: u16 = 111;
/// RFC 7234 warn-code for disconnected operation.
pub const CODE_DISCONNECTED_OPERATION: u16 = 112;
/// RFC 7234 warn-code for heuristic expiration.
pub const CODE_HEURISTIC_EXPIRATION: u16 = 113;
/// RFC 7234 warn-code for a miscellaneous warning.
pub const CODE_MISCELLANEOUS_WARNING: u16 = 199;
/// RFC 7234 warn-code for an applied transformation.
pub const CODE_TRANSFORMATION_APPLIED: u16 = 214;
/// RFC 7234 warn-code for a miscellaneous persistent warning.
pub const CODE_MISCELLANEOUS_PERSISTENT_WARNING: u16 = 299;

/// Parsed, bounded `Warning` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Warning {
  items: Vec<WarningValue>,
}

/// A single RFC 7234 `warning-value`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarningValue {
  code: u16,
  agent: String,
  text: String,
  date: Option<SystemTime>,
}

/// An error returned when `Warning` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarningParseError {
  message: String,
}

impl WarningParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for WarningParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for WarningParseError {}

impl Warning {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, WarningParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, WarningParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut items = Vec::new();
    for value in values {
      if value.len() > MAX_WARNING_VALUE_BYTES {
        return Err(WarningParseError::new("Warning header value is too large"));
      }
      parse_field(value, &mut items)?;
    }
    if items.is_empty() {
      return Err(WarningParseError::new("invalid Warning value"));
    }
    Ok(Self { items })
  }

  pub fn items(&self) -> &[WarningValue] {
    &self.items
  }

  pub fn len(&self) -> usize {
    self.items.len()
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .items
      .iter()
      .map(WarningValue::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl WarningValue {
  pub fn code(&self) -> u16 {
    self.code
  }

  pub fn agent(&self) -> &str {
    &self.agent
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn date(&self) -> Option<SystemTime> {
    self.date
  }

  pub fn header_value(&self) -> String {
    let mut value = format!(
      "{:03} {} \"{}\"",
      self.code,
      self.agent,
      escape_quoted(&self.text)
    );
    if let Some(date) = self.date {
      value.push_str(" \"");
      value.push_str(&httpdate::fmt_http_date(date));
      value.push('"');
    }
    value
  }

  pub fn warning_value(&self) -> String {
    self.header_value()
  }
}

fn parse_field(value: &str, items: &mut Vec<WarningValue>) -> Result<(), WarningParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(WarningParseError::new("invalid Warning value"));
  }
  loop {
    if items.len() >= MAX_WARNING_ITEMS {
      return Err(WarningParseError::new("too many Warning values"));
    }
    let item = parse_warning_value(value, &mut position)?;
    items.push(item);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(WarningParseError::new("invalid Warning value"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(WarningParseError::new("invalid Warning value"));
    }
  }
}

fn parse_warning_value(
  value: &str,
  position: &mut usize,
) -> Result<WarningValue, WarningParseError> {
  let bytes = value.as_bytes();
  let code = parse_warn_code(bytes, position)?;
  require_ows(bytes, position)?;
  let agent = parse_agent(value, position)?;
  if agent.len() > MAX_WARNING_AGENT_BYTES {
    return Err(WarningParseError::new("Warning agent is too large"));
  }
  require_ows(bytes, position)?;
  let text = parse_quoted_string(value, position)?;
  if text.len() > MAX_WARNING_TEXT_BYTES {
    return Err(WarningParseError::new("Warning text is too large"));
  }
  let date = if matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    skip_ows(bytes, position);
    if bytes.get(*position) == Some(&b'"') {
      let date = parse_quoted_http_date(value, position)?;
      skip_ows(bytes, position);
      Some(date)
    } else {
      None
    }
  } else {
    None
  };
  Ok(WarningValue {
    code,
    agent: agent.to_string(),
    text,
    date,
  })
}

fn parse_warn_code(bytes: &[u8], position: &mut usize) -> Result<u16, WarningParseError> {
  let start = *position;
  while *position < bytes.len() && bytes[*position].is_ascii_digit() {
    *position += 1;
  }
  if *position - start != 3 {
    return Err(WarningParseError::new("invalid Warning code"));
  }
  let code = std::str::from_utf8(&bytes[start..*position])
    .expect("ASCII digits are valid UTF-8")
    .parse::<u16>()
    .expect("3 ASCII digits fit in u16");
  Ok(code)
}

fn parse_agent<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, WarningParseError> {
  let start = *position;
  let bytes = value.as_bytes();
  while *position < bytes.len() && !matches!(bytes[*position], b' ' | b'\t') {
    *position += 1;
  }
  if *position == start {
    return Err(WarningParseError::new("invalid Warning agent"));
  }
  Ok(&value[start..*position])
}

fn parse_quoted_http_date(
  value: &str,
  position: &mut usize,
) -> Result<SystemTime, WarningParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'"') {
    return Err(WarningParseError::new("invalid Warning HTTP-date"));
  }
  *position += 1;
  let start = *position;
  while *position < bytes.len() && bytes[*position] != b'"' {
    *position += 1;
  }
  if *position == bytes.len() {
    return Err(WarningParseError::new("invalid Warning HTTP-date"));
  }
  let date = &value[start..*position];
  *position += 1;
  httpdate::parse_http_date(date).map_err(|_| WarningParseError::new("invalid Warning HTTP-date"))
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, WarningParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'"') {
    return Err(WarningParseError::new("invalid Warning quoted-string"));
  }
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = bytes.get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed)
          .map_err(|_| WarningParseError::new("invalid Warning quoted-string"));
      }
      b'\\' => {
        let Some(&escaped) = bytes.get(*position) else {
          return Err(WarningParseError::new("invalid Warning quoted-string"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(WarningParseError::new("invalid Warning quoted-string"));
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_qdtext(byte) => parsed.push(byte),
      _ => return Err(WarningParseError::new("invalid Warning quoted-string")),
    }
  }
  Err(WarningParseError::new("invalid Warning quoted-string"))
}

fn require_ows(bytes: &[u8], position: &mut usize) -> Result<(), WarningParseError> {
  if !matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    return Err(WarningParseError::new("invalid Warning value"));
  }
  skip_ows(bytes, position);
  Ok(())
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while *position < bytes.len() && matches!(bytes[*position], b' ' | b'\t') {
    *position += 1;
  }
}

fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}
