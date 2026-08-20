//! Bounded, policy-free WebDAV `DAV` response metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to apply WebDAV behavior.

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in each `DAV` field value.
pub const MAX_DAV_VALUE_BYTES: usize = 64 * 1024;
/// Maximum bytes accepted across all `DAV` field values.
pub const MAX_DAV_AGGREGATE_VALUE_BYTES: usize = 64 * 1024;
/// Maximum compliance-class members accepted across all field values.
pub const MAX_DAV_CLASSES: usize = 32;

/// Parsed, bounded WebDAV `DAV` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dav {
  classes: Vec<DavClass>,
}

impl Dav {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DavParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DavParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut aggregate_len = 0usize;
    let mut classes = Vec::new();

    for value in values {
      if value.len() > MAX_DAV_VALUE_BYTES {
        return Err(DavParseError::new("DAV header value is too large"));
      }
      aggregate_len = aggregate_len
        .checked_add(value.len())
        .ok_or_else(|| DavParseError::new("DAV header aggregate value is too large"))?;
      if aggregate_len > MAX_DAV_AGGREGATE_VALUE_BYTES {
        return Err(DavParseError::new(
          "DAV header aggregate value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(DavParseError::new("invalid DAV control byte"));
      }

      for member in split_dav_members(value)? {
        let member = member.trim_matches([' ', '\t']);
        let class = DavClass::parse_member(member)?;
        if classes.iter().any(|known| known == &class) {
          return Err(DavParseError::new("duplicate DAV compliance class"));
        }
        if classes.len() >= MAX_DAV_CLASSES {
          return Err(DavParseError::new("too many DAV compliance classes"));
        }
        classes.push(class);
      }
    }

    if classes.is_empty() {
      return Err(DavParseError::new("invalid DAV compliance class"));
    }

    Ok(Self { classes })
  }

  pub fn classes(&self) -> &[DavClass] {
    &self.classes
  }

  pub fn header_value(&self) -> String {
    self
      .classes
      .iter()
      .map(DavClass::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

/// A single WebDAV compliance class or extension advertised in `DAV`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DavClass {
  One,
  Two,
  Three,
  ExtensionToken(String),
  CodedUrl(String),
}

impl DavClass {
  fn parse_member(member: &str) -> Result<Self, DavParseError> {
    match member {
      "1" => Ok(Self::One),
      "2" => Ok(Self::Two),
      "3" => Ok(Self::Three),
      _ if is_coded_url(member) => {
        let uri = &member[1..member.len() - 1];
        validate_absolute_uri(uri)?;
        Ok(Self::CodedUrl(uri.to_string()))
      }
      _ if is_http_token(member) => Ok(Self::ExtensionToken(member.to_string())),
      _ => Err(DavParseError::new("invalid DAV compliance class")),
    }
  }

  pub fn header_value(&self) -> String {
    match self {
      Self::One => "1".to_string(),
      Self::Two => "2".to_string(),
      Self::Three => "3".to_string(),
      Self::ExtensionToken(token) => token.clone(),
      Self::CodedUrl(uri) => format!("<{uri}>"),
    }
  }
}

/// An error returned when `DAV` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DavParseError {
  message: String,
}

impl DavParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DavParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DavParseError {}

fn split_dav_members(value: &str) -> Result<Vec<&str>, DavParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut in_coded_url = false;

  for (index, byte) in value.bytes().enumerate() {
    match byte {
      b'<' if !in_coded_url => in_coded_url = true,
      b'<' => return Err(DavParseError::new("invalid DAV Coded-URL")),
      b'>' if in_coded_url => in_coded_url = false,
      b'>' => return Err(DavParseError::new("invalid DAV Coded-URL")),
      b',' if !in_coded_url => {
        members.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }

  if in_coded_url {
    return Err(DavParseError::new("invalid DAV Coded-URL"));
  }

  members.push(&value[start..]);
  Ok(members)
}

fn is_coded_url(member: &str) -> bool {
  member.len() >= 3 && member.starts_with('<') && member.ends_with('>')
}

fn validate_absolute_uri(uri: &str) -> Result<(), DavParseError> {
  if uri.is_empty() || uri.bytes().any(|byte| byte <= 0x20 || byte == 0x7f) {
    return Err(DavParseError::new("invalid DAV Coded-URL"));
  }
  let parsed = Url::parse(uri).map_err(|_| DavParseError::new("invalid DAV Coded-URL"))?;
  if parsed.scheme().is_empty() {
    return Err(DavParseError::new("invalid DAV Coded-URL"));
  }
  Ok(())
}

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
