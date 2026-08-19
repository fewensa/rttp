//! Bounded, policy-free `Accept-Encoding` request metadata parsing.
//!
//! This module validates one or more RFC 9110 `Accept-Encoding` field values
//! as an ordered list of coding tokens with optional quality weights. Callers
//! decide whether and how to negotiate, compress, or decompress content.
//! Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

pub const MAX_ACCEPT_ENCODING_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_ENCODINGS: usize = 32;

/// Parsed, bounded `Accept-Encoding` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptEncoding {
  codings: Vec<AcceptEncodingCoding>,
}

/// One coding from a parsed `Accept-Encoding` list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptEncodingCoding {
  coding: String,
  quality: u16,
  quality_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptEncodingParseError {
  message: String,
}

impl AcceptEncodingParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptEncodingParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptEncodingParseError {}

impl AcceptEncoding {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptEncodingParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptEncodingParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut codings = Vec::new();
    for value in values {
      if value.len() > MAX_ACCEPT_ENCODING_VALUE_BYTES {
        return Err(AcceptEncodingParseError::new(
          "Accept-Encoding header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AcceptEncodingParseError::new(
          "invalid Accept-Encoding control byte",
        ));
      }
      for member in value.split(',') {
        let coding = parse_accept_encoding_member(member)?;
        if codings
          .iter()
          .any(|known: &AcceptEncodingCoding| known.coding.eq_ignore_ascii_case(&coding.coding))
        {
          return Err(AcceptEncodingParseError::new(
            "duplicate Accept-Encoding coding",
          ));
        }
        if codings.len() >= MAX_ACCEPT_ENCODINGS {
          return Err(AcceptEncodingParseError::new(
            "too many Accept-Encoding codings",
          ));
        }
        codings.push(coding);
      }
    }

    if codings.is_empty() {
      return Err(AcceptEncodingParseError::new(
        "invalid Accept-Encoding coding",
      ));
    }
    Ok(Self { codings })
  }

  pub fn from_codings<I, C>(codings: I) -> Result<Self, AcceptEncodingParseError>
  where
    I: IntoIterator<Item = C>,
    C: AsRef<str>,
  {
    let mut value = String::new();

    for (index, coding) in codings.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(coding.as_ref());
      if value.len() > MAX_ACCEPT_ENCODING_VALUE_BYTES {
        return Err(AcceptEncodingParseError::new(
          "Accept-Encoding header value is too large",
        ));
      }
    }

    Self::parse(value)
  }

  pub fn codings(&self) -> &[AcceptEncodingCoding] {
    &self.codings
  }

  pub fn len(&self) -> usize {
    self.codings.len()
  }

  pub fn is_empty(&self) -> bool {
    self.codings.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .codings
      .iter()
      .map(AcceptEncodingCoding::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AcceptEncodingCoding {
  pub fn coding(&self) -> &str {
    &self.coding
  }

  /// Returns the q-value as thousandths, where `1000` is the default quality
  /// of `1` and `0` means not acceptable.
  pub fn quality(&self) -> u16 {
    self.quality
  }

  pub fn is_wildcard(&self) -> bool {
    self.coding.eq_ignore_ascii_case("*")
  }

  fn header_value(&self) -> String {
    match &self.quality_text {
      Some(quality_text) => format!("{};q={quality_text}", self.coding),
      None => self.coding.clone(),
    }
  }
}

fn parse_accept_encoding_member(
  member: &str,
) -> Result<AcceptEncodingCoding, AcceptEncodingParseError> {
  let mut parts = member.split(';');
  let coding = trim_ows(parts.next().unwrap_or_default());
  if coding.is_empty() || !is_token(coding) {
    return Err(AcceptEncodingParseError::new(
      "invalid Accept-Encoding coding",
    ));
  }
  let Some(parameter) = parts.next() else {
    return Ok(AcceptEncodingCoding {
      coding: coding.to_owned(),
      quality: 1000,
      quality_text: None,
    });
  };
  if parts.next().is_some() {
    return Err(AcceptEncodingParseError::new(
      "invalid Accept-Encoding q-value",
    ));
  }
  let Some((name, qvalue)) = trim_ows(parameter).split_once('=') else {
    return Err(AcceptEncodingParseError::new(
      "invalid Accept-Encoding q-value",
    ));
  };
  if !trim_ows(name).eq_ignore_ascii_case("q") {
    return Err(AcceptEncodingParseError::new(
      "invalid Accept-Encoding q-value",
    ));
  }
  let qvalue = trim_ows(qvalue);
  Ok(AcceptEncodingCoding {
    coding: coding.to_owned(),
    quality: parse_accept_encoding_qvalue(qvalue)?,
    quality_text: Some(qvalue.to_owned()),
  })
}

fn parse_accept_encoding_qvalue(qvalue: &str) -> Result<u16, AcceptEncodingParseError> {
  let Some((whole, fraction)) = qvalue.split_once('.') else {
    return match qvalue {
      "0" => Ok(0),
      "1" => Ok(1000),
      _ => Err(AcceptEncodingParseError::new(
        "invalid Accept-Encoding q-value",
      )),
    };
  };
  if !matches!(whole, "0" | "1")
    || fraction.len() > 3
    || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    || (whole == "1" && !fraction.bytes().all(|byte| byte == b'0'))
  {
    return Err(AcceptEncodingParseError::new(
      "invalid Accept-Encoding q-value",
    ));
  }
  let fractional = if fraction.is_empty() {
    0
  } else {
    fraction
      .parse::<u16>()
      .map_err(|_| AcceptEncodingParseError::new("invalid Accept-Encoding q-value"))?
  };
  Ok(if whole == "1" {
    1000
  } else {
    fractional * 10_u16.pow(3 - fraction.len() as u32)
  })
}

fn trim_ows(value: &str) -> &str {
  value.trim_matches([' ', '\t'])
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}
