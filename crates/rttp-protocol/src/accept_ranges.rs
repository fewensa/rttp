//! Bounded, policy-free `Accept-Ranges` response metadata parsing.
//!
//! This module validates one or more RFC 9110 `Accept-Ranges` field values
//! as an ordered list of range-unit tokens, preserving each unit's spelling
//! and wire order. The `none` sentinel is represented as an empty unit list.
//! Callers decide whether and how to serve range requests; this parser never
//! generates range requests or fails open.

use std::error::Error;
use std::fmt;

pub const MAX_ACCEPT_RANGES_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_RANGES_UNITS: usize = 256;

/// Parsed, bounded `Accept-Ranges` response metadata.
///
/// `none` is represented as an empty unit list so callers can distinguish an
/// explicit `Accept-Ranges: none` declaration from an absent header (which
/// facades report as `Ok(None)` before parsing).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptRanges {
  units: Vec<String>,
}

impl AcceptRanges {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptRangesParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptRangesParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut units: Vec<String> = Vec::new();
    let mut none_seen = false;

    for value in values {
      if value.len() > MAX_ACCEPT_RANGES_VALUE_BYTES {
        return Err(AcceptRangesParseError::new(
          "Accept-Ranges header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AcceptRangesParseError::new(
          "invalid Accept-Ranges control byte",
        ));
      }
      for member in value.split(',') {
        let unit = member.trim_matches([' ', '\t']);
        if unit.is_empty() || !is_http_token(unit) {
          return Err(AcceptRangesParseError::new(
            "invalid Accept-Ranges range unit",
          ));
        }
        if unit.eq_ignore_ascii_case("none") {
          if none_seen {
            return Err(AcceptRangesParseError::new(
              "duplicate Accept-Ranges range unit",
            ));
          }
          if !units.is_empty() {
            return Err(AcceptRangesParseError::new(
              "Accept-Ranges none cannot be combined with range units",
            ));
          }
          none_seen = true;
          continue;
        }
        if units
          .iter()
          .any(|known: &String| known.eq_ignore_ascii_case(unit))
        {
          return Err(AcceptRangesParseError::new(
            "duplicate Accept-Ranges range unit",
          ));
        }
        if units.len() >= MAX_ACCEPT_RANGES_UNITS {
          return Err(AcceptRangesParseError::new(
            "too many Accept-Ranges range units",
          ));
        }
        units.push(unit.to_owned());
      }
    }

    if none_seen {
      if !units.is_empty() {
        return Err(AcceptRangesParseError::new(
          "Accept-Ranges none cannot be combined with range units",
        ));
      }
      return Ok(Self { units: Vec::new() });
    }
    if units.is_empty() {
      return Err(AcceptRangesParseError::new(
        "invalid Accept-Ranges range unit",
      ));
    }

    Ok(Self { units })
  }

  /// Builds metadata from an ordered unit list, validating the resulting
  /// single header value with the same bounds as parsing.
  pub fn from_units<I, U>(units: I) -> Result<Self, AcceptRangesParseError>
  where
    I: IntoIterator<Item = U>,
    U: AsRef<str>,
  {
    let mut value = String::new();

    for (index, unit) in units.into_iter().enumerate() {
      if index >= MAX_ACCEPT_RANGES_UNITS {
        return Err(AcceptRangesParseError::new(
          "too many Accept-Ranges range units",
        ));
      }
      let unit = unit.as_ref();
      if unit.trim().eq_ignore_ascii_case("none") {
        return Err(AcceptRangesParseError::new(
          "Accept-Ranges none must use the none helper",
        ));
      }
      let separator_len = if index > 0 { 2 } else { 0 };
      if value.len() + separator_len + unit.len() > MAX_ACCEPT_RANGES_VALUE_BYTES {
        return Err(AcceptRangesParseError::new(
          "Accept-Ranges header value is too large",
        ));
      }
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(unit);
    }

    Self::parse(value)
  }

  pub fn none() -> Self {
    Self { units: Vec::new() }
  }

  pub fn units(&self) -> Vec<&str> {
    self.units.iter().map(String::as_str).collect()
  }

  pub fn is_none(&self) -> bool {
    self.units.is_empty()
  }

  pub fn accepts_bytes(&self) -> bool {
    self
      .units
      .iter()
      .any(|unit| unit.eq_ignore_ascii_case("bytes"))
  }

  pub fn header_value(&self) -> String {
    if self.units.is_empty() {
      "none".to_string()
    } else {
      self.units.join(", ")
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptRangesParseError {
  message: String,
}

impl AcceptRangesParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptRangesParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptRangesParseError {}

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
