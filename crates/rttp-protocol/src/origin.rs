//! Bounded, policy-free parsing for the HTTP `Origin` request header.
//!
//! This module only validates one serialized HTTP(S) origin or the opaque
//! `null` value. Callers retain responsibility for origin comparison and CORS
//! policy decisions.

use std::error::Error;
use std::fmt;
use url::Host;

pub const MAX_ORIGIN_VALUE_BYTES: usize = 64 * 1024;

/// An HTTP `Origin` field value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Origin {
  /// The opaque origin serialized as `null`.
  Null,
  /// An HTTP(S) tuple origin.
  Tuple(OriginTuple),
}

/// A parsed HTTP(S) tuple origin.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OriginTuple {
  scheme: OriginScheme,
  host: String,
  port: Option<u16>,
}

/// The scheme of an HTTP(S) tuple origin.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OriginScheme {
  Http,
  Https,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginParseError {
  message: String,
}

impl Origin {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, OriginParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, OriginParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    if value == "null" {
      return Ok(Self::Null);
    }
    parse_tuple(value).map(Self::Tuple)
  }

  pub fn header_value(&self) -> String {
    match self {
      Self::Null => "null".to_string(),
      Self::Tuple(origin) => origin.header_value(),
    }
  }

  pub fn tuple(&self) -> Option<&OriginTuple> {
    match self {
      Self::Null => None,
      Self::Tuple(origin) => Some(origin),
    }
  }
}

impl OriginTuple {
  pub fn scheme(&self) -> OriginScheme {
    self.scheme
  }

  pub fn host(&self) -> &str {
    &self.host
  }

  pub fn port(&self) -> Option<u16> {
    self.port
  }

  pub fn header_value(&self) -> String {
    let scheme = self.scheme.as_str();
    match self.port {
      Some(port) => format!("{scheme}://{}:{port}", self.host),
      None => format!("{scheme}://{}", self.host),
    }
  }
}

impl OriginScheme {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Http => "http",
      Self::Https => "https",
    }
  }

  fn default_port(self) -> u16 {
    match self {
      Self::Http => 80,
      Self::Https => 443,
    }
  }
}

impl OriginParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for OriginParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for OriginParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, OriginParseError>
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
    return Err(OriginParseError::new("duplicate Origin header fields"));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() || value.contains(',') {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_value(value: &str) -> Result<(), OriginParseError> {
  if value.len() > MAX_ORIGIN_VALUE_BYTES {
    return Err(OriginParseError::new("Origin header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(OriginParseError::new("invalid Origin header control byte"));
  }
  Ok(())
}

fn parse_tuple(value: &str) -> Result<OriginTuple, OriginParseError> {
  let (scheme, authority) = value.split_once("://").ok_or_else(invalid_value)?;
  let scheme = match scheme {
    "http" => OriginScheme::Http,
    "https" => OriginScheme::Https,
    _ => return Err(invalid_value()),
  };
  let (host, port) = parse_authority(authority)?;
  let port = port.filter(|port| *port != scheme.default_port());
  Ok(OriginTuple { scheme, host, port })
}

fn parse_authority(value: &str) -> Result<(String, Option<u16>), OriginParseError> {
  if value.is_empty() || value.contains(['/', '?', '#', '@']) {
    return Err(invalid_value());
  }
  if let Some(value) = value.strip_prefix('[') {
    let (host, port) = value.split_once(']').ok_or_else(invalid_value)?;
    let host = Host::parse(&format!("[{host}]")).map_err(|_| invalid_value())?;
    if !matches!(host, Host::Ipv6(_)) {
      return Err(invalid_value());
    }
    let port = match port {
      "" => None,
      port => parse_port(port.strip_prefix(':').ok_or_else(invalid_value)?)?,
    };
    return Ok((host.to_string(), port));
  }
  let (host, port) = match value.split_once(':') {
    Some((host, port)) => {
      if port.contains(':') {
        return Err(invalid_value());
      }
      if port.is_empty() {
        return Err(invalid_value());
      }
      (host, parse_port(port)?)
    }
    None => (value, None),
  };
  let host = Host::parse(host).map_err(|_| invalid_value())?;
  Ok((host.to_string(), port))
}

fn parse_port(value: &str) -> Result<Option<u16>, OriginParseError> {
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(invalid_value());
  }
  value.parse().map(Some).map_err(|_| invalid_value())
}

fn invalid_value() -> OriginParseError {
  OriginParseError::new("invalid Origin header value")
}
