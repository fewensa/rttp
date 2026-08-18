//! Bounded, policy-free `Authentication-Info` response metadata parsing.
//!
//! This module validates RFC 7615 / RFC 9110 `#auth-param` lists only. Any
//! well-formed parameter name is accepted; Digest names such as `nextnonce`
//! and `rspauth` are ordinary opaque metadata. Callers own scheme policy,
//! including `rspauth` verification, nonce bookkeeping, credential storage,
//! and `Authorization` generation.
//!
//! ```
//! use rttp_protocol::authentication_info::AuthenticationInfo;
//!
//! let info = AuthenticationInfo::parse(
//!   r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth, rspauth="6629fae49393a05397450978507c4ef1", cnonce="0a4f113b", nc=00000001"#,
//! )
//! .expect("valid Authentication-Info");
//! assert_eq!(
//!   info.parameter("nextnonce"),
//!   Some("6629fae49393a05397450978507c4ef1")
//! );
//! assert_eq!(info.parameter("qop"), Some("auth"));
//! assert_eq!(info.parameter("nc"), Some("00000001"));
//! ```

use std::error::Error;
use std::fmt;

use crate::http1::{is_qdtext, is_quoted_pair_char, is_token, is_token_byte};

/// Maximum bytes accepted in an `Authentication-Info` field value.
pub const MAX_AUTHENTICATION_INFO_VALUE_BYTES: usize = 64 * 1024;
/// Maximum auth-param members accepted across the combined field set.
pub const MAX_AUTHENTICATION_INFO_PARAMETERS: usize = 256;
/// Maximum bytes accepted in a single unescaped auth-param value.
pub const MAX_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Authentication-Info` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticationInfo {
  parameters: Vec<AuthenticationInfoParameter>,
}

/// A single `auth-param` name and value from `Authentication-Info`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticationInfoParameter {
  name: String,
  value: String,
}

/// An error returned when `Authentication-Info` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticationInfoParseError {
  message: String,
}

impl AuthenticationInfoParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AuthenticationInfoParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AuthenticationInfoParseError {}

impl AuthenticationInfo {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AuthenticationInfoParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AuthenticationInfoParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parameters = Vec::new();
    for value in values {
      if value.len() > MAX_AUTHENTICATION_INFO_VALUE_BYTES {
        return Err(AuthenticationInfoParseError::new(
          "Authentication-Info header value is too large",
        ));
      }
      parse_field(value, &mut parameters)?;
    }
    if parameters.is_empty() {
      return Err(AuthenticationInfoParseError::new(
        "invalid Authentication-Info parameter",
      ));
    }
    Ok(Self { parameters })
  }

  pub fn parameters(&self) -> &[AuthenticationInfoParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|parameter| parameter.value.as_str())
  }

  pub fn len(&self) -> usize {
    self.parameters.len()
  }

  pub fn is_empty(&self) -> bool {
    self.parameters.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .parameters
      .iter()
      .map(AuthenticationInfoParameter::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AuthenticationInfoParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  fn header_value(&self) -> String {
    if is_token(&self.value) {
      format!("{}={}", self.name, self.value)
    } else {
      format!(
        "{}=\"{}\"",
        self.name,
        self.value.replace('\\', "\\\\").replace('"', "\\\"")
      )
    }
  }
}

fn parse_field(
  value: &str,
  parameters: &mut Vec<AuthenticationInfoParameter>,
) -> Result<(), AuthenticationInfoParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(AuthenticationInfoParseError::new(
      "invalid Authentication-Info parameter",
    ));
  }
  loop {
    let name = parse_token(
      value,
      &mut position,
      "invalid Authentication-Info parameter",
    )?
    .to_ascii_lowercase();
    skip_ows(bytes, &mut position);
    if bytes.get(position) != Some(&b'=') {
      return Err(AuthenticationInfoParseError::new(
        "invalid Authentication-Info parameter",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    let parameter_value = parse_parameter_value(value, &mut position)?;
    if parameter_value.len() > MAX_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES {
      return Err(AuthenticationInfoParseError::new(
        "Authentication-Info parameter value is too large",
      ));
    }
    if parameters
      .iter()
      .any(|known| known.name.eq_ignore_ascii_case(&name))
    {
      return Err(AuthenticationInfoParseError::new(
        "duplicate Authentication-Info parameter",
      ));
    }
    if parameters.len() >= MAX_AUTHENTICATION_INFO_PARAMETERS {
      return Err(AuthenticationInfoParseError::new(
        "too many Authentication-Info parameters",
      ));
    }
    parameters.push(AuthenticationInfoParameter {
      name,
      value: parameter_value,
    });
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(AuthenticationInfoParseError::new(
        "invalid Authentication-Info parameter",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(AuthenticationInfoParseError::new(
        "invalid Authentication-Info parameter",
      ));
    }
  }
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<String, AuthenticationInfoParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) == Some(&b'"') {
    *position += 1;
    let mut parsed = Vec::new();
    while let Some(&byte) = bytes.get(*position) {
      *position += 1;
      match byte {
        b'"' => {
          return String::from_utf8(parsed).map_err(|_| {
            AuthenticationInfoParseError::new("invalid Authentication-Info parameter")
          });
        }
        b'\\' => {
          let Some(&escaped) = bytes.get(*position) else {
            return Err(AuthenticationInfoParseError::new(
              "invalid Authentication-Info parameter",
            ));
          };
          if !is_quoted_pair_char(escaped) {
            return Err(AuthenticationInfoParseError::new(
              "invalid Authentication-Info parameter",
            ));
          }
          *position += 1;
          parsed.push(escaped);
        }
        _ if is_qdtext(byte) => parsed.push(byte),
        _ => {
          return Err(AuthenticationInfoParseError::new(
            "invalid Authentication-Info parameter",
          ));
        }
      }
    }
    Err(AuthenticationInfoParseError::new(
      "invalid Authentication-Info parameter",
    ))
  } else {
    Ok(parse_token(value, position, "invalid Authentication-Info parameter")?.to_string())
  }
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, AuthenticationInfoParseError> {
  let start = *position;
  let bytes = value.as_bytes();
  while *position < bytes.len() && is_token_byte(bytes[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(AuthenticationInfoParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while *position < bytes.len() && is_ows(bytes[*position]) {
    *position += 1;
  }
}

fn is_ows(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t')
}
