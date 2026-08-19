//! Bounded parsing for HTTP entity tags and conditional entity-tag headers.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

pub const MAX_ENTITY_TAG_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_IF_MATCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_IF_NONE_MATCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONDITIONAL_ENTITY_TAGS: usize = 256;

/// A parsed HTTP entity tag.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EntityTag {
  opaque_tag: String,
  weak: bool,
}

/// Parsed, bounded `If-Match` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfMatch {
  value: ConditionalEntityTags,
}

/// Parsed, bounded `If-None-Match` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfNoneMatch {
  value: ConditionalEntityTags,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConditionalEntityTags {
  Wildcard,
  Tags(Vec<EntityTag>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityTagParseError {
  message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalEntityTagParseError {
  message: String,
}

pub type IfMatchParseError = ConditionalEntityTagParseError;
pub type IfNoneMatchParseError = ConditionalEntityTagParseError;

impl EntityTagParseError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl ConditionalEntityTagParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for EntityTagParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl fmt::Display for ConditionalEntityTagParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for EntityTagParseError {}
impl Error for ConditionalEntityTagParseError {}

impl EntityTag {
  pub fn strong(value: impl AsRef<str>) -> Self {
    Self::new(false, value)
  }

  pub fn weak(value: impl AsRef<str>) -> Self {
    Self::new(true, value)
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, EntityTagParseError> {
    let value = value.as_ref();
    validate_length(value, MAX_ENTITY_TAG_VALUE_BYTES, "entity tag")
      .map_err(EntityTagParseError::new)?;
    parse_entity_tag(trim_ows(value)).map_err(EntityTagParseError::new)
  }

  pub fn opaque_tag(&self) -> &str {
    &self.opaque_tag
  }

  pub fn is_weak(&self) -> bool {
    self.weak
  }

  pub fn header_value(&self) -> String {
    if self.weak {
      format!("W/\"{}\"", self.opaque_tag)
    } else {
      format!("\"{}\"", self.opaque_tag)
    }
  }

  pub fn strong_matches(&self, other: &Self) -> bool {
    !self.weak && !other.weak && self.opaque_tag == other.opaque_tag
  }

  pub fn weak_matches(&self, other: &Self) -> bool {
    self.opaque_tag == other.opaque_tag
  }

  fn new(weak: bool, opaque_tag: impl AsRef<str>) -> Self {
    let opaque_tag = opaque_tag.as_ref();
    assert!(
      is_valid_entity_tag_opaque_tag(opaque_tag),
      "entity tag opaque value must be valid for an HTTP ETag header"
    );
    assert!(
      serialized_entity_tag_len(weak, opaque_tag) <= MAX_ENTITY_TAG_VALUE_BYTES,
      "entity tag header value must not exceed the maximum ETag header length"
    );
    Self {
      opaque_tag: opaque_tag.to_string(),
      weak,
    }
  }
}

impl IfMatch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, IfMatchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, IfMatchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      value: parse_conditional_values(values, "If-Match", MAX_IF_MATCH_VALUE_BYTES)?,
    })
  }

  pub fn is_wildcard(&self) -> bool {
    matches!(self.value, ConditionalEntityTags::Wildcard)
  }

  pub fn entity_tags(&self) -> &[EntityTag] {
    self.value.entity_tags()
  }

  pub fn header_value(&self) -> String {
    self.value.header_value()
  }
}

impl IfNoneMatch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, IfNoneMatchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, IfNoneMatchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      value: parse_conditional_values(values, "If-None-Match", MAX_IF_NONE_MATCH_VALUE_BYTES)?,
    })
  }

  pub fn is_wildcard(&self) -> bool {
    matches!(self.value, ConditionalEntityTags::Wildcard)
  }

  pub fn entity_tags(&self) -> &[EntityTag] {
    self.value.entity_tags()
  }

  pub fn header_value(&self) -> String {
    self.value.header_value()
  }
}

impl ConditionalEntityTags {
  fn entity_tags(&self) -> &[EntityTag] {
    match self {
      Self::Wildcard => &[],
      Self::Tags(entity_tags) => entity_tags,
    }
  }

  fn header_value(&self) -> String {
    match self {
      Self::Wildcard => "*".to_string(),
      Self::Tags(entity_tags) => entity_tags
        .iter()
        .map(EntityTag::header_value)
        .collect::<Vec<_>>()
        .join(", "),
    }
  }
}

fn parse_conditional_values<'a, I>(
  values: I,
  header_name: &str,
  maximum_length: usize,
) -> Result<ConditionalEntityTags, ConditionalEntityTagParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut entity_tags = Vec::new();
  let mut seen = HashSet::new();
  let mut wildcard = false;
  let mut saw_value = false;

  for value in values {
    validate_length(value, maximum_length, header_name)
      .map_err(ConditionalEntityTagParseError::new)?;
    let bytes = value.as_bytes();
    let mut position = 0;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ConditionalEntityTagParseError::new(format!(
        "invalid {header_name} entity tag"
      )));
    }

    loop {
      saw_value = true;
      if bytes.get(position) == Some(&b'*') {
        position += 1;
        skip_ows(bytes, &mut position);
        if wildcard || !entity_tags.is_empty() || position != bytes.len() {
          return Err(ConditionalEntityTagParseError::new(format!(
            "invalid {header_name} wildcard"
          )));
        }
        wildcard = true;
        break;
      }
      if wildcard || entity_tags.len() >= MAX_CONDITIONAL_ENTITY_TAGS {
        return Err(ConditionalEntityTagParseError::new(format!(
          "too many {header_name} entity tags"
        )));
      }

      let start = position;
      let entity_tag = parse_entity_tag_at(value, &mut position).map_err(|_| {
        ConditionalEntityTagParseError::new(format!("invalid {header_name} entity tag"))
      })?;
      if !seen.insert(entity_tag.clone()) {
        return Err(ConditionalEntityTagParseError::new(format!(
          "duplicate {header_name} entity tag"
        )));
      }
      entity_tags.push(entity_tag);
      debug_assert!(position > start);
      skip_ows(bytes, &mut position);
      if position == bytes.len() {
        break;
      }
      if bytes[position] != b',' {
        return Err(ConditionalEntityTagParseError::new(format!(
          "invalid {header_name} entity tag"
        )));
      }
      position += 1;
      skip_ows(bytes, &mut position);
      if position == bytes.len() {
        return Err(ConditionalEntityTagParseError::new(format!(
          "invalid {header_name} entity tag"
        )));
      }
    }
  }

  if !saw_value {
    return Err(ConditionalEntityTagParseError::new(format!(
      "invalid {header_name} entity tag"
    )));
  }
  if wildcard {
    Ok(ConditionalEntityTags::Wildcard)
  } else {
    Ok(ConditionalEntityTags::Tags(entity_tags))
  }
}

fn parse_entity_tag(value: &str) -> Result<EntityTag, String> {
  let mut position = 0;
  let entity_tag = parse_entity_tag_at(value, &mut position)?;
  if position != value.len() {
    return Err("invalid entity tag".to_string());
  }
  Ok(entity_tag)
}

fn parse_entity_tag_at(value: &str, position: &mut usize) -> Result<EntityTag, String> {
  let bytes = value.as_bytes();
  let weak = if bytes.get(*position..*position + 2) == Some(b"W/") {
    *position += 2;
    true
  } else {
    false
  };
  if bytes.get(*position) != Some(&b'"') {
    return Err("invalid entity tag".to_string());
  }
  *position += 1;
  let start = *position;
  while let Some(&byte) = bytes.get(*position) {
    match byte {
      b'"' => {
        let opaque_tag = value[start..*position].to_string();
        *position += 1;
        return Ok(EntityTag { opaque_tag, weak });
      }
      0x21 | 0x23..=0x7e | 0x80..=0xff => *position += 1,
      _ => return Err("invalid entity tag".to_string()),
    }
  }
  Err("invalid entity tag".to_string())
}

fn is_valid_entity_tag_opaque_tag(opaque_tag: &str) -> bool {
  opaque_tag
    .bytes()
    .all(|byte| matches!(byte, b'\x21' | b'\x23'..=b'\x7e' | b'\x80'..=b'\xff'))
}

fn serialized_entity_tag_len(weak: bool, opaque_tag: &str) -> usize {
  opaque_tag.len() + if weak { b"W/\"\"".len() } else { b"\"\"".len() }
}

fn validate_length(value: &str, maximum_length: usize, name: &str) -> Result<(), String> {
  if value.len() > maximum_length {
    return Err(format!("{name} header value is too large"));
  }
  Ok(())
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}
