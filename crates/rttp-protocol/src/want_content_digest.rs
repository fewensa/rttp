//! Bounded, policy-free `Want-Content-Digest` request metadata parsing.
//!
//! This module validates RFC 9530 integrity-preference dictionaries. Callers
//! decide whether and how to attach `Content-Digest` or select an algorithm.
//! Unknown well-formed algorithm keys are retained as opaque data. Unrecognized
//! Structured Fields parameters are ignored after the integer preference is
//! validated. Unparsable input is an error; this parser never fails open to an
//! empty preference set.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Dictionary, ListEntry, Parser};

pub const MAX_WANT_CONTENT_DIGEST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_WANT_CONTENT_DIGEST_ALGORITHMS: usize = 32;

/// Parsed, bounded `Want-Content-Digest` algorithm preferences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WantContentDigest {
  entries: Vec<WantContentDigestEntry>,
}

/// One algorithm preference from a `Want-Content-Digest` dictionary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WantContentDigestEntry {
  algorithm: String,
  preference: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WantContentDigestParseError {
  message: String,
}

impl WantContentDigestParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for WantContentDigestParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for WantContentDigestParseError {}

impl WantContentDigest {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, WantContentDigestParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, WantContentDigestParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut entries = Vec::new();
    for value in values {
      if value.len() > MAX_WANT_CONTENT_DIGEST_VALUE_BYTES {
        return Err(WantContentDigestParseError::new(
          "Want-Content-Digest header value is too large",
        ));
      }
      parse_field(value, &mut entries)?;
    }
    if entries.is_empty() {
      return Err(WantContentDigestParseError::new(
        "Want-Content-Digest field must contain an entry",
      ));
    }
    Ok(Self { entries })
  }

  pub fn entries(&self) -> &[WantContentDigestEntry] {
    &self.entries
  }

  pub fn entry(&self, algorithm: impl AsRef<str>) -> Option<&WantContentDigestEntry> {
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
      .map(WantContentDigestEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl WantContentDigestEntry {
  pub fn algorithm(&self) -> &str {
    &self.algorithm
  }

  pub fn preference(&self) -> u8 {
    self.preference
  }

  fn header_value(&self) -> String {
    format!("{}={}", self.algorithm, self.preference)
  }
}

fn parse_field(
  value: &str,
  entries: &mut Vec<WantContentDigestEntry>,
) -> Result<(), WantContentDigestParseError> {
  reject_noncanonical_preference_integers(value)?;
  let dictionary = Parser::new(value)
    .parse::<Dictionary>()
    .map_err(|_| invalid_member())?;
  if dictionary.is_empty() {
    return Err(WantContentDigestParseError::new(
      "Want-Content-Digest field must contain an entry",
    ));
  }
  if top_level_member_count(value) != dictionary.len() {
    return Err(WantContentDigestParseError::new(
      "duplicate Want-Content-Digest dictionary key",
    ));
  }

  for (key, member) in dictionary {
    let ListEntry::Item(item) = member else {
      return Err(invalid_member());
    };
    let BareItem::Integer(preference) = item.bare_item else {
      return Err(invalid_member());
    };
    let preference = i64::from(preference);
    if !(0..=10).contains(&preference) {
      return Err(invalid_member());
    }
    let algorithm = key.as_str().to_owned();
    if entries.iter().any(|entry| entry.algorithm == algorithm) {
      return Err(WantContentDigestParseError::new(
        "duplicate Want-Content-Digest dictionary key",
      ));
    }
    if entries.len() >= MAX_WANT_CONTENT_DIGEST_ALGORITHMS {
      return Err(WantContentDigestParseError::new(
        "too many Want-Content-Digest algorithms",
      ));
    }
    entries.push(WantContentDigestEntry {
      algorithm,
      preference: u8::try_from(preference).map_err(|_| invalid_member())?,
    });
  }
  Ok(())
}

fn reject_noncanonical_preference_integers(value: &str) -> Result<(), WantContentDigestParseError> {
  let bytes = value.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] != b'=' {
      index += 1;
      continue;
    }
    index += 1;
    while matches!(bytes.get(index), Some(b' ' | b'\t')) {
      index += 1;
    }
    if bytes.get(index) == Some(&b'+') {
      return Err(invalid_member());
    }
  }
  Ok(())
}

fn top_level_member_count(value: &str) -> usize {
  value
    .split(',')
    .filter(|member| !member.trim_matches([' ', '\t']).is_empty())
    .count()
}

fn invalid_member() -> WantContentDigestParseError {
  WantContentDigestParseError::new("invalid Want-Content-Digest dictionary member")
}
