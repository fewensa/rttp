use std::error::Error;
use std::fmt;

pub use crate::media_type::{MediaType, MediaTypeParameter};

pub const MAX_ACCEPT_POST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_POST_MEDIA_TYPES: usize = 256;

/// Parsed, bounded `Accept-Post` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPost {
  media_types: Vec<MediaType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPostParseError {
  message: String,
}

impl AcceptPostParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptPostParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptPostParseError {}

impl AcceptPost {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptPostParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptPostParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut media_types = Vec::new();
    for value in values {
      let field = crate::media_type::parse_values(
        [value],
        "Accept-Post",
        MAX_ACCEPT_POST_VALUE_BYTES,
        MAX_ACCEPT_POST_MEDIA_TYPES - media_types.len(),
      )
      .map_err(|message| AcceptPostParseError { message })?;
      let accept_post = Self { media_types: field };
      if accept_post.header_value().len() > MAX_ACCEPT_POST_VALUE_BYTES {
        return Err(AcceptPostParseError::new(
          "Accept-Post header value is too large",
        ));
      }
      media_types.extend(accept_post.media_types);
    }
    if media_types.is_empty() {
      return Err(AcceptPostParseError::new("invalid Accept-Post media type"));
    }
    Ok(Self { media_types })
  }

  /// Validates supplied media types as one bounded `Accept-Post` field value.
  pub fn from_media_types<I, M>(media_types: I) -> Result<Self, AcceptPostParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let mut value = String::with_capacity(MAX_ACCEPT_POST_VALUE_BYTES);

    for (index, media_type) in media_types.into_iter().enumerate() {
      if index >= MAX_ACCEPT_POST_MEDIA_TYPES {
        return Err(AcceptPostParseError::new(
          "too many Accept-Post media types",
        ));
      }

      let media_type = media_type.as_ref();
      let separator_bytes = if index > 0 { 2 } else { 0 };
      let Some(value_length) = value
        .len()
        .checked_add(separator_bytes)
        .and_then(|length| length.checked_add(media_type.len()))
      else {
        return Err(AcceptPostParseError::new(
          "Accept-Post header value is too large",
        ));
      };
      if value_length > MAX_ACCEPT_POST_VALUE_BYTES {
        return Err(AcceptPostParseError::new(
          "Accept-Post header value is too large",
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
