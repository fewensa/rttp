//! Bounded, policy-free parsing for HTTP `Alternates` response metadata.
//!
//! This module parses RFC 2295-style variant descriptions into ordered
//! [`Alternates`] metadata. Each variant retains its quoted URI-reference,
//! source quality text, and ordered attributes. URIs are validated
//! structurally as RFC 3986 URI-references and stored as raw text; they are
//! never resolved, fetched, ranked, or selected.
//!
//! Each field value is bounded to [`MAX_ALTERNATES_VALUE_BYTES`], the
//! combined field bytes are bounded to [`MAX_ALTERNATES_AGGREGATE_VALUE_BYTES`],
//! the variant count is bounded to [`MAX_ALTERNATES_VARIANTS`], each variant
//! holds at most [`MAX_ALTERNATES_ATTRIBUTES`] attributes, and each quoted URI
//! or attribute value is bounded to 64 KiB. Attribute names are matched
//! case-insensitively, stored lowercase, and must be unique within a variant.
//! Duplicate variants are rejected by exact stored URI, quality text, and
//! normalized attributes.
//!
//! Parsing is syntax validation only: this module does not implement
//! transparent content negotiation, variant selection, request replay,
//! redirects, automatic retrieval, cache storage, `Vary` matching, or
//! quality ranking.
//!
//! # Examples
//!
//! ```
//! use rttp_protocol::alternates::Alternates;
//!
//! let alternates = Alternates::parse(
//!   r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }, { "/resource.fr.html" 0.8 {type "text/html; charset=utf-8"} {language fr} }"#,
//! )
//! .unwrap();
//! assert_eq!("/resource.en.html", alternates.variants()[0].uri());
//! assert_eq!("1.0", alternates.variants()[0].quality());
//! assert_eq!(
//!   Some("text/html"),
//!   alternates.variants()[0].attribute("type")
//! );
//! assert_eq!(Some("fr"), alternates.variants()[1].attribute("language"));
//! ```

use std::error::Error;
use std::fmt;

use url::Url;

/// Maximum bytes accepted in a single `Alternates` field value.
pub const MAX_ALTERNATES_VALUE_BYTES: usize = 64 * 1024;

/// Maximum bytes accepted across all `Alternates` field values.
pub const MAX_ALTERNATES_AGGREGATE_VALUE_BYTES: usize = 64 * 1024;

/// Maximum variant members accepted across all field values.
pub const MAX_ALTERNATES_VARIANTS: usize = 256;

/// Maximum attributes retained on a single variant.
pub const MAX_ALTERNATES_ATTRIBUTES: usize = 256;

/// Maximum bytes accepted in a quoted URI after unescape.
pub const MAX_ALTERNATES_URI_BYTES: usize = 64 * 1024;

/// Maximum bytes accepted in a single attribute value after unescape.
pub const MAX_ALTERNATES_ATTRIBUTE_VALUE_BYTES: usize = 64 * 1024;

/// Bounded `Alternates` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternates {
  variants: Vec<AlternateVariant>,
}

/// A single parsed variant description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlternateVariant {
  uri: String,
  quality: String,
  attributes: Vec<AlternateAttribute>,
}

/// A single parsed variant attribute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlternateAttribute {
  name: String,
  value: String,
}

/// An error returned when `Alternates` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlternatesParseError {
  message: String,
}

impl Alternates {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AlternatesParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AlternatesParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut variants = Vec::new();
    let mut aggregate_len = 0usize;
    let mut seen_field = false;

    for value in values {
      if value.len() > MAX_ALTERNATES_VALUE_BYTES {
        return Err(AlternatesParseError::new(
          "Alternates header value is too large",
        ));
      }
      aggregate_len = aggregate_len.checked_add(value.len()).ok_or_else(|| {
        AlternatesParseError::new("Alternates header aggregate value is too large")
      })?;
      if aggregate_len > MAX_ALTERNATES_AGGREGATE_VALUE_BYTES {
        return Err(AlternatesParseError::new(
          "Alternates header aggregate value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AlternatesParseError::new("invalid Alternates control byte"));
      }
      seen_field = true;
      parse_field(value, &mut variants)?;
    }

    if !seen_field || variants.is_empty() {
      return Err(AlternatesParseError::new("invalid Alternates entry"));
    }

    Ok(Self { variants })
  }

  pub fn variants(&self) -> &[AlternateVariant] {
    &self.variants
  }

  pub fn len(&self) -> usize {
    self.variants.len()
  }

  pub fn is_empty(&self) -> bool {
    self.variants.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .variants
      .iter()
      .map(AlternateVariant::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AlternateVariant {
  pub fn uri(&self) -> &str {
    &self.uri
  }

  pub fn quality(&self) -> &str {
    &self.quality
  }

  pub fn attributes(&self) -> &[AlternateAttribute] {
    &self.attributes
  }

  pub fn attribute(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .attributes
      .iter()
      .find(|attribute| attribute.name.eq_ignore_ascii_case(name.as_ref()))
      .map(AlternateAttribute::value)
  }

  fn header_value(&self) -> String {
    let mut value = format!("{{ \"{}\" {}", escape_quoted(&self.uri), self.quality);
    for attribute in &self.attributes {
      value.push(' ');
      value.push_str(&attribute.header_value());
    }
    value.push_str(" }");
    value
  }

  fn same_stored_variant(&self, other: &Self) -> bool {
    self.uri == other.uri && self.quality == other.quality && self.attributes == other.attributes
  }
}

impl AlternateAttribute {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  fn header_value(&self) -> String {
    if is_token(&self.value) || (self.name == "type" && is_media_type(&self.value)) {
      format!("{{{} {}}}", self.name, self.value)
    } else {
      format!("{{{} \"{}\"}}", self.name, escape_quoted(&self.value))
    }
  }
}

impl AlternatesParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AlternatesParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AlternatesParseError {}

fn parse_field(
  value: &str,
  variants: &mut Vec<AlternateVariant>,
) -> Result<(), AlternatesParseError> {
  let mut position = 0usize;
  skip_ows(value.as_bytes(), &mut position);
  if position == value.len() {
    return Err(AlternatesParseError::new("invalid Alternates entry"));
  }

  loop {
    if variants.len() >= MAX_ALTERNATES_VARIANTS {
      return Err(AlternatesParseError::new("too many Alternates variants"));
    }
    let variant = parse_variant(value, &mut position)?;
    if variants
      .iter()
      .any(|known| known.same_stored_variant(&variant))
    {
      return Err(AlternatesParseError::new("duplicate Alternates variant"));
    }
    variants.push(variant);
    skip_ows(value.as_bytes(), &mut position);
    if position == value.len() {
      return Ok(());
    }
    if value.as_bytes()[position] != b',' {
      return Err(AlternatesParseError::new("invalid Alternates entry"));
    }
    position += 1;
    skip_ows(value.as_bytes(), &mut position);
    if position == value.len() {
      return Err(AlternatesParseError::new("invalid Alternates entry"));
    }
  }
}

fn parse_variant(
  value: &str,
  position: &mut usize,
) -> Result<AlternateVariant, AlternatesParseError> {
  skip_ows(value.as_bytes(), position);
  expect_byte(value, position, b'{', "invalid Alternates entry")?;
  skip_ows(value.as_bytes(), position);
  let uri = parse_quoted_string(value, position)?;
  if uri.len() > MAX_ALTERNATES_URI_BYTES {
    return Err(AlternatesParseError::new("Alternates URI is too large"));
  }
  validate_uri(&uri)?;
  skip_ows(value.as_bytes(), position);
  let quality = parse_qvalue(value, position)?;
  skip_ows(value.as_bytes(), position);

  let mut attributes = Vec::new();
  while value.as_bytes().get(*position) == Some(&b'{') {
    if attributes.len() >= MAX_ALTERNATES_ATTRIBUTES {
      return Err(AlternatesParseError::new("too many Alternates attributes"));
    }
    let attribute = parse_attribute(value, position)?;
    if attributes
      .iter()
      .any(|known: &AlternateAttribute| known.name == attribute.name)
    {
      return Err(AlternatesParseError::new("duplicate Alternates attribute"));
    }
    attributes.push(attribute);
    skip_ows(value.as_bytes(), position);
  }

  expect_byte(value, position, b'}', "invalid Alternates entry")?;
  Ok(AlternateVariant {
    uri,
    quality,
    attributes,
  })
}

fn parse_attribute(
  value: &str,
  position: &mut usize,
) -> Result<AlternateAttribute, AlternatesParseError> {
  expect_byte(value, position, b'{', "invalid Alternates attribute")?;
  skip_ows(value.as_bytes(), position);
  let name =
    parse_token(value, position, "invalid Alternates attribute name")?.to_ascii_lowercase();
  skip_ows(value.as_bytes(), position);
  let parsed_value = if name == "length" {
    if value.as_bytes().get(*position) == Some(&b'"') {
      return Err(AlternatesParseError::new("invalid Alternates length"));
    }
    parse_length_digits(value, position)?
  } else if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)?
  } else if name == "type" {
    parse_media_type_or_token(value, position)?
  } else {
    parse_token(value, position, "invalid Alternates attribute value")?.to_string()
  };
  if parsed_value.len() > MAX_ALTERNATES_ATTRIBUTE_VALUE_BYTES {
    return Err(AlternatesParseError::new(
      "Alternates attribute value is too large",
    ));
  }
  skip_ows(value.as_bytes(), position);
  expect_byte(value, position, b'}', "invalid Alternates attribute")?;
  Ok(AlternateAttribute {
    name,
    value: parsed_value,
  })
}

fn parse_qvalue(value: &str, position: &mut usize) -> Result<String, AlternatesParseError> {
  let start = *position;
  let Some(&whole) = value.as_bytes().get(*position) else {
    return Err(AlternatesParseError::new("invalid Alternates qvalue"));
  };
  if whole != b'0' && whole != b'1' {
    return Err(AlternatesParseError::new("invalid Alternates qvalue"));
  }
  *position += 1;
  if value.as_bytes().get(*position) == Some(&b'.') {
    *position += 1;
    let fraction_start = *position;
    while *position < value.len() && value.as_bytes()[*position].is_ascii_digit() {
      *position += 1;
    }
    let fraction = &value[fraction_start..*position];
    if fraction.len() > 3 || (whole == b'1' && !fraction.bytes().all(|byte| byte == b'0')) {
      return Err(AlternatesParseError::new("invalid Alternates qvalue"));
    }
  }
  Ok(value[start..*position].to_string())
}

fn parse_length_digits(value: &str, position: &mut usize) -> Result<String, AlternatesParseError> {
  let start = *position;
  while *position < value.len() && value.as_bytes()[*position].is_ascii_digit() {
    *position += 1;
  }
  if *position == start {
    return Err(AlternatesParseError::new("invalid Alternates length"));
  }
  let digits = &value[start..*position];
  if digits.parse::<u64>().is_err() {
    return Err(AlternatesParseError::new("invalid Alternates length"));
  }
  Ok(digits.to_string())
}

fn parse_media_type_or_token(
  value: &str,
  position: &mut usize,
) -> Result<String, AlternatesParseError> {
  let first = parse_token(value, position, "invalid Alternates type")?;
  if value.as_bytes().get(*position) != Some(&b'/') {
    return Ok(first.to_string());
  }
  *position += 1;
  let second = parse_token(value, position, "invalid Alternates type")?;
  Ok(format!("{first}/{second}"))
}

fn parse_quoted_string(value: &str, position: &mut usize) -> Result<String, AlternatesParseError> {
  if value.as_bytes().get(*position) != Some(&b'"') {
    return Err(AlternatesParseError::new(
      "invalid Alternates quoted-string",
    ));
  }
  *position += 1;
  let mut parsed = String::new();
  let mut unescaped_start = *position;
  let mut escaped = false;
  while let Some(&byte) = value.as_bytes().get(*position) {
    if escaped {
      if !(byte == b'\t' || (0x20..=0x7e).contains(&byte) || byte >= 0x80) {
        return Err(AlternatesParseError::new(
          "invalid Alternates quoted-string",
        ));
      }
      if byte >= 0x80 {
        let Some(character) = value[*position..].chars().next() else {
          return Err(AlternatesParseError::new(
            "invalid Alternates quoted-string",
          ));
        };
        parsed.push(character);
        *position += character.len_utf8();
      } else {
        parsed.push(byte as char);
        *position += 1;
      }
      escaped = false;
      unescaped_start = *position;
    } else if byte == b'\\' {
      parsed.push_str(&value[unescaped_start..*position]);
      *position += 1;
      escaped = true;
    } else if byte == b'"' {
      parsed.push_str(&value[unescaped_start..*position]);
      *position += 1;
      return Ok(parsed);
    } else if byte == b'\t' || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
    {
      *position += 1;
    } else {
      return Err(AlternatesParseError::new(
        "invalid Alternates quoted-string",
      ));
    }
  }
  Err(AlternatesParseError::new(
    "invalid Alternates quoted-string",
  ))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, AlternatesParseError> {
  let start = *position;
  while *position < value.len() && is_token_byte(value.as_bytes()[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(AlternatesParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn validate_uri(uri: &str) -> Result<(), AlternatesParseError> {
  if uri.is_empty()
    || uri.bytes().any(|byte| !is_uri_reference_byte(byte))
    || !has_valid_percent_escapes(uri)
  {
    return Err(AlternatesParseError::new("invalid Alternates URI"));
  }
  let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
  Url::options()
    .base_url(Some(&base))
    .parse(uri)
    .map_err(|_| AlternatesParseError::new("invalid Alternates URI"))?;
  Ok(())
}

fn expect_byte(
  value: &str,
  position: &mut usize,
  expected: u8,
  message: &str,
) -> Result<(), AlternatesParseError> {
  if value.as_bytes().get(*position) == Some(&expected) {
    *position += 1;
    Ok(())
  } else {
    Err(AlternatesParseError::new(message))
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t')) {
    *position += 1;
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
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
      | b'^'
      | b'_'
      | b'`'
      | b'|'
      | b'~'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

fn is_media_type(value: &str) -> bool {
  let Some((type_name, subtype)) = value.split_once('/') else {
    return false;
  };
  is_token(type_name) && is_token(subtype) && !subtype.contains('/')
}

fn is_uri_reference_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'%'
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
      | b'-'
      | b'.'
      | b'_'
      | b'~'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

fn has_valid_percent_escapes(target: &str) -> bool {
  let mut bytes = target.bytes();
  while let Some(byte) = bytes.next() {
    if byte != b'%' {
      continue;
    }
    let Some(high) = bytes.next() else {
      return false;
    };
    let Some(low) = bytes.next() else {
      return false;
    };
    if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
      return false;
    }
  }
  true
}

fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}
