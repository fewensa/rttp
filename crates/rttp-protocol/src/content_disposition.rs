//! Bounded, policy-free `Content-Disposition` response metadata parsing.
//!
//! This module validates one RFC 6266 `Content-Disposition` field value only.
//! It stores the disposition type, ordered parameters, and the independent
//! `filename` and `filename*` strings. Callers retain download, filesystem,
//! display-name, MIME sniffing, cache, redirect, retry, negotiation, and
//! status-code policy. The parser never decodes RFC 5987 `filename*` values
//! or chooses between `filename` and `filename*`.

use std::error::Error;
use std::fmt;

pub const MAX_CONTENT_DISPOSITION_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CONTENT_DISPOSITION_PARAMETERS: usize = 256;
pub const MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Content-Disposition` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDisposition {
  disposition_type: String,
  parameters: Vec<ContentDispositionParameter>,
}

/// One parameter from a parsed `Content-Disposition` field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDispositionParameter {
  name: String,
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDispositionParseError {
  message: String,
}

impl ContentDispositionParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentDispositionParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentDispositionParseError {}

impl ContentDisposition {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentDispositionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentDispositionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_field_value(value)
  }

  pub fn new(disposition_type: impl AsRef<str>) -> Result<Self, ContentDispositionParseError> {
    let disposition_type = disposition_type.as_ref().trim().to_ascii_lowercase();
    if !crate::media_type::is_token(&disposition_type) {
      return Err(ContentDispositionParseError::new(
        "invalid Content-Disposition disposition type",
      ));
    }
    let parsed = Self {
      disposition_type,
      parameters: Vec::new(),
    };
    if parsed.header_value().len() > MAX_CONTENT_DISPOSITION_VALUE_BYTES {
      return Err(ContentDispositionParseError::new(
        "Content-Disposition header value is too large",
      ));
    }
    Ok(parsed)
  }

  pub fn inline() -> Self {
    Self {
      disposition_type: "inline".to_string(),
      parameters: Vec::new(),
    }
  }

  pub fn attachment() -> Self {
    Self {
      disposition_type: "attachment".to_string(),
      parameters: Vec::new(),
    }
  }

  pub fn with_parameter<N, V>(
    mut self,
    name: N,
    value: V,
  ) -> Result<Self, ContentDispositionParseError>
  where
    N: AsRef<str>,
    V: AsRef<str>,
  {
    let name = name.as_ref().trim().to_ascii_lowercase();
    let value = value.as_ref();
    if !crate::media_type::is_token(&name) {
      return Err(ContentDispositionParseError::new(
        "invalid Content-Disposition parameter name",
      ));
    }
    if value.is_empty() || !value.is_ascii() || value.bytes().any(|byte| byte.is_ascii_control()) {
      return Err(ContentDispositionParseError::new(
        "invalid Content-Disposition parameter value",
      ));
    }
    if value.len() > MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES {
      return Err(ContentDispositionParseError::new(
        "Content-Disposition parameter value is too large",
      ));
    }
    if name == "filename*" && !is_content_disposition_ext_value(value) {
      return Err(ContentDispositionParseError::new(
        "invalid Content-Disposition filename* parameter",
      ));
    }
    if self
      .parameters
      .iter()
      .any(|parameter| parameter.name.eq_ignore_ascii_case(&name))
    {
      return Err(ContentDispositionParseError::new(
        "duplicate Content-Disposition parameter",
      ));
    }
    if self.parameters.len() >= MAX_CONTENT_DISPOSITION_PARAMETERS {
      return Err(ContentDispositionParseError::new(
        "too many Content-Disposition parameters",
      ));
    }

    let mut candidate = self.header_value();
    candidate.push_str("; ");
    candidate.push_str(&name);
    candidate.push('=');
    candidate.push_str(&crate::media_type::serialize_parameter_value(value));
    if candidate.len() > MAX_CONTENT_DISPOSITION_VALUE_BYTES {
      return Err(ContentDispositionParseError::new(
        "Content-Disposition header value is too large",
      ));
    }

    self.parameters.push(ContentDispositionParameter {
      name,
      value: value.to_string(),
    });
    Ok(self)
  }

  pub fn disposition_type(&self) -> &str {
    &self.disposition_type
  }

  pub fn parameters(&self) -> &[ContentDispositionParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&ContentDispositionParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn filename(&self) -> Option<&str> {
    self
      .parameter("filename")
      .map(ContentDispositionParameter::value)
  }

  pub fn filename_ext(&self) -> Option<&str> {
    self
      .parameter("filename*")
      .map(ContentDispositionParameter::value)
  }

  pub fn header_value(&self) -> String {
    let mut value = self.disposition_type.clone();
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(parameter.name());
      value.push('=');
      value.push_str(&crate::media_type::serialize_parameter_value(
        parameter.value(),
      ));
    }
    value
  }
}

impl ContentDispositionParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, ContentDispositionParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_bounded_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_value(value)?;
  }
  if has_duplicate {
    return Err(ContentDispositionParseError::new(
      "duplicate Content-Disposition header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), ContentDispositionParseError> {
  if value.len() > MAX_CONTENT_DISPOSITION_VALUE_BYTES {
    return Err(ContentDispositionParseError::new(
      "Content-Disposition header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte != b'\t' && (byte <= 0x1f || byte == 0x7f))
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn parse_field_value(value: &str) -> Result<ContentDisposition, ContentDispositionParseError> {
  let members = split_members(value)?;
  let Some(disposition_type) = members.first().copied() else {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition disposition type",
    ));
  };
  let disposition_type = trim_http_optional_whitespace(disposition_type).to_ascii_lowercase();
  if !crate::media_type::is_token(&disposition_type) {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition disposition type",
    ));
  }

  let mut parameters = Vec::new();
  for member in members.iter().skip(1) {
    if parameters.len() >= MAX_CONTENT_DISPOSITION_PARAMETERS {
      return Err(ContentDispositionParseError::new(
        "too many Content-Disposition parameters",
      ));
    }
    let parameter = parse_parameter(member)?;
    if parameters
      .iter()
      .any(|seen: &ContentDispositionParameter| seen.name.eq_ignore_ascii_case(parameter.name()))
    {
      return Err(ContentDispositionParseError::new(
        "duplicate Content-Disposition parameter",
      ));
    }
    parameters.push(parameter);
  }

  Ok(ContentDisposition {
    disposition_type,
    parameters,
  })
}

fn split_members(value: &str) -> Result<Vec<&str>, ContentDispositionParseError> {
  let mut parts = Vec::new();
  let mut quoted = false;
  let mut escaped = false;
  let mut start = 0usize;

  for (index, ch) in value.char_indices() {
    if escaped {
      escaped = false;
      continue;
    }

    match ch {
      '\\' if quoted => escaped = true,
      '"' => quoted = !quoted,
      ';' if !quoted => {
        parts.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }

  if quoted || escaped {
    return Err(malformed_quoted_string());
  }

  parts.push(&value[start..]);
  if parts
    .iter()
    .any(|part| trim_http_optional_whitespace(part).is_empty())
  {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition member",
    ));
  }
  Ok(parts)
}

fn parse_parameter(
  value: &str,
) -> Result<ContentDispositionParameter, ContentDispositionParseError> {
  let value = trim_http_optional_whitespace(value);
  let Some((name, raw_value)) = value.split_once('=') else {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition parameter",
    ));
  };
  let name = trim_http_optional_whitespace(name).to_ascii_lowercase();
  let raw_value = trim_http_optional_whitespace(raw_value);
  if !crate::media_type::is_token(&name) {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition parameter name",
    ));
  }
  if raw_value.len() > MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES {
    return Err(ContentDispositionParseError::new(
      "Content-Disposition parameter value is too large",
    ));
  }

  let (parsed_value, value_was_quoted) = parse_parameter_value(raw_value)?;
  if name == "filename*" && (value_was_quoted || !is_content_disposition_ext_value(&parsed_value)) {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition filename* parameter",
    ));
  }

  Ok(ContentDispositionParameter {
    name,
    value: parsed_value,
  })
}

fn parse_parameter_value(value: &str) -> Result<(String, bool), ContentDispositionParseError> {
  if value.is_empty() {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition parameter value",
    ));
  }
  if value.starts_with('"') {
    return parse_quoted_string(value).map(|value| (value, true));
  }
  if value.contains('"') || !crate::media_type::is_token(value) {
    return Err(ContentDispositionParseError::new(
      "invalid Content-Disposition parameter value",
    ));
  }
  Ok((value.to_string(), false))
}

fn parse_quoted_string(value: &str) -> Result<String, ContentDispositionParseError> {
  let mut chars = value.chars();
  if chars.next() != Some('"') {
    return Err(malformed_quoted_string());
  }

  let mut parsed = String::new();
  let mut closed = false;
  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(malformed_quoted_string());
        };
        if !is_quoted_pair_char(escaped) {
          return Err(malformed_quoted_string());
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => return Err(malformed_quoted_string()),
    }
  }

  if !closed || chars.next().is_some() || parsed.is_empty() {
    return Err(malformed_quoted_string());
  }
  Ok(parsed)
}

fn is_content_disposition_ext_value(value: &str) -> bool {
  let mut parts = value.splitn(3, '\'');
  let Some(charset) = parts.next() else {
    return false;
  };
  let Some(language) = parts.next() else {
    return false;
  };
  let Some(encoded_value) = parts.next() else {
    return false;
  };

  !charset.is_empty()
    && crate::media_type::is_token(charset)
    && language.bytes().all(is_content_disposition_language_byte)
    && !encoded_value.is_empty()
    && is_content_disposition_ext_value_chars(encoded_value)
}

fn is_content_disposition_ext_value_chars(value: &str) -> bool {
  let mut bytes = value.bytes();
  while let Some(byte) = bytes.next() {
    if byte == b'%' {
      let Some(first) = bytes.next() else {
        return false;
      };
      let Some(second) = bytes.next() else {
        return false;
      };
      if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
        return false;
      }
    } else if !is_content_disposition_attr_char(byte) {
      return false;
    }
  }
  true
}

fn is_content_disposition_attr_char(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
    )
}

fn is_content_disposition_language_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.')
}

fn is_qdtext(ch: char) -> bool {
  matches!(ch, '\t' | ' ' | '!' | '#'..='[' | ']'..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn is_quoted_pair_char(ch: char) -> bool {
  matches!(ch, '\t' | ' '..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn invalid_value() -> ContentDispositionParseError {
  ContentDispositionParseError::new("invalid Content-Disposition header value")
}

fn malformed_quoted_string() -> ContentDispositionParseError {
  ContentDispositionParseError::new("malformed Content-Disposition quoted-string")
}
