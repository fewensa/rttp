//! Bounded, policy-free `Upgrade` metadata parsing.
//!
//! This module validates one or more HTTP/1 `Upgrade` field values as an
//! ordered list of protocol-name tokens. Callers own connection semantics and
//! any bytes transferred after a successful upgrade.

use std::error::Error;
use std::fmt;

pub const MAX_UPGRADE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_UPGRADE_PROTOCOLS: usize = 32;

/// Parsed, bounded `Upgrade` metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upgrade {
  protocols: Vec<String>,
}

impl Upgrade {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, UpgradeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, UpgradeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut protocols = Vec::new();

    for value in values {
      if value.len() > MAX_UPGRADE_VALUE_BYTES {
        return Err(UpgradeParseError::new("Upgrade header value is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(UpgradeParseError::new("invalid Upgrade control byte"));
      }
      for member in value.split(',') {
        let protocol = member.trim_matches([' ', '\t']);
        if protocol.is_empty() || !is_http_token(protocol) {
          return Err(UpgradeParseError::new("invalid Upgrade protocol"));
        }
        if protocols.len() >= MAX_UPGRADE_PROTOCOLS {
          return Err(UpgradeParseError::new("too many Upgrade protocols"));
        }
        protocols.push(protocol.to_owned());
      }
    }

    if protocols.is_empty() {
      return Err(UpgradeParseError::new("invalid Upgrade protocol"));
    }

    Ok(Self { protocols })
  }

  pub fn protocols(&self) -> Vec<&str> {
    self.protocols.iter().map(String::as_str).collect()
  }

  pub fn len(&self) -> usize {
    self.protocols.len()
  }

  pub fn is_empty(&self) -> bool {
    self.protocols.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.protocols.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeParseError {
  message: String,
}

impl UpgradeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for UpgradeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for UpgradeParseError {}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_http_token_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!'
        | b'#'
        | b'$'
        | b'%'
        | b'&'
        | b'\''
        | b'*'
        | b'+'
        | b'-'
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~'
    )
}
