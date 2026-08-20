//! Bounded, policy-free parsing for the HTTP `Alt-Used` response header.
//!
//! This module validates one `uri-host` plus optional port using the shared
//! authority grammar. It reports declared response metadata only; callers own
//! alternative service selection, origin handling, and connection policy.

use std::error::Error;
use std::fmt;

use crate::host::{Host, HostParseError, MAX_HOST_VALUE_BYTES};

/// Maximum bytes accepted in an `Alt-Used` field value.
pub const MAX_ALT_USED_VALUE_BYTES: usize = MAX_HOST_VALUE_BYTES;

/// A parsed HTTP `Alt-Used` field value.
///
/// The stored text is the OWS-trimmed host and optional port from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AltUsed {
  authority: Host,
}

/// An error returned when `Alt-Used` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AltUsedParseError {
  message: String,
}

impl AltUsed {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AltUsedParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AltUsedParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Host::parse_values(values)
      .map(|authority| Self { authority })
      .map_err(AltUsedParseError::from_host)
  }

  pub fn host(&self) -> &str {
    self.authority.host()
  }

  pub fn port(&self) -> Option<&str> {
    self.authority.port()
  }

  pub fn header_value(&self) -> String {
    self.authority.header_value()
  }
}

impl AltUsedParseError {
  fn from_host(error: HostParseError) -> Self {
    let message = error
      .to_string()
      .replace("Host", "Alt-Used")
      .replace("host", "Alt-Used");
    Self { message }
  }
}

impl fmt::Display for AltUsedParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AltUsedParseError {}
