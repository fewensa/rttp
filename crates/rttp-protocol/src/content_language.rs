//! Bounded, policy-free `Content-Language` response metadata parsing.
//!
//! This module validates one or more RFC 9110 `Content-Language` field values
//! as an ordered list of language tags. Callers decide whether and how to
//! negotiate or select language variants. Unparsable input is an error; this
//! parser never fails open.

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
    let mut tags = Vec::new();

    for value in values {
      if value.len() > MAX_CONTENT_LANGUAGE_VALUE_BYTES {
        return Err(ContentLanguageParseError::new(
          "Content-Language header value is too large",
        ));
      }

      for tag in value.split(',') {
        let tag = tag.trim_matches([' ', '\t']);
        if !is_valid_language_tag(tag) {
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
        tags.push(tag.to_string());
      }
    }

    if tags.is_empty() {
      return Err(ContentLanguageParseError::new(
        "invalid Content-Language tag",
      ));
    }

    Ok(Self { tags })
  }

  pub fn from_languages<I, L>(languages: I) -> Result<Self, ContentLanguageParseError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    let mut value = String::new();

    for (index, language) in languages.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(language.as_ref());
      if value.len() > MAX_CONTENT_LANGUAGE_VALUE_BYTES {
        return Err(ContentLanguageParseError::new(
          "Content-Language header value is too large",
        ));
      }
    }

    Self::parse(value)
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

fn is_valid_language_tag(value: &str) -> bool {
  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };

  if primary.is_empty()
    || primary.len() > 8
    || !primary.bytes().all(|byte| byte.is_ascii_alphabetic())
  {
    return false;
  }

  subtags.all(|subtag| {
    !subtag.is_empty()
      && subtag.len() <= 8
      && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
  })
}
