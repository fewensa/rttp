//! Bounded parsing for `Delta-Base` response metadata.

use std::error::Error;
use std::fmt;

use crate::entity_tag::{EntityTag, MAX_ENTITY_TAG_VALUE_BYTES};

pub const MAX_DELTA_BASE_VALUE_BYTES: usize = MAX_ENTITY_TAG_VALUE_BYTES;

/// Parsed, bounded `Delta-Base` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaBase {
  entity_tag: EntityTag,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaBaseParseError {
  message: String,
}

impl DeltaBaseParseError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DeltaBaseParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DeltaBaseParseError {}

impl DeltaBase {
  pub fn new(entity_tag: EntityTag) -> Self {
    Self { entity_tag }
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, DeltaBaseParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DeltaBaseParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Err(DeltaBaseParseError::new("missing Delta-Base header value"));
    };
    if values.next().is_some() {
      return Err(DeltaBaseParseError::new("multiple Delta-Base headers"));
    }
    if value.len() > MAX_DELTA_BASE_VALUE_BYTES {
      return Err(DeltaBaseParseError::new(
        "Delta-Base header value is too large",
      ));
    }
    let entity_tag = EntityTag::parse(value)
      .map_err(|_| DeltaBaseParseError::new("invalid Delta-Base entity tag"))?;
    Ok(Self::new(entity_tag))
  }

  pub fn entity_tag(&self) -> &EntityTag {
    &self.entity_tag
  }

  pub fn into_entity_tag(self) -> EntityTag {
    self.entity_tag
  }

  pub fn header_value(&self) -> String {
    self.entity_tag.header_value()
  }
}
