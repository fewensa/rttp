use std::error::Error;
use std::fmt;

pub use crate::media_type::{MediaType, MediaTypeParameter};

pub const MAX_ACCEPT_PATCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_PATCH_MEDIA_TYPES: usize = 256;

/// Parsed, bounded `Accept-Patch` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPatch {
  media_types: Vec<MediaType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPatchParseError {
  message: String,
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
    crate::media_type::parse_values(
      values,
      "Accept-Patch",
      MAX_ACCEPT_PATCH_VALUE_BYTES,
      MAX_ACCEPT_PATCH_MEDIA_TYPES,
    )
    .map(|media_types| Self { media_types })
    .map_err(|message| AcceptPatchParseError { message })
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
