//! Bounded, policy-free `Cross-Origin-Opener-Policy-Report-Only` response
//! metadata parsing.
//!
//! This module validates the response field value only. Callers decide whether
//! and how to use the report-only metadata. Unparsable input is an error; this
//! parser never fails open to `unsafe-none`. It does not isolate browsing
//! contexts, validate `Reporting-Endpoints` members, or send reports.
//!
//! ```
//! use rttp_protocol::cross_origin_opener_policy::CrossOriginOpenerPolicy;
//! use rttp_protocol::cross_origin_opener_policy_report_only::CrossOriginOpenerPolicyReportOnly;
//!
//! let policy = CrossOriginOpenerPolicyReportOnly::parse(
//!   r#"same-origin; report-to="coop-reporting""#,
//! )
//! .expect("valid Cross-Origin-Opener-Policy-Report-Only");
//! assert_eq!(policy.policy(), CrossOriginOpenerPolicy::SameOrigin);
//! assert_eq!(policy.report_to(), Some("coop-reporting"));
//! assert_eq!(
//!   policy.header_value(),
//!   r#"same-origin; report-to="coop-reporting""#
//! );
//! ```

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sfv::{BareItem, Item, Parser};

use crate::cross_origin_opener_policy::CrossOriginOpenerPolicy;

pub const MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_PARAMETERS: usize = 256;
pub const MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// The report-only opener policy declared by
/// `Cross-Origin-Opener-Policy-Report-Only`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginOpenerPolicyReportOnly {
  policy: CrossOriginOpenerPolicy,
  parameters: Vec<CrossOriginOpenerPolicyReportOnlyParameter>,
}

/// One opaque Structured Fields parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginOpenerPolicyReportOnlyParameter {
  name: String,
  value: CrossOriginOpenerPolicyReportOnlyBareItem,
}

/// An uninterpreted Structured Fields parameter value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossOriginOpenerPolicyReportOnlyBareItem {
  Boolean(bool),
  Integer(i64),
  Decimal(String),
  String(String),
  Token(String),
  ByteSequence(Vec<u8>),
  Date(i64),
  DisplayString(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossOriginOpenerPolicyReportOnlyParseError {
  message: String,
}

impl CrossOriginOpenerPolicyReportOnlyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CrossOriginOpenerPolicyReportOnlyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CrossOriginOpenerPolicyReportOnlyParseError {}

impl CrossOriginOpenerPolicyReportOnly {
  pub fn parse(
    value: impl AsRef<str>,
  ) -> Result<Self, CrossOriginOpenerPolicyReportOnlyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CrossOriginOpenerPolicyReportOnlyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    reject_duplicate_parameters(value)?;
    let item = Parser::new(value)
      .parse::<Item>()
      .map_err(|_| invalid_value())?;
    let BareItem::Token(token) = item.bare_item else {
      return Err(invalid_value());
    };
    let policy =
      CrossOriginOpenerPolicy::from_directive_token(token.as_str()).ok_or_else(invalid_value)?;
    if item.params.len() > MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_PARAMETERS {
      return Err(CrossOriginOpenerPolicyReportOnlyParseError::new(
        "too many Cross-Origin-Opener-Policy-Report-Only parameters",
      ));
    }
    Ok(Self {
      policy,
      parameters: convert_parameters(item.params)?,
    })
  }

  pub fn policy(&self) -> CrossOriginOpenerPolicy {
    self.policy
  }

  pub fn report_to(&self) -> Option<&str> {
    self.parameters.iter().find_map(|parameter| {
      if parameter.name != "report-to" {
        return None;
      }
      match &parameter.value {
        CrossOriginOpenerPolicyReportOnlyBareItem::String(value)
        | CrossOriginOpenerPolicyReportOnlyBareItem::Token(value) => Some(value.as_str()),
        _ => None,
      }
    })
  }

  pub fn parameters(&self) -> &[CrossOriginOpenerPolicyReportOnlyParameter] {
    &self.parameters
  }

  pub fn header_value(&self) -> String {
    let mut value = self.policy.header_value().to_owned();
    append_parameters(&mut value, &self.parameters);
    value
  }
}

impl CrossOriginOpenerPolicyReportOnlyParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &CrossOriginOpenerPolicyReportOnlyBareItem {
    &self.value
  }
}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, CrossOriginOpenerPolicyReportOnlyParseError>
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
    return Err(CrossOriginOpenerPolicyReportOnlyParseError::new(
      "duplicate Cross-Origin-Opener-Policy-Report-Only header fields",
    ));
  }
  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  if value.len() > MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_VALUE_BYTES {
    return Err(CrossOriginOpenerPolicyReportOnlyParseError::new(
      "Cross-Origin-Opener-Policy-Report-Only header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(invalid_value());
  }
  Ok(())
}

fn convert_parameters(
  parameters: sfv::Parameters,
) -> Result<
  Vec<CrossOriginOpenerPolicyReportOnlyParameter>,
  CrossOriginOpenerPolicyReportOnlyParseError,
> {
  parameters
    .into_iter()
    .map(|(name, value)| {
      let parameter = CrossOriginOpenerPolicyReportOnlyParameter {
        name: name.as_str().to_owned(),
        value: convert_bare_item(value),
      };
      if parameter_value_bytes(&parameter.value)
        > MAX_CROSS_ORIGIN_OPENER_POLICY_REPORT_ONLY_PARAMETER_VALUE_BYTES
      {
        return Err(CrossOriginOpenerPolicyReportOnlyParseError::new(
          "Cross-Origin-Opener-Policy-Report-Only parameter value is too large",
        ));
      }
      Ok(parameter)
    })
    .collect()
}

fn convert_bare_item(value: BareItem) -> CrossOriginOpenerPolicyReportOnlyBareItem {
  match value {
    BareItem::Boolean(value) => CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(value),
    BareItem::Integer(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::Integer(i64::from(value))
    }
    BareItem::Decimal(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::Decimal(value.to_string())
    }
    BareItem::String(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::String(value.as_str().to_owned())
    }
    BareItem::Token(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::Token(value.as_str().to_owned())
    }
    BareItem::ByteSequence(value) => CrossOriginOpenerPolicyReportOnlyBareItem::ByteSequence(value),
    BareItem::Date(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::Date(i64::from(value.unix_seconds()))
    }
    BareItem::DisplayString(value) => {
      CrossOriginOpenerPolicyReportOnlyBareItem::DisplayString(value)
    }
  }
}

fn parameter_value_bytes(value: &CrossOriginOpenerPolicyReportOnlyBareItem) -> usize {
  match value {
    CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(_) => 2,
    CrossOriginOpenerPolicyReportOnlyBareItem::Integer(value) => value.to_string().len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::Decimal(value) => value.len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::String(value) => value.len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::Token(value) => value.len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::ByteSequence(value) => value.len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::Date(value) => value.to_string().len(),
    CrossOriginOpenerPolicyReportOnlyBareItem::DisplayString(value) => value.len(),
  }
}

fn reject_duplicate_parameters(
  value: &str,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  skip_token(bytes, &mut position)?;
  let mut seen = HashSet::new();
  while bytes.get(position) == Some(&b';') {
    position += 1;
    skip_sp(bytes, &mut position);
    let name = parse_key(value, &mut position)?;
    if !seen.insert(name) {
      return Err(CrossOriginOpenerPolicyReportOnlyParseError::new(
        "duplicate Cross-Origin-Opener-Policy-Report-Only parameter",
      ));
    }
    if bytes.get(position) == Some(&b'=') {
      position += 1;
      skip_bare_item(bytes, &mut position)?;
    }
  }
  skip_ows(bytes, &mut position);
  if position != bytes.len() {
    return Err(invalid_value());
  }
  Ok(())
}

fn parse_key<'a>(
  value: &'a str,
  position: &mut usize,
) -> Result<&'a str, CrossOriginOpenerPolicyReportOnlyParseError> {
  let start = *position;
  skip_key(value.as_bytes(), position)?;
  Ok(&value[start..*position])
}

fn skip_key(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  if !matches!(bytes.get(*position), Some(b'a'..=b'z' | b'*')) {
    return Err(invalid_value());
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

fn skip_token(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
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
    Err(invalid_value())
  } else {
    Ok(())
  }
}

fn skip_bare_item(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  match bytes.get(*position) {
    Some(b'?') => {
      if matches!(bytes.get(*position + 1), Some(b'0' | b'1')) {
        *position += 2;
        Ok(())
      } else {
        Err(invalid_value())
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
    _ => Err(invalid_value()),
  }
}

fn skip_string(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  *position += 1;
  while *position < bytes.len() {
    match bytes[*position] {
      b'\\' => {
        *position += 1;
        if *position >= bytes.len() {
          return Err(invalid_value());
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
  Err(invalid_value())
}

fn skip_display_string(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
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
  Err(invalid_value())
}

fn skip_byte_sequence(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  *position += 1;
  while *position < bytes.len() {
    if bytes[*position] == b':' {
      *position += 1;
      return Ok(());
    }
    *position += 1;
  }
  Err(invalid_value())
}

fn skip_number(
  bytes: &[u8],
  position: &mut usize,
) -> Result<(), CrossOriginOpenerPolicyReportOnlyParseError> {
  if bytes.get(*position) == Some(&b'-') {
    *position += 1;
  }
  let start = *position;
  while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
    *position += 1;
  }
  if *position == start {
    return Err(invalid_value());
  }
  if bytes.get(*position) == Some(&b'.') {
    *position += 1;
    let fraction_start = *position;
    while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      *position += 1;
    }
    if *position == fraction_start {
      return Err(invalid_value());
    }
  }
  Ok(())
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

fn append_parameters(
  output: &mut String,
  parameters: &[CrossOriginOpenerPolicyReportOnlyParameter],
) {
  for parameter in parameters {
    output.push_str("; ");
    output.push_str(&parameter.name);
    match &parameter.value {
      CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(true) => {}
      CrossOriginOpenerPolicyReportOnlyBareItem::Boolean(false) => output.push_str("=?0"),
      CrossOriginOpenerPolicyReportOnlyBareItem::Integer(value) => {
        output.push('=');
        output.push_str(&value.to_string());
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::Decimal(value) => {
        output.push('=');
        output.push_str(value);
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::String(value) => {
        output.push_str("=\"");
        output.push_str(&escape_sf_string(value));
        output.push('"');
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::Token(value) => {
        output.push('=');
        output.push_str(value);
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::ByteSequence(value) => {
        output.push_str("=:");
        output.push_str(&STANDARD.encode(value));
        output.push(':');
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::Date(value) => {
        output.push_str("=@");
        output.push_str(&value.to_string());
      }
      CrossOriginOpenerPolicyReportOnlyBareItem::DisplayString(value) => {
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

fn invalid_value() -> CrossOriginOpenerPolicyReportOnlyParseError {
  CrossOriginOpenerPolicyReportOnlyParseError::new(
    "invalid Cross-Origin-Opener-Policy-Report-Only header value",
  )
}
