//! Bounded, policy-free `Proxy-Authenticate` challenge metadata.
//!
//! This module parses proxy authentication challenges so callers can inspect
//! proxy schemes and parameters. It does not select a challenge, generate
//! `Proxy-Authorization`, retry requests, or verify authentication state.

use std::error::Error;
use std::fmt;

pub const MAX_PROXY_AUTHENTICATE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PROXY_AUTHENTICATE_CHALLENGES: usize = 256;
pub const MAX_PROXY_AUTHENTICATE_PARAMETERS: usize = 256;
pub const MAX_PROXY_AUTHENTICATE_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Proxy-Authenticate` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticate {
  challenges: Vec<ProxyAuthenticateChallenge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticateChallenge {
  scheme: String,
  token68: Option<String>,
  parameters: Vec<ProxyAuthenticateParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticateParameter {
  name: String,
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthenticateParseError {
  message: String,
}

impl ProxyAuthenticateParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ProxyAuthenticateParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ProxyAuthenticateParseError {}

impl ProxyAuthenticate {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ProxyAuthenticateParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ProxyAuthenticateParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut challenges = Vec::new();
    let mut combined = String::new();
    let mut saw_value = false;
    for value in values {
      if value.len() > MAX_PROXY_AUTHENTICATE_VALUE_BYTES {
        return Err(ProxyAuthenticateParseError::new(
          "Proxy-Authenticate header value is too large",
        ));
      }
      let separator_len = if saw_value { 2 } else { 0 };
      let combined_len = combined
        .len()
        .checked_add(separator_len)
        .and_then(|length| length.checked_add(value.len()))
        .ok_or_else(|| {
          ProxyAuthenticateParseError::new("Proxy-Authenticate header value is too large")
        })?;
      if combined_len > MAX_PROXY_AUTHENTICATE_VALUE_BYTES {
        return Err(ProxyAuthenticateParseError::new(
          "Proxy-Authenticate header value is too large",
        ));
      }
      if saw_value {
        combined.push_str(", ");
      }
      combined.push_str(value);
      saw_value = true;
    }
    parse_field(&combined, &mut challenges)?;
    if challenges.is_empty() {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate challenge",
      ));
    }
    Ok(Self { challenges })
  }

  pub fn challenges(&self) -> &[ProxyAuthenticateChallenge] {
    &self.challenges
  }

  pub fn len(&self) -> usize {
    self.challenges.len()
  }

  pub fn is_empty(&self) -> bool {
    self.challenges.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .challenges
      .iter()
      .map(ProxyAuthenticateChallenge::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ProxyAuthenticateChallenge {
  pub fn scheme(&self) -> &str {
    &self.scheme
  }
  pub fn token68(&self) -> Option<&str> {
    self.token68.as_deref()
  }
  pub fn parameters(&self) -> &[ProxyAuthenticateParameter] {
    &self.parameters
  }
  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|parameter| parameter.value.as_str())
  }
  fn header_value(&self) -> String {
    let mut value = self.scheme.clone();
    if let Some(token68) = &self.token68 {
      value.push(' ');
      value.push_str(token68);
    } else if !self.parameters.is_empty() {
      value.push(' ');
      value.push_str(
        &self
          .parameters
          .iter()
          .map(ProxyAuthenticateParameter::header_value)
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    value
  }
}

impl ProxyAuthenticateParameter {
  pub fn name(&self) -> &str {
    &self.name
  }
  pub fn value(&self) -> &str {
    &self.value
  }
  fn header_value(&self) -> String {
    if !self.name.eq_ignore_ascii_case("realm") && is_token(&self.value) {
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
  challenges: &mut Vec<ProxyAuthenticateChallenge>,
) -> Result<(), ProxyAuthenticateParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ProxyAuthenticateParseError::new(
      "invalid Proxy-Authenticate challenge",
    ));
  }
  while position < bytes.len() {
    let scheme =
      parse_token(value, &mut position, "invalid Proxy-Authenticate scheme")?.to_string();
    if challenges.len() >= MAX_PROXY_AUTHENTICATE_CHALLENGES {
      return Err(ProxyAuthenticateParseError::new(
        "too many Proxy-Authenticate challenges",
      ));
    }
    let before_whitespace = position;
    skip_ows(bytes, &mut position);
    let mut challenge = ProxyAuthenticateChallenge {
      scheme,
      token68: None,
      parameters: Vec::new(),
    };
    if position < bytes.len() && bytes[position] != b',' {
      if position == before_whitespace {
        return Err(ProxyAuthenticateParseError::new(
          "invalid Proxy-Authenticate challenge",
        ));
      }
      if looks_like_parameter(value, position) {
        parse_parameters(value, &mut position, &mut challenge.parameters)?;
      } else {
        let start = position;
        while position < bytes.len() && bytes[position] != b',' && !is_ows(bytes[position]) {
          position += 1;
        }
        let token68 = &value[start..position];
        if !is_token68(token68) {
          return Err(ProxyAuthenticateParseError::new(
            "invalid Proxy-Authenticate token68",
          ));
        }
        challenge.token68 = Some(token68.to_string());
        skip_ows(bytes, &mut position);
      }
    }
    challenges.push(challenge);
    if position == bytes.len() {
      break;
    }
    if bytes[position] != b',' {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate challenge",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate challenge",
      ));
    }
  }
  Ok(())
}

fn parse_parameters(
  value: &str,
  position: &mut usize,
  parameters: &mut Vec<ProxyAuthenticateParameter>,
) -> Result<(), ProxyAuthenticateParseError> {
  loop {
    let name = parse_token(value, position, "invalid Proxy-Authenticate parameter name")?
      .to_ascii_lowercase();
    skip_ows(value.as_bytes(), position);
    if value.as_bytes().get(*position) != Some(&b'=') {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate parameter",
      ));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    let parameter_value = parse_parameter_value(value, position)?;
    if parameter_value.len() > MAX_PROXY_AUTHENTICATE_PARAMETER_VALUE_BYTES {
      return Err(ProxyAuthenticateParseError::new(
        "Proxy-Authenticate parameter value is too large",
      ));
    }
    if parameters
      .iter()
      .any(|known| known.name.eq_ignore_ascii_case(&name))
    {
      return Err(ProxyAuthenticateParseError::new(
        "duplicate Proxy-Authenticate parameter",
      ));
    }
    if parameters.len() >= MAX_PROXY_AUTHENTICATE_PARAMETERS {
      return Err(ProxyAuthenticateParseError::new(
        "too many Proxy-Authenticate parameters",
      ));
    }
    parameters.push(ProxyAuthenticateParameter {
      name,
      value: parameter_value,
    });
    skip_ows(value.as_bytes(), position);
    if *position == value.len() {
      return Ok(());
    }
    if value.as_bytes()[*position] != b',' {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate parameter",
      ));
    }
    let comma = *position;
    *position += 1;
    skip_ows(value.as_bytes(), position);
    if *position == value.len() {
      return Err(ProxyAuthenticateParseError::new(
        "invalid Proxy-Authenticate parameter",
      ));
    }
    if !looks_like_parameter(value, *position) {
      *position = comma;
      return Ok(());
    }
  }
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<String, ProxyAuthenticateParseError> {
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
          return Err(ProxyAuthenticateParseError::new(
            "invalid Proxy-Authenticate quoted-string",
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
        return Err(ProxyAuthenticateParseError::new(
          "invalid Proxy-Authenticate quoted-string",
        ));
      } else {
        *position += 1;
      }
    }
    Err(ProxyAuthenticateParseError::new(
      "invalid Proxy-Authenticate quoted-string",
    ))
  } else {
    Ok(
      parse_token(
        value,
        position,
        "invalid Proxy-Authenticate parameter value",
      )?
      .to_string(),
    )
  }
}

fn looks_like_parameter(value: &str, mut position: usize) -> bool {
  let bytes = value.as_bytes();
  let start = position;
  while position < bytes.len() && is_token_byte(bytes[position]) {
    position += 1;
  }
  if position > start && bytes.get(position) == Some(&b'=') {
    if value[start..position].eq_ignore_ascii_case("realm") {
      return true;
    }
    let mut end = position;
    while end < bytes.len() && bytes[end] != b',' && !is_ows(bytes[end]) {
      end += 1;
    }
    if is_token68(&value[start..end]) {
      return false;
    }
  }
  position > start && {
    skip_ows(bytes, &mut position);
    bytes.get(position) == Some(&b'=')
  }
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, ProxyAuthenticateParseError> {
  let start = *position;
  let bytes = value.as_bytes();
  while *position < bytes.len() && is_token_byte(bytes[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(ProxyAuthenticateParseError::new(message))
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
fn is_token68(value: &str) -> bool {
  let value = value.as_bytes();
  let mut base = 0;
  while base < value.len()
    && matches!(value[base], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'+' | b'/')
  {
    base += 1;
  }
  base > 0 && value[base..].iter().all(|byte| *byte == b'=')
}
fn is_token_byte(byte: u8) -> bool {
  matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}
