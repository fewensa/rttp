use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub const MAX_DIGEST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_DIGEST_ENTRIES: usize = 256;
pub const MAX_DIGEST_ENTRY_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded HTTP Digest Fields response metadata.
///
/// The same Structured Fields dictionary syntax is used by both `Digest` and
/// `Repr-Digest`; callers select the field through their response helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Digest {
  entries: Vec<DigestEntry>,
}

/// Parsed `Repr-Digest` response metadata.
///
/// `Repr-Digest` uses the same bounded Structured Fields dictionary as
/// `Digest`, but refers to a representation rather than message content.
pub type ReprDigest = Digest;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestEntry {
  algorithm: String,
  value: Vec<u8>,
}

/// An entry in parsed `Repr-Digest` response metadata.
pub type ReprDigestEntry = DigestEntry;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DigestParseError {
  message: String,
}

impl DigestParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DigestParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DigestParseError {}

impl Digest {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DigestParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DigestParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut entries = Vec::new();
    for value in values {
      if value.len() > MAX_DIGEST_VALUE_BYTES {
        return Err(DigestParseError::new("Digest field value is too large"));
      }
      parse_field(value, &mut entries)?;
    }
    if entries.is_empty() {
      return Err(DigestParseError::new("Digest field must contain an entry"));
    }
    Ok(Self { entries })
  }

  pub fn entries(&self) -> &[DigestEntry] {
    &self.entries
  }

  pub fn entry(&self, algorithm: impl AsRef<str>) -> Option<&DigestEntry> {
    self
      .entries
      .iter()
      .find(|entry| entry.algorithm == algorithm.as_ref())
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
      .map(DigestEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl DigestEntry {
  pub fn algorithm(&self) -> &str {
    &self.algorithm
  }

  pub fn value(&self) -> &[u8] {
    &self.value
  }

  fn header_value(&self) -> String {
    format!("{}=:{}:", self.algorithm, STANDARD.encode(&self.value))
  }
}

fn parse_field(value: &str, entries: &mut Vec<DigestEntry>) -> Result<(), DigestParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(DigestParseError::new("Digest field must contain an entry"));
  }

  loop {
    if entries.len() >= MAX_DIGEST_ENTRIES {
      return Err(DigestParseError::new("too many Digest field entries"));
    }
    let algorithm = parse_key(value, &mut position)?.to_string();
    if bytes.get(position) != Some(&b'=') {
      return Err(DigestParseError::new("invalid Digest dictionary member"));
    }
    position += 1;
    let parsed_value = parse_byte_sequence(value, &mut position)?;
    if parsed_value.len() > MAX_DIGEST_ENTRY_VALUE_BYTES {
      return Err(DigestParseError::new("Digest entry value is too large"));
    }
    if entries.iter().any(|entry| entry.algorithm == algorithm) {
      return Err(DigestParseError::new("duplicate Digest dictionary key"));
    }
    entries.push(DigestEntry {
      algorithm,
      value: parsed_value,
    });

    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(DigestParseError::new("invalid Digest dictionary separator"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(DigestParseError::new("invalid Digest dictionary separator"));
    }
  }
}

fn parse_key<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, DigestParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  if !matches!(bytes.get(*position), Some(b'a'..=b'z' | b'*')) {
    return Err(DigestParseError::new("invalid Digest dictionary key"));
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

fn parse_byte_sequence(value: &str, position: &mut usize) -> Result<Vec<u8>, DigestParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b':') {
    return Err(DigestParseError::new(
      "Digest value must be a byte sequence",
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
    return Err(DigestParseError::new("invalid Digest byte sequence"));
  }
  let encoded = &value[start..*position];
  *position += 1;
  STANDARD
    .decode(encoded)
    .map_err(|_| DigestParseError::new("invalid Digest byte sequence"))
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}
