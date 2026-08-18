use std::error::Error;
use std::fmt;

use sfv::{Dictionary, FieldType, ListEntry, Parser};

pub const MAX_NO_VARY_SEARCH_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_NO_VARY_SEARCH_PARAMETERS: usize = 256;
pub const MAX_NO_VARY_SEARCH_EXTENSIONS: usize = 64;

/// Parsed, bounded `No-Vary-Search` response metadata.
///
/// This represents the declared metadata only. It does not implement cache-key
/// matching, URL normalization, navigation handling, request replay, or storage
/// policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoVarySearch {
  key_order: Option<bool>,
  params: Option<NoVarySearchParams>,
  except: Vec<String>,
  extensions: Vec<NoVarySearchExtension>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoVarySearchParams {
  None,
  All,
  Names(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoVarySearchExtension {
  key: String,
  value: Option<String>,
}

impl NoVarySearch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, NoVarySearchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, NoVarySearchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut metadata = Self {
      key_order: None,
      params: None,
      except: Vec::new(),
      extensions: Vec::new(),
    };
    let mut saw_member = false;

    for value in values {
      if value.len() > MAX_NO_VARY_SEARCH_VALUE_BYTES {
        return Err(NoVarySearchParseError::new(
          "No-Vary-Search header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(NoVarySearchParseError::new(
          "invalid No-Vary-Search control byte",
        ));
      }
      for member in split_top_level(value)? {
        saw_member = true;
        parse_member(member, &mut metadata)?;
      }
    }

    if !saw_member {
      return Err(NoVarySearchParseError::new("invalid No-Vary-Search value"));
    }
    if !metadata.except.is_empty() && metadata.params != Some(NoVarySearchParams::All) {
      return Err(NoVarySearchParseError::new(
        "No-Vary-Search except requires params",
      ));
    }
    Ok(metadata)
  }

  pub fn key_order(&self) -> Option<bool> {
    self.key_order
  }

  pub fn params(&self) -> Option<&NoVarySearchParams> {
    self.params.as_ref()
  }

  pub fn except(&self) -> &[String] {
    &self.except
  }

  pub fn extensions(&self) -> &[NoVarySearchExtension] {
    &self.extensions
  }

  pub fn ignores_all_query_params(&self) -> bool {
    self.params == Some(NoVarySearchParams::All)
  }

  pub fn ignored_params(&self) -> Option<&[String]> {
    match self.params.as_ref() {
      Some(NoVarySearchParams::Names(names)) => Some(names),
      _ => None,
    }
  }

  pub fn header_value(&self) -> String {
    let mut members = Vec::new();
    if let Some(key_order) = self.key_order {
      members.push(if key_order {
        "key-order".to_owned()
      } else {
        "key-order=?0".to_owned()
      });
    }
    if let Some(params) = &self.params {
      match params {
        NoVarySearchParams::None => members.push("params=?0".to_owned()),
        NoVarySearchParams::All => members.push("params".to_owned()),
        NoVarySearchParams::Names(names) => {
          members.push(format!("params={}", format_string_list(names)));
        }
      }
    }
    if !self.except.is_empty() {
      members.push(format!("except={}", format_string_list(&self.except)));
    }
    members.extend(
      self
        .extensions
        .iter()
        .map(NoVarySearchExtension::header_value),
    );
    members.join(", ")
  }
}

impl NoVarySearchParams {
  pub fn names(&self) -> Option<&[String]> {
    match self {
      Self::None => None,
      Self::All => None,
      Self::Names(names) => Some(names),
    }
  }
}

impl NoVarySearchExtension {
  pub fn key(&self) -> &str {
    &self.key
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  pub fn header_value(&self) -> String {
    match &self.value {
      Some(value) => format!("{}={}", self.key, value),
      None => self.key.clone(),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoVarySearchParseError {
  message: String,
}

impl NoVarySearchParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for NoVarySearchParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for NoVarySearchParseError {}

fn parse_member(member: &str, metadata: &mut NoVarySearch) -> Result<(), NoVarySearchParseError> {
  let member = member.trim_matches([' ', '\t']);
  if member.is_empty() {
    return Err(NoVarySearchParseError::new("invalid No-Vary-Search member"));
  }
  let dictionary = parse_single_member_dictionary(member)?;
  let (key, entry) = dictionary
    .first()
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search member"))?;
  let key = key.to_string();

  if !matches!(key.as_str(), "key-order" | "params" | "except") {
    let extension = parse_extension_member(member)?;
    return store_extension(extension, metadata);
  }

  if entry_has_params(entry) {
    return Err(NoVarySearchParseError::new(
      "No-Vary-Search reserved member parameters are unsupported",
    ));
  }

  let serialized = dictionary
    .serialize()
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search member"))?;
  let suffix = serialized
    .strip_prefix(&key)
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search member"))?;
  let value = match suffix.as_bytes().first() {
    None => None,
    Some(b'=') => Some(&suffix[1..]),
    Some(b';') => None,
    _ => return Err(NoVarySearchParseError::new("invalid No-Vary-Search member")),
  };

  parse_keyed_member(&key, value, metadata)
}

fn parse_keyed_member(
  key: &str,
  value: Option<&str>,
  metadata: &mut NoVarySearch,
) -> Result<(), NoVarySearchParseError> {
  match key {
    "key-order" => {
      metadata.key_order = Some(parse_boolean(value)?);
    }
    "params" => {
      metadata.params = Some(match value {
        None => NoVarySearchParams::All,
        Some("?1") => NoVarySearchParams::All,
        Some("?0") => NoVarySearchParams::None,
        Some(value) => NoVarySearchParams::Names(parse_string_list(value)?),
      });
    }
    "except" => {
      let value =
        value.ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search except"))?;
      metadata.except = parse_string_list(value)?;
    }
    _ => {
      let extension = parse_extension_parts(key, value)?;
      store_extension(extension, metadata)?;
    }
  }

  Ok(())
}

fn parse_extension_parts(
  key: &str,
  value: Option<&str>,
) -> Result<NoVarySearchExtension, NoVarySearchParseError> {
  let member = value.map_or_else(|| key.to_owned(), |value| format!("{key}={value}"));
  parse_extension_member(&member)
}

fn parse_extension_member(member: &str) -> Result<NoVarySearchExtension, NoVarySearchParseError> {
  let dictionary = parse_single_member_dictionary(member)?;
  let (key, _) = dictionary
    .first()
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search extension"))?;
  let key = key.to_string();
  let serialized = dictionary
    .serialize()
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search extension"))?;
  let suffix = serialized
    .strip_prefix(&key)
    .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search extension"))?;

  let value = match suffix.as_bytes().first() {
    None => None,
    Some(b'=') => Some(suffix[1..].to_owned()),
    Some(b';') => Some(format!("?1{suffix}")),
    _ => {
      return Err(NoVarySearchParseError::new(
        "invalid No-Vary-Search extension",
      ));
    }
  };

  Ok(NoVarySearchExtension { key, value })
}

fn parse_single_member_dictionary(member: &str) -> Result<Dictionary, NoVarySearchParseError> {
  let dictionary = Parser::new(member)
    .parse::<Dictionary>()
    .map_err(|_| NoVarySearchParseError::new("invalid No-Vary-Search member"))?;
  if dictionary.len() != 1 {
    return Err(NoVarySearchParseError::new("invalid No-Vary-Search member"));
  }
  Ok(dictionary)
}

fn entry_has_params(entry: &ListEntry) -> bool {
  match entry {
    ListEntry::Item(item) => !item.params.is_empty(),
    ListEntry::InnerList(inner_list) => !inner_list.params.is_empty(),
  }
}

fn store_extension(
  extension: NoVarySearchExtension,
  metadata: &mut NoVarySearch,
) -> Result<(), NoVarySearchParseError> {
  metadata
    .extensions
    .retain(|existing| existing.key != extension.key);
  if metadata.extensions.len() >= MAX_NO_VARY_SEARCH_EXTENSIONS {
    return Err(NoVarySearchParseError::new(
      "too many No-Vary-Search extensions",
    ));
  }
  metadata.extensions.push(extension);
  Ok(())
}

fn parse_boolean(value: Option<&str>) -> Result<bool, NoVarySearchParseError> {
  match value {
    None | Some("?1") => Ok(true),
    Some("?0") => Ok(false),
    _ => Err(NoVarySearchParseError::new(
      "invalid No-Vary-Search boolean",
    )),
  }
}

fn parse_string_list(value: &str) -> Result<Vec<String>, NoVarySearchParseError> {
  let bytes = value.as_bytes();
  if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
    return Err(NoVarySearchParseError::new(
      "invalid No-Vary-Search string list",
    ));
  }

  let mut position = 1usize;
  let mut names = Vec::new();
  while position + 1 < bytes.len() {
    skip_spaces(bytes, &mut position);
    if position + 1 >= bytes.len() {
      break;
    }
    if names.len() >= MAX_NO_VARY_SEARCH_PARAMETERS {
      return Err(NoVarySearchParseError::new(
        "too many No-Vary-Search parameters",
      ));
    }
    names.push(parse_quoted_string(value, &mut position)?);
    skip_spaces(bytes, &mut position);
  }
  if position != bytes.len() - 1 || names.is_empty() {
    return Err(NoVarySearchParseError::new(
      "invalid No-Vary-Search string list",
    ));
  }
  Ok(names)
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
) -> Result<String, NoVarySearchParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'"') {
    return Err(NoVarySearchParseError::new("invalid No-Vary-Search string"));
  }
  *position += 1;
  let mut result = String::new();
  while *position < bytes.len() {
    match bytes[*position] {
      b'"' => {
        *position += 1;
        return Ok(result);
      }
      b'\\' => {
        *position += 1;
        match bytes.get(*position) {
          Some(b'"' | b'\\') => {
            result.push(bytes[*position] as char);
            *position += 1;
          }
          _ => {
            return Err(NoVarySearchParseError::new("invalid No-Vary-Search string"));
          }
        }
      }
      byte if (0x20..=0x7e).contains(&byte) => {
        result.push(byte as char);
        *position += 1;
      }
      _ => {
        return Err(NoVarySearchParseError::new("invalid No-Vary-Search string"));
      }
    }
  }
  Err(NoVarySearchParseError::new("invalid No-Vary-Search string"))
}

fn split_top_level(value: &str) -> Result<Vec<&str>, NoVarySearchParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut in_string = false;
  let mut escaped = false;
  let mut depth = 0usize;
  for (index, byte) in value.bytes().enumerate() {
    if in_string {
      if escaped {
        escaped = false;
      } else if byte == b'\\' {
        escaped = true;
      } else if byte == b'"' {
        in_string = false;
      }
      continue;
    }

    match byte {
      b'"' => in_string = true,
      b'(' => depth += 1,
      b')' => {
        depth = depth
          .checked_sub(1)
          .ok_or_else(|| NoVarySearchParseError::new("invalid No-Vary-Search inner list"))?;
      }
      b',' if depth == 0 => {
        members.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }
  if in_string || depth != 0 {
    return Err(NoVarySearchParseError::new("invalid No-Vary-Search value"));
  }
  members.push(&value[start..]);
  Ok(members)
}

fn format_string_list(names: &[String]) -> String {
  let members = names
    .iter()
    .map(|name| format!("\"{}\"", escape_string(name)))
    .collect::<Vec<_>>()
    .join(" ");
  format!("({members})")
}

fn escape_string(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn skip_spaces(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
