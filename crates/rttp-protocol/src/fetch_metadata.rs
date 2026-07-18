//! Bounded, policy-free `Sec-Fetch-*` request metadata parsing.
//!
//! This module validates Fetch Metadata values only. Callers decide whether to
//! enforce any request policy; parsing never applies browser security policy.

use std::error::Error;
use std::fmt;

pub const MAX_FETCH_METADATA_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_FETCH_SITE_VALUE_BYTES: usize = MAX_FETCH_METADATA_VALUE_BYTES;
pub const MAX_SEC_FETCH_MODE_VALUE_BYTES: usize = MAX_FETCH_METADATA_VALUE_BYTES;
pub const MAX_SEC_FETCH_DEST_VALUE_BYTES: usize = MAX_FETCH_METADATA_VALUE_BYTES;
pub const MAX_SEC_FETCH_USER_VALUE_BYTES: usize = MAX_FETCH_METADATA_VALUE_BYTES;

macro_rules! fetch_metadata_enum {
  ($doc:literal, $name:ident, $header_name:literal, $max_value_bytes:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    #[doc = $doc]
    pub enum $name {
      $($variant),+
    }

    impl $name {
      pub fn parse(value: impl AsRef<str>) -> Result<Self, FetchMetadataParseError> {
        Self::parse_values([value.as_ref()])
      }

      pub fn parse_values<'a, I>(values: I) -> Result<Self, FetchMetadataParseError>
      where
        I: IntoIterator<Item = &'a str>,
      {
        match parse_singleton(values, $header_name, $max_value_bytes)? {
          $($value => Ok(Self::$variant),)+
          _ => Err(invalid_value($header_name)),
        }
      }

      pub fn as_str(self) -> &'static str {
        match self {
          $(Self::$variant => $value,)+
        }
      }

      pub fn header_value(self) -> &'static str {
        self.as_str()
      }
    }
  };
}

fetch_metadata_enum!("The request-site relationship declared by `Sec-Fetch-Site`.", SecFetchSite, "Sec-Fetch-Site", MAX_SEC_FETCH_SITE_VALUE_BYTES {
  CrossSite => "cross-site",
  SameOrigin => "same-origin",
  SameSite => "same-site",
  None => "none",
});

fetch_metadata_enum!("The request mode declared by `Sec-Fetch-Mode`.", SecFetchMode, "Sec-Fetch-Mode", MAX_SEC_FETCH_MODE_VALUE_BYTES {
  Cors => "cors",
  Navigate => "navigate",
  NoCors => "no-cors",
  SameOrigin => "same-origin",
  Websocket => "websocket",
});

fetch_metadata_enum!("The request destination declared by `Sec-Fetch-Dest`.", SecFetchDest, "Sec-Fetch-Dest", MAX_SEC_FETCH_DEST_VALUE_BYTES {
  Empty => "empty",
  Audio => "audio",
  AudioWorklet => "audioworklet",
  Document => "document",
  Embed => "embed",
  FencedFrame => "fencedframe",
  Font => "font",
  Frame => "frame",
  Iframe => "iframe",
  Image => "image",
  Json => "json",
  Manifest => "manifest",
  Object => "object",
  PaintWorklet => "paintworklet",
  Report => "report",
  Script => "script",
  ServiceWorker => "serviceworker",
  SharedWorker => "sharedworker",
  Style => "style",
  Text => "text",
  Track => "track",
  Video => "video",
  WebIdentity => "webidentity",
  Worker => "worker",
  Xslt => "xslt",
});

/// The sole permitted `Sec-Fetch-User` value, serialized as `?1`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SecFetchUser;

impl SecFetchUser {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, FetchMetadataParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, FetchMetadataParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_singleton(values, "Sec-Fetch-User", MAX_SEC_FETCH_USER_VALUE_BYTES)? {
      "?1" => Ok(Self),
      _ => Err(invalid_value("Sec-Fetch-User")),
    }
  }

  pub fn header_value(self) -> &'static str {
    "?1"
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchMetadataParseError {
  message: String,
}

pub type SecFetchSiteParseError = FetchMetadataParseError;
pub type SecFetchModeParseError = FetchMetadataParseError;
pub type SecFetchDestParseError = FetchMetadataParseError;
pub type SecFetchUserParseError = FetchMetadataParseError;

impl FetchMetadataParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for FetchMetadataParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for FetchMetadataParseError {}

pub fn parse_optional_value<T, I>(
  values: I,
  header_name: &str,
) -> Result<Option<T>, FetchMetadataParseError>
where
  T: FetchMetadataValue,
  I: IntoIterator,
  I::Item: AsRef<str>,
{
  let mut values = values.into_iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  if values.next().is_some() {
    return Err(FetchMetadataParseError::new(format!(
      "duplicate {header_name} headers"
    )));
  }
  T::parse(value.as_ref()).map(Some)
}

pub trait FetchMetadataValue: Sized {
  fn parse(value: &str) -> Result<Self, FetchMetadataParseError>;
}

macro_rules! impl_fetch_metadata_value {
  ($($type:ty),+ $(,)?) => {
    $(impl FetchMetadataValue for $type {
      fn parse(value: &str) -> Result<Self, FetchMetadataParseError> {
        Self::parse(value)
      }
    })+
  };
}

impl_fetch_metadata_value!(SecFetchSite, SecFetchMode, SecFetchDest, SecFetchUser);

fn parse_singleton<'a, I>(
  values: I,
  header_name: &str,
  max_value_bytes: usize,
) -> Result<&'a str, FetchMetadataParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(|| invalid_value(header_name))?;
  validate_bounded_value(value, header_name, max_value_bytes)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value, header_name, max_value_bytes)?;
  }
  if has_duplicate {
    return Err(FetchMetadataParseError::new(format!(
      "duplicate {header_name} header fields"
    )));
  }
  let value = trim_ows(value);
  if value.is_empty() {
    return Err(invalid_value(header_name));
  }
  Ok(value)
}

fn validate_bounded_value(
  value: &str,
  header_name: &str,
  max_value_bytes: usize,
) -> Result<(), FetchMetadataParseError> {
  if value.len() > max_value_bytes {
    return Err(FetchMetadataParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  if value.bytes().any(|byte| byte.is_ascii_control()) {
    return Err(FetchMetadataParseError::new(
      "invalid Sec-Fetch control byte",
    ));
  }
  Ok(())
}

fn invalid_value(header_name: &str) -> FetchMetadataParseError {
  FetchMetadataParseError::new(format!("invalid {header_name} header value"))
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}
