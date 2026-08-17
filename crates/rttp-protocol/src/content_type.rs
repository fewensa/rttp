//! Bounded, policy-free `Content-Type` metadata parsing.
//!
//! This module validates one RFC 9110 `media-type` field value only. Callers
//! retain MIME sniffing, charset application, negotiation, and body
//! interpretation policy.

use std::error::Error;
use std::fmt;

pub use crate::media_type::{MediaType, MediaTypeParameter};

pub const MAX_CONTENT_TYPE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONTENT_TYPE_PARAMETERS: usize = crate::media_type::MAX_MEDIA_TYPE_PARAMETERS;

/// Parsed, bounded `Content-Type` representation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentType {
  media_type: MediaType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentTypeParseError {
  message: String,
}

impl ContentTypeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentTypeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentTypeParseError {}

impl ContentType {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentTypeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentTypeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let media_type =
      crate::media_type::parse_values([value], "Content-Type", MAX_CONTENT_TYPE_VALUE_BYTES, 1)
        .map_err(ContentTypeParseError::new)?
        .into_iter()
        .next()
        .ok_or_else(|| ContentTypeParseError::new("invalid Content-Type media type"))?;
    reject_duplicate_parameters(&media_type)?;
    Ok(Self { media_type })
  }

  pub fn media_type(&self) -> &MediaType {
    &self.media_type
  }

  pub fn type_(&self) -> &str {
    self.media_type.type_()
  }

  pub fn subtype(&self) -> &str {
    self.media_type.subtype()
  }

  pub fn parameters(&self) -> &[MediaTypeParameter] {
    self.media_type.parameters()
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .media_type
      .parameters()
      .iter()
      .find(|parameter| parameter.name().eq_ignore_ascii_case(name.as_ref()))
      .map(MediaTypeParameter::value)
  }

  pub fn header_value(&self) -> String {
    self.media_type.header_value()
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, ContentTypeParseError>
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
    return Err(ContentTypeParseError::new(
      "duplicate Content-Type header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), ContentTypeParseError> {
  if value.len() > MAX_CONTENT_TYPE_VALUE_BYTES {
    return Err(ContentTypeParseError::new(
      "Content-Type header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte != b'\t' && (byte <= 0x1f || byte == 0x7f))
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn reject_duplicate_parameters(media_type: &MediaType) -> Result<(), ContentTypeParseError> {
  let parameters = media_type.parameters();
  for (index, parameter) in parameters.iter().enumerate() {
    if parameters[..index]
      .iter()
      .any(|seen| seen.name().eq_ignore_ascii_case(parameter.name()))
    {
      return Err(ContentTypeParseError::new(
        "duplicate Content-Type parameter",
      ));
    }
  }
  Ok(())
}

fn invalid_value() -> ContentTypeParseError {
  ContentTypeParseError::new("invalid Content-Type header value")
}
