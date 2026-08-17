//! Bounded, policy-free `Strict-Transport-Security` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply HSTS. Unparsable input is an error; this parser never
//! enables HTTPS-only navigation, host storage, or preload-list policy.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::http1::{is_qdtext, is_quoted_pair_char, is_token_byte};

/// Maximum bytes accepted in a `Strict-Transport-Security` field value.
pub const MAX_STRICT_TRANSPORT_SECURITY_VALUE_BYTES: usize = 64 * 1024;
/// Maximum semicolon-separated slots accepted in one field, including empty slots.
pub const MAX_STRICT_TRANSPORT_SECURITY_DIRECTIVES: usize = 256;

/// Parsed, bounded `Strict-Transport-Security` response metadata.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StrictTransportSecurity {
  max_age: u64,
  include_sub_domains: bool,
  preload: bool,
}

impl StrictTransportSecurity {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, StrictTransportSecurityParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, StrictTransportSecurityParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_field(value)
  }

  pub const fn max_age(self) -> u64 {
    self.max_age
  }

  pub const fn include_sub_domains(self) -> bool {
    self.include_sub_domains
  }

  pub const fn preload(self) -> bool {
    self.preload
  }

  pub fn header_value(self) -> String {
    match (self.include_sub_domains, self.preload) {
      (false, false) => format!("max-age={}", self.max_age),
      (true, false) => format!("max-age={}; includeSubDomains", self.max_age),
      (false, true) => format!("max-age={}; preload", self.max_age),
      (true, true) => format!("max-age={}; includeSubDomains; preload", self.max_age),
    }
  }
}

/// An error returned when `Strict-Transport-Security` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrictTransportSecurityParseError {
  message: String,
}

impl StrictTransportSecurityParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for StrictTransportSecurityParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for StrictTransportSecurityParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, StrictTransportSecurityParseError>
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
    return Err(StrictTransportSecurityParseError::new(
      "duplicate Strict-Transport-Security header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), StrictTransportSecurityParseError> {
  if value.len() > MAX_STRICT_TRANSPORT_SECURITY_VALUE_BYTES {
    return Err(StrictTransportSecurityParseError::new(
      "Strict-Transport-Security header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn parse_field(value: &str) -> Result<StrictTransportSecurity, StrictTransportSecurityParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  let mut slot_count = 0usize;
  let mut seen = HashSet::new();
  let mut max_age = None;
  let mut include_sub_domains = false;
  let mut preload = false;

  loop {
    if slot_count >= MAX_STRICT_TRANSPORT_SECURITY_DIRECTIVES {
      return Err(StrictTransportSecurityParseError::new(
        "too many Strict-Transport-Security directives",
      ));
    }
    slot_count += 1;
    skip_ows(bytes, &mut position);
    if bytes.get(position).is_some_and(|byte| is_token_byte(*byte)) {
      apply_directive(
        parse_directive(value, &mut position)?,
        &mut seen,
        &mut max_age,
        &mut include_sub_domains,
        &mut preload,
      )?;
    }
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      break;
    }
    if bytes[position] != b';' {
      return Err(invalid_value());
    }
    position += 1;
  }

  let Some(max_age) = max_age else {
    return Err(invalid_value());
  };
  Ok(StrictTransportSecurity {
    max_age,
    include_sub_domains,
    preload,
  })
}

struct ParsedDirective {
  name: String,
  value: Option<String>,
}

fn parse_directive(
  value: &str,
  position: &mut usize,
) -> Result<ParsedDirective, StrictTransportSecurityParseError> {
  let name = parse_token(value, position)?.to_string();
  skip_ows(value.as_bytes(), position);
  let value = if value.as_bytes().get(*position) == Some(&b'=') {
    *position += 1;
    skip_ows(value.as_bytes(), position);
    Some(parse_directive_value(value, position)?)
  } else {
    None
  };
  Ok(ParsedDirective { name, value })
}

fn parse_directive_value(
  value: &str,
  position: &mut usize,
) -> Result<String, StrictTransportSecurityParseError> {
  if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)
  } else {
    Ok(parse_token(value, position)?.to_string())
  }
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
) -> Result<String, StrictTransportSecurityParseError> {
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed).map_err(|_| invalid_value());
      }
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(invalid_value());
        };
        if !is_quoted_pair_char(escaped) {
          return Err(invalid_value());
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_qdtext(byte) => parsed.push(byte),
      _ => return Err(invalid_value()),
    }
  }
  Err(invalid_value())
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
) -> Result<&'a str, StrictTransportSecurityParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(invalid_value())
  } else {
    Ok(&value[start..*position])
  }
}

fn apply_directive(
  directive: ParsedDirective,
  seen: &mut HashSet<String>,
  max_age: &mut Option<u64>,
  include_sub_domains: &mut bool,
  preload: &mut bool,
) -> Result<(), StrictTransportSecurityParseError> {
  let name_key = directive.name.to_ascii_lowercase();
  if !seen.insert(name_key) {
    return Err(StrictTransportSecurityParseError::new(
      "duplicate Strict-Transport-Security directive",
    ));
  }

  if directive.name.eq_ignore_ascii_case("max-age") {
    let Some(value) = directive.value else {
      return Err(invalid_value());
    };
    *max_age = Some(parse_max_age(&value)?);
    return Ok(());
  }
  if directive.name.eq_ignore_ascii_case("includesubdomains") {
    if directive.value.is_some() {
      return Err(invalid_value());
    }
    *include_sub_domains = true;
    return Ok(());
  }
  if directive.name.eq_ignore_ascii_case("preload") {
    if directive.value.is_some() {
      return Err(invalid_value());
    }
    *preload = true;
    return Ok(());
  }
  Ok(())
}

fn parse_max_age(value: &str) -> Result<u64, StrictTransportSecurityParseError> {
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_value());
  }
  value.parse().map_err(|_| invalid_value())
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn invalid_value() -> StrictTransportSecurityParseError {
  StrictTransportSecurityParseError::new("invalid Strict-Transport-Security header value")
}
