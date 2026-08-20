//! Bounded, policy-free `Accept-Charset` request metadata parsing.
//!
//! This module validates one or more RFC 9110 `Accept-Charset` field values
//! as an ordered list of charset-range tokens with optional quality weights.
//! Callers decide whether and how to negotiate, transcode, or select a
//! representation. Unparsable input is an error; this parser never fails open.

use std::error::Error;
use std::fmt;

use crate::http1::is_token;

pub const MAX_ACCEPT_CHARSET_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_ACCEPT_CHARSETS: usize = 32;

/// Parsed, bounded `Accept-Charset` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptCharset {
  ranges: Vec<AcceptCharsetRange>,
}

/// One charset range from a parsed `Accept-Charset` list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptCharsetRange {
  charset: String,
  quality: u16,
  quality_text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptCharsetParseError {
  message: String,
}

impl AcceptCharsetParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AcceptCharsetParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AcceptCharsetParseError {}

impl AcceptCharset {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptCharsetParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptCharsetParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut ranges = Vec::new();
    for value in values {
      if value.len() > MAX_ACCEPT_CHARSET_VALUE_BYTES {
        return Err(AcceptCharsetParseError::new(
          "Accept-Charset header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(AcceptCharsetParseError::new(
          "invalid Accept-Charset control byte",
        ));
      }
      for member in value.split(',') {
        let range = parse_accept_charset_member(member)?;
        if ranges
          .iter()
          .any(|known: &AcceptCharsetRange| known.charset.eq_ignore_ascii_case(&range.charset))
        {
          return Err(AcceptCharsetParseError::new(
            "duplicate Accept-Charset range",
          ));
        }
        if ranges.len() >= MAX_ACCEPT_CHARSETS {
          return Err(AcceptCharsetParseError::new(
            "too many Accept-Charset ranges",
          ));
        }
        ranges.push(range);
      }
    }

    if ranges.is_empty() {
      return Err(AcceptCharsetParseError::new("invalid Accept-Charset range"));
    }
    Ok(Self { ranges })
  }

  pub fn from_charsets<I, C>(charsets: I) -> Result<Self, AcceptCharsetParseError>
  where
    I: IntoIterator<Item = C>,
    C: AsRef<str>,
  {
    let mut value = String::new();

    for (index, charset) in charsets.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(charset.as_ref());
      if value.len() > MAX_ACCEPT_CHARSET_VALUE_BYTES {
        return Err(AcceptCharsetParseError::new(
          "Accept-Charset header value is too large",
        ));
      }
    }

    Self::parse(value)
  }

  pub fn charsets(&self) -> &[AcceptCharsetRange] {
    &self.ranges
  }

  pub fn ranges(&self) -> &[AcceptCharsetRange] {
    &self.ranges
  }

  pub fn len(&self) -> usize {
    self.ranges.len()
  }

  pub fn is_empty(&self) -> bool {
    self.ranges.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .ranges
      .iter()
      .map(AcceptCharsetRange::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl AcceptCharsetRange {
  pub fn charset(&self) -> &str {
    &self.charset
  }

  /// Returns the q-value as thousandths, where `1000` is the default quality
  /// of `1` and `0` means not acceptable.
  pub fn quality(&self) -> u16 {
    self.quality
  }

  pub fn is_wildcard(&self) -> bool {
    self.charset.eq_ignore_ascii_case("*")
  }

  fn header_value(&self) -> String {
    match &self.quality_text {
      Some(quality_text) => format!("{};q={quality_text}", self.charset),
      None => self.charset.clone(),
    }
  }
}

fn parse_accept_charset_member(
  member: &str,
) -> Result<AcceptCharsetRange, AcceptCharsetParseError> {
  let mut parts = member.split(';');
  let charset = trim_ows(parts.next().unwrap_or_default());
  if charset.is_empty() || !is_token(charset) {
    return Err(AcceptCharsetParseError::new("invalid Accept-Charset range"));
  }
  let Some(parameter) = parts.next() else {
    return Ok(AcceptCharsetRange {
      charset: charset.to_owned(),
      quality: 1000,
      quality_text: None,
    });
  };
  if parts.next().is_some() {
    return Err(AcceptCharsetParseError::new(
      "invalid Accept-Charset q-value",
    ));
  }
  let Some((name, qvalue)) = trim_ows(parameter).split_once('=') else {
    return Err(AcceptCharsetParseError::new(
      "invalid Accept-Charset q-value",
    ));
  };
  if !trim_ows(name).eq_ignore_ascii_case("q") {
    return Err(AcceptCharsetParseError::new(
      "invalid Accept-Charset q-value",
    ));
  }
  let qvalue = trim_ows(qvalue);
  Ok(AcceptCharsetRange {
    charset: charset.to_owned(),
    quality: parse_accept_charset_qvalue(qvalue)?,
    quality_text: Some(qvalue.to_owned()),
  })
}

fn parse_accept_charset_qvalue(qvalue: &str) -> Result<u16, AcceptCharsetParseError> {
  let Some((whole, fraction)) = qvalue.split_once('.') else {
    return match qvalue {
      "0" => Ok(0),
      "1" => Ok(1000),
      _ => Err(AcceptCharsetParseError::new(
        "invalid Accept-Charset q-value",
      )),
    };
  };
  if !matches!(whole, "0" | "1")
    || fraction.len() > 3
    || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    || (whole == "1" && !fraction.bytes().all(|byte| byte == b'0'))
  {
    return Err(AcceptCharsetParseError::new(
      "invalid Accept-Charset q-value",
    ));
  }
  let fractional = if fraction.is_empty() {
    0
  } else {
    fraction
      .parse::<u16>()
      .map_err(|_| AcceptCharsetParseError::new("invalid Accept-Charset q-value"))?
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
