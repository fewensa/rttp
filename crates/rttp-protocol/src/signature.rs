//! Bounded, policy-free RFC 9421 `Signature` field parse and format.
//!
//! This module validates labeled Structured Fields dictionaries whose members
//! are byte sequences. Well-formed item parameters are accepted as syntax and
//! discarded. The parser does not sign, verify, look up keys, or parse
//! `Signature-Input`.

use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub const MAX_SIGNATURE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SIGNATURE_ENTRIES: usize = 256;
pub const MAX_SIGNATURE_ENTRY_PARAMETERS: usize = 256;
pub const MAX_SIGNATURE_ENTRY_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded HTTP `Signature` field metadata.
///
/// Labels and signature bytes are retained as opaque dictionary members. This
/// type does not perform cryptographic signing, verification, or key lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
  entries: Vec<SignatureEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureEntry {
  label: String,
  value: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureParseError {
  message: String,
}

impl SignatureParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SignatureParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SignatureParseError {}

impl Signature {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SignatureParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SignatureParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut entries = Vec::new();
    for value in values {
      if value.len() > MAX_SIGNATURE_VALUE_BYTES {
        return Err(SignatureParseError::new(
          "Signature field value is too large",
        ));
      }
      parse_field(value, &mut entries)?;
    }
    if entries.is_empty() {
      return Err(SignatureParseError::new(
        "Signature field must contain an entry",
      ));
    }
    Ok(Self { entries })
  }

  pub fn entries(&self) -> &[SignatureEntry] {
    &self.entries
  }

  pub fn entry(&self, label: impl AsRef<str>) -> Option<&SignatureEntry> {
    self
      .entries
      .iter()
      .find(|entry| entry.label == label.as_ref())
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .entries
      .iter()
      .map(SignatureEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SignatureEntry {
  pub fn label(&self) -> &str {
    &self.label
  }

  pub fn value(&self) -> &[u8] {
    &self.value
  }

  fn header_value(&self) -> String {
    format!("{}=:{}:", self.label, STANDARD.encode(&self.value))
  }
}

fn parse_field(value: &str, entries: &mut Vec<SignatureEntry>) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(SignatureParseError::new(
      "Signature field must contain an entry",
    ));
  }

  loop {
    if entries.len() >= MAX_SIGNATURE_ENTRIES {
      return Err(SignatureParseError::new("too many Signature field entries"));
    }
    let label = parse_key(value, &mut position)?.to_string();
    if bytes.get(position) != Some(&b'=') {
      return Err(SignatureParseError::new(
        "invalid Signature dictionary member",
      ));
    }
    position += 1;
    let parsed_value = parse_byte_sequence(value, &mut position)?;
    if parsed_value.len() > MAX_SIGNATURE_ENTRY_VALUE_BYTES {
      return Err(SignatureParseError::new(
        "Signature entry value is too large",
      ));
    }
    parse_parameters(value, &mut position)?;
    if entries.iter().any(|entry| entry.label == label) {
      return Err(SignatureParseError::new(
        "duplicate Signature dictionary key",
      ));
    }
    entries.push(SignatureEntry {
      label,
      value: parsed_value,
    });

    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(SignatureParseError::new(
        "invalid Signature dictionary separator",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(SignatureParseError::new(
        "invalid Signature dictionary separator",
      ));
    }
  }
}

fn parse_key<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, SignatureParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  if !matches!(bytes.get(*position), Some(b'a'..=b'z' | b'*')) {
    return Err(SignatureParseError::new("invalid Signature dictionary key"));
  }
  *position += 1;
  while matches!(
    bytes.get(*position),
    Some(b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
  ) {
    *position += 1;
  }
  Ok(&value[start..*position])
}

fn parse_byte_sequence(value: &str, position: &mut usize) -> Result<Vec<u8>, SignatureParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b':') {
    return Err(SignatureParseError::new(
      "Signature value must be a byte sequence",
    ));
  }
  *position += 1;
  let start = *position;
  while matches!(
    bytes.get(*position),
    Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=')
  ) {
    *position += 1;
  }
  if bytes.get(*position) != Some(&b':') {
    return Err(SignatureParseError::new("invalid Signature byte sequence"));
  }
  let encoded = &value[start..*position];
  *position += 1;
  STANDARD
    .decode(encoded)
    .map_err(|_| SignatureParseError::new("invalid Signature byte sequence"))
}

fn parse_parameters(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  let mut parameter_count = 0usize;
  while bytes.get(*position) == Some(&b';') {
    parameter_count += 1;
    if parameter_count > MAX_SIGNATURE_ENTRY_PARAMETERS {
      return Err(SignatureParseError::new(
        "too many Signature entry parameters",
      ));
    }
    *position += 1;
    skip_sp(bytes, position);
    parse_key(value, position)?;
    if bytes.get(*position) == Some(&b'=') {
      *position += 1;
      parse_bare_item(value, position)?;
    }
  }
  Ok(())
}

fn parse_bare_item(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  match value.as_bytes().get(*position) {
    Some(b'?') => parse_boolean(value, position),
    Some(b':') => parse_byte_sequence(value, position).map(|_| ()),
    Some(b'"') => parse_string(value, position),
    Some(b'-' | b'0'..=b'9') => parse_number(value, position),
    Some(b'*' | b'a'..=b'z' | b'A'..=b'Z') => parse_token(value, position),
    _ => Err(SignatureParseError::new(
      "invalid Signature parameter value",
    )),
  }
}

fn parse_boolean(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  if matches!(
    (bytes.get(*position), bytes.get(*position + 1)),
    (Some(b'?'), Some(b'0' | b'1'))
  ) {
    *position += 2;
    Ok(())
  } else {
    Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ))
  }
}

fn parse_string(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'"') {
    return Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ));
  }
  *position += 1;
  while let Some(byte) = bytes.get(*position) {
    match byte {
      b'"' => {
        *position += 1;
        return Ok(());
      }
      b'\\' => {
        *position += 1;
        if !matches!(bytes.get(*position), Some(b'"' | b'\\')) {
          return Err(SignatureParseError::new(
            "invalid Signature parameter value",
          ));
        }
      }
      0x20..=0x7e => {}
      _ => {
        return Err(SignatureParseError::new(
          "invalid Signature parameter value",
        ))
      }
    }
    *position += 1;
  }
  Err(SignatureParseError::new(
    "invalid Signature parameter value",
  ))
}

fn parse_number(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) == Some(&b'-') {
    *position += 1;
  }
  let whole_start = *position;
  while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
    *position += 1;
  }
  let whole_len = *position - whole_start;
  if whole_len == 0 {
    return Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ));
  }
  if bytes.get(*position) != Some(&b'.') {
    if whole_len > 15 {
      return Err(SignatureParseError::new(
        "invalid Signature parameter value",
      ));
    }
    return Ok(());
  }
  if whole_len > 12 {
    return Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ));
  }
  *position += 1;
  let fraction_start = *position;
  while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
    *position += 1;
  }
  if !(1..=3).contains(&(*position - fraction_start)) {
    return Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ));
  }
  Ok(())
}

fn parse_token(value: &str, position: &mut usize) -> Result<(), SignatureParseError> {
  let bytes = value.as_bytes();
  if !matches!(bytes.get(*position), Some(b'*' | b'a'..=b'z' | b'A'..=b'Z')) {
    return Err(SignatureParseError::new(
      "invalid Signature parameter value",
    ));
  }
  *position += 1;
  while matches!(
    bytes.get(*position),
    Some(
      b'*'
      | b'a'..=b'z'
      | b'A'..=b'Z'
      | b'0'..=b'9'
      | b'!'
      | b'#'
      | b'$'
      | b'%'
      | b'&'
      | b'\''
      | b'+'
      | b'-'
      | b'.'
      | b'^'
      | b'_'
      | b'`'
      | b'|'
      | b'~'
      | b':'
      | b'/',
    )
  ) {
    *position += 1;
  }
  Ok(())
}

fn skip_sp(bytes: &[u8], position: &mut usize) {
  while bytes.get(*position) == Some(&b' ') {
    *position += 1;
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}
