//! Bounded, policy-free parsing for the HTTP `From` request header.
//!
//! This module validates one mailbox value in a conservative ASCII subset:
//! either a bare `addr-spec` (`local-part@domain`) or a single `name-addr`
//! (`Display Name <local-part@domain>`). Surrounding SP and HTAB are trimmed as
//! optional whitespace. For `name-addr`, display-name spacing is normalized to
//! one SP before the angle address; the address itself is otherwise preserved.
//!
//! Each field is bounded to 64 KiB. Empty values, duplicate fields, comments,
//! groups, route syntax, obsolete folding, encoded words, quoted local parts,
//! domain literals, comma-separated lists, non-ASCII bytes, controls other than
//! HTAB, and malformed local/domain syntax are errors.
//!
//! Parsing is syntax validation only. Callers retain responsibility for
//! identity, ownership, privacy, deliverability, authorization, DNS, and SMTP
//! decisions.
//!
//! # Examples
//!
//! ```
//! use rttp_protocol::from::From;
//!
//! let bare = From::parse("ops@example.test").unwrap();
//! assert_eq!(bare.header_value(), "ops@example.test");
//!
//! let named = From::parse("Ops Team <ops@example.test>").unwrap();
//! assert_eq!(named.header_value(), "Ops Team <ops@example.test>");
//! ```

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `From` field value.
pub const MAX_FROM_VALUE_BYTES: usize = 64 * 1024;

/// A parsed HTTP `From` field value.
///
/// The stored text is the OWS-trimmed and syntax-normalized mailbox value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct From {
  header_value: String,
  address: String,
  local_part: String,
  domain: String,
  display_name: Option<String>,
}

/// An error returned when `From` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FromParseError {
  message: String,
}

impl From {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, FromParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, FromParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_mailbox(value)
  }

  pub fn header_value(&self) -> String {
    self.header_value.clone()
  }

  pub fn address(&self) -> &str {
    &self.address
  }

  pub fn local_part(&self) -> &str {
    &self.local_part
  }

  pub fn domain(&self) -> &str {
    &self.domain
  }

  pub fn display_name(&self) -> Option<&str> {
    self.display_name.as_deref()
  }
}

impl FromParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for FromParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for FromParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, FromParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_value(value)?;
  }
  if has_duplicate {
    return Err(FromParseError::new("duplicate From header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  Ok(value)
}

fn validate_value(value: &str) -> Result<(), FromParseError> {
  if value.len() > MAX_FROM_VALUE_BYTES {
    return Err(FromParseError::new("From header value is too large"));
  }
  if !value.is_ascii() {
    return Err(FromParseError::new("invalid From header non-ASCII byte"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(FromParseError::new("invalid From header control byte"));
  }
  Ok(())
}

fn parse_mailbox(value: &str) -> Result<From, FromParseError> {
  reject_ambiguous_text(value)?;

  if value.contains(['<', '>']) {
    return parse_name_addr(value);
  }

  let (local_part, domain) = parse_addr_spec(value)?;
  let address = format!("{local_part}@{domain}");
  Ok(From {
    header_value: address.clone(),
    address,
    local_part,
    domain,
    display_name: None,
  })
}

fn reject_ambiguous_text(value: &str) -> Result<(), FromParseError> {
  if value.contains([',', ':', '(', ')', '\r', '\n']) {
    return Err(invalid_value());
  }
  if value.contains("=?") {
    return Err(invalid_value());
  }
  Ok(())
}

fn parse_name_addr(value: &str) -> Result<From, FromParseError> {
  if value.matches('<').count() != 1 || value.matches('>').count() != 1 {
    return Err(invalid_value());
  }
  let (display_name, rest) = value.split_once('<').ok_or_else(invalid_value)?;
  let address = rest.strip_suffix('>').ok_or_else(invalid_value)?;
  if display_name.is_empty() || address.is_empty() {
    return Err(invalid_value());
  }
  if !display_name.ends_with([' ', '\t']) || address.starts_with([' ', '\t']) {
    return Err(invalid_value());
  }

  let display_name = normalize_display_name(display_name.trim_matches([' ', '\t']))?;
  let (local_part, domain) = parse_addr_spec(address)?;
  let address = format!("{local_part}@{domain}");
  let header_value = format!("{display_name} <{address}>");

  Ok(From {
    header_value,
    address,
    local_part,
    domain,
    display_name: Some(display_name),
  })
}

fn normalize_display_name(value: &str) -> Result<String, FromParseError> {
  if value.is_empty() {
    return Err(invalid_value());
  }
  if value.starts_with('.') {
    return Err(invalid_value());
  }

  let mut words = Vec::new();
  for word in value.split([' ', '\t']) {
    if word.is_empty() {
      continue;
    }
    if !word.bytes().all(is_display_word_byte) {
      return Err(invalid_value());
    }
    words.push(word);
  }
  if words.is_empty() {
    return Err(invalid_value());
  }
  Ok(words.join(" "))
}

fn parse_addr_spec(value: &str) -> Result<(String, String), FromParseError> {
  if value.matches('@').count() != 1 {
    return Err(invalid_value());
  }
  let (local_part, domain) = value.split_once('@').ok_or_else(invalid_value)?;
  if !is_dot_atom(local_part) || !is_domain(domain) {
    return Err(invalid_value());
  }
  Ok((local_part.to_string(), domain.to_string()))
}

fn is_dot_atom(value: &str) -> bool {
  !value.is_empty()
    && !value.starts_with('.')
    && !value.ends_with('.')
    && value
      .split('.')
      .all(|part| !part.is_empty() && part.bytes().all(is_addr_atom_byte))
}

fn is_domain(value: &str) -> bool {
  !value.is_empty()
    && !value.starts_with('.')
    && !value.ends_with('.')
    && value.split('.').all(|label| {
      !label.is_empty()
        && label.len() <= 63
        && label.starts_with(|byte: char| byte.is_ascii_alphanumeric())
        && label.ends_with(|byte: char| byte.is_ascii_alphanumeric())
        && label
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn is_addr_atom_byte(byte: u8) -> bool {
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
        | b'/'
        | b'='
        | b'?'
        | b'^'
        | b'_'
        | b'`'
        | b'{'
        | b'|'
        | b'}'
        | b'~'
    )
}

fn is_display_word_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'\'')
}

fn invalid_value() -> FromParseError {
  FromParseError::new("invalid From header value")
}
