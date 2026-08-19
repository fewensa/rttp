//! Bounded, policy-free RFC 9211 `Cache-Status` response metadata parsing.
//!
//! This module reports the Structured Fields list of cache identifiers and
//! parameters only. It does not store cache entries, compute freshness,
//! revalidate, select endpoints, retry, or change response acceptance.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sfv::{BareItem, List, ListEntry, Parser};

pub const MAX_CACHE_STATUS_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CACHE_STATUS_MEMBERS: usize = 256;
pub const MAX_CACHE_STATUS_PARAMETERS: usize = 256;
pub const MAX_CACHE_STATUS_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Cache-Status` response metadata.
///
/// This preserves declared cache-chain members without applying cache policy,
/// freshness decisions, forwarding behavior, or response acceptance.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheStatus {
  members: Vec<CacheStatusMember>,
}

/// A cache identifier that distinguishes `sf-token` from `sf-string`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheStatusIdentifier {
  Token(String),
  String(String),
}

/// One RFC 9211 `Cache-Status` list member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStatusMember {
  identifier: CacheStatusIdentifier,
  hit: Option<bool>,
  fwd: Option<String>,
  fwd_status: Option<i64>,
  ttl: Option<i64>,
  stored: Option<bool>,
  collapsed: Option<bool>,
  key: Option<String>,
  detail: Option<CacheStatusIdentifier>,
  extensions: Vec<CacheStatusParameter>,
}

/// A well-formed extension parameter retained as metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStatusParameter {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheStatusParseError {
  message: String,
}

impl CacheStatusParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CacheStatusParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CacheStatusParseError {}

impl CacheStatus {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CacheStatusParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CacheStatusParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let combined = collect_list_values(values)?;
    let raw_members = split_list_members(&combined)?;
    let parsed = Parser::new(trim_ows(&combined))
      .parse::<List>()
      .map_err(|_| invalid_value())?;
    if parsed.is_empty() {
      return Err(invalid_value());
    }
    if parsed.len() > MAX_CACHE_STATUS_MEMBERS {
      return Err(CacheStatusParseError::new("too many Cache-Status members"));
    }
    if parsed.len() != raw_members.len() {
      return Err(invalid_value());
    }

    let mut members = Vec::with_capacity(parsed.len());
    for (entry, raw_member) in parsed.into_iter().zip(raw_members) {
      let ListEntry::Item(item) = entry else {
        return Err(invalid_value());
      };
      reject_duplicate_parameters(raw_member)?;
      members.push(parse_member(item.bare_item, &item.params)?);
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[CacheStatusMember] {
    &self.members
  }

  pub fn len(&self) -> usize {
    self.members.len()
  }

  pub fn is_empty(&self) -> bool {
    self.members.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .members
      .iter()
      .map(CacheStatusMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl CacheStatusIdentifier {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Token(value) | Self::String(value) => value,
    }
  }

  pub fn is_token(&self) -> bool {
    matches!(self, Self::Token(_))
  }

  pub fn is_string(&self) -> bool {
    matches!(self, Self::String(_))
  }

  fn header_value(&self) -> String {
    match self {
      Self::Token(value) => value.clone(),
      Self::String(value) => format!("\"{}\"", escape_sf_string(value)),
    }
  }
}

impl CacheStatusMember {
  pub fn identifier(&self) -> &CacheStatusIdentifier {
    &self.identifier
  }

  pub fn hit(&self) -> Option<bool> {
    self.hit
  }

  pub fn fwd(&self) -> Option<&str> {
    self.fwd.as_deref()
  }

  pub fn fwd_status(&self) -> Option<i64> {
    self.fwd_status
  }

  pub fn ttl(&self) -> Option<i64> {
    self.ttl
  }

  pub fn stored(&self) -> Option<bool> {
    self.stored
  }

  pub fn collapsed(&self) -> Option<bool> {
    self.collapsed
  }

  pub fn key(&self) -> Option<&str> {
    self.key.as_deref()
  }

  pub fn detail(&self) -> Option<&CacheStatusIdentifier> {
    self.detail.as_ref()
  }

  pub fn extensions(&self) -> &[CacheStatusParameter] {
    &self.extensions
  }

  fn header_value(&self) -> String {
    let mut value = self.identifier.header_value();
    append_boolean_parameter(&mut value, "hit", self.hit);
    if let Some(fwd) = &self.fwd {
      value.push_str("; fwd=");
      value.push_str(fwd);
    }
    if let Some(fwd_status) = self.fwd_status {
      value.push_str("; fwd-status=");
      value.push_str(&fwd_status.to_string());
    }
    if let Some(ttl) = self.ttl {
      value.push_str("; ttl=");
      value.push_str(&ttl.to_string());
    }
    append_boolean_parameter(&mut value, "stored", self.stored);
    append_boolean_parameter(&mut value, "collapsed", self.collapsed);
    if let Some(key) = &self.key {
      value.push_str("; key=\"");
      value.push_str(&escape_sf_string(key));
      value.push('"');
    }
    if let Some(detail) = &self.detail {
      value.push_str("; detail=");
      value.push_str(&detail.header_value());
    }
    for parameter in &self.extensions {
      value.push_str("; ");
      value.push_str(parameter.name());
      if let Some(parameter_value) = parameter.value() {
        value.push('=');
        value.push_str(parameter_value);
      }
    }
    value
  }
}

impl CacheStatusParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

fn collect_list_values<'a, I>(values: I) -> Result<String, CacheStatusParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_field(value)?;
  let mut combined = value.to_owned();
  for value in values {
    validate_field(value)?;
    combined.push(',');
    combined.push_str(value);
  }
  Ok(combined)
}

fn validate_field(value: &str) -> Result<(), CacheStatusParseError> {
  if value.len() > MAX_CACHE_STATUS_VALUE_BYTES {
    return Err(CacheStatusParseError::new(
      "Cache-Status header value is too large",
    ));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(CacheStatusParseError::new(
      "invalid Cache-Status control byte",
    ));
  }
  Ok(())
}

fn parse_member(
  identifier: BareItem,
  parameters: &sfv::Parameters,
) -> Result<CacheStatusMember, CacheStatusParseError> {
  if parameters.len() > MAX_CACHE_STATUS_PARAMETERS {
    return Err(CacheStatusParseError::new(
      "too many Cache-Status parameters",
    ));
  }

  let mut member = CacheStatusMember {
    identifier: parse_identifier(identifier)?,
    hit: None,
    fwd: None,
    fwd_status: None,
    ttl: None,
    stored: None,
    collapsed: None,
    key: None,
    detail: None,
    extensions: Vec::new(),
  };

  for (name, value) in parameters {
    let name = name.as_str();
    match name {
      "hit" => member.hit = Some(parse_boolean(value.clone())?),
      "fwd" => member.fwd = Some(parse_token(value.clone(), "fwd")?),
      "fwd-status" => member.fwd_status = Some(parse_integer(value.clone(), "fwd-status")?),
      "ttl" => member.ttl = Some(parse_integer(value.clone(), "ttl")?),
      "stored" => member.stored = Some(parse_boolean(value.clone())?),
      "collapsed" => member.collapsed = Some(parse_boolean(value.clone())?),
      "key" => member.key = Some(parse_string(value.clone(), "key")?),
      "detail" => member.detail = Some(parse_identifier(value.clone())?),
      _ => member
        .extensions
        .push(parse_extension(name, value.clone())?),
    }
  }

  Ok(member)
}

fn parse_identifier(value: BareItem) -> Result<CacheStatusIdentifier, CacheStatusParseError> {
  match value {
    BareItem::Token(value) => Ok(CacheStatusIdentifier::Token(value.as_str().to_owned())),
    BareItem::String(value) => Ok(CacheStatusIdentifier::String(value.as_str().to_owned())),
    _ => Err(invalid_value()),
  }
}

fn parse_boolean(value: BareItem) -> Result<bool, CacheStatusParseError> {
  match value {
    BareItem::Boolean(value) => Ok(value),
    _ => Err(invalid_value()),
  }
}

fn parse_token(value: BareItem, name: &str) -> Result<String, CacheStatusParseError> {
  match value {
    BareItem::Token(value) => {
      let serialized = value.as_str();
      reject_oversized_parameter(name, serialized.len())?;
      Ok(serialized.to_owned())
    }
    _ => Err(invalid_value()),
  }
}

fn parse_integer(value: BareItem, name: &str) -> Result<i64, CacheStatusParseError> {
  match value {
    BareItem::Integer(value) => {
      let value = i64::from(value);
      reject_oversized_parameter(name, value.to_string().len())?;
      Ok(value)
    }
    _ => Err(invalid_value()),
  }
}

fn parse_string(value: BareItem, name: &str) -> Result<String, CacheStatusParseError> {
  match value {
    BareItem::String(value) => {
      let serialized = format!("\"{}\"", escape_sf_string(value.as_str()));
      reject_oversized_parameter(name, serialized.len())?;
      Ok(value.as_str().to_owned())
    }
    _ => Err(invalid_value()),
  }
}

fn parse_extension(
  name: &str,
  value: BareItem,
) -> Result<CacheStatusParameter, CacheStatusParseError> {
  let serialized = serialize_bare_item(&value);
  if let Some(serialized) = &serialized {
    reject_oversized_parameter(name, serialized.len())?;
  }
  Ok(CacheStatusParameter {
    name: name.to_owned(),
    value: serialized,
  })
}

fn reject_oversized_parameter(
  _name: &str,
  value_bytes: usize,
) -> Result<(), CacheStatusParseError> {
  if value_bytes > MAX_CACHE_STATUS_PARAMETER_VALUE_BYTES {
    return Err(CacheStatusParseError::new(
      "Cache-Status parameter value is too large",
    ));
  }
  Ok(())
}

fn reject_duplicate_parameters(member: &str) -> Result<(), CacheStatusParseError> {
  let keys = raw_parameter_keys(member)?;
  if keys.len() > MAX_CACHE_STATUS_PARAMETERS {
    return Err(CacheStatusParseError::new(
      "too many Cache-Status parameters",
    ));
  }
  let mut seen = HashSet::with_capacity(keys.len());
  for key in keys {
    if !seen.insert(key) {
      return Err(CacheStatusParseError::new(
        "duplicate Cache-Status parameter",
      ));
    }
  }
  Ok(())
}

fn raw_parameter_keys(member: &str) -> Result<Vec<String>, CacheStatusParseError> {
  let bytes = member.as_bytes();
  let mut index = 0usize;
  skip_ows(bytes, &mut index);
  skip_identifier(bytes, &mut index)?;
  let mut keys = Vec::new();
  loop {
    skip_ows(bytes, &mut index);
    if index >= bytes.len() {
      break;
    }
    if bytes[index] != b';' {
      return Err(invalid_value());
    }
    index += 1;
    skip_ows(bytes, &mut index);
    let start = index;
    while index < bytes.len() && is_key_char(bytes[index]) {
      index += 1;
    }
    if start == index {
      return Err(invalid_value());
    }
    keys.push(member[start..index].to_owned());
    skip_ows(bytes, &mut index);
    if index < bytes.len() && bytes[index] == b'=' {
      index += 1;
      skip_bare_item(bytes, &mut index)?;
    }
  }
  Ok(keys)
}

fn skip_identifier(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  match bytes.get(*index) {
    Some(b'"') => skip_quoted_string(bytes, index),
    Some(byte) if is_token_start(*byte) => {
      *index += 1;
      while matches!(bytes.get(*index), Some(byte) if is_token_char(*byte)) {
        *index += 1;
      }
      Ok(())
    }
    _ => Err(invalid_value()),
  }
}

fn skip_bare_item(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  match bytes.get(*index) {
    Some(b'"') => skip_quoted_string(bytes, index),
    Some(b':') => skip_byte_sequence(bytes, index),
    Some(b'?') => {
      *index += 1;
      match bytes.get(*index) {
        Some(b'0' | b'1') => {
          *index += 1;
          Ok(())
        }
        _ => Err(invalid_value()),
      }
    }
    Some(b'%') => {
      *index += 1;
      if bytes.get(*index) != Some(&b'"') {
        return Err(invalid_value());
      }
      skip_quoted_string(bytes, index)
    }
    Some(b'@') => {
      *index += 1;
      skip_integer_digits(bytes, index)
    }
    Some(b'-' | b'0'..=b'9') => skip_number(bytes, index),
    Some(byte) if is_token_start(*byte) => {
      *index += 1;
      while matches!(bytes.get(*index), Some(byte) if is_token_char(*byte)) {
        *index += 1;
      }
      Ok(())
    }
    _ => Err(invalid_value()),
  }
}

fn skip_quoted_string(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  if bytes.get(*index) != Some(&b'"') {
    return Err(invalid_value());
  }
  *index += 1;
  let mut escaped = false;
  while *index < bytes.len() {
    let byte = bytes[*index];
    *index += 1;
    if escaped {
      escaped = false;
      continue;
    }
    if byte == b'\\' {
      escaped = true;
      continue;
    }
    if byte == b'"' {
      return Ok(());
    }
  }
  Err(invalid_value())
}

fn skip_byte_sequence(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  if bytes.get(*index) != Some(&b':') {
    return Err(invalid_value());
  }
  *index += 1;
  while *index < bytes.len() {
    if bytes[*index] == b':' {
      *index += 1;
      return Ok(());
    }
    *index += 1;
  }
  Err(invalid_value())
}

fn skip_integer_digits(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  if matches!(bytes.get(*index), Some(b'-')) {
    *index += 1;
  }
  let start = *index;
  while matches!(bytes.get(*index), Some(b'0'..=b'9')) {
    *index += 1;
  }
  if start == *index {
    return Err(invalid_value());
  }
  Ok(())
}

fn skip_number(bytes: &[u8], index: &mut usize) -> Result<(), CacheStatusParseError> {
  skip_integer_digits(bytes, index)?;
  if bytes.get(*index) == Some(&b'.') {
    *index += 1;
    let start = *index;
    while matches!(bytes.get(*index), Some(b'0'..=b'9')) {
      *index += 1;
    }
    if start == *index {
      return Err(invalid_value());
    }
  }
  Ok(())
}

fn split_list_members(value: &str) -> Result<Vec<&str>, CacheStatusParseError> {
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
        depth = depth.checked_sub(1).ok_or_else(invalid_value)?;
      }
      b',' if depth == 0 => {
        members.push(trim_ows(&value[start..index]));
        start = index + 1;
      }
      _ => {}
    }
  }
  if in_string || depth != 0 {
    return Err(invalid_value());
  }
  members.push(trim_ows(&value[start..]));
  if members.iter().any(|member| member.is_empty()) {
    return Err(invalid_value());
  }
  Ok(members)
}

fn serialize_bare_item(value: &BareItem) -> Option<String> {
  match value {
    BareItem::Boolean(true) => None,
    BareItem::Boolean(false) => Some("?0".to_owned()),
    BareItem::Integer(value) => Some(i64::from(*value).to_string()),
    BareItem::Decimal(value) => Some(value.to_string()),
    BareItem::String(value) => Some(format!("\"{}\"", escape_sf_string(value.as_str()))),
    BareItem::Token(value) => Some(value.as_str().to_owned()),
    BareItem::ByteSequence(value) => Some(format!(":{}:", STANDARD.encode(value))),
    BareItem::Date(value) => Some(format!("@{}", i64::from(value.unix_seconds()))),
    BareItem::DisplayString(value) => Some(format!("%\"{}\"", escape_display_string(value))),
  }
}

fn append_boolean_parameter(output: &mut String, name: &str, value: Option<bool>) {
  match value {
    Some(true) => {
      output.push_str("; ");
      output.push_str(name);
    }
    Some(false) => {
      output.push_str("; ");
      output.push_str(name);
      output.push_str("=?0");
    }
    None => {}
  }
}

fn escape_sf_string(value: &str) -> String {
  let mut escaped = String::new();
  for byte in value.bytes() {
    match byte {
      b'\\' | b'"' => {
        escaped.push('\\');
        escaped.push(byte as char);
      }
      _ => escaped.push(byte as char),
    }
  }
  escaped
}

fn escape_display_string(value: &str) -> String {
  let mut escaped = String::new();
  for byte in value.bytes() {
    match byte {
      b'\\' | b'"' => {
        escaped.push('\\');
        escaped.push(byte as char);
      }
      _ => escaped.push(byte as char),
    }
  }
  escaped
}

fn skip_ows(bytes: &[u8], index: &mut usize) {
  while matches!(bytes.get(*index), Some(b' ' | b'\t')) {
    *index += 1;
  }
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_key_char(byte: u8) -> bool {
  matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
}

fn is_token_start(byte: u8) -> bool {
  byte.is_ascii_alphabetic() || byte == b'*'
}

fn is_token_char(byte: u8) -> bool {
  matches!(
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
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'^'
      | b'_'
      | b'`'
      | b'a'..=b'z'
      | b'|'
      | b'~'
      | b':'
      | b'/'
  )
}

fn invalid_value() -> CacheStatusParseError {
  CacheStatusParseError::new("invalid Cache-Status header value")
}
