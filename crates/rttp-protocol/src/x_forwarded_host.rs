//! Bounded, policy-free parsing for `X-Forwarded-Host` request metadata.
//!
//! This module validates ordered host authority values only. It does not decide
//! which proxy hops are trusted, change virtual-host routing, or rewrite the
//! request authority.

use std::error::Error;
use std::fmt;

use crate::host::{Host, HostParseError};

/// Maximum bytes accepted in one `X-Forwarded-Host` field value, in the
/// combined raw field set including `", "` separator overhead, and in the
/// combined serialized field value.
pub const MAX_X_FORWARDED_HOST_VALUE_BYTES: usize = 64 * 1024;
/// Maximum `X-Forwarded-Host` authority values accepted across all fields.
pub const MAX_X_FORWARDED_HOSTS: usize = 256;

/// Parsed, bounded `X-Forwarded-Host` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedHost {
  hosts: Vec<Host>,
}

/// An error returned when `X-Forwarded-Host` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XForwardedHostParseError {
  message: String,
}

impl XForwardedHostParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for XForwardedHostParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for XForwardedHostParseError {}

impl From<HostParseError> for XForwardedHostParseError {
  fn from(_: HostParseError) -> Self {
    invalid_host()
  }
}

impl XForwardedHost {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, XForwardedHostParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, XForwardedHostParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut hosts = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      validate_value(value, &mut total_bytes)?;
      parse_field(value, &mut hosts)?;
    }
    if hosts.is_empty() {
      return Err(invalid_host());
    }
    let forwarded_host = Self { hosts };
    if forwarded_host.header_value().len() > MAX_X_FORWARDED_HOST_VALUE_BYTES {
      return Err(XForwardedHostParseError::new(
        "X-Forwarded-Host header value is too large",
      ));
    }
    Ok(forwarded_host)
  }

  pub fn hosts(&self) -> &[Host] {
    &self.hosts
  }

  pub fn len(&self) -> usize {
    self.hosts.len()
  }

  pub fn is_empty(&self) -> bool {
    self.hosts.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .hosts
      .iter()
      .map(Host::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

fn validate_value(value: &str, total_bytes: &mut usize) -> Result<(), XForwardedHostParseError> {
  if value.len() > MAX_X_FORWARDED_HOST_VALUE_BYTES {
    return Err(XForwardedHostParseError::new(
      "X-Forwarded-Host header value is too large",
    ));
  }
  let separator = if *total_bytes > 0 { 2 } else { 0 };
  *total_bytes = total_bytes
    .saturating_add(separator)
    .saturating_add(value.len());
  if *total_bytes > MAX_X_FORWARDED_HOST_VALUE_BYTES {
    return Err(XForwardedHostParseError::new(
      "X-Forwarded-Host header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(XForwardedHostParseError::new(
      "invalid X-Forwarded-Host control byte",
    ));
  }
  Ok(())
}

fn parse_field(value: &str, hosts: &mut Vec<Host>) -> Result<(), XForwardedHostParseError> {
  for raw_host in value.split(',') {
    if hosts.len() >= MAX_X_FORWARDED_HOSTS {
      return Err(XForwardedHostParseError::new(
        "too many X-Forwarded-Host values",
      ));
    }
    let value = raw_host.trim_matches([' ', '\t']);
    if value.is_empty() {
      return Err(invalid_host());
    }
    hosts.push(Host::parse(value)?);
  }
  Ok(())
}

fn invalid_host() -> XForwardedHostParseError {
  XForwardedHostParseError::new("invalid X-Forwarded-Host value")
}
