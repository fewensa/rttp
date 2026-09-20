//! Bounded, policy-free RFC 9421 `Accept-Signature` field parse and format.
//!
//! This module validates the Structured Fields dictionary used to request
//! HTTP Message Signatures. Covered components and their parameters are kept
//! in wire order. The `created` and `expires` request parameters are retained
//! as valueless parameters, while the other registered request parameters are
//! type-checked without being interpreted. This module does not sign, verify,
//! look up keys, or make signature policy decisions.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::signature_input::{
  append_bare_item, SignatureInputBareItem, SignatureInputComponent, SignatureInputParameter,
};
use sfv::{BareItem, Dictionary, ListEntry, Parser};

/// Maximum bytes accepted in one `Accept-Signature` field value.
pub const MAX_ACCEPT_SIGNATURE_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative bytes accepted across all supplied field values.
pub const MAX_ACCEPT_SIGNATURE_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum signature requests accepted across all supplied field values.
pub const MAX_ACCEPT_SIGNATURE_ENTRIES: usize = 256;
/// Maximum covered components accepted on one signature request.
pub const MAX_ACCEPT_SIGNATURE_ENTRY_COMPONENTS: usize = 256;
/// Maximum request parameters accepted on one signature request.
pub const MAX_ACCEPT_SIGNATURE_ENTRY_PARAMETERS: usize = 256;
/// Maximum parameters accepted on one covered component.
pub const MAX_ACCEPT_SIGNATURE_COMPONENT_PARAMETERS: usize = 256;
/// Maximum bytes represented by one parameter value.
pub const MAX_ACCEPT_SIGNATURE_PARAMETER_VALUE_BYTES: usize = 64 * 1024;
/// Maximum bytes accepted in one signature-request label.
pub const MAX_ACCEPT_SIGNATURE_LABEL_BYTES: usize = 64 * 1024;
/// Maximum bytes accepted in one covered-component identifier.
pub const MAX_ACCEPT_SIGNATURE_COMPONENT_VALUE_BYTES: usize = 64 * 1024;
/// Maximum canonical bytes accepted in one signature request.
pub const MAX_ACCEPT_SIGNATURE_ENTRY_VALUE_BYTES: usize = 64 * 1024;

pub const MAX_ACCEPT_SIGNATURE_MEMBERS: usize = MAX_ACCEPT_SIGNATURE_ENTRIES;
pub const MAX_ACCEPT_SIGNATURE_COVERED_COMPONENTS: usize = MAX_ACCEPT_SIGNATURE_ENTRY_COMPONENTS;
pub const MAX_ACCEPT_SIGNATURE_PARAMETERS: usize = MAX_ACCEPT_SIGNATURE_ENTRY_PARAMETERS;

/// Parsed, bounded HTTP `Accept-Signature` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptSignature {
  entries: Vec<AcceptSignatureEntry>,
}

/// One labeled `Accept-Signature` signature request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptSignatureEntry {
  label: String,
  components: Vec<SignatureInputComponent>,
  parameters: Vec<AcceptSignatureParameter>,
}

/// One `Accept-Signature` request parameter.
///
/// A `None` value represents the Structured Fields valueless form. In
/// particular, `created` and `expires` are retained this way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptSignatureParameter {
  name: String,
  value: Option<SignatureInputBareItem>,
}

/// Signature-Input's component representation is safe to share here: both
/// fields carry an ordered string identifier and ordered Structured Fields
/// component parameters. Entry-level request parameters remain distinct so
/// valueless `created` and `expires` cannot be confused with `?1`.
pub type AcceptSignatureComponent = SignatureInputComponent;
pub type AcceptSignatureCoveredComponent = SignatureInputComponent;
pub type AcceptSignatureComponentParameter = SignatureInputParameter;
pub type AcceptSignatureBareItem = SignatureInputBareItem;
pub type AcceptSignatureParameterValue = SignatureInputBareItem;
pub type AcceptSignatureMember = AcceptSignatureEntry;
pub type AcceptSignatureDecimal = String;

/// An error returned when `Accept-Signature` metadata is malformed or exceeds
/// one of its bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptSignatureParseError {
  message: String,
}

impl AcceptSignatureParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptSignatureParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptSignatureParseError {}

impl AcceptSignature {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptSignatureParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptSignatureParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut entries = Vec::new();
    let mut total_bytes = 0usize;

    for value in values {
      validate_field(value)?;
      total_bytes = total_bytes
        .checked_add(value.len())
        .filter(|total| *total <= MAX_ACCEPT_SIGNATURE_TOTAL_BYTES)
        .ok_or_else(|| {
          AcceptSignatureParseError::new("Accept-Signature field values are too large")
        })?;
      parse_field(value, &mut entries)?;
    }

    if entries.is_empty() {
      return Err(AcceptSignatureParseError::new(
        "Accept-Signature field must contain an entry",
      ));
    }
    Ok(Self { entries })
  }

  pub fn entries(&self) -> &[AcceptSignatureEntry] {
    &self.entries
  }

  pub fn members(&self) -> &[AcceptSignatureMember] {
    &self.entries
  }

  pub fn entry(&self, label: impl AsRef<str>) -> Option<&AcceptSignatureEntry> {
    self
      .entries
      .iter()
      .find(|entry| entry.label == label.as_ref())
  }

  pub fn member(&self, label: impl AsRef<str>) -> Option<&AcceptSignatureMember> {
    self.entry(label)
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Returns the canonical Structured Fields serialization.
  pub fn header_value(&self) -> String {
    self
      .entries
      .iter()
      .map(AcceptSignatureEntry::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AcceptSignatureEntry {
  pub fn label(&self) -> &str {
    &self.label
  }

  pub fn components(&self) -> &[AcceptSignatureComponent] {
    &self.components
  }

  pub fn covered_components(&self) -> &[AcceptSignatureCoveredComponent] {
    &self.components
  }

  pub fn parameters(&self) -> &[AcceptSignatureParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&AcceptSignatureParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  pub fn created(&self) -> bool {
    self
      .parameter("created")
      .is_some_and(AcceptSignatureParameter::is_valueless)
  }

  pub fn expires(&self) -> bool {
    self
      .parameter("expires")
      .is_some_and(AcceptSignatureParameter::is_valueless)
  }

  pub fn requests_created(&self) -> bool {
    self.created()
  }

  pub fn requests_expires(&self) -> bool {
    self.expires()
  }

  pub fn nonce(&self) -> Option<&str> {
    self.string_parameter("nonce")
  }

  pub fn alg(&self) -> Option<&str> {
    self.string_parameter("alg")
  }

  pub fn algorithm(&self) -> Option<&str> {
    self.alg()
  }

  pub fn keyid(&self) -> Option<&str> {
    self.string_parameter("keyid")
  }

  pub fn key_id(&self) -> Option<&str> {
    self.keyid()
  }

  pub fn tag(&self) -> Option<&str> {
    self.string_parameter("tag")
  }

  fn string_parameter(&self, name: &str) -> Option<&str> {
    self
      .parameter(name)
      .and_then(|parameter| match parameter.value() {
        Some(SignatureInputBareItem::String(value)) => Some(value.as_str()),
        _ => None,
      })
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

impl AcceptSignatureParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&AcceptSignatureParameterValue> {
    self.value.as_ref()
  }

  pub fn is_valueless(&self) -> bool {
    self.value.is_none()
  }
}

#[derive(Clone, Debug)]
struct ScannedField {
  members: Vec<ScannedMember>,
}

#[derive(Clone, Debug)]
struct ScannedMember {
  label: String,
  component_parameters: Vec<Vec<ScannedParameter>>,
  parameters: Vec<ScannedParameter>,
}

#[derive(Clone, Debug)]
struct ScannedParameter {
  name: String,
  valueless: bool,
  value_bytes: usize,
}

fn parse_field(
  value: &str,
  entries: &mut Vec<AcceptSignatureEntry>,
) -> Result<(), AcceptSignatureParseError> {
  let dictionary = Parser::new(value)
    .parse::<Dictionary>()
    .map_err(|_| invalid_member())?;
  if dictionary.is_empty() {
    return Err(AcceptSignatureParseError::new(
      "Accept-Signature field must contain an entry",
    ));
  }

  let scanned = scan_field(value)?;
  if scanned.members.len() != dictionary.len() {
    return Err(duplicate_label());
  }
  if entries
    .len()
    .checked_add(scanned.members.len())
    .is_none_or(|count| count > MAX_ACCEPT_SIGNATURE_ENTRIES)
  {
    return Err(AcceptSignatureParseError::new(
      "too many Accept-Signature entries",
    ));
  }

  for scanned_member in scanned.members {
    if entries
      .iter()
      .any(|entry| entry.label == scanned_member.label)
    {
      return Err(duplicate_label());
    }

    let Some(ListEntry::InnerList(inner_list)) = dictionary.get(scanned_member.label.as_str())
    else {
      return Err(invalid_member());
    };
    if inner_list.items.len() > MAX_ACCEPT_SIGNATURE_ENTRY_COMPONENTS {
      return Err(AcceptSignatureParseError::new(
        "too many Accept-Signature entry components",
      ));
    }
    if inner_list.params.len() > MAX_ACCEPT_SIGNATURE_ENTRY_PARAMETERS {
      return Err(AcceptSignatureParseError::new(
        "too many Accept-Signature entry parameters",
      ));
    }
    if inner_list.items.len() != scanned_member.component_parameters.len()
      || inner_list.params.len() != scanned_member.parameters.len()
    {
      return Err(invalid_member());
    }

    let mut components = Vec::with_capacity(inner_list.items.len());
    for (item, scanned_parameters) in inner_list
      .items
      .iter()
      .zip(scanned_member.component_parameters.iter())
    {
      let BareItem::String(identifier) = &item.bare_item else {
        return Err(invalid_member());
      };
      if identifier.as_str().len() > MAX_ACCEPT_SIGNATURE_COMPONENT_VALUE_BYTES {
        return Err(AcceptSignatureParseError::new(
          "Accept-Signature component identifier is too large",
        ));
      }
      if item.params.len() > MAX_ACCEPT_SIGNATURE_COMPONENT_PARAMETERS {
        return Err(AcceptSignatureParseError::new(
          "too many Accept-Signature component parameters",
        ));
      }
      if item.params.len() != scanned_parameters.len() {
        return Err(invalid_member());
      }
      let parameters = convert_component_parameters(&item.params)?;
      components.push(SignatureInputComponent::from_parts(
        identifier.as_str().to_owned(),
        parameters,
      ));
    }

    let parameters = convert_entry_parameters(&inner_list.params, &scanned_member.parameters)?;
    let entry = AcceptSignatureEntry {
      label: scanned_member.label,
      components,
      parameters,
    };
    if entry.header_value().len() > MAX_ACCEPT_SIGNATURE_ENTRY_VALUE_BYTES {
      return Err(AcceptSignatureParseError::new(
        "Accept-Signature entry is too large",
      ));
    }
    entries.push(entry);
  }
  Ok(())
}

fn convert_component_parameters(
  parameters: &sfv::Parameters,
) -> Result<Vec<SignatureInputParameter>, AcceptSignatureParseError> {
  parameters
    .iter()
    .map(|(name, value)| {
      let value = convert_bare_item(value)?;
      validate_parameter_value(&value)?;
      Ok(SignatureInputParameter::from_parts(
        name.as_str().to_owned(),
        value,
      ))
    })
    .collect()
}

fn convert_entry_parameters(
  parameters: &sfv::Parameters,
  scanned_parameters: &[ScannedParameter],
) -> Result<Vec<AcceptSignatureParameter>, AcceptSignatureParseError> {
  parameters
    .iter()
    .map(|(name, value)| {
      let name = name.as_str();
      let scanned = scanned_parameters
        .iter()
        .find(|parameter| parameter.name == name)
        .ok_or_else(invalid_member)?;
      if scanned.value_bytes > MAX_ACCEPT_SIGNATURE_PARAMETER_VALUE_BYTES {
        return Err(AcceptSignatureParseError::new(
          "Accept-Signature parameter value is too large",
        ));
      }

      let value = if scanned.valueless {
        validate_named_parameter(name, None)?;
        None
      } else {
        let value = convert_bare_item(value)?;
        validate_parameter_value(&value)?;
        validate_named_parameter(name, Some(&value))?;
        Some(value)
      };
      Ok(AcceptSignatureParameter {
        name: name.to_owned(),
        value,
      })
    })
    .collect()
}

fn validate_named_parameter(
  name: &str,
  value: Option<&SignatureInputBareItem>,
) -> Result<(), AcceptSignatureParseError> {
  match name {
    "created" | "expires" => {
      if value.is_some() {
        return Err(AcceptSignatureParseError::new(
          "Accept-Signature created and expires parameters must be valueless",
        ));
      }
    }
    "nonce" | "alg" | "keyid" | "tag"
      if !matches!(value, Some(SignatureInputBareItem::String(_))) =>
    {
      return Err(AcceptSignatureParseError::new(
        "Accept-Signature registered parameters must be strings",
      ));
    }
    _ => {}
  }
  Ok(())
}

fn convert_bare_item(
  value: &BareItem,
) -> Result<SignatureInputBareItem, AcceptSignatureParseError> {
  Ok(match value {
    BareItem::Boolean(value) => SignatureInputBareItem::Boolean(*value),
    BareItem::Integer(value) => SignatureInputBareItem::Integer(i64::from(*value)),
    BareItem::Decimal(value) => SignatureInputBareItem::Decimal(value.to_string()),
    BareItem::String(value) => SignatureInputBareItem::String(value.as_str().to_owned()),
    BareItem::Token(value) => SignatureInputBareItem::Token(value.as_str().to_owned()),
    BareItem::ByteSequence(value) => SignatureInputBareItem::ByteSequence(value.clone()),
    BareItem::Date(value) => SignatureInputBareItem::Date(i64::from(value.unix_seconds())),
    BareItem::DisplayString(value) => SignatureInputBareItem::DisplayString(value.clone()),
  })
}

fn validate_parameter_value(
  value: &SignatureInputBareItem,
) -> Result<(), AcceptSignatureParseError> {
  if parameter_value_bytes(value) > MAX_ACCEPT_SIGNATURE_PARAMETER_VALUE_BYTES {
    return Err(AcceptSignatureParseError::new(
      "Accept-Signature parameter value is too large",
    ));
  }
  Ok(())
}

fn parameter_value_bytes(value: &SignatureInputBareItem) -> usize {
  match value {
    SignatureInputBareItem::Boolean(_) => 2,
    SignatureInputBareItem::Integer(value) => value.to_string().len(),
    SignatureInputBareItem::Decimal(value) => value.len(),
    SignatureInputBareItem::String(value) => value.len(),
    SignatureInputBareItem::Token(value) => value.len(),
    SignatureInputBareItem::ByteSequence(value) => value.len(),
    SignatureInputBareItem::Date(value) => value.to_string().len(),
    SignatureInputBareItem::DisplayString(value) => value.len(),
  }
}

fn append_parameters(output: &mut String, parameters: &[AcceptSignatureParameter]) {
  for parameter in parameters {
    output.push(';');
    output.push_str(&parameter.name);
    match &parameter.value {
      None | Some(SignatureInputBareItem::Boolean(true)) => {}
      Some(value) => {
        output.push('=');
        append_bare_item(output, value);
      }
    }
  }
}

fn validate_field(value: &str) -> Result<(), AcceptSignatureParseError> {
  if value.len() > MAX_ACCEPT_SIGNATURE_VALUE_BYTES {
    return Err(AcceptSignatureParseError::new(
      "Accept-Signature field value is too large",
    ));
  }
  if value.bytes().any(is_invalid_field_byte) {
    return Err(AcceptSignatureParseError::new(
      "Accept-Signature field value contains a non-ASCII or control byte",
    ));
  }
  Ok(())
}

fn is_invalid_field_byte(byte: u8) -> bool {
  byte >= 0x80 || (byte < 0x20 && byte != b'\t') || byte == 0x7f
}

fn scan_field(value: &str) -> Result<ScannedField, AcceptSignatureParseError> {
  let bytes = value.as_bytes();
  let mut cursor = Cursor { bytes, position: 0 };
  cursor.skip_ows();
  if cursor.is_at_end() {
    return Err(invalid_member());
  }

  let mut members = Vec::new();
  let mut labels = HashSet::new();
  loop {
    let label = cursor.parse_key()?;
    if label.len() > MAX_ACCEPT_SIGNATURE_LABEL_BYTES {
      return Err(AcceptSignatureParseError::new(
        "Accept-Signature label is too large",
      ));
    }
    if !labels.insert(label.clone()) {
      return Err(duplicate_label());
    }
    cursor.expect(b'=')?;
    let component_parameters = cursor.scan_inner_list()?;
    let parameters = cursor.scan_parameters()?;
    members.push(ScannedMember {
      label,
      component_parameters,
      parameters,
    });

    cursor.skip_ows();
    if cursor.is_at_end() {
      break;
    }
    cursor.expect(b',')?;
    cursor.skip_ows();
    if cursor.is_at_end() {
      return Err(invalid_member());
    }
  }
  Ok(ScannedField { members })
}

struct Cursor<'a> {
  bytes: &'a [u8],
  position: usize,
}

impl Cursor<'_> {
  fn is_at_end(&self) -> bool {
    self.position >= self.bytes.len()
  }

  fn peek(&self) -> Option<u8> {
    self.bytes.get(self.position).copied()
  }

  fn expect(&mut self, expected: u8) -> Result<(), AcceptSignatureParseError> {
    if self.peek() == Some(expected) {
      self.position += 1;
      Ok(())
    } else {
      Err(invalid_member())
    }
  }

  fn skip_ows(&mut self) {
    while matches!(self.peek(), Some(b' ' | b'\t')) {
      self.position += 1;
    }
  }

  fn skip_sp(&mut self) {
    while self.peek() == Some(b' ') {
      self.position += 1;
    }
  }

  fn parse_key(&mut self) -> Result<String, AcceptSignatureParseError> {
    let start = self.position;
    if !matches!(self.peek(), Some(b'a'..=b'z' | b'*')) {
      return Err(invalid_member());
    }
    self.position += 1;
    while matches!(
      self.peek(),
      Some(b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
    ) {
      self.position += 1;
    }
    String::from_utf8(self.bytes[start..self.position].to_vec()).map_err(|_| invalid_member())
  }

  fn scan_inner_list(&mut self) -> Result<Vec<Vec<ScannedParameter>>, AcceptSignatureParseError> {
    self.expect(b'(')?;
    self.skip_sp();
    if self.peek() == Some(b')') {
      self.position += 1;
      return Ok(Vec::new());
    }

    let mut components = Vec::new();
    loop {
      self.skip_bare_item()?;
      let parameters = self.scan_parameters()?;
      components.push(parameters);

      match self.peek() {
        Some(b')') => {
          self.position += 1;
          return Ok(components);
        }
        Some(b' ') => {
          self.skip_sp();
          if self.peek() == Some(b')') {
            self.position += 1;
            return Ok(components);
          }
        }
        _ => return Err(invalid_member()),
      }
    }
  }

  fn scan_parameters(&mut self) -> Result<Vec<ScannedParameter>, AcceptSignatureParseError> {
    let mut parameters = Vec::new();
    let mut names = HashSet::new();
    while self.peek() == Some(b';') {
      self.position += 1;
      self.skip_sp();
      let name = self.parse_key()?;
      if !names.insert(name.clone()) {
        return Err(AcceptSignatureParseError::new(
          "duplicate Accept-Signature parameter",
        ));
      }
      let (valueless, value_bytes) = if self.peek() == Some(b'=') {
        self.position += 1;
        let start = self.position;
        self.skip_bare_item()?;
        (false, self.position - start)
      } else {
        (true, 0)
      };
      parameters.push(ScannedParameter {
        name,
        valueless,
        value_bytes,
      });
    }
    Ok(parameters)
  }

  fn skip_bare_item(&mut self) -> Result<(), AcceptSignatureParseError> {
    match self.peek() {
      Some(b'?') => {
        self.position += 1;
        if matches!(self.peek(), Some(b'0' | b'1')) {
          self.position += 1;
          Ok(())
        } else {
          Err(invalid_member())
        }
      }
      Some(b':') => self.skip_byte_sequence(),
      Some(b'"') => self.skip_string(),
      Some(b'%') if self.bytes.get(self.position + 1) == Some(&b'"') => self.skip_display_string(),
      Some(b'@') => {
        self.position += 1;
        self.skip_number()
      }
      Some(b'-' | b'0'..=b'9') => self.skip_number(),
      Some(b'*' | b'A'..=b'Z' | b'a'..=b'z') => self.skip_token(),
      _ => Err(invalid_member()),
    }
  }

  fn skip_string(&mut self) -> Result<(), AcceptSignatureParseError> {
    self.expect(b'"')?;
    while let Some(byte) = self.peek() {
      match byte {
        b'\\' => {
          self.position += 1;
          if !matches!(self.peek(), Some(b'\\' | b'"')) {
            return Err(invalid_member());
          }
          self.position += 1;
        }
        b'"' => {
          self.position += 1;
          return Ok(());
        }
        _ => self.position += 1,
      }
    }
    Err(invalid_member())
  }

  fn skip_display_string(&mut self) -> Result<(), AcceptSignatureParseError> {
    self.position += 2;
    while let Some(byte) = self.peek() {
      match byte {
        b'%' => {
          self.position += 1;
          if !self.peek().is_some_and(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid_member());
          }
          self.position += 1;
          if !self.peek().is_some_and(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid_member());
          }
          self.position += 1;
        }
        b'"' => {
          self.position += 1;
          return Ok(());
        }
        _ => self.position += 1,
      }
    }
    Err(invalid_member())
  }

  fn skip_byte_sequence(&mut self) -> Result<(), AcceptSignatureParseError> {
    self.position += 1;
    while let Some(byte) = self.peek() {
      self.position += 1;
      if byte == b':' {
        return Ok(());
      }
    }
    Err(invalid_member())
  }

  fn skip_number(&mut self) -> Result<(), AcceptSignatureParseError> {
    if self.peek() == Some(b'-') {
      self.position += 1;
    }
    let start = self.position;
    while matches!(self.peek(), Some(b'0'..=b'9')) {
      self.position += 1;
    }
    if self.position == start {
      return Err(invalid_member());
    }
    if self.peek() == Some(b'.') {
      self.position += 1;
      let fraction_start = self.position;
      while matches!(self.peek(), Some(b'0'..=b'9')) {
        self.position += 1;
      }
      if self.position == fraction_start {
        return Err(invalid_member());
      }
    }
    Ok(())
  }

  fn skip_token(&mut self) -> Result<(), AcceptSignatureParseError> {
    let start = self.position;
    while matches!(
      self.peek(),
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
      self.position += 1;
    }
    if self.position == start {
      Err(invalid_member())
    } else {
      Ok(())
    }
  }
}

fn invalid_member() -> AcceptSignatureParseError {
  AcceptSignatureParseError::new("invalid Accept-Signature dictionary member")
}

fn duplicate_label() -> AcceptSignatureParseError {
  AcceptSignatureParseError::new("duplicate Accept-Signature dictionary key")
}
