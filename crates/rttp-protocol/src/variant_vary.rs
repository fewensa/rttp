//! Bounded, policy-free RFC 2295 `Variant-Vary` response metadata parsing.
//!
//! This module validates the `Variant-Vary` response field as a wildcard or an
//! ordered list of HTTP field-name tokens. It exposes metadata only: callers
//! decide whether and how to apply negotiation, variant, or cache policy.

use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in one `Variant-Vary` field value.
pub const MAX_VARIANT_VARY_VALUE_BYTES: usize = 64 * 1024;
/// Maximum canonical serialized bytes accepted for a parsed `Variant-Vary` value.
pub const MAX_VARIANT_VARY_TOTAL_BYTES: usize = 64 * 1024;
/// Maximum field names accepted in the `Variant-Vary` field.
pub const MAX_VARIANT_VARY_FIELD_NAMES: usize = 256;

/// Parsed, bounded RFC 2295 `Variant-Vary` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantVary {
  any: bool,
  field_names: Vec<String>,
}

impl VariantVary {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, VariantVaryParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, VariantVaryParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut any = false;
    let mut field_names = Vec::new();

    for value in values {
      if value.len() > MAX_VARIANT_VARY_VALUE_BYTES {
        return Err(VariantVaryParseError::new(
          "Variant-Vary header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(VariantVaryParseError::new(
          "invalid Variant-Vary control byte",
        ));
      }
      for member in value.split(',') {
        let field_name = member.trim_matches([' ', '\t']);
        if field_name == "*" {
          if any {
            return Err(VariantVaryParseError::new(
              "duplicate Variant-Vary field name",
            ));
          }
          if !field_names.is_empty() {
            return Err(VariantVaryParseError::new(
              "invalid Variant-Vary field name",
            ));
          }
          any = true;
          continue;
        }
        if any || !is_http_token(field_name) {
          return Err(VariantVaryParseError::new(
            "invalid Variant-Vary field name",
          ));
        }
        let normalized = field_name.to_ascii_lowercase();
        if field_names.contains(&normalized) {
          return Err(VariantVaryParseError::new(
            "duplicate Variant-Vary field name",
          ));
        }
        if field_names.len() >= MAX_VARIANT_VARY_FIELD_NAMES {
          return Err(VariantVaryParseError::new(
            "too many Variant-Vary field names",
          ));
        }
        field_names.push(normalized);
      }
    }

    if !any && field_names.is_empty() {
      return Err(VariantVaryParseError::new(
        "invalid Variant-Vary field name",
      ));
    }
    let parsed = Self { any, field_names };
    if parsed.header_value().len() > MAX_VARIANT_VARY_TOTAL_BYTES {
      return Err(VariantVaryParseError::new(
        "Variant-Vary header list is too large",
      ));
    }
    Ok(parsed)
  }

  pub fn is_any(&self) -> bool {
    self.any
  }

  pub fn field_names(&self) -> Vec<&str> {
    self.field_names.iter().map(String::as_str).collect()
  }

  pub fn contains_field_name(&self, field_name: impl AsRef<str>) -> bool {
    let field_name = field_name.as_ref();
    if !is_http_token(field_name) {
      return false;
    }
    let field_name = field_name.to_ascii_lowercase();
    self.field_names.iter().any(|name| name == &field_name)
  }

  pub fn len(&self) -> usize {
    self.field_names.len()
  }

  pub fn is_empty(&self) -> bool {
    self.field_names.is_empty()
  }

  pub fn header_value(&self) -> String {
    if self.any {
      "*".to_owned()
    } else {
      self.field_names.join(", ")
    }
  }
}

/// An error returned when `Variant-Vary` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantVaryParseError {
  message: String,
}

impl VariantVaryParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for VariantVaryParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for VariantVaryParseError {}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_http_token_byte(byte: u8) -> bool {
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
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~'
    )
}
