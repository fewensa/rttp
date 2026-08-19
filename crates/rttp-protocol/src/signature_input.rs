//! Bounded, policy-free RFC 9421 `Signature-Input` field parse and format.
//!
//! This module validates labeled Structured Fields dictionaries whose members
//! are inner lists of component-identifier strings. Well-formed member and
//! component parameters are retained as opaque data and are not interpreted.
//! The parser does not sign, verify, look up keys, canonicalize covered
//! components, or apply cryptographic policy.

use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sfv::{BareItem, Dictionary, ListEntry, Parser};

pub const MAX_SIGNATURE_INPUT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SIGNATURE_INPUT_ENTRIES: usize = 256;
pub const MAX_SIGNATURE_INPUT_MEMBERS: usize = MAX_SIGNATURE_INPUT_ENTRIES;
pub const MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS: usize = 256;
pub const MAX_SIGNATURE_INPUT_PARAMETERS: usize = MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS;
pub const MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS: usize = 256;
pub const MAX_SIGNATURE_INPUT_COVERED_COMPONENTS: usize = MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS;
pub const MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS: usize = 256;
pub const MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES: usize = MAX_SIGNATURE_INPUT_VALUE_BYTES;

/// Parsed, bounded HTTP `Signature-Input` field metadata.
///
/// Labels, covered-component identifiers, and well-formed parameters are
/// retained as opaque dictionary members. This type does not perform
/// cryptographic signing, verification, key lookup, or covered-component
/// canonicalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInput {
  entries: Vec<SignatureInputEntry>,
}

/// One labeled `Signature-Input` dictionary member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInputEntry {
  label: String,
  components: Vec<SignatureInputComponent>,
  parameters: Vec<SignatureInputParameter>,
}

/// One covered-component identifier and its opaque parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInputComponent {
  identifier: String,
  parameters: Vec<SignatureInputParameter>,
}

/// One opaque Structured Fields parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInputParameter {
  name: String,
  value: SignatureInputBareItem,
}

/// An uninterpreted Structured Fields parameter value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureInputBareItem {
  Boolean(bool),
  Integer(i64),
  Decimal(String),
  String(String),
  Token(String),
  ByteSequence(Vec<u8>),
  Date(i64),
  DisplayString(String),
}

pub type SignatureInputMember = SignatureInputEntry;
pub type SignatureCoveredComponent = SignatureInputComponent;
pub type SignatureParameter = SignatureInputParameter;
pub type SignatureParameterValue = SignatureInputBareItem;
pub type SignatureDecimal = String;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInputParseError {
  message: String,
}

impl SignatureInputParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SignatureInputParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SignatureInputParseError {}

impl SignatureInput {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SignatureInputParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SignatureInputParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut entries = Vec::new();
    let mut member_count = 0usize;
    for value in values {
      if value.len() > MAX_SIGNATURE_INPUT_VALUE_BYTES {
        return Err(SignatureInputParseError::new(
          "Signature-Input field value is too large",
        ));
      }
      parse_field(value, &mut entries, &mut member_count)?;
    }
    if entries.is_empty() {
      return Err(SignatureInputParseError::new(
        "Signature-Input field must contain an entry",
      ));
    }
    Ok(Self { entries })
  }

  pub fn entries(&self) -> &[SignatureInputEntry] {
    &self.entries
  }

  pub fn members(&self) -> &[SignatureInputMember] {
    &self.entries
  }

  pub fn entry(&self, label: impl AsRef<str>) -> Option<&SignatureInputEntry> {
    self
      .entries
      .iter()
      .find(|entry| entry.label == label.as_ref())
  }

  pub fn member(&self, label: impl AsRef<str>) -> Option<&SignatureInputMember> {
    self.entry(label)
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .entries
      .iter()
      .map(SignatureInputEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SignatureInputEntry {
  pub fn label(&self) -> &str {
    &self.label
  }

  pub fn components(&self) -> &[SignatureInputComponent] {
    &self.components
  }

  pub fn covered_components(&self) -> &[SignatureCoveredComponent] {
    &self.components
  }

  pub fn parameters(&self) -> &[SignatureInputParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&SignatureInputParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  fn header_value(&self) -> String {
    let mut value = format!("{}=(", self.label);
    for (index, component) in self.components.iter().enumerate() {
      if index > 0 {
        value.push(' ');
      }
      value.push_str(&component.header_value());
    }
    value.push(')');
    append_parameters(&mut value, &self.parameters);
    value
  }
}

impl SignatureInputComponent {
  pub fn identifier(&self) -> &str {
    &self.identifier
  }

  pub fn parameters(&self) -> &[SignatureInputParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&SignatureInputParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  fn header_value(&self) -> String {
    let mut value = format!("\"{}\"", escape_sf_string(&self.identifier));
    append_parameters(&mut value, &self.parameters);
    value
  }
}

impl SignatureInputParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &SignatureInputBareItem {
    &self.value
  }

  pub fn is_valueless(&self) -> bool {
    false
  }
}

fn parse_field(
  value: &str,
  entries: &mut Vec<SignatureInputEntry>,
  member_count: &mut usize,
) -> Result<(), SignatureInputParseError> {
  let field_member_count = count_top_level_members(value);
  if member_count
    .checked_add(field_member_count)
    .is_none_or(|count| count > MAX_SIGNATURE_INPUT_MEMBERS)
  {
    return Err(SignatureInputParseError::new(
      "too many Signature-Input field entries",
    ));
  }

  let dictionary = Parser::new(value)
    .parse::<Dictionary>()
    .map_err(|_| invalid_member())?;
  if dictionary.is_empty() {
    return Err(SignatureInputParseError::new(
      "Signature-Input field must contain an entry",
    ));
  }
  *member_count += field_member_count;

  for (key, member) in dictionary {
    let ListEntry::InnerList(inner_list) = member else {
      return Err(invalid_member());
    };
    let label = key.as_str().to_owned();
    if inner_list.items.is_empty() {
      return Err(invalid_member());
    }
    if inner_list.items.len() > MAX_SIGNATURE_INPUT_ENTRY_COMPONENTS {
      return Err(SignatureInputParseError::new(
        "too many Signature-Input entry components",
      ));
    }
    if inner_list.params.len() > MAX_SIGNATURE_INPUT_ENTRY_PARAMETERS {
      return Err(SignatureInputParseError::new(
        "too many Signature-Input entry parameters",
      ));
    }
    let mut components = Vec::with_capacity(inner_list.items.len());
    for item in inner_list.items {
      let BareItem::String(identifier) = item.bare_item else {
        return Err(invalid_member());
      };
      if item.params.len() > MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS {
        return Err(SignatureInputParseError::new(
          "too many Signature-Input component parameters",
        ));
      }
      components.push(SignatureInputComponent {
        identifier: identifier.as_str().to_owned(),
        parameters: convert_parameters(item.params)?,
      });
    }
    let entry = SignatureInputEntry {
      label,
      components,
      parameters: convert_parameters(inner_list.params)?,
    };
    if let Some(existing) = entries
      .iter_mut()
      .find(|existing| existing.label == entry.label)
    {
      *existing = entry;
    } else {
      entries.push(entry);
    }
  }
  Ok(())
}

fn count_top_level_members(value: &str) -> usize {
  let bytes = value.as_bytes();
  let mut count = usize::from(!value.trim().is_empty());
  let mut depth = 0usize;
  let mut previous_significant = None;
  let mut index = 0usize;

  while index < bytes.len() {
    let byte = bytes[index];
    match byte {
      b'"' => {
        index = skip_string(bytes, index + 1);
        previous_significant = Some(byte);
      }
      b'%' if bytes.get(index + 1) == Some(&b'"') => {
        index = skip_display_string(bytes, index + 2);
        previous_significant = Some(b'"');
      }
      b':' if previous_significant == Some(b'=') => {
        index = skip_byte_sequence(bytes, index + 1);
        previous_significant = Some(byte);
      }
      b'(' => {
        depth = depth.saturating_add(1);
        previous_significant = Some(byte);
        index += 1;
      }
      b')' => {
        depth = depth.saturating_sub(1);
        previous_significant = Some(byte);
        index += 1;
      }
      b',' if depth == 0 => {
        count += 1;
        previous_significant = Some(byte);
        index += 1;
      }
      b' ' | b'\t' => {
        index += 1;
      }
      _ => {
        previous_significant = Some(byte);
        index += 1;
      }
    }
  }

  count
}

fn skip_string(bytes: &[u8], mut index: usize) -> usize {
  while index < bytes.len() {
    match bytes[index] {
      b'\\' => index += 2,
      b'"' => return index + 1,
      _ => index += 1,
    }
  }
  index
}

fn skip_display_string(bytes: &[u8], mut index: usize) -> usize {
  while index < bytes.len() {
    match bytes[index] {
      b'%' => index += 3,
      b'"' => return index + 1,
      _ => index += 1,
    }
  }
  index
}

fn skip_byte_sequence(bytes: &[u8], mut index: usize) -> usize {
  while index < bytes.len() {
    if bytes[index] == b':' {
      return index + 1;
    }
    index += 1;
  }
  index
}

fn convert_parameters(
  parameters: sfv::Parameters,
) -> Result<Vec<SignatureInputParameter>, SignatureInputParseError> {
  parameters
    .into_iter()
    .map(|(name, value)| {
      Ok(SignatureInputParameter {
        name: name.as_str().to_owned(),
        value: convert_bare_item(value)?,
      })
    })
    .collect()
}

fn convert_bare_item(value: BareItem) -> Result<SignatureInputBareItem, SignatureInputParseError> {
  Ok(match value {
    BareItem::Boolean(value) => SignatureInputBareItem::Boolean(value),
    BareItem::Integer(value) => SignatureInputBareItem::Integer(i64::from(value)),
    BareItem::Decimal(value) => SignatureInputBareItem::Decimal(value.to_string()),
    BareItem::String(value) => SignatureInputBareItem::String(value.as_str().to_owned()),
    BareItem::Token(value) => SignatureInputBareItem::Token(value.as_str().to_owned()),
    BareItem::ByteSequence(value) => SignatureInputBareItem::ByteSequence(value),
    BareItem::Date(value) => SignatureInputBareItem::Date(i64::from(value.unix_seconds())),
    BareItem::DisplayString(value) => SignatureInputBareItem::DisplayString(value),
  })
}

fn append_parameters(output: &mut String, parameters: &[SignatureInputParameter]) {
  for parameter in parameters {
    output.push(';');
    output.push_str(&parameter.name);
    match &parameter.value {
      SignatureInputBareItem::Boolean(true) => {}
      SignatureInputBareItem::Boolean(false) => output.push_str("=?0"),
      SignatureInputBareItem::Integer(value) => {
        output.push('=');
        output.push_str(&value.to_string());
      }
      SignatureInputBareItem::Decimal(value) => {
        output.push('=');
        output.push_str(value);
      }
      SignatureInputBareItem::String(value) => {
        output.push_str("=\"");
        output.push_str(&escape_sf_string(value));
        output.push('"');
      }
      SignatureInputBareItem::Token(value) => {
        output.push('=');
        output.push_str(value);
      }
      SignatureInputBareItem::ByteSequence(value) => {
        output.push_str("=:");
        output.push_str(&STANDARD.encode(value));
        output.push(':');
      }
      SignatureInputBareItem::Date(value) => {
        output.push_str("=@");
        output.push_str(&value.to_string());
      }
      SignatureInputBareItem::DisplayString(value) => {
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

fn invalid_member() -> SignatureInputParseError {
  SignatureInputParseError::new("invalid Signature-Input dictionary member")
}
