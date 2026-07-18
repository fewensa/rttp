//! Bounded, policy-free Fetch Metadata request header parsing.
//!
//! This module only parses `Sec-Fetch-*` request metadata. Callers decide how
//! to use the parsed values; parsing never applies request policy.

use std::error::Error;
use std::fmt;

pub const MAX_SEC_FETCH_SITE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_FETCH_MODE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_FETCH_DEST_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SEC_FETCH_USER_VALUE_BYTES: usize = 64 * 1024;

/// The request-site relationship declared by `Sec-Fetch-Site`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecFetchSite {
  CrossSite,
  SameOrigin,
  SameSite,
  None,
}

/// The request mode declared by `Sec-Fetch-Mode`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecFetchMode {
  Cors,
  Navigate,
  NoCors,
  SameOrigin,
  Websocket,
}

/// The request destination declared by `Sec-Fetch-Dest`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecFetchDest {
  Empty,
  Audio,
  AudioWorklet,
  Document,
  Embed,
  FencedFrame,
  Font,
  Frame,
  Iframe,
  Image,
  Json,
  Manifest,
  Object,
  PaintWorklet,
  Report,
  Script,
  ServiceWorker,
  SharedWorker,
  Style,
  Text,
  Track,
  Video,
  WebIdentity,
  Worker,
  Xslt,
}

/// User activation declared by `Sec-Fetch-User`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecFetchUser {
  Activated,
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

impl SecFetchSite {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecFetchSiteParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecFetchSiteParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_singleton(values, "Sec-Fetch-Site", MAX_SEC_FETCH_SITE_VALUE_BYTES)? {
      "cross-site" => Ok(Self::CrossSite),
      "same-origin" => Ok(Self::SameOrigin),
      "same-site" => Ok(Self::SameSite),
      "none" => Ok(Self::None),
      _ => Err(invalid_value("Sec-Fetch-Site")),
    }
  }

  pub fn header_value(self) -> &'static str {
    match self {
      Self::CrossSite => "cross-site",
      Self::SameOrigin => "same-origin",
      Self::SameSite => "same-site",
      Self::None => "none",
    }
  }
}

impl SecFetchMode {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecFetchModeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecFetchModeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_singleton(values, "Sec-Fetch-Mode", MAX_SEC_FETCH_MODE_VALUE_BYTES)? {
      "cors" => Ok(Self::Cors),
      "navigate" => Ok(Self::Navigate),
      "no-cors" => Ok(Self::NoCors),
      "same-origin" => Ok(Self::SameOrigin),
      "websocket" => Ok(Self::Websocket),
      _ => Err(invalid_value("Sec-Fetch-Mode")),
    }
  }

  pub fn header_value(self) -> &'static str {
    match self {
      Self::Cors => "cors",
      Self::Navigate => "navigate",
      Self::NoCors => "no-cors",
      Self::SameOrigin => "same-origin",
      Self::Websocket => "websocket",
    }
  }
}

impl SecFetchDest {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecFetchDestParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecFetchDestParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_singleton(values, "Sec-Fetch-Dest", MAX_SEC_FETCH_DEST_VALUE_BYTES)? {
      "empty" => Ok(Self::Empty),
      "audio" => Ok(Self::Audio),
      "audioworklet" => Ok(Self::AudioWorklet),
      "document" => Ok(Self::Document),
      "embed" => Ok(Self::Embed),
      "fencedframe" => Ok(Self::FencedFrame),
      "font" => Ok(Self::Font),
      "frame" => Ok(Self::Frame),
      "iframe" => Ok(Self::Iframe),
      "image" => Ok(Self::Image),
      "json" => Ok(Self::Json),
      "manifest" => Ok(Self::Manifest),
      "object" => Ok(Self::Object),
      "paintworklet" => Ok(Self::PaintWorklet),
      "report" => Ok(Self::Report),
      "script" => Ok(Self::Script),
      "serviceworker" => Ok(Self::ServiceWorker),
      "sharedworker" => Ok(Self::SharedWorker),
      "style" => Ok(Self::Style),
      "text" => Ok(Self::Text),
      "track" => Ok(Self::Track),
      "video" => Ok(Self::Video),
      "webidentity" => Ok(Self::WebIdentity),
      "worker" => Ok(Self::Worker),
      "xslt" => Ok(Self::Xslt),
      _ => Err(invalid_value("Sec-Fetch-Dest")),
    }
  }

  pub fn header_value(self) -> &'static str {
    match self {
      Self::Empty => "empty",
      Self::Audio => "audio",
      Self::AudioWorklet => "audioworklet",
      Self::Document => "document",
      Self::Embed => "embed",
      Self::FencedFrame => "fencedframe",
      Self::Font => "font",
      Self::Frame => "frame",
      Self::Iframe => "iframe",
      Self::Image => "image",
      Self::Json => "json",
      Self::Manifest => "manifest",
      Self::Object => "object",
      Self::PaintWorklet => "paintworklet",
      Self::Report => "report",
      Self::Script => "script",
      Self::ServiceWorker => "serviceworker",
      Self::SharedWorker => "sharedworker",
      Self::Style => "style",
      Self::Text => "text",
      Self::Track => "track",
      Self::Video => "video",
      Self::WebIdentity => "webidentity",
      Self::Worker => "worker",
      Self::Xslt => "xslt",
    }
  }
}

impl SecFetchUser {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecFetchUserParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecFetchUserParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    match parse_singleton(values, "Sec-Fetch-User", MAX_SEC_FETCH_USER_VALUE_BYTES)? {
      "?1" => Ok(Self::Activated),
      _ => Err(invalid_value("Sec-Fetch-User")),
    }
  }

  pub fn header_value(self) -> &'static str {
    "?1"
  }
}

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
  if value.len() > max_value_bytes {
    return Err(FetchMetadataParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    if value.len() > max_value_bytes {
      return Err(FetchMetadataParseError::new(format!(
        "{header_name} header value is too large"
      )));
    }
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

fn invalid_value(header_name: &str) -> FetchMetadataParseError {
  FetchMetadataParseError::new(format!("invalid {header_name} header value"))
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}
