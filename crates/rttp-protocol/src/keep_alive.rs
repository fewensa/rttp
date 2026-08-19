//! Bounded, policy-free RFC 2068 `Keep-Alive` response metadata parsing.
//!
//! This module validates the legacy HTTP/1.x `Keep-Alive` response field
//! syntax only, as metadata for applications that interoperate with HTTP/1
//! peers. It never changes connection lifetime, connection pooling, or
//! HTTP/2 behavior. Unparsable input is an error; this parser never fails
//! open.
//!
//! ```
//! use rttp_protocol::keep_alive::KeepAlive;
//!
//! let keep_alive = KeepAlive::parse("timeout=5, max=100").expect("valid Keep-Alive");
//! assert_eq!(keep_alive.timeout(), 5);
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeepAlive {
  timeout: u64,
  max: Option<u64>,
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
        let name = parse_name(value, &mut position)?;
        skip_ows(bytes, &mut position);
        if bytes.get(position) != Some(&b'=') {
          return Err(invalid_value());
        }
        position += 1;
        skip_ows(bytes, &mut position);
        let parsed = parse_delta_seconds(value, &mut position)?;
        if name.eq_ignore_ascii_case("timeout") {
          if timeout.is_some() {
            return Err(KeepAliveParseError::new(
              "duplicate Keep-Alive timeout parameter",
            ));
          }
          timeout = Some(parsed);
        } else if name.eq_ignore_ascii_case("max") {
          if max.is_some() {
            return Err(KeepAliveParseError::new(
              "duplicate Keep-Alive max parameter",
            ));
          }
          max = Some(parsed);
        } else {
          return Err(invalid_value());
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
    let Some(timeout) = timeout else {
      return Err(invalid_value());
    };
    Ok(KeepAlive { timeout, max })
  }

  pub fn timeout(&self) -> u64 {
    self.timeout
  }

  pub fn max(&self) -> Option<u64> {
    self.max
  }

  pub fn header_value(&self) -> String {
    match self.max {
      Some(max) => format!("timeout={}, max={}", self.timeout, max),
      None => format!("timeout={}", self.timeout),
    }
  }
}

fn parse_name<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, KeepAliveParseError> {
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
