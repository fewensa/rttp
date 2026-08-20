//! Bounded, policy-free singleton HTTP-date response metadata parsing.
//!
//! This module validates response field values for `Date`, `Expires`, and
//! `Last-Modified` only. Callers decide how to apply clock, cache, validator,
//! or revalidation policy.

use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Maximum bytes accepted in a singleton response HTTP-date field value.
pub const MAX_RESPONSE_HTTP_DATE_VALUE_BYTES: usize = 64 * 1024;

macro_rules! http_date_metadata {
  ($type_name:ident, $error_name:ident, $header:literal, $type_doc:literal, $error_doc:literal) => {
    #[doc = $type_doc]
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct $type_name(SystemTime);

    #[doc = $error_doc]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct $error_name {
      message: String,
    }

    impl $error_name {
      fn new(message: impl Into<String>) -> Self {
        Self {
          message: message.into(),
        }
      }
    }

    impl fmt::Display for $error_name {
      fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
      }
    }

    impl Error for $error_name {}

    impl $type_name {
      pub const fn new(datetime: SystemTime) -> Self {
        Self(datetime)
      }

      pub fn parse(value: impl AsRef<str>) -> Result<Self, $error_name> {
        Self::parse_values([value.as_ref()])
      }

      pub fn parse_values<'a, I>(values: I) -> Result<Self, $error_name>
      where
        I: IntoIterator<Item = &'a str>,
      {
        parse_singleton(values, $header)
          .map(Self)
          .map_err($error_name::new)
      }

      pub const fn datetime(self) -> SystemTime {
        self.0
      }

      pub fn header_value(self) -> String {
        httpdate::fmt_http_date(self.0)
      }
    }
  };
}

http_date_metadata!(
  ResponseDate,
  ResponseDateParseError,
  "Date",
  "Parsed, bounded `Date` response metadata.",
  "An error returned when `Date` metadata is malformed or exceeds bounds."
);
http_date_metadata!(
  ResponseExpires,
  ResponseExpiresParseError,
  "Expires",
  "Parsed, bounded `Expires` response metadata.",
  "An error returned when `Expires` metadata is malformed or exceeds bounds."
);
http_date_metadata!(
  ResponseLastModified,
  ResponseLastModifiedParseError,
  "Last-Modified",
  "Parsed, bounded `Last-Modified` response metadata.",
  "An error returned when `Last-Modified` metadata is malformed or exceeds bounds."
);

fn parse_singleton<'a, I>(values: I, header: &str) -> Result<SystemTime, String>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(|| invalid_value(header))?;
  validate_bounded_value(value, header)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value, header)?;
  }
  if has_duplicate {
    return Err(format!("duplicate {header} header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value(header));
  }
  httpdate::parse_http_date(value).map_err(|_| invalid_value(header))
}

fn validate_bounded_value(value: &str, header: &str) -> Result<(), String> {
  if value.len() > MAX_RESPONSE_HTTP_DATE_VALUE_BYTES {
    return Err(format!("{header} header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(format!("invalid {header} control byte"));
  }
  Ok(())
}

fn invalid_value(header: &str) -> String {
  format!("invalid {header} HTTP-date")
}
