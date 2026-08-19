//! Bounded, policy-free `Reporting-Endpoints` response metadata parsing.
//!
//! This module validates one or more `Reporting-Endpoints` dictionary field
//! values as an ordered list of endpoint-name to quoted-URL members. Callers
//! decide whether and how to schedule, send, persist, or route reports.
//! Unparsable input is an error; this parser never fails open.
//!
//! Endpoint names are lowercase tokens that start with lowercase ASCII or `*`
//! and continue with lowercase ASCII, digits, `_`, `-`, `.`, or `*`. Each
//! member must use `name="url"` form. Quoted URLs unescape only `\\` and `\"`
//! and reject ASCII controls and obs-text. Duplicate names, empty dictionaries,
//! too many members, oversized field values, and oversized cumulative input
//! are errors.
//!
//! ```
//! use rttp_protocol::reporting_endpoints::ReportingEndpoints;
//!
//! let endpoints = ReportingEndpoints::parse(
//!   r#"default="https://reports.example/default", csp="https://reports.example/csp""#,
//! )
//! .expect("valid Reporting-Endpoints");
//! assert_eq!(
//!   endpoints.endpoint("default"),
//!   Some("https://reports.example/default")
//! );
//! ```

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Reporting-Endpoints` field value.
pub const MAX_REPORTING_ENDPOINTS_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across all supplied fields.
pub const MAX_REPORTING_ENDPOINTS_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum endpoint members accepted across the combined dictionary.
pub const MAX_REPORTING_ENDPOINTS: usize = 32;

/// Parsed, bounded `Reporting-Endpoints` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingEndpoints {
  endpoints: Vec<(String, String)>,
}

/// An error returned when `Reporting-Endpoints` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingEndpointsParseError {
  message: String,
}

impl ReportingEndpointsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ReportingEndpointsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ReportingEndpointsParseError {}

impl ReportingEndpoints {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ReportingEndpointsParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ReportingEndpointsParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut endpoints = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_REPORTING_ENDPOINTS_VALUE_BYTES {
        return Err(ReportingEndpointsParseError::new(
          "Reporting-Endpoints header value is too large",
        ));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_REPORTING_ENDPOINTS_TOTAL_BYTES {
        return Err(ReportingEndpointsParseError::new(
          "Reporting-Endpoints dictionary is too large",
        ));
      }
      parse_reporting_endpoints_value(value, &mut endpoints)?;
    }
    if endpoints.is_empty() {
      return Err(ReportingEndpointsParseError::new(
        "invalid Reporting-Endpoints dictionary",
      ));
    }
    Ok(Self { endpoints })
  }

  pub fn from_endpoints<I, N, U>(endpoints: I) -> Result<Self, ReportingEndpointsParseError>
  where
    I: IntoIterator<Item = (N, U)>,
    N: AsRef<str>,
    U: AsRef<str>,
  {
    let value = endpoints
      .into_iter()
      .map(|(name, url)| {
        format!(
          "{}=\"{}\"",
          name.as_ref(),
          escape_reporting_endpoint_url(url.as_ref())
        )
      })
      .collect::<Vec<_>>()
      .join(", ");
    Self::parse(value)
  }

  pub fn endpoints(&self) -> Vec<(&str, &str)> {
    self
      .endpoints
      .iter()
      .map(|(name, url)| (name.as_str(), url.as_str()))
      .collect()
  }

  pub fn endpoint(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .endpoints
      .iter()
      .find(|(known, _)| known == name.as_ref())
      .map(|(_, url)| url.as_str())
  }

  pub fn header_value(&self) -> String {
    self
      .endpoints
      .iter()
      .map(|(name, url)| format!("{name}=\"{}\"", escape_reporting_endpoint_url(url)))
      .collect::<Vec<_>>()
      .join(", ")
  }
}

fn parse_reporting_endpoints_value(
  value: &str,
  endpoints: &mut Vec<(String, String)>,
) -> Result<(), ReportingEndpointsParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  while position < bytes.len() {
    while position < bytes.len() && bytes[position].is_ascii_whitespace() {
      position += 1;
    }
    let name_start = position;
    while position < bytes.len()
      && is_reporting_endpoint_key_byte(bytes[position], position == name_start)
    {
      position += 1;
    }
    if position == name_start {
      return Err(ReportingEndpointsParseError::new(
        "invalid Reporting-Endpoints endpoint name",
      ));
    }
    let name = &value[name_start..position];
    if position >= bytes.len() || bytes[position] != b'=' {
      return Err(ReportingEndpointsParseError::new(
        "invalid Reporting-Endpoints dictionary",
      ));
    }
    position += 1;
    if position >= bytes.len() || bytes[position] != b'\"' {
      return Err(ReportingEndpointsParseError::new(
        "Reporting-Endpoints URL must be a quoted string",
      ));
    }
    position += 1;
    let mut url = String::new();
    loop {
      let Some(&byte) = bytes.get(position) else {
        return Err(ReportingEndpointsParseError::new(
          "malformed Reporting-Endpoints quoted string",
        ));
      };
      position += 1;
      match byte {
        b'\"' => break,
        b'\\' => {
          let Some(&escaped) = bytes.get(position) else {
            return Err(ReportingEndpointsParseError::new(
              "malformed Reporting-Endpoints quoted string",
            ));
          };
          if !matches!(escaped, b'\"' | b'\\') {
            return Err(ReportingEndpointsParseError::new(
              "malformed Reporting-Endpoints quoted string",
            ));
          }
          position += 1;
          url.push(escaped as char);
        }
        0..=31 | 127..=u8::MAX => {
          return Err(ReportingEndpointsParseError::new(
            "malformed Reporting-Endpoints quoted string",
          ))
        }
        _ => url.push(byte as char),
      }
    }
    if endpoints.iter().any(|(known, _)| known == name) {
      return Err(ReportingEndpointsParseError::new(
        "duplicate Reporting-Endpoints endpoint name",
      ));
    }
    if endpoints.len() >= MAX_REPORTING_ENDPOINTS {
      return Err(ReportingEndpointsParseError::new(
        "too many Reporting-Endpoints endpoints",
      ));
    }
    endpoints.push((name.to_string(), url));
    while position < bytes.len() && bytes[position].is_ascii_whitespace() {
      position += 1;
    }
    if position == bytes.len() {
      break;
    }
    if bytes[position] != b',' {
      return Err(ReportingEndpointsParseError::new(
        "invalid Reporting-Endpoints dictionary",
      ));
    }
    position += 1;
    if position == bytes.len() {
      return Err(ReportingEndpointsParseError::new(
        "invalid Reporting-Endpoints dictionary",
      ));
    }
  }
  Ok(())
}

fn is_reporting_endpoint_key_byte(byte: u8, first: bool) -> bool {
  if first {
    byte.is_ascii_lowercase() || byte == b'*'
  } else {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.' | b'*')
  }
}

fn escape_reporting_endpoint_url(url: &str) -> String {
  url.replace('\\', "\\\\").replace('\"', "\\\"")
}
