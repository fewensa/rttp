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

fn is_language_tag(value: &str) -> bool {
  if is_grandfathered_tag(value) {
    return true;
  }

  let subtags = value.split('-').collect::<Vec<_>>();
  if subtags.iter().any(|subtag| subtag.is_empty()) {
    return false;
  }
  if is_privateuse_subtags(&subtags) {
    return true;
  }

  let Some(language) = subtags.first() else {
    return false;
  };

  if !is_language_subtag(language) {
    return false;
  }

  let mut index = 1;

  if (2..=3).contains(&language.len()) {
    let extlang_end = usize::min(index + 3, subtags.len());
    while index < extlang_end && is_extlang_subtag(subtags[index]) {
      index += 1;
    }
  }

  if subtags
    .get(index)
    .is_some_and(|subtag| is_script_subtag(subtag))
  {
    index += 1;
  }

  if subtags
    .get(index)
    .is_some_and(|subtag| is_region_subtag(subtag))
  {
    index += 1;
  }

  let mut seen_variants: Vec<&str> = Vec::new();
  while subtags
    .get(index)
    .is_some_and(|subtag| is_variant_subtag(subtag))
  {
    let variant = subtags[index];
    if seen_variants
      .iter()
      .any(|known| known.eq_ignore_ascii_case(variant))
    {
      return false;
    }
    seen_variants.push(variant);
    index += 1;
  }

  let mut seen_extension_singletons: Vec<&str> = Vec::new();
  while subtags
    .get(index)
    .is_some_and(|subtag| is_extension_singleton(subtag))
  {
    let singleton = subtags[index];
    if seen_extension_singletons
      .iter()
      .any(|known| known.eq_ignore_ascii_case(singleton))
    {
      return false;
    }
    seen_extension_singletons.push(singleton);
    index += 1;
    let extension_start = index;
    while subtags
      .get(index)
      .is_some_and(|subtag| is_extension_subtag(subtag))
    {
      index += 1;
    }
    if index == extension_start {
      return false;
    }
  }

  if subtags
    .get(index)
    .is_some_and(|subtag| is_privateuse_prefix(subtag))
  {
    return is_privateuse_subtags(&subtags[index..]);
  }

  index == subtags.len()
}

fn is_language_subtag(value: &str) -> bool {
  ((2..=3).contains(&value.len()) || (4..=8).contains(&value.len()))
    && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_extlang_subtag(value: &str) -> bool {
  value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_script_subtag(value: &str) -> bool {
  value.len() == 4 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_region_subtag(value: &str) -> bool {
  (value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_alphabetic()))
    || (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_variant_subtag(value: &str) -> bool {
  ((5..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    || (value.len() == 4
      && value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_digit())
      && value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

fn is_extension_singleton(value: &str) -> bool {
  value.len() == 1
    && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    && !is_privateuse_prefix(value)
}

fn is_extension_subtag(value: &str) -> bool {
  (2..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_privateuse_subtag(value: &str) -> bool {
  (1..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_privateuse_subtags(subtags: &[&str]) -> bool {
  subtags.len() >= 2
    && is_privateuse_prefix(subtags[0])
    && subtags[1..]
      .iter()
      .all(|subtag| is_privateuse_subtag(subtag))
}

fn is_privateuse_prefix(value: &str) -> bool {
  value.eq_ignore_ascii_case("x")
}

fn is_grandfathered_tag(value: &str) -> bool {
  GRANDFATHERED_TAGS
    .iter()
    .any(|tag| tag.eq_ignore_ascii_case(value))
}

const GRANDFATHERED_TAGS: &[&str] = &[
  "art-lojban",
  "cel-gaulish",
  "en-GB-oed",
  "i-ami",
  "i-bnn",
  "i-default",
  "i-enochian",
  "i-hak",
  "i-klingon",
  "i-lux",
  "i-mingo",
  "i-navajo",
  "i-pwn",
  "i-tao",
  "i-tay",
  "i-tsu",
  "no-bok",
  "no-nyn",
  "sgn-BE-FR",
  "sgn-BE-NL",
  "sgn-CH-DE",
  "zh-guoyu",
  "zh-hakka",
  "zh-min",
  "zh-min-nan",
  "zh-xiang",
];

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
