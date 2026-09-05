//! Bounded, policy-free RFC 9110 `User-Agent` request metadata parsing.
//!
//! This module validates one `User-Agent` field value as an ordered sequence
//! of products and comments. It reports the declared metadata only; callers
//! retain responsibility for fingerprinting, product policy, defaults, and
//! application behavior.

use std::error::Error;
use std::fmt;

use crate::http1::{is_quoted_pair_char, is_token_byte};

/// Maximum bytes accepted in one `User-Agent` field value.
pub const MAX_USER_AGENT_VALUE_BYTES: usize = 64 * 1024;
/// Maximum product or comment members accepted in a `User-Agent` value.
pub const MAX_USER_AGENT_MEMBERS: usize = 256;
/// Maximum nesting depth accepted in a parenthesized `User-Agent` comment.
pub const MAX_USER_AGENT_COMMENT_DEPTH: usize = 128;

/// Parsed, bounded RFC 9110 `User-Agent` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct UserAgent {
  members: Vec<UserAgentMember>,
}

/// One ordered product or comment member from a `User-Agent` value.
///
/// A product member has a product token and may have a version token. A
/// comment member has the comment contents without its surrounding
/// parentheses. The fields are private so callers can only obtain validated,
/// read-only metadata through the accessors below.
#[derive(Clone, Eq, PartialEq)]
pub struct UserAgentMember {
  product: Option<String>,
  version: Option<String>,
  comment: Option<String>,
}

/// An error returned when `User-Agent` metadata is malformed or exceeds a
/// protocol bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserAgentParseError {
  message: String,
}

impl UserAgent {
  /// Parses one `User-Agent` field value.
  pub fn parse(value: impl AsRef<str>) -> Result<Self, UserAgentParseError> {
    Self::parse_values([value.as_ref()])
  }

  /// Parses the singleton `User-Agent` field.
  ///
  /// Exactly one field value is accepted. Repeated field lines are rejected
  /// rather than combined because `User-Agent` is a singleton field.
  pub fn parse_values<'a, I>(values: I) -> Result<Self, UserAgentParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    let mut members = Vec::new();
    parse_field(value, &mut members)?;
    Ok(Self { members })
  }

  /// Returns the validated product and comment members in wire order.
  pub fn members(&self) -> &[UserAgentMember] {
    &self.members
  }

  /// Returns the number of product and comment members.
  pub fn len(&self) -> usize {
    self.members.len()
  }

  /// Returns whether this value contains no members.
  pub fn is_empty(&self) -> bool {
    self.members.is_empty()
  }

  /// Serializes the metadata with canonical single-space member separators.
  ///
  /// Product and version token spelling, as well as comment contents and
  /// quoted-pair spelling, are retained from the accepted field value.
  pub fn header_value(&self) -> String {
    self
      .members
      .iter()
      .map(UserAgentMember::header_value)
      .collect::<Vec<_>>()
      .join(" ")
  }
}

impl fmt::Debug for UserAgent {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("UserAgent")
      .field("member_count", &self.members.len())
      .finish()
  }
}

impl UserAgentMember {
  /// Returns the product token for a product member.
  pub fn product(&self) -> Option<&str> {
    self.product.as_deref()
  }

  /// Returns the product-version token for a product member that has one.
  pub fn version(&self) -> Option<&str> {
    self.version.as_deref()
  }

  /// Returns the comment contents, without surrounding parentheses, for a
  /// comment member.
  pub fn comment(&self) -> Option<&str> {
    self.comment.as_deref()
  }

  /// Returns whether this member is a product.
  pub fn is_product(&self) -> bool {
    self.product.is_some()
  }

  /// Returns whether this member is a comment.
  pub fn is_comment(&self) -> bool {
    self.comment.is_some()
  }

  fn from_product(product: String, version: Option<String>) -> Self {
    Self {
      product: Some(product),
      version,
      comment: None,
    }
  }

  fn from_comment(comment: String) -> Self {
    Self {
      product: None,
      version: None,
      comment: Some(comment),
    }
  }

  fn header_value(&self) -> String {
    if let Some(product) = &self.product {
      let mut value = product.clone();
      if let Some(version) = &self.version {
        value.push('/');
        value.push_str(version);
      }
      value
    } else {
      let comment = self
        .comment
        .as_deref()
        .expect("UserAgentMember must contain a product or comment");
      format!("({comment})")
    }
  }
}

impl fmt::Debug for UserAgentMember {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("UserAgentMember")
      .field(
        "kind",
        &if self.is_product() {
          "product"
        } else {
          "comment"
        },
      )
      .field("has_version", &self.version.is_some())
      .finish()
  }
}

impl UserAgentParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for UserAgentParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for UserAgentParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, UserAgentParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut first = None;
  let mut duplicate = false;

  for value in values {
    validate_bounded_value(value)?;
    if first.is_some() {
      duplicate = true;
    } else {
      first = Some(value);
    }
  }

  let value = first.ok_or_else(invalid_value)?;
  if duplicate {
    return Err(UserAgentParseError::new(
      "duplicate User-Agent header fields",
    ));
  }
  Ok(value)
}

fn validate_bounded_value(value: &str) -> Result<(), UserAgentParseError> {
  if value.len() > MAX_USER_AGENT_VALUE_BYTES {
    return Err(UserAgentParseError::new(
      "User-Agent header value is too large",
    ));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(UserAgentParseError::new("invalid User-Agent control byte"));
  }
  Ok(())
}

fn parse_field(value: &str, members: &mut Vec<UserAgentMember>) -> Result<(), UserAgentParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(invalid_value());
  }

  let product = parse_product(value, &mut position)?;
  members.push(product);

  loop {
    let separator = skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if separator == 0 {
      return Err(invalid_value());
    }
    if members.len() >= MAX_USER_AGENT_MEMBERS {
      return Err(UserAgentParseError::new("too many User-Agent members"));
    }

    let member = if bytes[position] == b'(' {
      UserAgentMember::from_comment(parse_comment(value, &mut position)?)
    } else {
      parse_product(value, &mut position)?
    };
    members.push(member);
  }
}

fn parse_product(
  value: &str,
  position: &mut usize,
) -> Result<UserAgentMember, UserAgentParseError> {
  let product = parse_token(value, position).ok_or_else(invalid_product)?;
  let version = if value.as_bytes().get(*position) == Some(&b'/') {
    *position += 1;
    Some(
      parse_token(value, position)
        .ok_or_else(invalid_product)?
        .to_string(),
    )
  } else {
    None
  };
  Ok(UserAgentMember::from_product(product.to_string(), version))
}

fn parse_comment(value: &str, position: &mut usize) -> Result<String, UserAgentParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) != Some(&b'(') {
    return Err(invalid_comment());
  }

  *position += 1;
  let inner_start = *position;
  let mut depth = 1usize;

  while let Some(&byte) = bytes.get(*position) {
    match byte {
      b'(' => {
        if depth >= MAX_USER_AGENT_COMMENT_DEPTH {
          return Err(invalid_comment());
        }
        depth += 1;
        *position += 1;
      }
      b')' => {
        *position += 1;
        depth -= 1;
        if depth == 0 {
          return Ok(value[inner_start..*position - 1].to_string());
        }
      }
      b'\\' => {
        *position += 1;
        let Some(&escaped) = bytes.get(*position) else {
          return Err(invalid_comment());
        };
        if !is_quoted_pair_char(escaped) {
          return Err(invalid_comment());
        }
        *position += 1;
      }
      byte if is_comment_text_byte(byte) => *position += 1,
      _ => return Err(invalid_comment()),
    }
  }

  Err(invalid_comment())
}

fn parse_token<'a>(value: &'a str, position: &mut usize) -> Option<&'a str> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  (start != *position).then(|| &value[start..*position])
}

fn skip_ows(bytes: &[u8], position: &mut usize) -> usize {
  let start = *position;
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
  *position - start
}

fn is_comment_text_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'\t' | b' ' | 0x21..=0x27 | 0x2a..=0x5b | 0x5d..=0x7e
  ) || byte >= 0x80
}

fn invalid_value() -> UserAgentParseError {
  UserAgentParseError::new("invalid User-Agent value")
}

fn invalid_product() -> UserAgentParseError {
  UserAgentParseError::new("invalid User-Agent product")
}

fn invalid_comment() -> UserAgentParseError {
  UserAgentParseError::new("invalid User-Agent comment")
}
