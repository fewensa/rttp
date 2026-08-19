//! Bounded, policy-free RFC 9209 `Proxy-Status` response metadata parsing.
//!
//! This module validates one or more `Proxy-Status` fields as a Structured
//! Fields list of Token or String proxy identifiers with opaque parameters.
//! It does not interpret operational health, promote trailers, retry
//! requests, or generate origin `Proxy-Status` values.
//!
//! ```
//! use rttp_protocol::proxy_status::{ProxyStatus, ProxyStatusBareItem, ProxyStatusIdentifier};
//!
//! let status = ProxyStatus::parse("ExampleCDN; error=connection_timeout")
//!   .expect("valid Proxy-Status");
//! assert_eq!(
//!   status.members()[0].identifier(),
//!   &ProxyStatusIdentifier::Token("ExampleCDN".to_string())
//! );
//! assert_eq!(
//!   status.members()[0]
//!     .parameter("error")
//!     .map(|parameter| parameter.value()),
//!   Some(&ProxyStatusBareItem::Token("connection_timeout".to_string()))
//! );
//! ```

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sfv::{BareItem, List, ListEntry, Parser};

/// Maximum bytes accepted in one `Proxy-Status` field value.
pub const MAX_PROXY_STATUS_VALUE_BYTES: usize = 64 * 1024;
/// Maximum proxy identifiers accepted across combined fields.
pub const MAX_PROXY_STATUS_MEMBERS: usize = 256;
/// Maximum parameters accepted on one proxy identifier.
pub const MAX_PROXY_STATUS_PARAMETERS: usize = 256;
/// Maximum decoded bytes accepted in one parameter value.
pub const MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded RFC 9209 `Proxy-Status` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyStatus {
  members: Vec<ProxyStatusMember>,
}

/// One proxy identifier and its opaque parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyStatusMember {
  identifier: ProxyStatusIdentifier,
  parameters: Vec<ProxyStatusParameter>,
}

/// A Token or String proxy identifier from a `Proxy-Status` list member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxyStatusIdentifier {
  Token(String),
  String(String),
}

/// One opaque Structured Fields parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyStatusParameter {
  name: String,
  value: ProxyStatusBareItem,
}

/// An uninterpreted Structured Fields parameter value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxyStatusBareItem {
  Boolean(bool),
  Integer(i64),
  Decimal(String),
  String(String),
  Token(String),
  ByteSequence(Vec<u8>),
  Date(i64),
  DisplayString(String),
}

/// An error returned when `Proxy-Status` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyStatusParseError {
  message: String,
}

impl ProxyStatusParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ProxyStatusParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ProxyStatusParseError {}

impl ProxyStatus {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ProxyStatusParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ProxyStatusParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut members = Vec::new();
    for value in values {
      validate_field(value)?;
      parse_field(value, &mut members)?;
    }
    if members.is_empty() {
      return Err(invalid_list());
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[ProxyStatusMember] {
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
      .map(ProxyStatusMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ProxyStatusMember {
  pub fn identifier(&self) -> &ProxyStatusIdentifier {
    &self.identifier
  }

  pub fn parameters(&self) -> &[ProxyStatusParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&ProxyStatusParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  fn header_value(&self) -> String {
    let mut value = self.identifier.header_value();
    append_parameters(&mut value, &self.parameters);
    value
  }
}

impl ProxyStatusIdentifier {
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

impl ProxyStatusParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &ProxyStatusBareItem {
    &self.value
  }
}

fn validate_field(value: &str) -> Result<(), ProxyStatusParseError> {
  if value.len() > MAX_PROXY_STATUS_VALUE_BYTES {
    return Err(ProxyStatusParseError::new(
      "Proxy-Status header value is too large",
    ));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(ProxyStatusParseError::new(
      "Proxy-Status header value contains an invalid control byte",
    ));
  }
  Ok(())
}

fn parse_field(
  value: &str,
  members: &mut Vec<ProxyStatusMember>,
) -> Result<(), ProxyStatusParseError> {
  let list = Parser::new(value)
    .parse::<List>()
    .map_err(|_| invalid_list())?;
  if list.is_empty() {
    return Err(invalid_list());
  }
  reject_duplicate_parameters(value)?;

  for entry in list {
    if members.len() >= MAX_PROXY_STATUS_MEMBERS {
      return Err(ProxyStatusParseError::new("too many Proxy-Status members"));
    }
    let ListEntry::Item(item) = entry else {
      return Err(ProxyStatusParseError::new(
        "Proxy-Status members must be Token or String identifiers",
      ));
    };
    let identifier = match item.bare_item {
      BareItem::Token(token) => ProxyStatusIdentifier::Token(token.as_str().to_owned()),
      BareItem::String(string) => ProxyStatusIdentifier::String(string.as_str().to_owned()),
      _ => {
        return Err(ProxyStatusParseError::new(
          "Proxy-Status members must be Token or String identifiers",
        ))
      }
    };
    if item.params.len() > MAX_PROXY_STATUS_PARAMETERS {
      return Err(ProxyStatusParseError::new(
        "too many Proxy-Status parameters",
      ));
    }
    members.push(ProxyStatusMember {
      identifier,
      parameters: convert_parameters(item.params)?,
    });
  }
  Ok(())
}

fn convert_parameters(
  parameters: sfv::Parameters,
) -> Result<Vec<ProxyStatusParameter>, ProxyStatusParseError> {
  parameters
    .into_iter()
    .map(|(name, value)| {
      let parameter = ProxyStatusParameter {
        name: name.as_str().to_owned(),
        value: convert_bare_item(value)?,
      };
      if parameter_value_bytes(&parameter.value) > MAX_PROXY_STATUS_PARAMETER_VALUE_BYTES {
        return Err(ProxyStatusParseError::new(
          "Proxy-Status parameter value is too large",
        ));
      }
      Ok(parameter)
    })
    .collect()
}

fn convert_bare_item(value: BareItem) -> Result<ProxyStatusBareItem, ProxyStatusParseError> {
  Ok(match value {
    BareItem::Boolean(value) => ProxyStatusBareItem::Boolean(value),
    BareItem::Integer(value) => ProxyStatusBareItem::Integer(i64::from(value)),
    BareItem::Decimal(value) => ProxyStatusBareItem::Decimal(value.to_string()),
    BareItem::String(value) => ProxyStatusBareItem::String(value.as_str().to_owned()),
    BareItem::Token(value) => ProxyStatusBareItem::Token(value.as_str().to_owned()),
    BareItem::ByteSequence(value) => ProxyStatusBareItem::ByteSequence(value),
    BareItem::Date(value) => ProxyStatusBareItem::Date(i64::from(value.unix_seconds())),
    BareItem::DisplayString(value) => ProxyStatusBareItem::DisplayString(value),
  })
}

fn parameter_value_bytes(value: &ProxyStatusBareItem) -> usize {
  match value {
    ProxyStatusBareItem::Boolean(_) => 2,
    ProxyStatusBareItem::Integer(value) => value.to_string().len(),
    ProxyStatusBareItem::Decimal(value) => value.len(),
    ProxyStatusBareItem::String(value) => value.len(),
    ProxyStatusBareItem::Token(value) => value.len(),
    ProxyStatusBareItem::ByteSequence(value) => value.len(),
    ProxyStatusBareItem::Date(value) => value.to_string().len(),
    ProxyStatusBareItem::DisplayString(value) => value.len(),
  }
}

fn reject_duplicate_parameters(value: &str) -> Result<(), ProxyStatusParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(invalid_list());
  }

  loop {
    skip_identifier_or_inner_list(bytes, &mut position)?;
    let mut seen = HashSet::new();
    while bytes.get(position) == Some(&b';') {
      position += 1;
      skip_sp(bytes, &mut position);
      let name = parse_key(value, &mut position)?;
      if !seen.insert(name) {
        return Err(ProxyStatusParseError::new(
          "duplicate Proxy-Status parameter",
        ));
      }
      if bytes.get(position) == Some(&b'=') {
        position += 1;
        skip_bare_item(bytes, &mut position)?;
      }
    }
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(invalid_list());
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(invalid_list());
    }
  }
}

fn skip_identifier_or_inner_list(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), ProxyStatusParseError> {
  match bytes.get(*position) {
    Some(b'"') => skip_string(bytes, position),
    Some(b'(') => skip_inner_list(bytes, position),
    Some(b'%') if bytes.get(*position + 1) == Some(&b'"') => skip_display_string(bytes, position),
    Some(b':' | b'?' | b'@' | b'-' | b'0'..=b'9') => skip_bare_item(bytes, position),
    Some(b'*' | b'A'..=b'Z' | b'a'..=b'z') => skip_token(bytes, position),
    _ => Err(invalid_list()),
  }
}

fn skip_inner_list(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  *position += 1;
  loop {
    skip_sp(bytes, position);
    if bytes.get(*position) == Some(&b')') {
      *position += 1;
      return Ok(());
    }
    if *position >= bytes.len() {
      return Err(invalid_list());
    }
    skip_bare_item(bytes, position)?;
    while bytes.get(*position) == Some(&b';') {
      *position += 1;
      skip_sp(bytes, position);
      skip_key(bytes, position)?;
      if bytes.get(*position) == Some(&b'=') {
        *position += 1;
        skip_bare_item(bytes, position)?;
      }
    }
  }
}

fn skip_bare_item(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  match bytes.get(*position) {
    Some(b'?') => {
      if matches!(bytes.get(*position + 1), Some(b'0' | b'1')) {
        *position += 2;
        Ok(())
      } else {
        Err(invalid_list())
      }
    }
    Some(b':') => skip_byte_sequence(bytes, position),
    Some(b'"') => skip_string(bytes, position),
    Some(b'%') if bytes.get(*position + 1) == Some(&b'"') => skip_display_string(bytes, position),
    Some(b'@') => {
      *position += 1;
      skip_number(bytes, position)
    }
    Some(b'-' | b'0'..=b'9') => skip_number(bytes, position),
    Some(b'*' | b'A'..=b'Z' | b'a'..=b'z') => skip_token(bytes, position),
    _ => Err(invalid_list()),
  }
}

fn skip_string(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  *position += 1;
  while *position < bytes.len() {
    match bytes[*position] {
      b'\\' => {
        *position += 1;
        if *position >= bytes.len() {
          return Err(invalid_list());
        }
        *position += 1;
      }
      b'"' => {
        *position += 1;
        return Ok(());
      }
      _ => *position += 1,
    }
  }
  Err(invalid_list())
}

fn skip_display_string(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  *position += 2;
  while *position < bytes.len() {
    match bytes[*position] {
      b'%' => *position = position.saturating_add(3),
      b'"' => {
        *position += 1;
        return Ok(());
      }
      _ => *position += 1,
    }
  }
  Err(invalid_list())
}

fn skip_byte_sequence(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  *position += 1;
  while *position < bytes.len() {
    if bytes[*position] == b':' {
      *position += 1;
      return Ok(());
    }
    *position += 1;
  }
  Err(invalid_list())
}

fn skip_number(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  if bytes.get(*position) == Some(&b'-') {
    *position += 1;
  }
  let start = *position;
  while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
    *position += 1;
  }
  if *position == start {
    return Err(invalid_list());
  }
  if bytes.get(*position) == Some(&b'.') {
    *position += 1;
    let fraction_start = *position;
    while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      *position += 1;
    }
    if *position == fraction_start {
      return Err(invalid_list());
    }
  }
  Ok(())
}

fn skip_token(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  let start = *position;
  while matches!(
    bytes.get(*position),
    Some(
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
      | b':'
      | b'/'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z',
    )
  ) {
    *position += 1;
  }
  if *position == start {
    Err(invalid_list())
  } else {
    Ok(())
  }
}

fn parse_key<'a>(value: &'a str, position: &mut usize) -> Result<&'a str, ProxyStatusParseError> {
  let start = *position;
  skip_key(value.as_bytes(), position)?;
  Ok(&value[start..*position])
}

fn skip_key(bytes: &[u8], position: &mut usize) -> Result<(), ProxyStatusParseError> {
  if !matches!(bytes.get(*position), Some(b'a'..=b'z' | b'*')) {
    return Err(invalid_list());
  }
  *position += 1;
  while matches!(
    bytes.get(*position),
    Some(b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
  ) {
    *position += 1;
  }
  Ok(())
}

fn append_parameters(output: &mut String, parameters: &[ProxyStatusParameter]) {
  for parameter in parameters {
    output.push(';');
    output.push_str(&parameter.name);
    match &parameter.value {
      ProxyStatusBareItem::Boolean(true) => {}
      ProxyStatusBareItem::Boolean(false) => output.push_str("=?0"),
      ProxyStatusBareItem::Integer(value) => {
        output.push('=');
        output.push_str(&value.to_string());
      }
      ProxyStatusBareItem::Decimal(value) => {
        output.push('=');
        output.push_str(value);
      }
      ProxyStatusBareItem::String(value) => {
        output.push_str("=\"");
        output.push_str(&escape_sf_string(value));
        output.push('"');
      }
      ProxyStatusBareItem::Token(value) => {
        output.push('=');
        output.push_str(value);
      }
      ProxyStatusBareItem::ByteSequence(value) => {
        output.push_str("=:");
        output.push_str(&STANDARD.encode(value));
        output.push(':');
      }
      ProxyStatusBareItem::Date(value) => {
        output.push_str("=@");
        output.push_str(&value.to_string());
      }
      ProxyStatusBareItem::DisplayString(value) => {
        output.push_str("=%\"");
        output.push_str(&escape_display_string(value));
        output.push('"');
      }
    }
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
  for byte in value.as_bytes() {
    match byte {
      0x00..=0x1f | b'%' | b'"' | 0x7f..=0xff => {
        escaped.push_str(&format!("%{byte:02x}"));
      }
      _ => escaped.push(*byte as char),
    }
  }
  escaped
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}

fn skip_sp(bytes: &[u8], position: &mut usize) {
  while bytes.get(*position) == Some(&b' ') {
    *position += 1;
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn invalid_list() -> ProxyStatusParseError {
  ProxyStatusParseError::new("invalid Proxy-Status list")
}
