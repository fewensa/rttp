//! Bounded, policy-free `Proxy-Authentication-Info` response metadata parsing.
//!
//! This module validates one response field value only. Callers decide whether
//! and how to apply digest or proxy authentication policy.

use std::error::Error;
use std::fmt;

pub const MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS: usize = 256;
pub const MAX_PROXY_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Proxy-Authentication-Info` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticationInfo {
  parameters: Vec<ProxyAuthenticationInfoParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticationInfoParameter {
  name: String,
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticationInfoParseError {
  message: String,
}

impl ProxyAuthenticationInfoParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ProxyAuthenticationInfoParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ProxyAuthenticationInfoParseError {}

impl ProxyAuthenticationInfo {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ProxyAuthenticationInfoParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ProxyAuthenticationInfoParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parameters = Vec::new();
    for value in values {
      if value.len() > MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES {
        return Err(ProxyAuthenticationInfoParseError::new(
          "Proxy-Authentication-Info header value is too large",
        ));
      }
      parse_parameters(value, &mut parameters)?;
    }
    if parameters.is_empty() {
      return Err(ProxyAuthenticationInfoParseError::new(
        "invalid Proxy-Authentication-Info parameter",
      ));
    }
    Ok(Self { parameters })
  }

  pub fn parameters(&self) -> &[ProxyAuthenticationInfoParameter] {
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
      .map(ProxyAuthenticationInfoParameter::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ProxyAuthenticationInfoParameter {
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

fn parse_parameters(
  value: &str,
  parameters: &mut Vec<ProxyAuthenticationInfoParameter>,
) -> Result<(), ProxyAuthenticationInfoParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ProxyAuthenticationInfoParseError::new(
      "invalid Proxy-Authentication-Info parameter",
    ));
  }
  while position < bytes.len() {
    let name = parse_token(
      value,
      &mut position,
      "invalid Proxy-Authentication-Info parameter name",
    )?
    .to_ascii_lowercase();
    skip_ows(bytes, &mut position);
    if bytes.get(position) != Some(&b'=') {
      return Err(ProxyAuthenticationInfoParseError::new(
        "invalid Proxy-Authentication-Info parameter",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    let parameter_value = parse_parameter_value(value, &mut position)?;
    if parameter_value.len() > MAX_PROXY_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES {
      return Err(ProxyAuthenticationInfoParseError::new(
        "Proxy-Authentication-Info parameter value is too large",
      ));
    }
    if parameters
      .iter()
      .any(|known| known.name.eq_ignore_ascii_case(&name))
    {
      return Err(ProxyAuthenticationInfoParseError::new(
        "duplicate Proxy-Authentication-Info parameter",
      ));
    }
    if parameters.len() >= MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS {
      return Err(ProxyAuthenticationInfoParseError::new(
        "too many Proxy-Authentication-Info parameters",
      ));
    }
    parameters.push(ProxyAuthenticationInfoParameter {
      name,
      value: parameter_value,
    });
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(ProxyAuthenticationInfoParseError::new(
        "invalid Proxy-Authentication-Info parameter",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ProxyAuthenticationInfoParseError::new(
        "invalid Proxy-Authentication-Info parameter",
      ));
    }
  }
  Ok(())
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<String, ProxyAuthenticationInfoParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) == Some(&b'"') {
    *position += 1;
    let mut parsed = String::new();
    let mut unescaped_start = *position;
    let mut escaped = false;
    while *position < bytes.len() {
      let byte = bytes[*position];
      if escaped {
        *position += 1;
        if !(byte == b'\t' || (0x20..=0x7e).contains(&byte)) {
          return Err(ProxyAuthenticationInfoParseError::new(
            "invalid Proxy-Authentication-Info quoted-string",
          ));
        }
        parsed.push(byte as char);
        escaped = false;
        unescaped_start = *position;
      } else if byte == b'\\' {
        parsed.push_str(&value[unescaped_start..*position]);
        *position += 1;
        escaped = true;
      } else if byte == b'"' {
        parsed.push_str(&value[unescaped_start..*position]);
        *position += 1;
        return Ok(parsed);
      } else if !(byte == b'\t'
        || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff))
      {
        return Err(ProxyAuthenticationInfoParseError::new(
          "invalid Proxy-Authentication-Info quoted-string",
        ));
      } else {
        *position += 1;
      }
    }
    Err(ProxyAuthenticationInfoParseError::new(
      "invalid Proxy-Authentication-Info quoted-string",
    ))
  } else {
    Ok(
      parse_token(
        value,
        position,
        "invalid Proxy-Authentication-Info parameter value",
      )?
      .to_string(),
    )
  }
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, ProxyAuthenticationInfoParseError> {
  let start = *position;
  let bytes = value.as_bytes();
  while *position < bytes.len() && is_token_byte(bytes[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(ProxyAuthenticationInfoParseError::new(message))
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
fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}
fn is_token_byte(byte: u8) -> bool {
  matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}
