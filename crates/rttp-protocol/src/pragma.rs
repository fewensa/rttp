//! Bounded, policy-free RFC 9111 `Pragma` metadata parsing.
//!
//! This module validates one or more `Pragma` field values as an ordered list
//! of `pragma-directive` members: the defined `no-cache` token or an
//! `extension-pragma` token with an optional token or quoted-string value.
//! It reports declared metadata only; callers own any cache or intermediary
//! behavior, and this module never translates `Pragma` into `Cache-Control`.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

/// Maximum bytes accepted in one `Pragma` field value and in the combined
/// field set, including `", "` separator overhead between fields.
pub const MAX_PRAGMA_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PRAGMA_DIRECTIVES: usize = 256;
pub const MAX_PRAGMA_DIRECTIVE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Pragma` metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pragma {
  directives: Vec<PragmaDirective>,
}

/// One `pragma-directive` member, including extension directives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PragmaDirective {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PragmaParseError {
  message: String,
}

impl PragmaParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for PragmaParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for PragmaParseError {}

impl Pragma {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PragmaParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PragmaParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_PRAGMA_VALUE_BYTES {
        return Err(PragmaParseError::new("Pragma header value is too large"));
      }
      let separator = if total_bytes > 0 { 2 } else { 0 };
      total_bytes = total_bytes
        .saturating_add(separator)
        .saturating_add(value.len());
      if total_bytes > MAX_PRAGMA_VALUE_BYTES {
        return Err(PragmaParseError::new("Pragma header value is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(PragmaParseError::new("invalid Pragma control byte"));
      }
      parse_field(value, &mut directives)?;
    }
    if directives.is_empty() {
      return Err(invalid());
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[PragmaDirective] {
    &self.directives
  }

  /// Whether the defined valueless `no-cache` directive is present.
  pub fn no_cache(&self) -> bool {
    self
      .directives
      .iter()
      .any(|directive| directive.name.eq_ignore_ascii_case("no-cache"))
  }

  /// Extension directives: every member other than `no-cache`, in wire order.
  pub fn extensions(&self) -> Vec<&PragmaDirective> {
    self
      .directives
      .iter()
      .filter(|directive| !directive.name.eq_ignore_ascii_case("no-cache"))
      .collect()
  }

  pub fn len(&self) -> usize {
    self.directives.len()
  }

  pub fn is_empty(&self) -> bool {
    self.directives.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .directives
      .iter()
      .map(PragmaDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl PragmaDirective {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  fn header_value(&self) -> String {
    match &self.value {
      None => self.name.clone(),
      Some(value) if is_token(value) => format!("{}={value}", self.name),
      Some(value) => format!("{}=\"{}\"", self.name, escape_quoted(value)),
    }
  }
}

fn parse_field(value: &str, directives: &mut Vec<PragmaDirective>) -> Result<(), PragmaParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(invalid());
  }
  loop {
    let directive = parse_directive(value, &mut position)?;
    if directives
      .iter()
      .any(|known| known.name.eq_ignore_ascii_case(&directive.name))
    {
      return Err(PragmaParseError::new("duplicate Pragma directive"));
    }
    if directives.len() >= MAX_PRAGMA_DIRECTIVES {
      return Err(PragmaParseError::new("too many Pragma directives"));
    }
    directives.push(directive);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(invalid());
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(invalid());
    }
  }
}

fn parse_directive(value: &str, position: &mut usize) -> Result<PragmaDirective, PragmaParseError> {
  let name = parse_token(value, position)?.to_string();
  skip_ows(value.as_bytes(), position);
  let directive_value = if value.as_bytes().get(*position) == Some(&b'=') {
    *position += 1;
    skip_ows(value.as_bytes(), position);
    Some(parse_directive_value(value, position)?)
  } else {
    None
  };
  if name.eq_ignore_ascii_case("no-cache") && directive_value.is_some() {
    return Err(PragmaParseError::new(
      "no-cache Pragma directive must not have a value",
    ));
  }
  Ok(PragmaDirective {
    name,
    value: directive_value,
  })
}

fn parse_directive_value(value: &str, position: &mut usize) -> Result<String, PragmaParseError> {
  let parsed = if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)?
  } else {
    parse_token(value, position)?.to_string()
  };
  if parsed.len() > MAX_PRAGMA_DIRECTIVE_VALUE_BYTES {
    return Err(PragmaParseError::new("Pragma directive value is too large"));
  }
  Ok(parsed)
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, PragmaParseError> {
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed)
          .map_err(|_| PragmaParseError::new("malformed Pragma quoted-string"));
      }
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(PragmaParseError::new("malformed Pragma quoted-string"));
        };
        if !is_quoted_pair_byte(escaped) {
          return Err(PragmaParseError::new("malformed Pragma quoted-string"));
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_quoted_text_byte(byte) => parsed.push(byte),
      _ => return Err(PragmaParseError::new("malformed Pragma quoted-string")),
    }
  }
  Err(PragmaParseError::new("malformed Pragma quoted-string"))
}

fn parse_token<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, PragmaParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| crate::http1::is_token_byte(*byte))
  {
    *position += 1;
  }
  let token = &value[start..*position];
  if is_token(token) {
    Ok(token)
  } else {
    Err(invalid())
  }
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

fn is_quoted_text_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
}

fn is_quoted_pair_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x7e | 0x80..=0xff)
}

fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn invalid() -> PragmaParseError {
  PragmaParseError::new("invalid Pragma directive")
}
