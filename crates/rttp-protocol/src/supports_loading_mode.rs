//! Bounded, policy-free `Supports-Loading-Mode` response metadata parsing.
//!
//! This module validates the response field value as a Structured Fields list
//! of tokens. It reports declared metadata only: callers decide whether and
//! how to apply prerender or fenced-frame loading behavior. This parser does
//! not prerender documents, admit fenced frames, change navigation, or alter
//! resource loading.

use std::error::Error;
use std::fmt;

use sfv::{BareItem, List, ListEntry, Parser};

/// Maximum bytes accepted in one `Supports-Loading-Mode` field value.
pub const MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES: usize = 64 * 1024;

/// Maximum tokens accepted across all combined `Supports-Loading-Mode` fields.
pub const MAX_SUPPORTS_LOADING_MODE_TOKENS: usize = 256;

/// Parsed, bounded `Supports-Loading-Mode` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportsLoadingMode {
  tokens: Vec<String>,
}

impl SupportsLoadingMode {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SupportsLoadingModeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SupportsLoadingModeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tokens = Vec::new();
    let mut total_bytes = 0usize;
    for value in values {
      if value.len() > MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES {
        return Err(SupportsLoadingModeParseError::new(
          "Supports-Loading-Mode header value is too large",
        ));
      }
      total_bytes += value.len();
      if total_bytes > MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES {
        return Err(SupportsLoadingModeParseError::new(
          "combined Supports-Loading-Mode header values are too large",
        ));
      }
      parse_field(value, &mut tokens)?;
    }
    if tokens.is_empty() {
      return Err(SupportsLoadingModeParseError::new(
        "Supports-Loading-Mode field must contain a token",
      ));
    }
    Ok(Self { tokens })
  }

  /// Builds `Supports-Loading-Mode` metadata from declared tokens, retaining
  /// each token with its given spelling.
  pub fn from_tokens<I, S>(tokens: I) -> Result<Self, SupportsLoadingModeParseError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let mut parsed = Vec::new();
    let mut total_bytes = 0usize;
    for token in tokens {
      let token = token.as_ref();
      if !is_structured_token(token) {
        return Err(invalid_token());
      }
      total_bytes += token.len();
      if total_bytes > MAX_SUPPORTS_LOADING_MODE_VALUE_BYTES {
        return Err(SupportsLoadingModeParseError::new(
          "Supports-Loading-Mode header value is too large",
        ));
      }
      if parsed.len() >= MAX_SUPPORTS_LOADING_MODE_TOKENS {
        return Err(SupportsLoadingModeParseError::new(
          "too many Supports-Loading-Mode tokens",
        ));
      }
      if parsed
        .iter()
        .any(|known: &String| known.eq_ignore_ascii_case(token))
      {
        return Err(SupportsLoadingModeParseError::new(
          "duplicate Supports-Loading-Mode token",
        ));
      }
      parsed.push(token.to_string());
    }
    if parsed.is_empty() {
      return Err(SupportsLoadingModeParseError::new(
        "Supports-Loading-Mode field must contain a token",
      ));
    }
    Ok(Self { tokens: parsed })
  }

  /// Returns the declared tokens in wire order with their wire spelling.
  pub fn tokens(&self) -> &[String] {
    &self.tokens
  }

  /// Whether a token is declared, comparing ASCII case-insensitively.
  pub fn contains(&self, token: impl AsRef<str>) -> bool {
    self
      .tokens
      .iter()
      .any(|known| known.eq_ignore_ascii_case(token.as_ref()))
  }

  /// Whether the exact `fenced-frame` token is declared.
  pub fn contains_fenced_frame(&self) -> bool {
    self.tokens.iter().any(|token| token == "fenced-frame")
  }

  /// Whether the exact `credentialed-prerender` token is declared.
  pub fn contains_credentialed_prerender(&self) -> bool {
    self
      .tokens
      .iter()
      .any(|token| token == "credentialed-prerender")
  }

  /// Whether the exact `prerender-cross-origin-frames` token is declared.
  pub fn contains_prerender_cross_origin_frames(&self) -> bool {
    self
      .tokens
      .iter()
      .any(|token| token == "prerender-cross-origin-frames")
  }

  pub fn header_value(&self) -> String {
    self.tokens.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportsLoadingModeParseError {
  message: String,
}

impl SupportsLoadingModeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SupportsLoadingModeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SupportsLoadingModeParseError {}

fn parse_field(value: &str, tokens: &mut Vec<String>) -> Result<(), SupportsLoadingModeParseError> {
  let list = Parser::new(value)
    .parse::<List>()
    .map_err(|_| invalid_token())?;
  for member in list {
    let ListEntry::Item(item) = member else {
      return Err(invalid_token());
    };
    if !item.params.is_empty() {
      return Err(invalid_token());
    }
    let BareItem::Token(token) = item.bare_item else {
      return Err(invalid_token());
    };
    if tokens.len() >= MAX_SUPPORTS_LOADING_MODE_TOKENS {
      return Err(SupportsLoadingModeParseError::new(
        "too many Supports-Loading-Mode tokens",
      ));
    }
    if tokens
      .iter()
      .any(|known: &String| known.eq_ignore_ascii_case(token.as_str()))
    {
      return Err(SupportsLoadingModeParseError::new(
        "duplicate Supports-Loading-Mode token",
      ));
    }
    tokens.push(token.as_str().to_owned());
  }
  Ok(())
}

fn is_structured_token(value: &str) -> bool {
  let mut bytes = value.bytes();
  matches!(bytes.next(), Some(b'*' | b'a'..=b'z' | b'A'..=b'Z'))
    && bytes.all(|byte| {
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
    })
}

fn invalid_token() -> SupportsLoadingModeParseError {
  SupportsLoadingModeParseError::new("invalid Supports-Loading-Mode list member")
}
