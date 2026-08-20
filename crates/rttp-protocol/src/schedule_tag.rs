//! Bounded, policy-free `Schedule-Tag` response metadata parsing.
//!
//! This module validates one RFC 9110 entity-tag-shaped schedule validator
//! field value only. Surrounding SP and HTAB are trimmed as optional
//! whitespace. A successful parse stores that one entity tag and does not
//! create calendar versions, compare it to stored calendar state, inspect
//! calendars, or apply scheduling policy.

use std::error::Error;
use std::fmt;

use crate::entity_tag::EntityTag;

/// Maximum bytes accepted in a `Schedule-Tag` field value.
pub const MAX_SCHEDULE_TAG_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Schedule-Tag` response metadata.
///
/// The stored entity tag is the OWS-trimmed validator from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ScheduleTag(EntityTag);

/// An error returned when `Schedule-Tag` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleTagParseError {
  message: String,
}

impl ScheduleTag {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ScheduleTagParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ScheduleTagParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let entity_tag = EntityTag::parse(value).map_err(|_| invalid_value())?;
    Ok(Self(entity_tag))
  }

  pub fn entity_tag(&self) -> &EntityTag {
    &self.0
  }

  pub fn is_weak(&self) -> bool {
    self.0.is_weak()
  }

  pub fn opaque_tag(&self) -> &str {
    self.0.opaque_tag()
  }

  pub fn header_value(&self) -> String {
    self.0.header_value()
  }
}

impl ScheduleTagParseError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ScheduleTagParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ScheduleTagParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, ScheduleTagParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_bounded_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value)?;
  }
  if has_duplicate {
    return Err(ScheduleTagParseError::new(
      "duplicate Schedule-Tag header fields",
    ));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_bounded_value(value: &str) -> Result<(), ScheduleTagParseError> {
  if value.len() > MAX_SCHEDULE_TAG_VALUE_BYTES {
    return Err(ScheduleTagParseError::new(
      "Schedule-Tag header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ScheduleTagParseError::new(
      "invalid Schedule-Tag control byte",
    ));
  }
  Ok(())
}

fn invalid_value() -> ScheduleTagParseError {
  ScheduleTagParseError::new("invalid Schedule-Tag header value")
}
