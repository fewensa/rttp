use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub const MAX_DIGEST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_DIGEST_ENTRIES: usize = 256;
pub const MAX_DIGEST_ENTRY_PARAMETERS: usize = 256;
pub const MAX_DIGEST_ENTRY_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded HTTP Digest Fields response metadata.
///
/// The same Structured Fields dictionary syntax is used by both `Content-Digest` and
/// `Repr-Digest`; callers select the field through their response helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Digest {
  entries: Vec<DigestEntry>,
}

/// Parsed `Repr-Digest` response metadata.
///
/// `Repr-Digest` uses the same bounded Structured Fields dictionary as
/// `Content-Digest`, but refers to a representation rather than message content.
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
    parse_parameters(value, &mut position)?;
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

fn parse_parameters(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
  let bytes = value.as_bytes();
  let mut parameter_count = 0usize;
  while bytes.get(*position) == Some(&b';') {
    parameter_count += 1;
    if parameter_count > MAX_DIGEST_ENTRY_PARAMETERS {
      return Err(DigestParseError::new("too many Digest entry parameters"));
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

fn parse_bare_item(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
  match value.as_bytes().get(*position) {
    Some(b'?') => parse_boolean(value, position),
    Some(b':') => parse_byte_sequence(value, position).map(|_| ()),
    Some(b'"') => parse_string(value, position),
    Some(b'-' | b'0'..=b'9') => parse_number(value, position),
    Some(b'*' | b'a'..=b'z' | b'A'..=b'Z') => parse_token(value, position),
    _ => Err(DigestParseError::new("invalid Digest parameter value")),
  }
}

fn parse_boolean(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
  let bytes = value.as_bytes();
  if matches!(
    (bytes.get(*position), bytes.get(*position + 1)),
    (Some(b'?'), Some(b'0' | b'1'))
  ) {
    *position += 2;
    Ok(())
  } else {
    Err(DigestParseError::new("invalid Digest parameter value"))
  }
}

fn parse_string(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'"') {
    return Err(DigestParseError::new("invalid Digest parameter value"));
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
          return Err(DigestParseError::new("invalid Digest parameter value"));
        }
      }
      0x20..=0x7e => {}
      _ => return Err(DigestParseError::new("invalid Digest parameter value")),
    }
    *position += 1;
  }
  Err(DigestParseError::new("invalid Digest parameter value"))
}

fn parse_number(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
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
    return Err(DigestParseError::new("invalid Digest parameter value"));
  }
  if bytes.get(*position) != Some(&b'.') {
    if whole_len > 15 {
      return Err(DigestParseError::new("invalid Digest parameter value"));
    }
    return Ok(());
  }
  if whole_len > 12 {
    return Err(DigestParseError::new("invalid Digest parameter value"));
  }
  *position += 1;
  let fraction_start = *position;
  while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
    *position += 1;
  }
  if !(1..=3).contains(&(*position - fraction_start)) {
    return Err(DigestParseError::new("invalid Digest parameter value"));
  }
  Ok(())
}

fn parse_token(value: &str, position: &mut usize) -> Result<(), DigestParseError> {
  let bytes = value.as_bytes();
  if !matches!(bytes.get(*position), Some(b'*' | b'a'..=b'z' | b'A'..=b'Z')) {
    return Err(DigestParseError::new("invalid Digest parameter value"));
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

#[cfg(test)]
mod tests {
  use super::Digest;

  #[test]
  fn digest_accepts_structured_field_item_parameters() {
    let digest =
      Digest::parse("sha-256=:YWJj:;foo=bar;enabled;count=2, sha-512=:ZGVm:;note=\"ok\"")
        .expect("Digest should parse item parameters");

    assert_eq!(2, digest.len());
    assert_eq!(
      Some(&b"abc"[..]),
      digest.entry("sha-256").map(|entry| entry.value())
    );
    assert_eq!(
      Some(&b"def"[..]),
      digest.entry("sha-512").map(|entry| entry.value())
    );
    assert_eq!("sha-256=:YWJj:, sha-512=:ZGVm:", digest.header_value());
  }

  #[test]
  fn digest_rejects_malformed_item_parameters() {
    for value in [
      "sha-256=:YWJj:;foo=",
      "sha-256=:YWJj:;foo=1.",
      "sha-256=:YWJj:;Foo=bar",
      "sha-256=:YWJj:;\tfoo=bar",
    ] {
      assert!(Digest::parse(value).is_err(), "should reject {value:?}");
    }
  }
}
