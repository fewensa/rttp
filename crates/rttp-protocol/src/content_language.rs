//! Bounded, policy-free `Content-Language` representation metadata parsing.
//!
//! This module validates one or more RFC 9110 `Content-Language` field values
//! as an ordered list of concrete language tags. Callers decide whether and how
//! to select, negotiate, or localize representations. Unparsable input is an
//! error; this parser never fails open.

use std::error::Error;
use std::fmt;

pub const MAX_CONTENT_LANGUAGE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONTENT_LANGUAGE_TAGS: usize = 256;

/// Parsed, bounded `Content-Language` representation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLanguage {
  tags: Vec<String>,
}

impl ContentLanguage {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentLanguageParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tags: Vec<String> = Vec::new();

    for value in values {
      if value.len() > MAX_CONTENT_LANGUAGE_VALUE_BYTES {
        return Err(ContentLanguageParseError::new(
          "Content-Language header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(ContentLanguageParseError::new(
          "invalid Content-Language control byte",
        ));
      }
      for member in value.split(',') {
        let tag = member.trim_matches([' ', '\t']);
        if tag.is_empty() || !is_language_tag(tag) {
          return Err(ContentLanguageParseError::new(
            "invalid Content-Language tag",
          ));
        }
        if tags
          .iter()
          .any(|known: &String| known.eq_ignore_ascii_case(tag))
        {
          return Err(ContentLanguageParseError::new(
            "duplicate Content-Language tag",
          ));
        }
        if tags.len() >= MAX_CONTENT_LANGUAGE_TAGS {
          return Err(ContentLanguageParseError::new(
            "too many Content-Language tags",
          ));
        }
        tags.push(tag.to_owned());
      }
    }

    if tags.is_empty() {
      return Err(ContentLanguageParseError::new(
        "invalid Content-Language tag",
      ));
    }

    Ok(Self { tags })
  }

  pub fn tags(&self) -> Vec<&str> {
    self.tags.iter().map(String::as_str).collect()
  }

  pub fn len(&self) -> usize {
    self.tags.len()
  }

  pub fn is_empty(&self) -> bool {
    self.tags.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.tags.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLanguageParseError {
  message: String,
}

impl ContentLanguageParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentLanguageParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentLanguageParseError {}

fn is_language_tag(value: &str) -> bool {
  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };

  if !is_language_primary_subtag(primary) {
    return false;
  }

  subtags.all(is_language_subtag)
}

fn is_language_primary_subtag(value: &str) -> bool {
  (1..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_language_subtag(value: &str) -> bool {
  (1..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
