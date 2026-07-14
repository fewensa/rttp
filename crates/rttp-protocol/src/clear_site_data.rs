//! Bounded, policy-free `Clear-Site-Data` response metadata parsing.
//!
//! This module only parses directives. Callers decide whether and how to clear
//! any state; parsing never clears caches, cookies, storage, or execution contexts.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

pub const MAX_CLEAR_SITE_DATA_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CLEAR_SITE_DATA_DIRECTIVES: usize = 256;

/// A directive declared by the `Clear-Site-Data` response header.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ClearSiteDataDirective {
  Cache,
  Cookies,
  Storage,
  ExecutionContexts,
  Wildcard,
}

impl ClearSiteDataDirective {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Cache => "cache",
      Self::Cookies => "cookies",
      Self::Storage => "storage",
      Self::ExecutionContexts => "executionContexts",
      Self::Wildcard => "*",
    }
  }
}

/// Parsed, bounded `Clear-Site-Data` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClearSiteData {
  directives: Vec<ClearSiteDataDirective>,
}

impl ClearSiteData {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ClearSiteDataParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ClearSiteDataParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
      validate_field(value)?;
      parse_field(value, &mut directives, &mut seen)?;
    }
    if directives.is_empty() {
      return Err(ClearSiteDataParseError::new(
        "invalid Clear-Site-Data directive",
      ));
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[ClearSiteDataDirective] {
    &self.directives
  }

  pub fn is_wildcard(&self) -> bool {
    self.directives.contains(&ClearSiteDataDirective::Wildcard)
  }

  pub fn clears_cache(&self) -> bool {
    self.is_wildcard() || self.directives.contains(&ClearSiteDataDirective::Cache)
  }

  pub fn clears_cookies(&self) -> bool {
    self.is_wildcard() || self.directives.contains(&ClearSiteDataDirective::Cookies)
  }

  pub fn clears_storage(&self) -> bool {
    self.is_wildcard() || self.directives.contains(&ClearSiteDataDirective::Storage)
  }

  pub fn clears_execution_contexts(&self) -> bool {
    self.is_wildcard()
      || self
        .directives
        .contains(&ClearSiteDataDirective::ExecutionContexts)
  }

  pub fn header_value(&self) -> String {
    self
      .directives
      .iter()
      .map(|directive| format!("\"{}\"", directive.as_str()))
      .collect::<Vec<_>>()
      .join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClearSiteDataParseError {
  message: String,
}

impl ClearSiteDataParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ClearSiteDataParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ClearSiteDataParseError {}

fn validate_field(value: &str) -> Result<(), ClearSiteDataParseError> {
  if value.len() > MAX_CLEAR_SITE_DATA_VALUE_BYTES {
    return Err(ClearSiteDataParseError::new(
      "Clear-Site-Data header value is too large",
    ));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(ClearSiteDataParseError::new(
      "invalid Clear-Site-Data control byte",
    ));
  }
  Ok(())
}

fn parse_field(
  value: &str,
  directives: &mut Vec<ClearSiteDataDirective>,
  seen: &mut HashSet<ClearSiteDataDirective>,
) -> Result<(), ClearSiteDataParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ClearSiteDataParseError::new(
      "invalid Clear-Site-Data directive",
    ));
  }

  loop {
    if directives.len() >= MAX_CLEAR_SITE_DATA_DIRECTIVES {
      return Err(ClearSiteDataParseError::new(
        "too many Clear-Site-Data directives",
      ));
    }
    let directive = parse_quoted_directive(value, &mut position)?;
    if !seen.insert(directive) {
      return Err(ClearSiteDataParseError::new(
        "duplicate Clear-Site-Data directive",
      ));
    }
    directives.push(directive);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(ClearSiteDataParseError::new(
        "invalid Clear-Site-Data directive",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ClearSiteDataParseError::new(
        "invalid Clear-Site-Data directive",
      ));
    }
  }
}

fn parse_quoted_directive(
  value: &str,
  position: &mut usize,
) -> Result<ClearSiteDataDirective, ClearSiteDataParseError> {
  if value.as_bytes().get(*position) != Some(&b'"') {
    return Err(ClearSiteDataParseError::new(
      "Clear-Site-Data directives must be quoted strings",
    ));
  }
  *position += 1;
  let start = *position;
  while let Some(&byte) = value.as_bytes().get(*position) {
    match byte {
      b'"' => {
        let directive = match &value[start..*position] {
          "cache" => ClearSiteDataDirective::Cache,
          "cookies" => ClearSiteDataDirective::Cookies,
          "storage" => ClearSiteDataDirective::Storage,
          "executionContexts" => ClearSiteDataDirective::ExecutionContexts,
          "*" => ClearSiteDataDirective::Wildcard,
          _ => {
            return Err(ClearSiteDataParseError::new(
              "invalid Clear-Site-Data directive",
            ))
          }
        };
        *position += 1;
        return Ok(directive);
      }
      b'\\' | 0x00..=0x1f | 0x7f..=0xff => {
        return Err(ClearSiteDataParseError::new(
          "malformed Clear-Site-Data quoted-string",
        ))
      }
      _ => *position += 1,
    }
  }
  Err(ClearSiteDataParseError::new(
    "malformed Clear-Site-Data quoted-string",
  ))
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_known_directives_and_canonicalizes_header_values() {
    let metadata =
      ClearSiteData::parse("\"cache\", \"executionContexts\"").expect("directives should parse");
    assert_eq!(
      vec![
        ClearSiteDataDirective::Cache,
        ClearSiteDataDirective::ExecutionContexts,
      ],
      metadata.directives()
    );
    assert_eq!("\"cache\", \"executionContexts\"", metadata.header_value());
  }
}
