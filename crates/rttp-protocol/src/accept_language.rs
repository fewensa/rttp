//! Bounded, policy-free `Accept-Language` request metadata parsing.
//!
//! This module validates one or more RFC 9110 `Accept-Language` field values
//! as an ordered list of language ranges with optional q-values. Callers decide
//! whether and how to select, negotiate, or localize responses. Unparsable
//! input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a single `Accept-Language` field value.
pub const MAX_ACCEPT_LANGUAGE_VALUE_BYTES: usize = 64 * 1024;

/// Maximum number of language ranges accepted across all parsed fields.
pub const MAX_ACCEPT_LANGUAGE_RANGES: usize = 32;

/// Parsed, bounded `Accept-Language` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptLanguage {
  ranges: Vec<String>,
  qualities: Vec<Option<String>>,
}

impl AcceptLanguage {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptLanguageParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut ranges: Vec<String> = Vec::new();
    let mut qualities: Vec<Option<String>> = Vec::new();

    for value in values {
      if value.len() > MAX_ACCEPT_LANGUAGE_VALUE_BYTES {
        return Err(AcceptLanguageParseError::new(
          "Accept-Language header value is too large",
        ));
      }
      for item in value.split(',') {
        let (range, quality) = parse_accept_language_item(item.trim())?;
        if ranges.len() >= MAX_ACCEPT_LANGUAGE_RANGES {
          return Err(AcceptLanguageParseError::new(
            "too many Accept-Language ranges",
          ));
        }
        if ranges
          .iter()
          .any(|known: &String| known.eq_ignore_ascii_case(range))
        {
          return Err(AcceptLanguageParseError::new(
            "duplicate Accept-Language range",
          ));
        }
        ranges.push(range.to_owned());
        qualities.push(quality.map(str::to_owned));
      }
    }

    if ranges.is_empty() {
      return Err(AcceptLanguageParseError::new(
        "invalid Accept-Language range",
      ));
    }

    Ok(Self { ranges, qualities })
  }

  /// Validates and collects supplied language ranges, each optionally
  /// comma-separated and optionally carrying a `q` weight.
  pub fn from_ranges<I, L>(ranges: I) -> Result<Self, AcceptLanguageParseError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    let values: Vec<String> = ranges
      .into_iter()
      .map(|range| range.as_ref().to_owned())
      .collect();
    Self::parse_values(values.iter().map(String::as_str))
  }

  pub fn ranges(&self) -> Vec<&str> {
    self.ranges.iter().map(String::as_str).collect()
  }

  pub fn qualities(&self) -> Vec<Option<&str>> {
    self
      .qualities
      .iter()
      .map(|quality| quality.as_deref())
      .collect()
  }

  pub fn header_value(&self) -> String {
    self
      .ranges
      .iter()
      .zip(self.qualities.iter())
      .map(|(range, quality)| match quality {
        Some(quality) => format!("{range}; q={quality}"),
        None => range.clone(),
      })
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// An error returned when `Accept-Language` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptLanguageParseError {
  message: String,
}

impl AcceptLanguageParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptLanguageParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptLanguageParseError {}

fn parse_accept_language_item(
  value: &str,
) -> Result<(&str, Option<&str>), AcceptLanguageParseError> {
  let mut parts = value.split(';');
  let range = parts.next().unwrap_or_default().trim();
  if !is_valid_language_range(range) {
    return Err(AcceptLanguageParseError::new(
      "invalid Accept-Language range",
    ));
  }
  let Some(parameter) = parts.next() else {
    return Ok((range, None));
  };
  if parts.next().is_some() {
    return Err(AcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  }
  let Some((name, quality)) = parameter.trim().split_once('=') else {
    return Err(AcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  };
  let quality = quality.trim();
  if !name.trim().eq_ignore_ascii_case("q") || !is_valid_qvalue(quality) {
    return Err(AcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  }
  Ok((range, Some(quality)))
}

fn is_valid_language_range(value: &str) -> bool {
  if value == "*" {
    return true;
  }
  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };
  (1..=8).contains(&primary.len())
    && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
    && subtags.all(|subtag| {
      (1..=8).contains(&subtag.len()) && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn is_valid_qvalue(value: &str) -> bool {
  match value.split_once('.') {
    Some((whole, fraction)) => {
      (whole == "0" || whole == "1")
        && fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => value == "0" || value == "1",
  }
}
