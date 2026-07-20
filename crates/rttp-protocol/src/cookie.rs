//! Bounded, policy-free cookie metadata parsing.
//!
//! This module intentionally keeps cookie values and attributes opaque. It
//! does not implement a cookie jar, domain/path matching, expiry, or any other
//! storage and sending policy.

use crate::http1::is_token;
use std::error::Error;
use std::fmt;

/// Maximum number of parsed request cookies or response `Set-Cookie` fields.
pub const MAX_COOKIE_COUNT: usize = 256;
/// Maximum number of attributes retained for one `Set-Cookie` field.
pub const MAX_SET_COOKIE_ATTRIBUTES: usize = 64;
/// Maximum byte length of an individual cookie or attribute value.
pub const MAX_COOKIE_VALUE_BYTES: usize = 4 * 1024;
const MAX_COOKIE_FIELD_BYTES: usize = 64 * 1024;

/// One name/value pair from a request `Cookie` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpCookiePair {
  name: String,
  value: String,
}

impl HttpCookiePair {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

/// Parsed request `Cookie` metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpCookies {
  pairs: Vec<HttpCookiePair>,
}

impl HttpCookies {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpCookieParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut pairs = Vec::new();
    for value in values {
      validate_field(value)?;
      for member in value.split(';') {
        let pair = parse_pair(member)?;
        if pairs.len() >= MAX_COOKIE_COUNT {
          return Err(HttpCookieParseError::new("too many cookies"));
        }
        pairs.push(pair);
      }
    }
    if pairs.is_empty() {
      return Err(HttpCookieParseError::new("invalid Cookie header value"));
    }
    Ok(Self { pairs })
  }

  pub fn pairs(&self) -> &[HttpCookiePair] {
    &self.pairs
  }
}

/// One attribute from a response `Set-Cookie` field.
///
/// Attributes are intentionally unclassified so extension attributes are
/// retained alongside standardized ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpSetCookieAttribute {
  name: String,
  value: Option<String>,
}

impl HttpSetCookieAttribute {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

/// One bounded response `Set-Cookie` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpSetCookie {
  name: String,
  value: String,
  attributes: Vec<HttpSetCookieAttribute>,
}

impl HttpSetCookie {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    let value = value.as_ref();
    validate_field(value)?;
    let mut members = value.split(';');
    let pair = parse_pair(members.next().unwrap_or_default())?;
    let mut attributes = Vec::new();
    for member in members {
      let member = member.trim_matches([' ', '\t']);
      if member.is_empty() {
        return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
      }
      if attributes.len() >= MAX_SET_COOKIE_ATTRIBUTES {
        return Err(HttpCookieParseError::new("too many Set-Cookie attributes"));
      }
      let (name, value) = match member.split_once('=') {
        Some((name, value)) => {
          let value = value.trim_matches([' ', '\t']);
          validate_value(value)?;
          (name.trim_matches([' ', '\t']), Some(value.to_owned()))
        }
        None => (member, None),
      };
      if name.is_empty() {
        return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
      }
      attributes.push(HttpSetCookieAttribute {
        name: name.to_owned(),
        value,
      });
    }
    Ok(Self {
      name: pair.name,
      value: pair.value,
      attributes,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn attributes(&self) -> &[HttpSetCookieAttribute] {
    &self.attributes
  }
}

/// Parsed response `Set-Cookie` metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpSetCookies {
  cookies: Vec<HttpSetCookie>,
}

impl HttpSetCookies {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpCookieParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut cookies = Vec::new();
    for value in values {
      if cookies.len() >= MAX_COOKIE_COUNT {
        return Err(HttpCookieParseError::new("too many Set-Cookie fields"));
      }
      cookies.push(HttpSetCookie::parse(value)?);
    }
    if cookies.is_empty() {
      return Err(HttpCookieParseError::new("invalid Set-Cookie header value"));
    }
    Ok(Self { cookies })
  }

  pub fn cookies(&self) -> &[HttpSetCookie] {
    &self.cookies
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpCookieParseError {
  message: String,
}

impl HttpCookieParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpCookieParseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for HttpCookieParseError {}

fn parse_pair(value: &str) -> Result<HttpCookiePair, HttpCookieParseError> {
  let value = value.trim_matches([' ', '\t']);
  let Some((name, value)) = value.split_once('=') else {
    return Err(HttpCookieParseError::new("invalid cookie pair"));
  };
  let name = name.trim_matches([' ', '\t']);
  let value = value.trim_matches([' ', '\t']);
  if !is_token(name) {
    return Err(HttpCookieParseError::new("invalid cookie name"));
  }
  validate_value(value)?;
  Ok(HttpCookiePair {
    name: name.to_owned(),
    value: value.to_owned(),
  })
}

fn validate_field(value: &str) -> Result<(), HttpCookieParseError> {
  if value.len() > MAX_COOKIE_FIELD_BYTES {
    return Err(HttpCookieParseError::new(
      "cookie header value is too large",
    ));
  }
  if value.bytes().any(|byte| byte.is_ascii_control()) {
    return Err(HttpCookieParseError::new(
      "cookie header contains a control byte",
    ));
  }
  Ok(())
}

fn validate_value(value: &str) -> Result<(), HttpCookieParseError> {
  if value.len() > MAX_COOKIE_VALUE_BYTES {
    return Err(HttpCookieParseError::new("cookie value is too large"));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cookie_metadata_rejects_values_and_collections_above_its_bounds() {
    let oversized_value = format!("session={}", "a".repeat(MAX_COOKIE_VALUE_BYTES + 1));
    assert!(HttpCookies::parse(&oversized_value).is_err());
    assert!(HttpSetCookie::parse(&oversized_value).is_err());

    let pairs = std::iter::repeat_n("name=value", MAX_COOKIE_COUNT + 1)
      .collect::<Vec<_>>()
      .join(";");
    assert!(HttpCookies::parse(&pairs).is_err());

    let fields = std::iter::repeat_n("name=value", MAX_COOKIE_COUNT + 1);
    assert!(HttpSetCookies::parse_values(fields).is_err());
  }

  #[test]
  fn cookie_metadata_rejects_non_token_names() {
    assert!(HttpCookies::parse("bad name=value").is_err());
    assert!(HttpSetCookie::parse("bad name=value").is_err());
  }
}
