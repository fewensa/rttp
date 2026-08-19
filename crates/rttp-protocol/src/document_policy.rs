//! Bounded, policy-free `Document-Policy` response metadata parsing.
//!
//! This module validates the response field value as a WICG Document Policy
//! Structured Fields dictionary. It reports declared metadata only: callers
//! decide whether and how to enforce configuration points, compare required
//! policies, or send reports. This parser does not block document loads,
//! disable browser features, or attach `Sec-Required-Document-Policy`.
//!
//! Directive names are Structured Fields keys: lowercase tokens or `*`.
//! Unknown well-formed names are retained as opaque metadata; names are not
//! looked up against a browser configuration-point list. A member value is
//! one Structured Fields item of boolean (including a bare `?1`), integer,
//! decimal, or token. Inner lists, strings, byte sequences, dates, and
//! display strings are rejected. A well-formed `report-to` parameter is
//! accepted as a token or a quoted string and retained on the directive;
//! any other parameter name is rejected. Duplicate directive names, duplicate
//! parameters, empty dictionaries, too many directives, oversized field
//! values, and oversized cumulative input are errors. Unparsable input is an
//! error; this parser never fails open to an empty policy.
//!
//! ```
//! use rttp_protocol::document_policy::DocumentPolicy;
//!
//! let policy = DocumentPolicy::parse(
//!   "oversized-images=2.0, unsized-media=?0, *;report-to=default",
//! )
//! .expect("valid Document-Policy");
//! assert_eq!(policy.len(), 3);
//! assert_eq!(policy.directive("*").unwrap().report_to(), Some("default"));
//! assert_eq!(
//!   policy.header_value(),
//!   "oversized-images=2.0, unsized-media=?0, *;report-to=default"
//! );
//! ```

use std::error::Error;
use std::fmt;

use sfv::{BareItem, Dictionary, ListEntry, Parser};

/// Maximum bytes accepted in one `Document-Policy` field value.
pub const MAX_DOCUMENT_POLICY_VALUE_BYTES: usize = 64 * 1024;
/// Maximum cumulative raw field-value bytes accepted across all supplied fields.
pub const MAX_DOCUMENT_POLICY_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum directive members accepted across the combined dictionary.
pub const MAX_DOCUMENT_POLICY_DIRECTIVES: usize = 256;

/// Parsed, bounded `Document-Policy` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPolicy {
  directives: Vec<DocumentPolicyDirective>,
}

/// One configuration-point directive from a `Document-Policy` dictionary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPolicyDirective {
  name: String,
  value: DocumentPolicyValue,
  report_to: Option<DocumentPolicyReportTo>,
}

/// The declared value of one `Document-Policy` directive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentPolicyValue {
  /// `?1` (or a bare member) for true, `?0` for false.
  Boolean(bool),
  /// A signed integer value.
  Integer(i64),
  /// A decimal value in canonical Structured Fields form.
  Decimal(String),
  /// A token value.
  Token(String),
}

/// The retained `report-to` parameter spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
enum DocumentPolicyReportTo {
  Token(String),
  String(String),
}

/// An error returned when `Document-Policy` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPolicyParseError {
  message: String,
}

impl DocumentPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DocumentPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DocumentPolicyParseError {}

impl DocumentPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DocumentPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DocumentPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_DOCUMENT_POLICY_VALUE_BYTES {
        return Err(DocumentPolicyParseError::new(
          "Document-Policy header value is too large",
        ));
      }
      total_bytes = total_bytes.saturating_add(value.len());
      if total_bytes > MAX_DOCUMENT_POLICY_TOTAL_BYTES {
        return Err(DocumentPolicyParseError::new(
          "Document-Policy dictionary is too large",
        ));
      }
      parse_field(value, &mut directives)?;
    }
    if directives.is_empty() {
      return Err(DocumentPolicyParseError::new(
        "Document-Policy field must contain a directive",
      ));
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[DocumentPolicyDirective] {
    &self.directives
  }

  pub fn directive(&self, name: impl AsRef<str>) -> Option<&DocumentPolicyDirective> {
    self
      .directives
      .iter()
      .find(|directive| directive.name == name.as_ref())
  }

  pub fn len(&self) -> usize {
    self.directives.len()
  }

  pub fn is_empty(&self) -> bool {
    self.directives.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .directives
      .iter()
      .map(DocumentPolicyDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl DocumentPolicyDirective {
  /// Returns the configuration-point name token with its wire spelling.
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &DocumentPolicyValue {
    &self.value
  }

  /// Returns the retained `report-to` endpoint name, if declared.
  pub fn report_to(&self) -> Option<&str> {
    self.report_to.as_ref().map(DocumentPolicyReportTo::name)
  }

  fn header_value(&self) -> String {
    let mut value = match &self.value {
      DocumentPolicyValue::Boolean(true) => self.name.clone(),
      DocumentPolicyValue::Boolean(false) => format!("{}=?0", self.name),
      DocumentPolicyValue::Integer(integer) => format!("{}={}", self.name, integer),
      DocumentPolicyValue::Decimal(decimal) => format!("{}={}", self.name, decimal),
      DocumentPolicyValue::Token(token) => format!("{}={}", self.name, token),
    };
    if let Some(report_to) = &self.report_to {
      match report_to {
        DocumentPolicyReportTo::Token(name) => {
          value.push_str(&format!(";report-to={name}"));
        }
        DocumentPolicyReportTo::String(name) => {
          value.push_str(&format!(";report-to=\"{}\"", escape_sf_string(name)));
        }
      }
    }
    value
  }
}

impl DocumentPolicyReportTo {
  fn name(&self) -> &str {
    match self {
      Self::Token(name) => name,
      Self::String(name) => name,
    }
  }
}

fn parse_field(
  value: &str,
  directives: &mut Vec<DocumentPolicyDirective>,
) -> Result<(), DocumentPolicyParseError> {
  let dictionary = Parser::new(value)
    .parse::<Dictionary>()
    .map_err(|_| invalid_member())?;
  if dictionary.is_empty() {
    return Err(DocumentPolicyParseError::new(
      "Document-Policy field must contain a directive",
    ));
  }
  if top_level_member_count(value) != dictionary.len() {
    return Err(DocumentPolicyParseError::new(
      "duplicate Document-Policy directive name",
    ));
  }
  reject_duplicate_parameters(value)?;

  for (key, member) in dictionary {
    let name = key.as_str().to_owned();
    if directives.iter().any(|directive| directive.name == name) {
      return Err(DocumentPolicyParseError::new(
        "duplicate Document-Policy directive name",
      ));
    }
    if directives.len() >= MAX_DOCUMENT_POLICY_DIRECTIVES {
      return Err(DocumentPolicyParseError::new(
        "too many Document-Policy directives",
      ));
    }
    let ListEntry::Item(item) = member else {
      return Err(invalid_member());
    };
    let value = parse_value(item.bare_item)?;
    let report_to = parse_report_to(&item.params)?;
    directives.push(DocumentPolicyDirective {
      name,
      value,
      report_to,
    });
  }
  Ok(())
}

fn parse_value(bare_item: BareItem) -> Result<DocumentPolicyValue, DocumentPolicyParseError> {
  match bare_item {
    BareItem::Boolean(value) => Ok(DocumentPolicyValue::Boolean(value)),
    BareItem::Integer(value) => Ok(DocumentPolicyValue::Integer(i64::from(value))),
    BareItem::Decimal(value) => Ok(DocumentPolicyValue::Decimal(value.to_string())),
    BareItem::Token(value) => Ok(DocumentPolicyValue::Token(value.as_str().to_owned())),
    _ => Err(invalid_member()),
  }
}

fn parse_report_to(
  params: &sfv::Parameters,
) -> Result<Option<DocumentPolicyReportTo>, DocumentPolicyParseError> {
  let mut report_to = None;
  for (name, value) in params {
    if name.as_str() != "report-to" {
      return Err(invalid_member());
    }
    report_to = Some(match value {
      BareItem::Token(token) => DocumentPolicyReportTo::Token(token.as_str().to_owned()),
      BareItem::String(string) => DocumentPolicyReportTo::String(string.as_str().to_owned()),
      _ => return Err(invalid_member()),
    });
  }
  Ok(report_to)
}

fn reject_duplicate_parameters(value: &str) -> Result<(), DocumentPolicyParseError> {
  let bytes = value.as_bytes();
  let mut index = 0usize;
  while index < bytes.len() {
    skip_until_unquoted(bytes, &mut index, |byte| byte == b';' || byte == b',');
    let mut seen = Vec::new();
    while index < bytes.len() && bytes[index] == b';' {
      index += 1;
      skip_sp(bytes, &mut index);
      let start = index;
      while index < bytes.len() && is_sf_key_char(bytes[index]) {
        index += 1;
      }
      if start == index {
        return Err(invalid_member());
      }
      let name = &value[start..index];
      if seen.contains(&name) {
        return Err(DocumentPolicyParseError::new(
          "duplicate Document-Policy parameter",
        ));
      }
      seen.push(name);
      skip_sp(bytes, &mut index);
      if index < bytes.len() && bytes[index] == b'=' {
        index += 1;
        skip_until_unquoted(bytes, &mut index, |byte| byte == b';' || byte == b',');
      }
    }
    if index < bytes.len() && bytes[index] == b',' {
      index += 1;
    }
  }
  Ok(())
}

fn skip_until_unquoted(bytes: &[u8], index: &mut usize, stop: impl Fn(u8) -> bool) {
  let mut quoted = false;
  let mut escaped = false;
  while *index < bytes.len() {
    let byte = bytes[*index];
    if quoted {
      if escaped {
        escaped = false;
      } else if byte == b'\\' {
        escaped = true;
      } else if byte == b'"' {
        quoted = false;
      }
      *index += 1;
      continue;
    }
    if byte == b'"' {
      quoted = true;
      *index += 1;
      continue;
    }
    if stop(byte) {
      return;
    }
    *index += 1;
  }
}

fn skip_sp(bytes: &[u8], index: &mut usize) {
  while bytes.get(*index) == Some(&b' ') {
    *index += 1;
  }
}

fn is_sf_key_char(byte: u8) -> bool {
  matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*')
}

fn top_level_member_count(value: &str) -> usize {
  let mut count = 0;
  let mut start = 0;
  let mut quoted = false;
  let mut escaped = false;
  for (index, byte) in value.bytes().enumerate() {
    if quoted {
      if escaped {
        escaped = false;
      } else if byte == b'\\' {
        escaped = true;
      } else if byte == b'"' {
        quoted = false;
      }
    } else if byte == b'"' {
      quoted = true;
    } else if byte == b',' {
      if !value[start..index].trim_matches([' ', '\t']).is_empty() {
        count += 1;
      }
      start = index + 1;
    }
  }
  if !value[start..].trim_matches([' ', '\t']).is_empty() {
    count += 1;
  }
  count
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

fn invalid_member() -> DocumentPolicyParseError {
  DocumentPolicyParseError::new("invalid Document-Policy dictionary member")
}
