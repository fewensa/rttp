use std::error::Error;
use std::fmt;

pub use crate::media_type::{MediaType, MediaTypeParameter};

pub const MAX_ACCEPT_PATCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_PATCH_MEDIA_TYPES: usize = 256;
pub const MAX_ACCEPT_PATCH_PARAMETERS: usize = crate::media_type::MAX_MEDIA_TYPE_PARAMETERS;

/// Parsed, bounded `Accept-Patch` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPatch {
  media_types: Vec<MediaType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPatchParseError {
  message: String,
}

impl AcceptPatchParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptPatchParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptPatchParseError {}

impl AcceptPatch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptPatchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptPatchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut media_types = Vec::new();
    let mut canonical_bytes = 0usize;
    for value in values {
      let field = crate::media_type::parse_values(
        [value],
        "Accept-Patch",
        MAX_ACCEPT_PATCH_VALUE_BYTES,
        MAX_ACCEPT_PATCH_MEDIA_TYPES - media_types.len(),
      )
      .map_err(|message| AcceptPatchParseError { message })?;
      for media_type in field {
        let member_bytes = media_type.header_value().len();
        let separator_bytes = if media_types.is_empty() { 0 } else { 2 };
        let Some(next_bytes) = canonical_bytes
          .checked_add(separator_bytes)
          .and_then(|length| length.checked_add(member_bytes))
        else {
          return Err(AcceptPatchParseError::new(
            "Accept-Patch header value is too large",
          ));
        };
        if next_bytes > MAX_ACCEPT_PATCH_VALUE_BYTES {
          return Err(AcceptPatchParseError::new(
            "Accept-Patch header value is too large",
          ));
        }
        canonical_bytes = next_bytes;
        media_types.push(media_type);
      }
    }
    if media_types.is_empty() {
      return Err(AcceptPatchParseError::new(
        "invalid Accept-Patch media type",
      ));
    }
    Ok(Self { media_types })
  }

  /// Validates supplied media types as one bounded `Accept-Patch` field value.
  pub fn from_media_types<I, M>(media_types: I) -> Result<Self, AcceptPatchParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let mut value = String::with_capacity(MAX_ACCEPT_PATCH_VALUE_BYTES);

    for (index, media_type) in media_types.into_iter().enumerate() {
      if index >= MAX_ACCEPT_PATCH_MEDIA_TYPES {
        return Err(AcceptPatchParseError::new(
          "too many Accept-Patch media types",
        ));
      }

      let media_type = media_type.as_ref();
      let separator_bytes = if index > 0 { 2 } else { 0 };
      let Some(value_length) = value
        .len()
        .checked_add(separator_bytes)
        .and_then(|length| length.checked_add(media_type.len()))
      else {
        return Err(AcceptPatchParseError::new(
          "Accept-Patch header value is too large",
        ));
      };
      if value_length > MAX_ACCEPT_PATCH_VALUE_BYTES {
        return Err(AcceptPatchParseError::new(
          "Accept-Patch header value is too large",
        ));
      }

      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(media_type);
    }

    Self::parse(value)
  }

  pub fn media_types(&self) -> &[MediaType] {
    &self.media_types
  }

  pub fn len(&self) -> usize {
    self.media_types.len()
  }

  pub fn is_empty(&self) -> bool {
    self.media_types.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .media_types
      .iter()
      .map(MediaType::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}
