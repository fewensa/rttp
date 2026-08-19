//! Bounded, policy-free RFC 2068 `Keep-Alive` response metadata parsing.
//!
//! This module validates the legacy HTTP/1.x `Keep-Alive` response field
//! syntax only, as metadata for applications that interoperate with HTTP/1
//! peers. It never changes connection lifetime, connection pooling, or
//! HTTP/2 behavior. Unparsable input is an error; this parser never fails
//! open.
//!
//! `timeout` and `max` are the recognized RFC 2068 parameters and are both
//! optional. Unrecognized `name=token` parameters are preserved as bounded
//! extension metadata so that interoperable fields never fail to parse.
//!
//! ```
//! use rttp_protocol::keep_alive::KeepAlive;
//!
//! let keep_alive = KeepAlive::parse("timeout=5, max=100").expect("valid Keep-Alive");
//! assert_eq!(keep_alive.timeout(), Some(5));
//! assert_eq!(keep_alive.max(), Some(100));
//! ```

use std::error::Error;
use std::fmt;

use crate::http1::is_token_byte;

/// Maximum bytes accepted in a `Keep-Alive` field value.
pub const MAX_KEEP_ALIVE_VALUE_BYTES: usize = 64 * 1024;
/// Maximum keep-alive elements accepted across the combined field set.
pub const MAX_KEEP_ALIVE_ITEMS: usize = 256;

/// Parsed, bounded `Keep-Alive` response metadata.
///
/// `timeout` and `max` are optional; unrecognized parameters are preserved as
/// [`KeepAliveExtension`] entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeepAlive {
  timeout: Option<u64>,
  max: Option<u64>,
  extensions: Vec<KeepAliveExtension>,
}

/// A single unrecognized RFC 2068 `Keep-Alive` extension parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeepAliveExtension {
  name: String,
  value: String,
}

/// An error returned when `Keep-Alive` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeepAliveParseError {
  message: String,
}

impl KeepAliveParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for KeepAliveParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for KeepAliveParseError {}

impl KeepAlive {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, KeepAliveParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, KeepAliveParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut timeout = None;
    let mut max = None;
    let mut extensions = Vec::new();
    let mut items = 0usize;
    for value in values {
      if value.len() > MAX_KEEP_ALIVE_VALUE_BYTES {
        return Err(KeepAliveParseError::new(
          "Keep-Alive header value is too large",
        ));
      }
      let bytes = value.as_bytes();
      let mut position = 0;
      skip_ows(bytes, &mut position);
      if position == bytes.len() {
        return Err(invalid_value());
      }
      loop {
        if items >= MAX_KEEP_ALIVE_ITEMS {
          return Err(KeepAliveParseError::new("too many Keep-Alive parameters"));
        }
        items += 1;
        let name = parse_token(value, &mut position)?;
        skip_ows(bytes, &mut position);
        if bytes.get(position) != Some(&b'=') {
          return Err(invalid_value());
        }
        position += 1;
        skip_ows(bytes, &mut position);
        if name.eq_ignore_ascii_case("timeout") {
          let parsed = parse_delta_seconds(value, &mut position)?;
          if timeout.is_some() {
            return Err(KeepAliveParseError::new(
              "duplicate Keep-Alive timeout parameter",
            ));
          }
          timeout = Some(parsed);
        } else if name.eq_ignore_ascii_case("max") {
          let parsed = parse_delta_seconds(value, &mut position)?;
          if max.is_some() {
            return Err(KeepAliveParseError::new(
              "duplicate Keep-Alive max parameter",
            ));
          }
          max = Some(parsed);
        } else {
          extensions.push(KeepAliveExtension {
            name: name.to_string(),
            value: parse_token(value, &mut position)?.to_string(),
          });
        }
        skip_ows(bytes, &mut position);
        if position == bytes.len() {
          break;
        }
        if bytes[position] != b',' {
          return Err(invalid_value());
        }
        position += 1;
        skip_ows(bytes, &mut position);
        if position == bytes.len() {
          return Err(invalid_value());
        }
      }
    }
    if items == 0 {
      return Err(invalid_value());
    }
    Ok(KeepAlive {
      timeout,
      max,
      extensions,
    })
  }

  pub fn timeout(&self) -> Option<u64> {
    self.timeout
  }

  pub fn max(&self) -> Option<u64> {
    self.max
  }

  pub fn extensions(&self) -> &[KeepAliveExtension] {
    &self.extensions
  }

  pub fn header_value(&self) -> String {
    let mut members = Vec::new();
    if let Some(timeout) = self.timeout {
      members.push(format!("timeout={timeout}"));
    }
    if let Some(max) = self.max {
      members.push(format!("max={max}"));
    }
    members.extend(self.extensions.iter().map(KeepAliveExtension::header_value));
    members.join(", ")
  }
}

impl KeepAliveExtension {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  fn header_value(&self) -> String {
    format!("{}={}", self.name, self.value)
  }
}

fn parse_token<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, KeepAliveParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  while bytes
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if *position == start {
    return Err(invalid_value());
  }
  Ok(&value[start..*position])
}

fn parse_delta_seconds(value: &str, position: &mut usize) -> Result<u64, KeepAliveParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  while bytes
    .get(*position)
    .is_some_and(|byte| byte.is_ascii_digit())
  {
    *position += 1;
  }
  if *position == start {
    return Err(invalid_value());
  }
  value[start..*position]
    .parse::<u64>()
    .map_err(|_| KeepAliveParseError::new("Keep-Alive value is out of range"))
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn invalid_value() -> KeepAliveParseError {
  KeepAliveParseError::new("invalid Keep-Alive value")
}
