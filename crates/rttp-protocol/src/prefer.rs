//! Structured, policy-free parsing for RFC 7240 preference headers.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

pub const MAX_PREFER_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PREFERENCES: usize = 32;
pub const MAX_PREFERENCE_PARAMETERS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferenceKind {
  Return,
  RespondAsync,
  Wait,
  Handling,
  Extension,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preference {
  name: String,
  value: Option<PreferenceValue>,
  parameters: Vec<PreferenceParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreferenceValue {
  value: String,
  quoted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferenceParameter {
  name: String,
  value: Option<PreferenceValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prefer {
  preferences: Vec<Preference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferenceApplied {
  preferences: Vec<Preference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferParseError {
  message: String,
}

pub type PreferenceAppliedParseError = PreferParseError;

impl PreferParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for PreferParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for PreferParseError {}

impl Prefer {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PreferParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PreferParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      preferences: parse_values(values, "Prefer")?,
    })
  }

  pub fn preferences(&self) -> &[Preference] {
    &self.preferences
  }

  pub fn len(&self) -> usize {
    self.preferences.len()
  }

  pub fn is_empty(&self) -> bool {
    self.preferences.is_empty()
  }

  pub fn header_value(&self) -> String {
    header_value(&self.preferences)
  }
}

impl PreferenceApplied {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PreferenceAppliedParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PreferenceAppliedParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      preferences: parse_values(values, "Preference-Applied")?,
    })
  }

  pub fn preferences(&self) -> &[Preference] {
    &self.preferences
  }

  pub fn header_value(&self) -> String {
    header_value(&self.preferences)
  }
}

impl Preference {
  pub fn kind(&self) -> PreferenceKind {
    if self.name.eq_ignore_ascii_case("return") {
      PreferenceKind::Return
    } else if self.name.eq_ignore_ascii_case("respond-async") {
      PreferenceKind::RespondAsync
    } else if self.name.eq_ignore_ascii_case("wait") {
      PreferenceKind::Wait
    } else if self.name.eq_ignore_ascii_case("handling") {
      PreferenceKind::Handling
    } else {
      PreferenceKind::Extension
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_ref().map(|value| value.value.as_str())
  }

  pub fn parameters(&self) -> &[PreferenceParameter] {
    &self.parameters
  }

  fn header_value(&self) -> String {
    let mut value = self.name.clone();
    if let Some(preference_value) = &self.value {
      value.push('=');
      value.push_str(&preference_value.header_value());
    }
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(&parameter.header_value());
    }
    value
  }
}

impl PreferenceParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_ref().map(|value| value.value.as_str())
  }

  fn header_value(&self) -> String {
    self.value.as_ref().map_or_else(
      || self.name.clone(),
      |value| format!("{}={}", self.name, value.header_value()),
    )
  }
}

impl PreferenceValue {
  fn header_value(&self) -> String {
    if self.quoted {
      format!(
        "\"{}\"",
        self.value.replace('\\', "\\\\").replace('"', "\\\"")
      )
    } else {
      self.value.clone()
    }
  }
}

fn parse_values<'a, I>(values: I, header_name: &str) -> Result<Vec<Preference>, PreferParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut preferences = Vec::new();
  for value in values {
    if value.len() > MAX_PREFER_VALUE_BYTES {
      return Err(PreferParseError::new(format!(
        "{header_name} header value is too large"
      )));
    }
    let mut position = 0;
    skip_ows(value, &mut position);
    if position == value.len() {
      return Err(invalid(header_name));
    }
    loop {
      let preference = parse_preference(value, &mut position, header_name)?;
      if preferences
        .iter()
        .any(|known: &Preference| known.name.eq_ignore_ascii_case(&preference.name))
      {
        return Err(PreferParseError::new(format!(
          "duplicate {header_name} preference"
        )));
      }
      if preferences.len() >= MAX_PREFERENCES {
        return Err(PreferParseError::new(format!(
          "too many {header_name} preferences"
        )));
      }
      preferences.push(preference);
      skip_ows(value, &mut position);
      if position == value.len() {
        break;
      }
      if take_byte(value, &mut position) != Some(b',') {
        return Err(invalid(header_name));
      }
      skip_ows(value, &mut position);
      if position == value.len() {
        return Err(invalid(header_name));
      }
    }
  }
  if preferences.is_empty() {
    Err(invalid(header_name))
  } else {
    Ok(preferences)
  }
}

fn parse_preference(
  value: &str,
  position: &mut usize,
  header_name: &str,
) -> Result<Preference, PreferParseError> {
  let name = parse_token(value, position, header_name)?;
  skip_ows(value, position);
  let preference_value = if take_if(value, position, b'=') {
    skip_ows(value, position);
    Some(parse_value(value, position, header_name)?)
  } else {
    None
  };
  validate_known(
    &name,
    preference_value.as_ref().map(|value| value.value.as_str()),
    header_name,
  )?;
  let mut parameters = Vec::new();
  loop {
    skip_ows(value, position);
    if !take_if(value, position, b';') {
      break;
    }
    skip_ows(value, position);
    let parameter_name = parse_token(value, position, header_name)?;
    skip_ows(value, position);
    let parameter_value = if take_if(value, position, b'=') {
      skip_ows(value, position);
      Some(parse_value(value, position, header_name)?)
    } else {
      None
    };
    if parameters
      .iter()
      .any(|parameter: &PreferenceParameter| parameter.name.eq_ignore_ascii_case(&parameter_name))
    {
      return Err(PreferParseError::new(format!(
        "duplicate {header_name} preference parameter"
      )));
    }
    if parameters.len() >= MAX_PREFERENCE_PARAMETERS {
      return Err(PreferParseError::new(format!(
        "too many {header_name} preference parameters"
      )));
    }
    parameters.push(PreferenceParameter {
      name: parameter_name,
      value: parameter_value,
    });
  }
  Ok(Preference {
    name,
    value: preference_value,
    parameters,
  })
}

fn validate_known(
  name: &str,
  value: Option<&str>,
  header_name: &str,
) -> Result<(), PreferParseError> {
  let valid = if name.eq_ignore_ascii_case("return") {
    matches!(value, Some("minimal" | "representation"))
  } else if name.eq_ignore_ascii_case("respond-async") {
    value.is_none()
  } else if name.eq_ignore_ascii_case("wait") {
    value.is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
  } else if name.eq_ignore_ascii_case("handling") {
    matches!(value, Some("lenient" | "strict"))
  } else {
    true
  };
  if valid {
    Ok(())
  } else {
    Err(invalid(header_name))
  }
}

fn parse_token(
  value: &str,
  position: &mut usize,
  header_name: &str,
) -> Result<String, PreferParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| crate::http1::is_token_byte(*byte))
  {
    *position += 1;
  }
  let token = &value[start..*position];
  if is_token(token) {
    Ok(token.to_string())
  } else {
    Err(invalid(header_name))
  }
}

fn parse_value(
  value: &str,
  position: &mut usize,
  header_name: &str,
) -> Result<PreferenceValue, PreferParseError> {
  if take_if(value, position, b'\"') {
    let mut parsed = Vec::new();
    loop {
      let Some(byte) = take_byte(value, position) else {
        return Err(invalid(header_name));
      };
      match byte {
        b'\"' => break,
        b'\\' => {
          let Some(escaped) = take_byte(value, position) else {
            return Err(invalid(header_name));
          };
          if escaped.is_ascii_control() {
            return Err(invalid(header_name));
          }
          parsed.push(escaped);
        }
        byte if byte == b'\t' || (0x20..=0x7e).contains(&byte) || byte >= 0x80 => parsed.push(byte),
        _ => return Err(invalid(header_name)),
      }
    }
    Ok(PreferenceValue {
      value: String::from_utf8(parsed).map_err(|_| invalid(header_name))?,
      quoted: true,
    })
  } else {
    Ok(PreferenceValue {
      value: parse_token(value, position, header_name)?,
      quoted: false,
    })
  }
}

fn header_value(preferences: &[Preference]) -> String {
  preferences
    .iter()
    .map(Preference::header_value)
    .collect::<Vec<_>>()
    .join(", ")
}

fn invalid(header_name: &str) -> PreferParseError {
  PreferParseError::new(format!("invalid {header_name} preference"))
}

fn skip_ows(value: &str, position: &mut usize) {
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn take_if(value: &str, position: &mut usize, expected: u8) -> bool {
  if value.as_bytes().get(*position) == Some(&expected) {
    *position += 1;
    true
  } else {
    false
  }
}

fn take_byte(value: &str, position: &mut usize) -> Option<u8> {
  let byte = *value.as_bytes().get(*position)?;
  *position += 1;
  Some(byte)
}

#[cfg(test)]
mod tests {
  use super::{Prefer, PreferenceApplied};

  #[test]
  fn parses_and_formats_multiple_preferences_with_parameters() {
    let prefer =
      Prefer::parse("return=minimal, wait=10; source=client, vendor=\"a b\"; trace=enabled")
        .expect("Prefer should parse");
    assert_eq!(prefer.preferences().len(), 3);
    assert_eq!(
      prefer.preferences()[2].parameters()[0].value(),
      Some("enabled")
    );
    assert_eq!(
      prefer.header_value(),
      "return=minimal, wait=10; source=client, vendor=\"a b\"; trace=enabled"
    );
  }

  #[test]
  fn rejects_malformed_preferences() {
    for value in [
      "return=other",
      "respond-async=value",
      "wait=1.5",
      "extension=\"unterminated",
      "extension; =value",
    ] {
      assert!(Prefer::parse(value).is_err(), "{value} should be rejected");
    }
    assert!(PreferenceApplied::parse("handling=relaxed").is_err());
  }
}
