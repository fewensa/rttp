use std::error::Error;
use std::fmt;

pub const MAX_ALT_SVC_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ALT_SVC_ALTERNATIVES: usize = 256;
pub const MAX_ALT_SVC_PARAMETERS: usize = 256;
pub const MAX_ALT_SVC_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Alt-Svc` response metadata. This metadata does not select
/// endpoints or migrate connections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AltSvc {
  clear: bool,
  alternatives: Vec<AltSvcAlternative>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AltSvcAlternative {
  protocol_id: String,
  authority: String,
  max_age: Option<u64>,
  persist: Option<bool>,
  parameters: Vec<AltSvcParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AltSvcParameter {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AltSvcParseError {
  message: String,
}

impl AltSvcParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AltSvcParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AltSvcParseError {}

impl AltSvc {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AltSvcParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AltSvcParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut alternatives = Vec::new();
    let mut clear = false;
    let mut seen_field = false;
    for value in values {
      if value.len() > MAX_ALT_SVC_VALUE_BYTES {
        return Err(AltSvcParseError::new("Alt-Svc header value is too large"));
      }
      if value.trim().is_empty() {
        return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
      }
      seen_field = true;
      let mut position = 0;
      loop {
        skip_ows(value.as_bytes(), &mut position);
        if alternatives.len() >= MAX_ALT_SVC_ALTERNATIVES {
          return Err(AltSvcParseError::new("too many Alt-Svc alternatives"));
        }
        if value[position..].starts_with("clear") {
          let after_clear = position + "clear".len();
          let next = value.as_bytes().get(after_clear);
          if matches!(next, None | Some(b' ' | b'\t' | b',')) {
            if clear || !alternatives.is_empty() {
              return Err(AltSvcParseError::new("Alt-Svc clear must be exclusive"));
            }
            clear = true;
            position = after_clear;
          } else {
            parse_alternative(value, &mut position, &mut alternatives)?;
          }
        } else {
          if clear {
            return Err(AltSvcParseError::new("Alt-Svc clear must be exclusive"));
          }
          parse_alternative(value, &mut position, &mut alternatives)?;
        }
        skip_ows(value.as_bytes(), &mut position);
        if position == value.len() {
          break;
        }
        if value.as_bytes()[position] != b',' {
          return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
        }
        position += 1;
        skip_ows(value.as_bytes(), &mut position);
        if position == value.len() {
          return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
        }
      }
    }
    if !seen_field || (!clear && alternatives.is_empty()) {
      return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
    }
    Ok(Self {
      clear,
      alternatives,
    })
  }

  pub fn is_clear(&self) -> bool {
    self.clear
  }
  pub fn alternatives(&self) -> &[AltSvcAlternative] {
    &self.alternatives
  }
  pub fn len(&self) -> usize {
    self.alternatives.len()
  }
  pub fn is_empty(&self) -> bool {
    self.alternatives.is_empty()
  }

  pub fn header_value(&self) -> String {
    if self.clear {
      "clear".to_string()
    } else {
      self
        .alternatives
        .iter()
        .map(AltSvcAlternative::header_value)
        .collect::<Vec<_>>()
        .join(", ")
    }
  }
}

impl AltSvcAlternative {
  pub fn protocol_id(&self) -> &str {
    &self.protocol_id
  }
  pub fn authority(&self) -> &str {
    &self.authority
  }
  pub fn max_age(&self) -> Option<u64> {
    self.max_age
  }
  pub fn persist(&self) -> Option<bool> {
    self.persist
  }
  pub fn parameters(&self) -> &[AltSvcParameter] {
    &self.parameters
  }

  fn header_value(&self) -> String {
    let mut value = format!(
      "{}=\"{}\"",
      self.protocol_id,
      escape_quoted(&self.authority)
    );
    if let Some(max_age) = self.max_age {
      value.push_str(&format!("; ma={max_age}"));
    }
    if let Some(persist) = self.persist {
      value.push_str(if persist {
        "; persist=1"
      } else {
        "; persist=0"
      });
    }
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(parameter.name());
      if let Some(parameter_value) = parameter.value() {
        value.push('=');
        if is_token(parameter_value) {
          value.push_str(parameter_value);
        } else {
          value.push('"');
          value.push_str(&escape_quoted(parameter_value));
          value.push('"');
        }
      }
    }
    value
  }
}

impl AltSvcParameter {
  pub fn name(&self) -> &str {
    &self.name
  }
  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

fn parse_alternative(
  value: &str,
  position: &mut usize,
  alternatives: &mut Vec<AltSvcAlternative>,
) -> Result<(), AltSvcParseError> {
  let protocol_id = parse_token(value, position, "invalid Alt-Svc protocol id")?.to_string();
  skip_ows(value.as_bytes(), position);
  if value.as_bytes().get(*position) != Some(&b'=') {
    return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
  }
  *position += 1;
  skip_ows(value.as_bytes(), position);
  let authority = parse_quoted_string(value, position)?;
  validate_authority(&authority)?;
  let mut alternative = AltSvcAlternative {
    protocol_id,
    authority,
    max_age: None,
    persist: None,
    parameters: Vec::new(),
  };
  let mut parameter_count = 0usize;
  loop {
    skip_ows(value.as_bytes(), position);
    if matches!(value.as_bytes().get(*position), None | Some(b',')) {
      break;
    }
    if value.as_bytes()[*position] != b';' {
      return Err(AltSvcParseError::new("invalid Alt-Svc entry"));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    parameter_count += 1;
    if parameter_count > MAX_ALT_SVC_PARAMETERS {
      return Err(AltSvcParseError::new("too many Alt-Svc parameters"));
    }
    parse_parameter(value, position, &mut alternative)?;
  }
  alternatives.push(alternative);
  Ok(())
}

fn parse_parameter(
  value: &str,
  position: &mut usize,
  alternative: &mut AltSvcAlternative,
) -> Result<(), AltSvcParseError> {
  let name = parse_token(value, position, "invalid Alt-Svc parameter name")?.to_ascii_lowercase();
  skip_ows(value.as_bytes(), position);
  if value.as_bytes().get(*position) != Some(&b'=') {
    return Err(AltSvcParseError::new("invalid Alt-Svc parameter"));
  }
  *position += 1;
  skip_ows(value.as_bytes(), position);
  let quoted_value = value.as_bytes().get(*position) == Some(&b'"');
  let parameter_value = Some(if quoted_value {
    parse_quoted_string(value, position)?
  } else {
    parse_token(value, position, "invalid Alt-Svc parameter value")?.to_string()
  });
  if parameter_value
    .as_ref()
    .is_some_and(|parameter| parameter.len() > MAX_ALT_SVC_PARAMETER_VALUE_BYTES)
  {
    return Err(AltSvcParseError::new(
      "Alt-Svc parameter value is too large",
    ));
  }
  match name.as_str() {
    "ma" => {
      if quoted_value {
        return Err(AltSvcParseError::new("invalid Alt-Svc ma parameter"));
      }
      let max_age = parameter_value
        .ok_or_else(|| AltSvcParseError::new("invalid Alt-Svc ma parameter"))?
        .parse::<u64>()
        .map_err(|_| AltSvcParseError::new("invalid Alt-Svc ma parameter"))?;
      if alternative.max_age.replace(max_age).is_some() {
        return Err(AltSvcParseError::new("duplicate Alt-Svc ma parameter"));
      }
    }
    "persist" => {
      if quoted_value {
        return Err(AltSvcParseError::new("invalid Alt-Svc persist parameter"));
      }
      let persist = match parameter_value.as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => return Err(AltSvcParseError::new("invalid Alt-Svc persist parameter")),
      };
      if alternative.persist.replace(persist).is_some() {
        return Err(AltSvcParseError::new("duplicate Alt-Svc persist parameter"));
      }
    }
    _ => {
      if alternative
        .parameters
        .iter()
        .any(|parameter| parameter.name.eq_ignore_ascii_case(&name))
      {
        return Err(AltSvcParseError::new("duplicate Alt-Svc parameter"));
      }
      alternative.parameters.push(AltSvcParameter {
        name,
        value: parameter_value,
      });
    }
  }
  Ok(())
}

fn validate_authority(authority: &str) -> Result<(), AltSvcParseError> {
  let Some((host, port)) = authority.rsplit_once(':') else {
    return Err(AltSvcParseError::new("invalid Alt-Svc authority"));
  };
  if port.is_empty()
    || !port.bytes().all(|byte| byte.is_ascii_digit())
    || port.parse::<u16>().is_err()
  {
    return Err(AltSvcParseError::new("invalid Alt-Svc authority"));
  }
  if host.is_empty() {
    return Ok(());
  }
  let valid_host = if host.starts_with('[') && host.ends_with(']') {
    host[1..host.len() - 1]
      .bytes()
      .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b':' | b'.'))
  } else {
    host
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
  };
  if valid_host {
    Ok(())
  } else {
    Err(AltSvcParseError::new("invalid Alt-Svc authority"))
  }
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, AltSvcParseError> {
  if value.as_bytes().get(*position) != Some(&b'"') {
    return Err(AltSvcParseError::new("invalid Alt-Svc quoted-string"));
  }
  *position += 1;
  let mut parsed = String::new();
  let mut unescaped_start = *position;
  let mut escaped = false;
  while let Some(&byte) = value.as_bytes().get(*position) {
    if escaped {
      *position += 1;
      if !(byte == b'\t' || (0x20..=0x7e).contains(&byte)) {
        return Err(AltSvcParseError::new("invalid Alt-Svc quoted-string"));
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
    } else if byte == b'\t' || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
    {
      *position += 1;
    } else {
      return Err(AltSvcParseError::new("invalid Alt-Svc quoted-string"));
    }
  }
  Err(AltSvcParseError::new("invalid Alt-Svc quoted-string"))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, AltSvcParseError> {
  let start = *position;
  while *position < value.len() && is_token_byte(value.as_bytes()[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(AltSvcParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}
fn skip_ows(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}
fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}
fn is_token_byte(byte: u8) -> bool {
  matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}
fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
  use super::AltSvc;

  #[test]
  fn parses_alternatives_and_clear() {
    let alternatives =
      AltSvc::parse("h3=\":443\"; ma=60; persist=1; x=token").expect("valid Alt-Svc");
    assert_eq!("h3", alternatives.alternatives()[0].protocol_id());
    assert_eq!(Some(60), alternatives.alternatives()[0].max_age());
    assert!(AltSvc::parse("clear").expect("clear").is_clear());
  }

  #[test]
  fn preserves_utf8_quoted_extension_values() {
    let alt_svc = AltSvc::parse("h3=\":443\"; note=\"café\"").expect("valid Alt-Svc");
    assert_eq!("h3=\":443\"; note=\"café\"", alt_svc.header_value());
  }
}
