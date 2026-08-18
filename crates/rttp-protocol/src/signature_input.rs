//! Bounded, policy-free RFC 9421 `Signature-Input` metadata parsing.
//!
//! This module preserves signature labels, covered-component identifiers,
//! covered-component parameters, and signature parameters as syntax metadata.
//! It does not compute signatures, construct signature base strings, verify
//! message authenticity, select keys, or enforce algorithm policy.

use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub const MAX_SIGNATURE_INPUT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SIGNATURE_INPUT_MEMBERS: usize = 256;
pub const MAX_SIGNATURE_INPUT_COVERED_COMPONENTS: usize = 256;
pub const MAX_SIGNATURE_INPUT_PARAMETERS: usize = 256;
pub const MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS: usize = 256;
pub const MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded RFC 9421 `Signature-Input` metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInput {
  members: Vec<SignatureInputMember>,
}

/// A single labelled `Signature-Input` dictionary member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureInputMember {
  label: String,
  covered_components: Vec<SignatureCoveredComponent>,
  parameters: Vec<SignatureParameter>,
}

/// A covered component identifier and its ordered parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureCoveredComponent {
  identifier: String,
  parameters: Vec<SignatureParameter>,
}

/// A Structured Fields parameter on a covered component or signature member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureParameter {
  name: String,
  value: Option<SignatureParameterValue>,
}

/// A bounded Structured Fields bare item value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureParameterValue {
  Boolean(bool),
  Integer(i64),
  Decimal(SignatureDecimal),
  String(String),
  Token(String),
  ByteSequence(Vec<u8>),
}

/// A Structured Fields decimal value stored as an integer scaled by 1000.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureDecimal {
  scaled: i64,
}

/// An error returned when `Signature-Input` metadata is malformed or exceeds bounds.
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
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    let mut members = Vec::new();
    for value in values {
      if value.len() > MAX_SIGNATURE_INPUT_VALUE_BYTES {
        return Err(SignatureInputParseError::new(
          "Signature-Input header value is too large",
        ));
      }
      Parser::new(value, &mut members).parse_field()?;
    }
    if members.is_empty() {
      return Err(SignatureInputParseError::new(
        "Signature-Input must contain a member",
      ));
    }
    Ok(Self { members })
  }

  pub fn members(&self) -> &[SignatureInputMember] {
    &self.members
  }

  pub fn member(&self, label: impl AsRef<str>) -> Option<&SignatureInputMember> {
    self
      .members
      .iter()
      .find(|member| member.label == label.as_ref())
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
      .map(SignatureInputMember::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SignatureInputMember {
  pub fn label(&self) -> &str {
    &self.label
  }

  pub fn covered_components(&self) -> &[SignatureCoveredComponent] {
    &self.covered_components
  }

  pub fn parameters(&self) -> &[SignatureParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&SignatureParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  fn header_value(&self) -> String {
    let covered = self
      .covered_components
      .iter()
      .map(SignatureCoveredComponent::header_value)
      .collect::<Vec<_>>()
      .join(" ");
    format!(
      "{}=({}){}",
      self.label,
      covered,
      format_parameters(&self.parameters)
    )
  }
}

impl SignatureCoveredComponent {
  pub fn identifier(&self) -> &str {
    &self.identifier
  }

  pub fn parameters(&self) -> &[SignatureParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&SignatureParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name == name.as_ref())
  }

  fn header_value(&self) -> String {
    format!(
      "\"{}\"{}",
      escape_string(&self.identifier),
      format_parameters(&self.parameters)
    )
  }
}

impl SignatureParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&SignatureParameterValue> {
    self.value.as_ref()
  }

  pub fn is_valueless(&self) -> bool {
    self.value.is_none()
  }

  fn header_value(&self) -> String {
    match self.value.as_ref() {
      None | Some(SignatureParameterValue::Boolean(true)) => format!(";{}", self.name),
      Some(value) => format!(";{}={}", self.name, value.header_value()),
    }
  }
}

impl SignatureParameterValue {
  fn header_value(&self) -> String {
    match self {
      Self::Boolean(true) => "?1".to_string(),
      Self::Boolean(false) => "?0".to_string(),
      Self::Integer(value) => value.to_string(),
      Self::Decimal(value) => value.header_value(),
      Self::String(value) => format!("\"{}\"", escape_string(value)),
      Self::Token(value) => value.clone(),
      Self::ByteSequence(value) => format!(":{}:", STANDARD.encode(value)),
    }
  }
}

impl SignatureDecimal {
  pub fn from_scaled(scaled: i64) -> Self {
    Self { scaled }
  }

  pub fn scaled(self) -> i64 {
    self.scaled
  }

  fn header_value(self) -> String {
    if self.scaled == 0 {
      return "0.0".to_string();
    }
    let sign = if self.scaled < 0 { "-" } else { "" };
    let absolute = self.scaled.abs();
    let whole = absolute / 1000;
    let fraction = absolute % 1000;
    if fraction % 100 == 0 {
      format!("{sign}{whole}.{}", fraction / 100)
    } else if fraction % 10 == 0 {
      format!("{sign}{whole}.{:02}", fraction / 10)
    } else {
      format!("{sign}{whole}.{fraction:03}")
    }
  }
}

struct Parser<'a, 'm> {
  value: &'a str,
  position: usize,
  members: &'m mut Vec<SignatureInputMember>,
}

impl<'a, 'm> Parser<'a, 'm> {
  fn new(value: &'a str, members: &'m mut Vec<SignatureInputMember>) -> Self {
    Self {
      value,
      position: 0,
      members,
    }
  }

  fn parse_field(&mut self) -> Result<(), SignatureInputParseError> {
    self.skip_ows();
    if self.is_done() {
      return Err(SignatureInputParseError::new(
        "Signature-Input must contain a member",
      ));
    }
    loop {
      self.parse_member()?;
      self.skip_ows();
      if self.is_done() {
        return Ok(());
      }
      self.expect_byte(b',', "invalid Signature-Input dictionary separator")?;
      self.skip_ows();
      if self.is_done() {
        return Err(SignatureInputParseError::new(
          "invalid Signature-Input dictionary separator",
        ));
      }
    }
  }

  fn parse_member(&mut self) -> Result<(), SignatureInputParseError> {
    if self.members.len() >= MAX_SIGNATURE_INPUT_MEMBERS {
      return Err(SignatureInputParseError::new(
        "too many Signature-Input members",
      ));
    }
    let label = self.parse_key("invalid Signature-Input label")?;
    if self.members.iter().any(|member| member.label == label) {
      return Err(SignatureInputParseError::new(
        "duplicate Signature-Input label",
      ));
    }
    self.expect_byte(b'=', "Signature-Input member must be an inner list")?;
    let covered_components = self.parse_inner_list()?;
    let parameters = self.parse_parameters(MAX_SIGNATURE_INPUT_PARAMETERS)?;
    validate_signature_parameters(&parameters)?;
    self.members.push(SignatureInputMember {
      label,
      covered_components,
      parameters,
    });
    Ok(())
  }

  fn parse_inner_list(
    &mut self,
  ) -> Result<Vec<SignatureCoveredComponent>, SignatureInputParseError> {
    self.expect_byte(b'(', "Signature-Input member must be an inner list")?;
    self.skip_sp();
    let mut components = Vec::new();
    while self.peek() != Some(b')') {
      if components.len() >= MAX_SIGNATURE_INPUT_COVERED_COMPONENTS {
        return Err(SignatureInputParseError::new(
          "too many Signature-Input covered components",
        ));
      }
      let identifier = self.parse_string("Signature-Input covered component must be a string")?;
      let parameters = self.parse_parameters(MAX_SIGNATURE_INPUT_COMPONENT_PARAMETERS)?;
      components.push(SignatureCoveredComponent {
        identifier,
        parameters,
      });
      match self.peek() {
        Some(b')') => {}
        Some(b' ') => {
          self.skip_sp();
          if self.peek() == Some(b')') {
            break;
          }
        }
        _ => {
          return Err(SignatureInputParseError::new(
            "invalid Signature-Input covered component separator",
          ))
        }
      }
    }
    self.expect_byte(b')', "invalid Signature-Input inner list")?;
    Ok(components)
  }

  fn parse_parameters(
    &mut self,
    max_parameters: usize,
  ) -> Result<Vec<SignatureParameter>, SignatureInputParseError> {
    let mut parameters = Vec::new();
    while self.peek() == Some(b';') {
      self.position += 1;
      self.skip_sp();
      if parameters.len() >= max_parameters {
        return Err(SignatureInputParseError::new(
          "too many Signature-Input parameters",
        ));
      }
      let name = self.parse_key("invalid Signature-Input parameter name")?;
      if parameters
        .iter()
        .any(|parameter: &SignatureParameter| parameter.name == name)
      {
        return Err(SignatureInputParseError::new(
          "duplicate Signature-Input parameter",
        ));
      }
      let value = if self.peek() == Some(b'=') {
        self.position += 1;
        Some(self.parse_bare_item()?)
      } else {
        None
      };
      parameters.push(SignatureParameter { name, value });
    }
    Ok(parameters)
  }

  fn parse_bare_item(&mut self) -> Result<SignatureParameterValue, SignatureInputParseError> {
    match self.peek() {
      Some(b'?') => self.parse_boolean(),
      Some(b'"') => self
        .parse_string("invalid Signature-Input string parameter")
        .map(SignatureParameterValue::String),
      Some(b':') => self.parse_byte_sequence(),
      Some(b'-' | b'0'..=b'9') => self.parse_number(),
      Some(b'a'..=b'z' | b'A'..=b'Z' | b'*') => self.parse_token(),
      _ => Err(SignatureInputParseError::new(
        "invalid Signature-Input parameter value",
      )),
    }
  }

  fn parse_boolean(&mut self) -> Result<SignatureParameterValue, SignatureInputParseError> {
    if self.consume_bytes(b"?1") {
      Ok(SignatureParameterValue::Boolean(true))
    } else if self.consume_bytes(b"?0") {
      Ok(SignatureParameterValue::Boolean(false))
    } else {
      Err(SignatureInputParseError::new(
        "invalid Signature-Input boolean parameter",
      ))
    }
  }

  fn parse_number(&mut self) -> Result<SignatureParameterValue, SignatureInputParseError> {
    let start = self.position;
    if self.peek() == Some(b'-') {
      self.position += 1;
    }
    let digits_start = self.position;
    while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
      self.position += 1;
    }
    let whole_digits = self.position - digits_start;
    if whole_digits == 0 {
      return Err(SignatureInputParseError::new(
        "invalid Signature-Input numeric parameter",
      ));
    }
    if self.peek() == Some(b'.') {
      if whole_digits > 12 {
        return Err(SignatureInputParseError::new(
          "invalid Signature-Input decimal parameter",
        ));
      }
      self.position += 1;
      let fraction_start = self.position;
      while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
        self.position += 1;
      }
      let fraction_digits = self.position - fraction_start;
      if !(1..=3).contains(&fraction_digits) {
        return Err(SignatureInputParseError::new(
          "invalid Signature-Input decimal parameter",
        ));
      }
      let raw = &self.value[start..self.position];
      let scaled = parse_decimal_scaled(raw)?;
      Ok(SignatureParameterValue::Decimal(
        SignatureDecimal::from_scaled(scaled),
      ))
    } else {
      if whole_digits > 15 {
        return Err(SignatureInputParseError::new(
          "invalid Signature-Input integer parameter",
        ));
      }
      let value = self.value[start..self.position]
        .parse::<i64>()
        .map_err(|_| SignatureInputParseError::new("invalid Signature-Input integer parameter"))?;
      Ok(SignatureParameterValue::Integer(value))
    }
  }

  fn parse_string(&mut self, error: &str) -> Result<String, SignatureInputParseError> {
    self.expect_byte(b'"', error)?;
    let mut parsed = String::new();
    while let Some(byte) = self.peek() {
      self.position += 1;
      match byte {
        b'"' => {
          if parsed.len() > MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES {
            return Err(SignatureInputParseError::new(
              "Signature-Input parameter value is too large",
            ));
          }
          return Ok(parsed);
        }
        b'\\' => {
          let Some(escaped) = self.peek() else {
            return Err(SignatureInputParseError::new(error));
          };
          if !matches!(escaped, b'"' | b'\\') {
            return Err(SignatureInputParseError::new(error));
          }
          self.position += 1;
          parsed.push(escaped as char);
        }
        0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => parsed.push(byte as char),
        _ => return Err(SignatureInputParseError::new(error)),
      }
    }
    Err(SignatureInputParseError::new(error))
  }

  fn parse_token(&mut self) -> Result<SignatureParameterValue, SignatureInputParseError> {
    let start = self.position;
    self.position += 1;
    while self.peek().is_some_and(is_token_tail_byte) {
      self.position += 1;
    }
    Ok(SignatureParameterValue::Token(
      self.value[start..self.position].to_string(),
    ))
  }

  fn parse_byte_sequence(&mut self) -> Result<SignatureParameterValue, SignatureInputParseError> {
    self.position += 1;
    let start = self.position;
    while self.peek().is_some_and(is_base64_byte) {
      self.position += 1;
    }
    let encoded = &self.value[start..self.position];
    self.expect_byte(b':', "invalid Signature-Input byte sequence parameter")?;
    let value = STANDARD.decode(encoded).map_err(|_| {
      SignatureInputParseError::new("invalid Signature-Input byte sequence parameter")
    })?;
    if value.len() > MAX_SIGNATURE_INPUT_PARAMETER_VALUE_BYTES {
      return Err(SignatureInputParseError::new(
        "Signature-Input parameter value is too large",
      ));
    }
    Ok(SignatureParameterValue::ByteSequence(value))
  }

  fn parse_key(&mut self, error: &str) -> Result<String, SignatureInputParseError> {
    let start = self.position;
    if !matches!(self.peek(), Some(b'a'..=b'z' | b'*')) {
      return Err(SignatureInputParseError::new(error));
    }
    self.position += 1;
    while self.peek().is_some_and(is_key_tail_byte) {
      self.position += 1;
    }
    Ok(self.value[start..self.position].to_string())
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

  fn expect_byte(&mut self, byte: u8, error: &str) -> Result<(), SignatureInputParseError> {
    if self.peek() != Some(byte) {
      return Err(SignatureInputParseError::new(error));
    }
    self.position += 1;
    Ok(())
  }

  fn consume_bytes(&mut self, bytes: &[u8]) -> bool {
    if self.value.as_bytes()[self.position..].starts_with(bytes) {
      self.position += bytes.len();
      true
    } else {
      false
    }
  }

  fn peek(&self) -> Option<u8> {
    self.value.as_bytes().get(self.position).copied()
  }

  fn is_done(&self) -> bool {
    self.position == self.value.len()
  }
}

fn parse_decimal_scaled(value: &str) -> Result<i64, SignatureInputParseError> {
  let (negative, value) = value
    .strip_prefix('-')
    .map_or((false, value), |value| (true, value));
  let (whole, fraction) = value
    .split_once('.')
    .ok_or_else(|| SignatureInputParseError::new("invalid Signature-Input decimal parameter"))?;
  let whole = whole
    .parse::<i64>()
    .map_err(|_| SignatureInputParseError::new("invalid Signature-Input decimal parameter"))?;
  let mut fraction_value = fraction
    .parse::<i64>()
    .map_err(|_| SignatureInputParseError::new("invalid Signature-Input decimal parameter"))?;
  for _ in fraction.len()..3 {
    fraction_value *= 10;
  }
  let scaled = whole
    .checked_mul(1000)
    .and_then(|whole| whole.checked_add(fraction_value))
    .ok_or_else(|| SignatureInputParseError::new("invalid Signature-Input decimal parameter"))?;
  Ok(if negative { -scaled } else { scaled })
}

fn format_parameters(parameters: &[SignatureParameter]) -> String {
  parameters
    .iter()
    .map(SignatureParameter::header_value)
    .collect::<String>()
}

fn validate_signature_parameters(
  parameters: &[SignatureParameter],
) -> Result<(), SignatureInputParseError> {
  for parameter in parameters {
    let valid = match parameter.name.as_str() {
      "created" | "expires" => {
        matches!(parameter.value, Some(SignatureParameterValue::Integer(_)))
      }
      "nonce" | "alg" | "keyid" | "tag" => {
        matches!(parameter.value, Some(SignatureParameterValue::String(_)))
      }
      _ => true,
    };
    if !valid {
      return Err(SignatureInputParseError::new(
        "invalid Signature-Input signature parameter",
      ));
    }
  }
  Ok(())
}

fn escape_string(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn is_key_tail_byte(byte: u8) -> bool {
  matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
}

fn is_token_tail_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'!' | b'#'
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
      | b'a'..=b'z'
      | b'A'..=b'Z'
  )
}

fn is_base64_byte(byte: u8) -> bool {
  matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=')
}
