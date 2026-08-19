use std::error::Error;
use std::fmt;

use crate::http1::{is_qdtext, is_quoted_pair_char, is_token};

pub const MAX_ACCEPT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_MEDIA_RANGES: usize = 256;
pub const MAX_CLIENT_ACCEPT_MEDIA_RANGES: usize = 32;

/// Parsed, bounded `Accept` request metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accept {
  media_ranges: Vec<AcceptMediaRange>,
}

impl Accept {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptParseError> {
    Self::parse_values(std::iter::once(value.as_ref()))
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Self::parse_values_with_limit(values, MAX_ACCEPT_MEDIA_RANGES)
  }

  pub fn parse_values_with_limit<'a, I>(
    values: I,
    maximum_media_ranges: usize,
  ) -> Result<Self, AcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Self::parse_values_with_limit_and_extensions(values, maximum_media_ranges, true)
  }

  pub fn parse_request_builder_values_with_limit<'a, I>(
    values: I,
    maximum_media_ranges: usize,
  ) -> Result<Self, AcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Self::parse_values_with_limit_and_extensions(values, maximum_media_ranges, false)
  }

  fn parse_values_with_limit_and_extensions<'a, I>(
    values: I,
    maximum_media_ranges: usize,
    allow_extensions_after_quality: bool,
  ) -> Result<Self, AcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut media_ranges = Vec::new();
    for value in values {
      if value.len() > MAX_ACCEPT_VALUE_BYTES {
        return Err(AcceptParseError::new("Accept header value is too large"));
      }
      if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(AcceptParseError::new("invalid Accept header value"));
      }

      for member in split_accept_members(value)? {
        if media_ranges.len() >= maximum_media_ranges {
          return Err(AcceptParseError::new("too many Accept media ranges"));
        }
        media_ranges
          .push(AcceptMediaRange::parse_inner(member, allow_extensions_after_quality)?.media_range);
      }
    }

    if media_ranges.is_empty() {
      return Err(AcceptParseError::new("invalid Accept header value"));
    }

    Ok(Self { media_ranges })
  }

  pub fn media_ranges(&self) -> &[AcceptMediaRange] {
    &self.media_ranges
  }

  pub fn len(&self) -> usize {
    self.media_ranges.len()
  }

  pub fn is_empty(&self) -> bool {
    self.media_ranges.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .media_ranges
      .iter()
      .map(AcceptMediaRange::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// One media range from parsed `Accept` request metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptMediaRange {
  media_type: String,
  parameters: Vec<(String, String)>,
  quality: Option<u16>,
}

impl AcceptMediaRange {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptParseError> {
    Self::parse_inner(value.as_ref(), true).map(|parsed| parsed.media_range)
  }

  pub fn request_builder_member(
    media_range: &str,
    qvalue: Option<&str>,
  ) -> Result<String, AcceptParseError> {
    if media_range.bytes().any(|byte| byte.is_ascii_control()) {
      return Err(AcceptParseError::new("invalid Accept media range"));
    }
    let media_range = media_range.trim();
    let parsed = Self::parse_inner(media_range, false)?;
    let qvalue = qvalue.map(validate_accept_qvalue).transpose()?;
    if parsed.has_quality && qvalue.is_some() {
      return Err(AcceptParseError::new("duplicate Accept quality value"));
    }
    let member = qvalue.map_or_else(
      || media_range.to_string(),
      |qvalue| format!("{media_range};q={qvalue}"),
    );
    if member.len() > MAX_ACCEPT_VALUE_BYTES {
      return Err(AcceptParseError::new("Accept header value is too large"));
    }
    Ok(member)
  }

  fn parse_inner(
    value: &str,
    allow_extensions_after_quality: bool,
  ) -> Result<Parsed, AcceptParseError> {
    let mut parts = split_accept_parameters(value)?;
    let Some(media_type) = parts.first() else {
      return Err(AcceptParseError::new("invalid Accept media range"));
    };
    let media_type = parse_accept_media_type(media_type.trim())?;
    parts.remove(0);

    let mut parameters = Vec::new();
    let mut quality = None;
    let mut parsing_extensions = false;
    for part in parts {
      let part = part.trim();
      let (name, value) = match part.split_once('=') {
        Some((name, value)) => (name.trim().to_ascii_lowercase(), Some(value.trim())),
        None if parsing_extensions && allow_extensions_after_quality => {
          (part.to_ascii_lowercase(), None)
        }
        None => return Err(AcceptParseError::new("invalid Accept parameter")),
      };
      if !is_token(&name) {
        return Err(AcceptParseError::new("invalid Accept parameter name"));
      }
      if name == "q" {
        if quality.is_some() {
          return Err(AcceptParseError::new("duplicate Accept quality value"));
        }
        let Some(value) = value else {
          return Err(AcceptParseError::new("invalid Accept parameter"));
        };
        quality = Some(parse_accept_quality(value)?);
        parsing_extensions = true;
        continue;
      }
      if parsing_extensions {
        if let Some(value) = value {
          parse_accept_parameter_value(value)?;
        }
        continue;
      }
      let Some(value) = value else {
        return Err(AcceptParseError::new("invalid Accept parameter"));
      };
      if parameters.iter().any(|(known, _)| known == &name) {
        return Err(AcceptParseError::new("duplicate Accept parameter"));
      }
      parameters.push((name, parse_accept_parameter_value(value)?));
    }

    Ok(Parsed {
      has_quality: quality.is_some(),
      media_range: Self {
        media_type,
        parameters,
        quality,
      },
    })
  }

  pub fn media_type(&self) -> &str {
    &self.media_type
  }

  pub fn parameters(&self) -> Vec<(&str, &str)> {
    self
      .parameters
      .iter()
      .map(|(name, value)| (name.as_str(), value.as_str()))
      .collect()
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|(known, _)| known.eq_ignore_ascii_case(name.as_ref()))
      .map(|(_, value)| value.as_str())
  }

  pub fn quality(&self) -> Option<u16> {
    self.quality
  }

  pub fn header_value(&self) -> String {
    let mut value = self.media_type.clone();
    for (name, parameter_value) in &self.parameters {
      value.push_str("; ");
      value.push_str(name);
      value.push('=');
      value.push_str(&serialize_accept_parameter_value(parameter_value));
    }
    if let Some(quality) = self.quality {
      value.push_str(";q=");
      value.push_str(&format_quality(quality));
    }
    value
  }
}

struct Parsed {
  media_range: AcceptMediaRange,
  has_quality: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptParseError {
  message: String,
}

impl AcceptParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptParseError {}

fn split_accept_members(value: &str) -> Result<Vec<&str>, AcceptParseError> {
  split_accept_delimited(value, b',', "invalid Accept header value")
}

fn split_accept_parameters(value: &str) -> Result<Vec<&str>, AcceptParseError> {
  split_accept_delimited(value, b';', "invalid Accept parameter")
}

fn split_accept_delimited<'a>(
  value: &'a str,
  delimiter: u8,
  error: &'static str,
) -> Result<Vec<&'a str>, AcceptParseError> {
  let mut members = Vec::new();
  let mut quoted = false;
  let mut escaped = false;
  let mut start = 0usize;

  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      if !is_quoted_pair_char(byte) {
        return Err(AcceptParseError::new(error));
      }
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' => quoted = !quoted,
      byte if byte == delimiter && !quoted => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err(AcceptParseError::new(error));
        }
        members.push(member);
        start = index + 1;
      }
      _ => {}
    }
  }

  if quoted || escaped {
    return Err(AcceptParseError::new(error));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(AcceptParseError::new(error));
  }
  members.push(member);
  Ok(members)
}

fn parse_accept_media_type(value: &str) -> Result<String, AcceptParseError> {
  let Some((type_name, subtype)) = value.split_once('/') else {
    return Err(AcceptParseError::new("invalid Accept media range"));
  };
  if subtype.contains('/') {
    return Err(AcceptParseError::new("invalid Accept media range"));
  }
  let type_name = type_name.trim().to_ascii_lowercase();
  let subtype = subtype.trim().to_ascii_lowercase();
  if type_name == "*" && subtype != "*" {
    return Err(AcceptParseError::new("invalid Accept media range"));
  }
  if !(type_name == "*" || is_token(&type_name)) || !(subtype == "*" || is_token(&subtype)) {
    return Err(AcceptParseError::new("invalid Accept media range"));
  }
  Ok(format!("{type_name}/{subtype}"))
}

fn parse_accept_parameter_value(value: &str) -> Result<String, AcceptParseError> {
  if value.is_empty() {
    return Err(AcceptParseError::new("invalid Accept parameter value"));
  }
  if value.starts_with('"') {
    parse_accept_quoted_string(value)
  } else if value.contains('"') || !is_token(value) {
    Err(AcceptParseError::new("invalid Accept parameter value"))
  } else {
    Ok(value.to_string())
  }
}

fn parse_accept_quoted_string(value: &str) -> Result<String, AcceptParseError> {
  if !value.ends_with('"') || value.len() < 2 {
    return Err(AcceptParseError::new("invalid Accept parameter value"));
  }

  let inner = &value[1..value.len() - 1];
  let mut parsed = String::new();
  let mut escaped = false;
  for byte in inner.bytes() {
    if escaped {
      if !is_quoted_pair_char(byte) {
        return Err(AcceptParseError::new("invalid Accept parameter value"));
      }
      parsed.push(byte as char);
      escaped = false;
    } else if byte == b'\\' {
      escaped = true;
    } else if byte == b'"' || !is_qdtext(byte) {
      return Err(AcceptParseError::new("invalid Accept parameter value"));
    } else {
      parsed.push(byte as char);
    }
  }

  if escaped {
    return Err(AcceptParseError::new("invalid Accept parameter value"));
  }

  Ok(parsed)
}

fn parse_accept_quality(value: &str) -> Result<u16, AcceptParseError> {
  let value = validate_accept_qvalue(value)?;
  let valid = match value {
    "0" => Some(0),
    "1" => Some(1000),
    _ => {
      let Some((whole, fractional)) = value.split_once('.') else {
        return Err(AcceptParseError::new("invalid Accept quality value"));
      };
      let scale = 10u16.pow((3 - fractional.len()) as u32);
      match whole {
        "0" => fractional
          .parse::<u16>()
          .ok()
          .map(|fraction| fraction * scale),
        "1" => Some(1000),
        _ => None,
      }
    }
  };
  valid.ok_or_else(|| AcceptParseError::new("invalid Accept quality value"))
}

fn validate_accept_qvalue(qvalue: &str) -> Result<&str, AcceptParseError> {
  let value = qvalue.trim();
  let valid = match value.split_once('.') {
    Some((whole, fraction)) => {
      (whole == "0" || whole == "1")
        && !fraction.is_empty()
        && fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => value == "0" || value == "1",
  };
  if valid {
    Ok(value)
  } else {
    Err(AcceptParseError::new("invalid Accept quality value"))
  }
}

fn format_quality(quality: u16) -> String {
  match quality {
    0 => "0".to_string(),
    1000 => "1".to_string(),
    value if value % 100 == 0 => format!("0.{}", value / 100),
    value if value % 10 == 0 => format!("0.{:02}", value / 10),
    value => format!("0.{value:03}"),
  }
}

fn serialize_accept_parameter_value(value: &str) -> String {
  if is_token(value) {
    return value.to_string();
  }

  let mut serialized = String::from("\"");
  for character in value.chars() {
    if matches!(character, '"' | '\\') {
      serialized.push('\\');
    }
    serialized.push(character);
  }
  serialized.push('"');
  serialized
}
