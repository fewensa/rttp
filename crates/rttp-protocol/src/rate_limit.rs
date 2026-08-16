//! Bounded, policy-free `RateLimit-*` response metadata parsing.
//!
//! This module validates the standardized response field values only. Callers
//! decide whether and how to apply rate-limit behavior.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, List, ListEntry, Parser};

pub const MAX_RATE_LIMIT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_RATE_LIMIT_LIMIT_VALUE_BYTES: usize = MAX_RATE_LIMIT_VALUE_BYTES;
pub const MAX_RATE_LIMIT_REMAINING_VALUE_BYTES: usize = MAX_RATE_LIMIT_VALUE_BYTES;
pub const MAX_RATE_LIMIT_RESET_VALUE_BYTES: usize = MAX_RATE_LIMIT_VALUE_BYTES;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RateLimitLimit(Vec<RateLimitLimitItem>);

impl RateLimitLimit {
  pub fn new(items: impl IntoIterator<Item = RateLimitLimitItem>) -> Self {
    Self(items.into_iter().collect())
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, RateLimitParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, RateLimitParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let values = collect_list_values(values, "RateLimit-Limit", MAX_RATE_LIMIT_LIMIT_VALUE_BYTES)?;
    let values = Parser::new(trim_ows(&values))
      .parse::<List>()
      .map_err(|_| invalid_value("RateLimit-Limit"))?;
    if values.is_empty() {
      return Err(invalid_value("RateLimit-Limit"));
    }
    let items = values
      .into_iter()
      .map(parse_limit_item)
      .collect::<Result<_, _>>()?;
    Ok(Self(items))
  }

  pub fn items(&self) -> &[RateLimitLimitItem] {
    &self.0
  }

  pub fn header_value(&self) -> String {
    self
      .0
      .iter()
      .map(RateLimitLimitItem::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RateLimitLimitItem {
  value: u64,
  window: Option<u64>,
}

impl RateLimitLimitItem {
  pub const fn new(value: u64) -> Self {
    Self {
      value,
      window: None,
    }
  }

  pub const fn with_window(mut self, window: u64) -> Self {
    self.window = Some(window);
    self
  }

  pub const fn value(self) -> u64 {
    self.value
  }

  pub const fn window(self) -> Option<u64> {
    self.window
  }

  fn header_value(&self) -> String {
    match self.window {
      Some(window) => format!("{};w={window}", self.value),
      None => self.value.to_string(),
    }
  }
}

macro_rules! rate_limit_singleton_value {
  ($doc:literal, $name:ident, $header_name:literal, $max_value_bytes:ident) => {
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    #[doc = $doc]
    pub struct $name(u64);

    impl $name {
      pub const fn new(value: u64) -> Self {
        Self(value)
      }

      pub fn parse(value: impl AsRef<str>) -> Result<Self, RateLimitParseError> {
        Self::parse_values([value.as_ref()])
      }

      pub fn parse_values<'a, I>(values: I) -> Result<Self, RateLimitParseError>
      where
        I: IntoIterator<Item = &'a str>,
      {
        parse_singleton(values, $header_name, $max_value_bytes).map(Self)
      }

      pub const fn value(self) -> u64 {
        self.0
      }

      pub fn header_value(self) -> String {
        self.0.to_string()
      }
    }
  };
}

rate_limit_singleton_value!(
  "The remaining quota declared by the `RateLimit-Remaining` response header.",
  RateLimitRemaining,
  "RateLimit-Remaining",
  MAX_RATE_LIMIT_REMAINING_VALUE_BYTES
);
rate_limit_singleton_value!(
  "The seconds until reset declared by the `RateLimit-Reset` response header.",
  RateLimitReset,
  "RateLimit-Reset",
  MAX_RATE_LIMIT_RESET_VALUE_BYTES
);

impl RateLimitRemaining {
  pub const fn is_exhausted(self) -> bool {
    self.0 == 0
  }
}

impl RateLimitReset {
  pub const fn is_immediate(self) -> bool {
    self.0 == 0
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitParseError {
  message: String,
}

pub type RateLimitLimitParseError = RateLimitParseError;
pub type RateLimitRemainingParseError = RateLimitParseError;
pub type RateLimitResetParseError = RateLimitParseError;

impl RateLimitParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for RateLimitParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for RateLimitParseError {}

fn parse_singleton<'a, I>(
  values: I,
  header_name: &str,
  max_value_bytes: usize,
) -> Result<u64, RateLimitParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(|| invalid_value(header_name))?;
  validate_bounded_value(value, header_name, max_value_bytes)?;
  if let Some(duplicate) = values.next() {
    validate_bounded_value(duplicate, header_name, max_value_bytes)?;
    return Err(RateLimitParseError::new(format!(
      "duplicate {header_name} header fields"
    )));
  }
  let value = trim_ows(value);
  if value.is_empty() || value.bytes().any(|byte| !byte.is_ascii_digit()) {
    return Err(invalid_value(header_name));
  }
  parse_structured_integer(value, header_name)
}

fn collect_list_values<'a, I>(
  values: I,
  header_name: &str,
  max_value_bytes: usize,
) -> Result<String, RateLimitParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(|| invalid_value(header_name))?;
  validate_bounded_value(value, header_name, max_value_bytes)?;
  let mut combined = value.to_owned();
  for value in values {
    validate_bounded_value(value, header_name, max_value_bytes)?;
    combined.push(',');
    combined.push_str(value);
  }
  Ok(combined)
}

fn parse_limit_item(value: ListEntry) -> Result<RateLimitLimitItem, RateLimitParseError> {
  let ListEntry::Item(item) = value else {
    return Err(invalid_value("RateLimit-Limit"));
  };
  let value = parse_structured_bare_integer(item.bare_item, "RateLimit-Limit")?;
  let window = item
    .params
    .get("w")
    .map(|value| parse_structured_bare_integer(value.clone(), "RateLimit-Limit"))
    .transpose()?;
  Ok(RateLimitLimitItem { value, window })
}

fn parse_structured_integer(value: &str, header_name: &str) -> Result<u64, RateLimitParseError> {
  let value = Parser::new(trim_ows(value))
    .parse::<sfv::Item>()
    .map_err(|_| invalid_value(header_name))?;
  parse_structured_bare_integer(value.bare_item, header_name)
}

fn parse_structured_bare_integer(
  value: BareItem,
  header_name: &str,
) -> Result<u64, RateLimitParseError> {
  let BareItem::Integer(value) = value else {
    return Err(invalid_value(header_name));
  };
  u64::try_from(value).map_err(|_| invalid_value(header_name))
}

fn validate_bounded_value(
  value: &str,
  header_name: &str,
  max_value_bytes: usize,
) -> Result<(), RateLimitParseError> {
  if value.len() > max_value_bytes {
    return Err(RateLimitParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  Ok(())
}

fn invalid_value(header_name: &str) -> RateLimitParseError {
  RateLimitParseError::new(format!("invalid {header_name} header value"))
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}
