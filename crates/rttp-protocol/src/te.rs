//! Bounded, policy-free `TE` request metadata parsing.
//!
//! This module validates one or more RFC 9110 `TE` field values as an ordered
//! list of transfer codings with optional q-values. `chunked` is rejected
//! because request framing remains owned by the HTTP/1 implementation, and
//! `trailers` cannot carry a q-value. Callers decide whether and how to
//! interpret the codings. Unparsable input is an error; this parser never
//! fails open.

use std::error::Error;
use std::fmt;

pub const MAX_TE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_TE_CODINGS: usize = 32;

/// A validated `TE` transfer coding with an optional q-value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeCoding {
  coding: String,
  quality: Option<u16>,
}

impl TeCoding {
  pub fn coding(&self) -> &str {
    &self.coding
  }

  /// Returns the q-value as thousandths. `trailers` has no q-value.
  pub fn quality(&self) -> Option<u16> {
    self.quality
  }

  pub fn is_trailers(&self) -> bool {
    self.coding.eq_ignore_ascii_case("trailers")
  }
}

/// Bounded `TE` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Te {
  codings: Vec<TeCoding>,
}

impl Te {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut codings = Vec::new();

    for value in values {
      if value.len() > MAX_TE_VALUE_BYTES {
        return Err(TeParseError::new("TE header value is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(TeParseError::new("invalid TE control byte"));
      }
      for member in value.split(',') {
        let (coding, quality) = parse_te_member(member)?;
        if codings
          .iter()
          .any(|known: &TeCoding| known.coding.eq_ignore_ascii_case(coding))
        {
          return Err(TeParseError::new("duplicate TE coding"));
        }
        if codings.len() >= MAX_TE_CODINGS {
          return Err(TeParseError::new("too many TE codings"));
        }
        codings.push(TeCoding {
          coding: coding.to_string(),
          quality,
        });
      }
    }

    if codings.is_empty() {
      return Err(TeParseError::new("invalid TE coding"));
    }
    Ok(Self { codings })
  }

  pub fn codings(&self) -> &[TeCoding] {
    &self.codings
  }

  pub fn len(&self) -> usize {
    self.codings.len()
  }

  pub fn is_empty(&self) -> bool {
    self.codings.is_empty()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeParseError {
  message: String,
}

impl TeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TeParseError {}

fn parse_te_member(member: &str) -> Result<(&str, Option<u16>), TeParseError> {
  let mut parts = member.split(';');
  let coding = parts.next().unwrap_or_default().trim_matches([' ', '\t']);
  if coding.is_empty() || !is_http_token(coding) || coding.eq_ignore_ascii_case("chunked") {
    return Err(TeParseError::new("invalid TE coding"));
  }
  let Some(parameter) = parts.next() else {
    return Ok((
      coding,
      (!coding.eq_ignore_ascii_case("trailers")).then_some(1000),
    ));
  };
  if coding.eq_ignore_ascii_case("trailers") {
    return Err(TeParseError::new("TE trailers cannot carry a q-value"));
  }
  if parts.next().is_some() {
    return Err(TeParseError::new("invalid TE q-value"));
  }
  let Some((name, qvalue)) = parameter.trim_matches([' ', '\t']).split_once('=') else {
    return Err(TeParseError::new("invalid TE q-value"));
  };
  if !name.trim_matches([' ', '\t']).eq_ignore_ascii_case("q") {
    return Err(TeParseError::new("invalid TE q-value"));
  }
  Ok((
    coding,
    Some(parse_te_qvalue(qvalue.trim_matches([' ', '\t']))?),
  ))
}

fn parse_te_qvalue(qvalue: &str) -> Result<u16, TeParseError> {
  let Some((whole, fraction)) = qvalue.split_once('.') else {
    return match qvalue {
      "0" => Ok(0),
      "1" => Ok(1000),
      _ => Err(TeParseError::new("invalid TE q-value")),
    };
  };
  if !matches!(whole, "0" | "1")
    || fraction.len() > 3
    || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    || (whole == "1" && !fraction.bytes().all(|byte| byte == b'0'))
  {
    return Err(TeParseError::new("invalid TE q-value"));
  }
  let fractional = if fraction.is_empty() {
    0
  } else {
    fraction.parse::<u16>().expect("validated q-value digits")
  };
  Ok(if whole == "1" {
    1000
  } else {
    fractional * 10_u16.pow(3 - fraction.len() as u32)
  })
}

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
