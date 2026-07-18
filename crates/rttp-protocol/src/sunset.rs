//! RFC 8594 `Sunset` response metadata parsing.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SunsetParseError {
  message: String,
}

impl SunsetParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SunsetParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SunsetParseError {}

/// Parses RFC 8594 `Sunset` response metadata as an HTTP-date.
pub fn parse_sunset(value: impl AsRef<str>) -> Result<SystemTime, SunsetParseError> {
  httpdate::parse_http_date(value.as_ref())
    .map_err(|_| SunsetParseError::new("invalid Sunset HTTP-date"))
}

/// Parses an optional single RFC 8594 `Sunset` response field.
pub fn parse_sunset_values<'a, I>(values: I) -> Result<Option<SystemTime>, SunsetParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  if values.next().is_some() {
    return Err(SunsetParseError::new("multiple Sunset headers"));
  }
  parse_sunset(value).map(Some)
}
