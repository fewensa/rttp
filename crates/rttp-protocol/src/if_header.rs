//! Bounded, policy-free WebDAV `If` request metadata parsing.
//!
//! This module validates RFC 4918 section 10.4 `If` header field values as
//! typed, bounded condition lists only. It preserves list order, resource
//! tags, `Not`, state tokens, and entity tags, and it can re-emit the
//! canonical field text. It does not evaluate lock tokens, entity tags, or
//! other resource state, and it does not generate precondition outcomes such
//! as 412 Precondition Failed from this header.
//!
//! A parsed value is entirely untagged (`(a) (b)`) or entirely tagged
//! (`<src> (a) <dst> (Not <DAV:no-lock>)`); mixed productions are rejected.
//! Repeated lists and conditions inside one field are preserved. In the
//! tagged form a resource tag may introduce several lists, which are stored
//! as one list per condition group and re-emitted with the tag repeated
//! (`<src> (a) <src> (b)`); that emission is semantically equivalent under
//! RFC 4918 evaluation.
//!
//! State tokens are redacted from typed `Debug` and never appear in parse
//! errors. Resource tags and entity tags are not sensitive and may appear in
//! `Debug`.

use std::error::Error;
use std::fmt;

use url::Url;

use crate::entity_tag::EntityTag;

/// Maximum bytes accepted in one `If` field value.
pub const MAX_IF_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across all supplied fields.
pub const MAX_IF_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum condition lists accepted across the combined value.
pub const MAX_IF_LISTS: usize = 32;
/// Maximum conditions accepted across all lists in the combined value.
pub const MAX_IF_CONDITIONS: usize = 256;

/// Parsed, bounded WebDAV `If` request metadata.
///
/// The value is entirely untagged or entirely tagged. Lists and conditions
/// keep their wire order and are re-emitted with single SP separators.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct If {
  tagged: bool,
  lists: Vec<IfList>,
}

/// One RFC 4918 condition list, optionally bound to a resource tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfList {
  resource_tag: Option<IfResourceTag>,
  conditions: Vec<IfCondition>,
}

/// One condition inside a list: an optional `Not` and a state token or
/// entity-tag predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfCondition {
  negated: bool,
  predicate: IfPredicate,
}

/// A state token or entity-tag predicate in an `If` condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IfPredicate {
  StateToken(IfStateToken),
  EntityTag(EntityTag),
}

/// A WebDAV state token: an angle-bracketed absolute coded URL.
///
/// The token text is redacted from typed `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct IfStateToken {
  value: String,
}

/// A WebDAV resource tag: an angle-bracketed RFC 3986 `Simple-ref`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfResourceTag {
  value: String,
}

/// An error returned when `If` metadata is malformed or exceeds bounds.
///
/// Error messages name the header and the failure category only; they never
/// include state-token or resource-tag material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IfParseError {
  message: String,
}

impl If {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, IfParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, IfParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let value = values.next().ok_or_else(invalid_value)?;
    let mut total_bytes = value.len();
    if total_bytes > MAX_IF_TOTAL_BYTES {
      return Err(IfParseError::new("If header list is too large"));
    }
    validate_bounded_value(value)?;
    let mut has_duplicate = false;
    for value in values {
      has_duplicate = true;
      if value.len() > MAX_IF_VALUE_BYTES {
        return Err(IfParseError::new("If header value is too large"));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_IF_TOTAL_BYTES {
        return Err(IfParseError::new("If header list is too large"));
      }
      validate_bounded_value(value)?;
    }
    if has_duplicate {
      return Err(IfParseError::new("duplicate If header fields"));
    }

    let value = value.trim_matches([' ', '\t']);
    if value.is_empty() {
      return Err(invalid_value());
    }
    let (tagged, lists) = parse_if_value(value)?;
    Ok(Self { tagged, lists })
  }

  pub fn is_tagged(&self) -> bool {
    self.tagged
  }

  pub fn lists(&self) -> &[IfList] {
    &self.lists
  }

  pub fn header_value(&self) -> String {
    let mut parts = Vec::with_capacity(self.lists.len());
    for list in &self.lists {
      let mut list_text = String::from("(");
      for (index, condition) in list.conditions.iter().enumerate() {
        if index > 0 {
          list_text.push(' ');
        }
        if condition.negated {
          list_text.push_str("Not ");
        }
        match &condition.predicate {
          IfPredicate::StateToken(token) => list_text.push_str(&token.value),
          IfPredicate::EntityTag(entity_tag) => {
            list_text.push('[');
            list_text.push_str(&entity_tag.header_value());
            list_text.push(']');
          }
        }
      }
      list_text.push(')');
      if let Some(tag) = &list.resource_tag {
        parts.push(format!("{} {}", tag.value, list_text));
      } else {
        parts.push(list_text);
      }
    }
    parts.join(" ")
  }
}

impl IfList {
  pub fn resource_tag(&self) -> Option<&IfResourceTag> {
    self.resource_tag.as_ref()
  }

  pub fn conditions(&self) -> &[IfCondition] {
    &self.conditions
  }
}

impl IfCondition {
  pub fn is_negated(&self) -> bool {
    self.negated
  }

  pub fn predicate(&self) -> &IfPredicate {
    &self.predicate
  }
}

impl IfPredicate {
  pub fn is_state_token(&self) -> bool {
    matches!(self, Self::StateToken(_))
  }

  pub fn is_entity_tag(&self) -> bool {
    matches!(self, Self::EntityTag(_))
  }
}

impl IfStateToken {
  pub fn as_str(&self) -> &str {
    &self.value
  }
}

impl fmt::Debug for IfStateToken {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("IfStateToken")
      .field("token", &"[REDACTED]")
      .finish()
  }
}

impl IfResourceTag {
  pub fn as_str(&self) -> &str {
    &self.value
  }
}

impl IfParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for IfParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for IfParseError {}

fn parse_if_value(value: &str) -> Result<(bool, Vec<IfList>), IfParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  let tagged = match bytes.get(position).copied() {
    Some(b'<') => true,
    Some(b'(') => false,
    _ => return Err(invalid_value()),
  };

  let mut lists = Vec::new();
  let mut total_conditions = 0usize;
  let mut pending_tag: Option<IfResourceTag> = None;
  let mut tag_needs_list = false;
  loop {
    skip_ows(bytes, &mut position);
    match bytes.get(position).copied() {
      None => break,
      Some(b'(') => {
        let conditions = parse_list(value, &mut position, &mut total_conditions)?;
        if lists.len() >= MAX_IF_LISTS {
          return Err(IfParseError::new("too many If lists"));
        }
        lists.push(IfList {
          resource_tag: pending_tag.clone(),
          conditions,
        });
        tag_needs_list = false;
      }
      Some(b'<') => {
        if !tagged {
          return Err(invalid_value());
        }
        let tag = parse_resource_tag(value, &mut position)?;
        pending_tag = Some(tag);
        tag_needs_list = true;
      }
      Some(_) => return Err(invalid_value()),
    }
  }
  if tag_needs_list {
    return Err(IfParseError::new(
      "If resource tag must be followed by a condition list",
    ));
  }
  Ok((tagged, lists))
}

fn parse_list(
  value: &str,
  position: &mut usize,
  total_conditions: &mut usize,
) -> Result<Vec<IfCondition>, IfParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'(') {
    return Err(invalid_value());
  }
  *position += 1;
  let mut conditions = Vec::new();
  loop {
    skip_ows(bytes, position);
    if *position == bytes.len() {
      return Err(IfParseError::new("unterminated If condition list"));
    }
    if bytes[*position] == b')' {
      if conditions.is_empty() {
        return Err(invalid_value());
      }
      *position += 1;
      return Ok(conditions);
    }
    let condition = parse_condition(value, position)?;
    if *total_conditions >= MAX_IF_CONDITIONS {
      return Err(IfParseError::new("too many If conditions"));
    }
    *total_conditions += 1;
    conditions.push(condition);
  }
}

fn parse_condition(value: &str, position: &mut usize) -> Result<IfCondition, IfParseError> {
  let bytes = value.as_bytes();
  skip_ows(bytes, position);
  let negated = value[*position..].starts_with("Not");
  if negated {
    *position += 3;
    skip_ows(bytes, position);
  }
  let predicate = match bytes.get(*position).copied() {
    Some(b'[') => IfPredicate::EntityTag(parse_entity_tag_condition(value, position)?),
    Some(b'<') => IfPredicate::StateToken(parse_state_token(value, position)?),
    _ => return Err(IfParseError::new("invalid If condition")),
  };
  Ok(IfCondition { negated, predicate })
}

fn parse_entity_tag_condition(
  value: &str,
  position: &mut usize,
) -> Result<EntityTag, IfParseError> {
  let bytes = value.as_bytes();
  *position += 1;
  let tag_start = *position;
  if bytes.get(*position..*position + 2) == Some(b"W/") {
    *position += 2;
  }
  if bytes.get(*position) != Some(&b'"') {
    return Err(IfParseError::new("invalid If entity tag"));
  }
  *position += 1;
  while let Some(&byte) = bytes.get(*position) {
    match byte {
      b'"' => {
        let tag_text = &value[tag_start..*position + 1];
        *position += 1;
        if bytes.get(*position) != Some(&b']') {
          return Err(IfParseError::new("invalid If entity tag"));
        }
        *position += 1;
        return EntityTag::parse(tag_text).map_err(|_| IfParseError::new("invalid If entity tag"));
      }
      0x21 | 0x23..=0x7e | 0x80..=0xff => *position += 1,
      _ => return Err(IfParseError::new("invalid If entity tag")),
    }
  }
  Err(IfParseError::new("invalid If entity tag"))
}

fn parse_state_token(value: &str, position: &mut usize) -> Result<IfStateToken, IfParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  let mut index = *position + 1;
  while index < bytes.len() {
    if bytes[index] == b'>' {
      let text = &value[start..index + 1];
      *position = index + 1;
      let value = parse_coded_url(text).map_err(|_| invalid_state_token())?;
      return Ok(IfStateToken { value });
    }
    index += 1;
  }
  Err(invalid_state_token())
}

fn parse_resource_tag(value: &str, position: &mut usize) -> Result<IfResourceTag, IfParseError> {
  let bytes = value.as_bytes();
  let start = *position;
  let mut index = *position + 1;
  while index < bytes.len() {
    if bytes[index] == b'>' {
      let text = &value[start..index + 1];
      *position = index + 1;
      validate_simple_ref(text).map_err(|_| invalid_resource_tag())?;
      return Ok(IfResourceTag {
        value: text.to_string(),
      });
    }
    index += 1;
  }
  Err(invalid_resource_tag())
}

fn parse_coded_url(value: &str) -> Result<String, IfParseError> {
  if value.len() < 2 || !value.starts_with('<') || !value.ends_with('>') {
    return Err(invalid_state_token());
  }
  let uri = &value[1..value.len() - 1];
  if uri.is_empty() || uri.contains(['<', '>', ' ', '\t']) || !uri.bytes().all(is_visible_byte) {
    return Err(invalid_state_token());
  }
  let parsed = Url::parse(uri).map_err(|_| invalid_state_token())?;
  if parsed.fragment().is_some() {
    return Err(invalid_state_token());
  }
  Ok(value.to_string())
}

fn validate_simple_ref(text: &str) -> Result<(), IfParseError> {
  if text.len() < 2 || !text.starts_with('<') || !text.ends_with('>') {
    return Err(invalid_resource_tag());
  }
  let reference = &text[1..text.len() - 1];
  if reference.is_empty()
    || reference.contains(['<', '>', ' ', '\t'])
    || !reference.bytes().all(|byte| byte > 0x20 && byte < 0x7f)
  {
    return Err(invalid_resource_tag());
  }
  if !is_uri_text(reference) {
    return Err(invalid_resource_tag());
  }
  if let Some(colon) = reference.find(':') {
    if is_valid_scheme(&reference[..colon]) {
      let parsed = Url::parse(reference).map_err(|_| invalid_resource_tag())?;
      if parsed.fragment().is_some() {
        return Err(invalid_resource_tag());
      }
      return Ok(());
    }
  }
  if reference.starts_with('/') && !reference.starts_with("//") {
    return Ok(());
  }
  Err(invalid_resource_tag())
}

fn validate_bounded_value(value: &str) -> Result<(), IfParseError> {
  if value.len() > MAX_IF_VALUE_BYTES {
    return Err(IfParseError::new("If header value is too large"));
  }
  if value.bytes().any(is_invalid_byte) {
    return Err(IfParseError::new("invalid If control byte"));
  }
  Ok(())
}

fn is_invalid_byte(byte: u8) -> bool {
  (byte.is_ascii_control() && byte != b'\t') || byte >= 0x80
}

fn is_valid_scheme(scheme: &str) -> bool {
  let mut bytes = scheme.bytes();
  match bytes.next() {
    Some(first) if first.is_ascii_alphabetic() => {}
    _ => return false,
  }
  bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn is_uri_text(value: &str) -> bool {
  let bytes = value.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    let byte = bytes[index];
    if byte == b'%' {
      if index + 2 >= bytes.len()
        || !bytes[index + 1].is_ascii_hexdigit()
        || !bytes[index + 2].is_ascii_hexdigit()
      {
        return false;
      }
      index += 3;
      continue;
    }
    if !is_uri_byte(byte) {
      return false;
    }
    index += 1;
  }
  true
}

fn is_uri_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'-'
        | b'.'
        | b'_'
        | b'~'
        | b':'
        | b'/'
        | b'?'
        | b'['
        | b']'
        | b'@'
        | b'!'
        | b'$'
        | b'&'
        | b'\''
        | b'('
        | b')'
        | b'*'
        | b'+'
        | b','
        | b';'
        | b'='
    )
}

fn is_visible_byte(byte: u8) -> bool {
  (0x21..=0x7e).contains(&byte)
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn invalid_value() -> IfParseError {
  IfParseError::new("invalid If header value")
}

fn invalid_state_token() -> IfParseError {
  IfParseError::new("invalid If state token")
}

fn invalid_resource_tag() -> IfParseError {
  IfParseError::new("invalid If resource tag")
}
