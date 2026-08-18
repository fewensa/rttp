use std::error::Error;
use std::fmt;

use crate::cache_control::{
  CacheControl, CacheControlDirective, CacheControlParseError, MAX_CACHE_CONTROL_DIRECTIVES,
  MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES, MAX_CACHE_CONTROL_VALUE_BYTES,
};

pub const MAX_CDN_CACHE_CONTROL_VALUE_BYTES: usize = MAX_CACHE_CONTROL_VALUE_BYTES;
pub const MAX_CDN_CACHE_CONTROL_DIRECTIVES: usize = MAX_CACHE_CONTROL_DIRECTIVES;
pub const MAX_CDN_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES: usize =
  MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES;

/// Parsed, bounded `CDN-Cache-Control` response metadata.
///
/// This preserves CDN-specific extension directives without applying cache
/// policy, freshness decisions, or surrogate behavior.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CdnCacheControl {
  inner: CacheControl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CdnCacheControlParseError {
  message: String,
}

impl CdnCacheControlParseError {
  fn from_cache_control(error: CacheControlParseError) -> Self {
    Self {
      message: error
        .to_string()
        .replace("Cache-Control", "CDN-Cache-Control"),
    }
  }
}

impl fmt::Display for CdnCacheControlParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CdnCacheControlParseError {}

impl CdnCacheControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CdnCacheControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CdnCacheControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    CacheControl::parse_values(values)
      .map(|inner| Self { inner })
      .map_err(CdnCacheControlParseError::from_cache_control)
  }

  pub fn directives(&self) -> &[CacheControlDirective] {
    self.inner.directives()
  }

  pub fn len(&self) -> usize {
    self.inner.len()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.inner.header_value()
  }
}
