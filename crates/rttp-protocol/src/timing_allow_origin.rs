//! Bounded, policy-free `Timing-Allow-Origin` response metadata parsing.
//!
//! This module validates serialized origins only. Callers decide whether and
//! how timing information is exposed.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use url::Url;

pub const MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_TIMING_ALLOW_ORIGIN_ORIGINS: usize = 256;

/// Parsed, bounded `Timing-Allow-Origin` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimingAllowOrigin {
  Wildcard,
  Origins(Vec<String>),
}

impl TimingAllowOrigin {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TimingAllowOriginParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TimingAllowOriginParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut origins = Vec::new();
    let mut seen = HashSet::new();
    let mut wildcard = false;

    for value in values {
      if value.len() > MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES {
        return Err(TimingAllowOriginParseError::new(
          "Timing-Allow-Origin header value is too large",
        ));
      }
      if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(invalid_value());
      }

      for member in value.split(',') {
        let member = member.trim_matches([' ', '\t']);
        if member.is_empty() {
          return Err(invalid_value());
        }
        if member == "*" {
          if wildcard || !origins.is_empty() {
            return Err(invalid_value());
          }
          wildcard = true;
          continue;
        }
        if wildcard || origins.len() >= MAX_TIMING_ALLOW_ORIGIN_ORIGINS {
          return Err(invalid_value());
        }

        let origin = parse_serialized_origin(member)?;
        if !seen.insert(origin.clone()) {
          return Err(TimingAllowOriginParseError::new(
            "duplicate Timing-Allow-Origin origin",
          ));
        }
        origins.push(origin);
      }
    }

    match (wildcard, origins.is_empty()) {
      (true, true) => Ok(Self::Wildcard),
      (false, false) => Ok(Self::Origins(origins)),
      _ => Err(invalid_value()),
    }
  }

  pub const fn is_wildcard(&self) -> bool {
    matches!(self, Self::Wildcard)
  }

  pub fn origins(&self) -> &[String] {
    match self {
      Self::Wildcard => &[],
      Self::Origins(origins) => origins,
    }
  }

  pub fn header_value(&self) -> String {
    match self {
      Self::Wildcard => "*".to_string(),
      Self::Origins(origins) => origins.join(", "),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimingAllowOriginParseError {
  message: String,
}

impl TimingAllowOriginParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TimingAllowOriginParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TimingAllowOriginParseError {}

fn parse_serialized_origin(value: &str) -> Result<String, TimingAllowOriginParseError> {
  let url = Url::parse(value).map_err(|_| invalid_value())?;
  if url.cannot_be_a_base() {
    return Err(invalid_value());
  }
  let origin = url.origin().ascii_serialization();
  if origin == "null" || value != origin {
    return Err(invalid_value());
  }
  Ok(origin)
}

fn invalid_value() -> TimingAllowOriginParseError {
  TimingAllowOriginParseError::new("invalid Timing-Allow-Origin header value")
}
