//! Bounded, policy-free parsing for the HTTP `Host` request header.
//!
//! This module validates one `uri-host` plus optional port using the inbound
//! Host authority grammar (`host[:port]`, including bracketed IPv6). Callers
//! retain virtual-host routing, origin comparison, and scheme defaults.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `Host` field value.
pub const MAX_HOST_VALUE_BYTES: usize = 64 * 1024;

/// A parsed HTTP `Host` field value.
///
/// The stored text is the OWS-trimmed host and optional port from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Host {
  host: String,
  port: Option<String>,
}

/// An error returned when `Host` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostParseError {
  message: String,
}

impl Host {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HostParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HostParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_authority(value)
  }

  pub fn host(&self) -> &str {
    &self.host
  }

  pub fn port(&self) -> Option<&str> {
    self.port.as_deref()
  }

  pub fn header_value(&self) -> String {
    match &self.port {
      Some(port) => format!("{}:{port}", self.host),
      None => self.host.clone(),
    }
  }
}

impl HostParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HostParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HostParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, HostParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_value(value)?;
  }
  if has_duplicate {
    return Err(HostParseError::new("duplicate Host header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_value(value: &str) -> Result<(), HostParseError> {
  if value.len() > MAX_HOST_VALUE_BYTES {
    return Err(HostParseError::new("Host header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(HostParseError::new("invalid Host header control byte"));
  }
  Ok(())
}

fn parse_authority(value: &str) -> Result<Host, HostParseError> {
  if value
    .bytes()
    .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'@'))
  {
    return Err(invalid_value());
  }

  if let Some(rest) = value.strip_prefix('[') {
    let Some(end) = rest.find(']') else {
      return Err(invalid_value());
    };
    let host = &rest[..end];
    let suffix = &rest[end + 1..];
    if host.is_empty() || host.bytes().any(|byte| matches!(byte, b'[' | b']')) {
      return Err(invalid_value());
    }
    let port = if suffix.is_empty() {
      None
    } else {
      let Some(port) = suffix.strip_prefix(':') else {
        return Err(invalid_value());
      };
      if !is_valid_port(port) {
        return Err(invalid_value());
      }
      Some(port.to_string())
    };
    return Ok(Host {
      host: format!("[{host}]"),
      port,
    });
  }

  let colon_count = value.bytes().filter(|byte| *byte == b':').count();
  match colon_count {
    0 if is_valid_reg_name_or_ipv4(value) => Ok(Host {
      host: value.to_string(),
      port: None,
    }),
    1 => {
      let Some((host, port)) = value.rsplit_once(':') else {
        return Err(invalid_value());
      };
      if !is_valid_reg_name_or_ipv4(host) || !is_valid_port(port) {
        return Err(invalid_value());
      }
      Ok(Host {
        host: host.to_string(),
        port: Some(port.to_string()),
      })
    }
    _ => Err(invalid_value()),
  }
}

fn is_valid_reg_name_or_ipv4(host: &str) -> bool {
  !host.is_empty()
    && host
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
}

fn is_valid_port(port: &str) -> bool {
  !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

fn invalid_value() -> HostParseError {
  HostParseError::new("invalid Host header value")
}
