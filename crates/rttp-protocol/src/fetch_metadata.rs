//! Bounded, policy-free `Sec-Fetch-*` request metadata parsing.
//!
//! This module validates Fetch Metadata values only. Callers decide whether to
//! enforce any request policy; parsing never applies browser security policy.

use std::error::Error;
use std::fmt;

pub const MAX_FETCH_METADATA_VALUE_BYTES: usize = 64 * 1024;

macro_rules! fetch_metadata_enum {
  ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum $name {
      $($variant),+
    }

    impl $name {
      pub fn parse(value: impl AsRef<str>) -> Result<Self, FetchMetadataParseError> {
        let value = value.as_ref();
        validate_value(value)?;
        match value {
          $($value => Ok(Self::$variant),)+
          _ => Err(FetchMetadataParseError::new(concat!("invalid ", stringify!($name), " value"))),
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

fetch_metadata_enum!(SecFetchSite {
  SameOrigin => "same-origin",
  SameSite => "same-site",
  CrossSite => "cross-site",
  None => "none",
});

fetch_metadata_enum!(SecFetchMode {
  Navigate => "navigate",
  Cors => "cors",
  NoCors => "no-cors",
  SameOrigin => "same-origin",
  Websocket => "websocket",
});

fetch_metadata_enum!(SecFetchDest {
  Audio => "audio",
  AudioWorklet => "audioworklet",
  Document => "document",
  Embed => "embed",
  Empty => "empty",
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
  Track => "track",
  Video => "video",
  Worker => "worker",
  Xslt => "xslt",
});

/// The sole permitted `Sec-Fetch-User` value, serialized as `?1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecFetchUser;

impl SecFetchUser {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, FetchMetadataParseError> {
    let value = value.as_ref();
    validate_value(value)?;
    if value == "?1" {
      Ok(Self)
    } else {
      Err(FetchMetadataParseError::new("invalid SecFetchUser value"))
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

fn validate_value(value: &str) -> Result<(), FetchMetadataParseError> {
  if value.len() > MAX_FETCH_METADATA_VALUE_BYTES {
    return Err(FetchMetadataParseError::new(
      "Sec-Fetch header value is too large",
    ));
  }
  if value.bytes().any(|byte| byte.is_ascii_control()) {
    return Err(FetchMetadataParseError::new(
      "invalid Sec-Fetch control byte",
    ));
  }
  Ok(())
}
