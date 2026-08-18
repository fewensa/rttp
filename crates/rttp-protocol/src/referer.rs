//! Bounded, policy-free parsing for the HTTP `Referer` request header.
//!
//! This module validates one RFC 9110 URI reference (`absolute-URI` /
//! `partial-URI`). Surrounding SP and HTAB are trimmed as optional whitespace.
//! A successful parse stores that trimmed text and `header_value()` returns the
//! same bytes; the parser does not canonicalize scheme, host, port, path,
//! query, or userinfo.
//!
//! Each field is bounded to 64 KiB. ASCII controls other than HTAB, empty
//! values, duplicate fields, fragments, interior whitespace, non-URI bytes,
//! broken percent-encoding, and values the structural URL parser cannot accept
//! as an absolute or relative reference are errors. Exotic RFC 3986 forms that
//! the `url` crate rejects are treated as malformed.
//!
//! Parsing is syntax validation only. Callers retain responsibility for trust,
//! logging, CSRF, redaction, and `Referrer-Policy` decisions. Successful parse
//! of `javascript:`, `data:`, userinfo, or a relative path such as `null` is
//! not an application-safety judgment.
//!
//! # Examples
//!
//! ```
//! use rttp_protocol::referer::Referer;
//!
//! let absolute = Referer::parse("https://example.test/path?q=1").unwrap();
//! assert_eq!(absolute.header_value(), "https://example.test/path?q=1");
//!
//! let relative = Referer::parse("/relative").unwrap();
//! assert_eq!(relative.header_value(), "/relative");
//!
//! let scheme_relative = Referer::parse("//cdn.example/x.js").unwrap();
//! assert_eq!(scheme_relative.header_value(), "//cdn.example/x.js");
//! ```

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a `Referer` field value.
pub const MAX_REFERER_VALUE_BYTES: usize = 64 * 1024;

/// A parsed HTTP `Referer` field value.
///
/// The stored text is the OWS-trimmed URI reference from the wire.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Referer(String);

/// An error returned when `Referer` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefererParseError {
  message: String,
}

impl Referer {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, RefererParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, RefererParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    Ok(Self(value))
  }

  pub fn header_value(&self) -> String {
    self.0.clone()
  }
}

impl RefererParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for RefererParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for RefererParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<String, RefererParseError>
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
    return Err(RefererParseError::new("duplicate Referer header fields"));
  }

  let value = value.trim_matches([' ', '\t']);
  if value.is_empty() {
    return Err(invalid_value());
  }
  if !is_uri_reference_text(value) {
    return Err(invalid_value());
  }
  if value.contains('#') {
    return Err(invalid_value());
  }
  if !is_structural_uri_reference(value) {
    return Err(invalid_value());
  }
  Ok(value.to_string())
}

fn validate_value(value: &str) -> Result<(), RefererParseError> {
  if value.len() > MAX_REFERER_VALUE_BYTES {
    return Err(RefererParseError::new("Referer header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(RefererParseError::new(
      "invalid Referer header control byte",
    ));
  }
  Ok(())
}

fn is_uri_reference_text(value: &str) -> bool {
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
        | b'#'
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

fn is_structural_uri_reference(value: &str) -> bool {
  if Url::parse(value).is_ok() {
    return true;
  }
  let Ok(base) = Url::parse("https://rttp.invalid/") else {
    return false;
  };
  Url::options().base_url(Some(&base)).parse(value).is_ok()
}

fn invalid_value() -> RefererParseError {
  RefererParseError::new("invalid Referer header value")
}
