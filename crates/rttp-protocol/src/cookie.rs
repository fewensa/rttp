//! Bounded, policy-free cookie metadata parsing.
//!
//! This module parses request `Cookie` pairs and response `Set-Cookie` fields
//! as metadata only. It does not implement a cookie jar, domain/path matching,
//! expiry enforcement, SameSite or partitioning policy, persistence, redirect
//! handling, or automatic request `Cookie` emission.

use crate::http1::is_token;
use std::error::Error;
use std::fmt;

/// Maximum number of parsed request cookies or response `Set-Cookie` fields.
pub const MAX_COOKIE_COUNT: usize = 256;
/// Maximum number of attributes retained for one `Set-Cookie` field.
pub const MAX_SET_COOKIE_ATTRIBUTES: usize = 64;
/// Maximum byte length of an individual cookie or attribute value.
pub const MAX_COOKIE_VALUE_BYTES: usize = 4 * 1024;
/// Maximum byte length of one cookie header field value.
pub const MAX_COOKIE_FIELD_BYTES: usize = 64 * 1024;
/// Maximum combined raw `Set-Cookie` field bytes accepted in one collection.
pub const MAX_SET_COOKIE_TOTAL_BYTES: usize = 64 * 1024;

/// One name/value pair from a request `Cookie` field.
#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for HttpCookiePair {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpCookiePair")
      .field("name", &self.name)
      .field("value", &"[REDACTED]")
      .finish()
  }
}

/// Parsed request `Cookie` metadata.
#[derive(Clone, PartialEq, Eq)]
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

impl fmt::Debug for HttpCookies {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpCookies")
      .field("pair_count", &self.pairs.len())
      .field(
        "names",
        &self
          .pairs
          .iter()
          .map(HttpCookiePair::name)
          .collect::<Vec<_>>(),
      )
      .finish()
  }
}

/// Classification of one `Set-Cookie` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpSetCookieAttributeKind {
  Expires,
  MaxAge,
  Domain,
  Path,
  Secure,
  HttpOnly,
  SameSite,
  Partitioned,
  Priority,
  Extension,
}

impl HttpSetCookieAttributeKind {
  fn parse(name: &str) -> Self {
    match name.to_ascii_lowercase().as_str() {
      "expires" => Self::Expires,
      "max-age" => Self::MaxAge,
      "domain" => Self::Domain,
      "path" => Self::Path,
      "secure" => Self::Secure,
      "httponly" => Self::HttpOnly,
      "samesite" => Self::SameSite,
      "partitioned" => Self::Partitioned,
      "priority" => Self::Priority,
      _ => Self::Extension,
    }
  }
}

/// Recognized `SameSite` cookie attribute values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpSameSite {
  Strict,
  Lax,
  None,
}

impl HttpSameSite {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Strict => "Strict",
      Self::Lax => "Lax",
      Self::None => "None",
    }
  }

  fn parse(value: &str) -> Result<Self, HttpCookieParseError> {
    match value.to_ascii_lowercase().as_str() {
      "strict" => Ok(Self::Strict),
      "lax" => Ok(Self::Lax),
      "none" => Ok(Self::None),
      _ => Err(HttpCookieParseError::new("invalid SameSite attribute")),
    }
  }
}

/// One attribute from a response `Set-Cookie` field.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpSetCookieAttribute {
  name: String,
  value: Option<String>,
  quoted: bool,
  kind: HttpSetCookieAttributeKind,
}

impl HttpSetCookieAttribute {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  pub fn is_quoted(&self) -> bool {
    self.quoted
  }

  pub fn kind(&self) -> HttpSetCookieAttributeKind {
    self.kind
  }

  fn header_value(&self) -> String {
    match &self.value {
      Some(value) if self.quoted => format!("{}=\"{}\"", self.name, value),
      Some(value) => format!("{}={}", self.name, value),
      None => self.name.clone(),
    }
  }
}

impl fmt::Debug for HttpSetCookieAttribute {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpSetCookieAttribute")
      .field("name", &self.name)
      .field("value", &self.value.as_ref().map(|_| "[REDACTED]"))
      .field("quoted", &self.quoted)
      .field("kind", &self.kind)
      .finish()
  }
}

/// One bounded response `Set-Cookie` field.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpSetCookie {
  name: String,
  value: String,
  value_quoted: bool,
  attributes: Vec<HttpSetCookieAttribute>,
}

impl HttpSetCookie {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    let value = value.as_ref();
    validate_field(value)?;
    let members = split_unquoted_members(value)?;
    let mut members = members.into_iter();
    let (name, cookie_value, value_quoted) =
      parse_set_cookie_pair(members.next().unwrap_or_default())?;
    let mut cookie = Self {
      name,
      value: cookie_value,
      value_quoted,
      attributes: Vec::new(),
    };
    for member in members {
      let attribute = parse_set_cookie_attribute(member)?;
      cookie = cookie.push_parsed_attribute(attribute)?;
    }
    cookie.enforce_field_bound()?;
    Ok(cookie)
  }

  pub fn new(name: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    let name = name.as_ref().trim_matches([' ', '\t']);
    let value = value.as_ref();
    if !is_token(name) {
      return Err(HttpCookieParseError::new("invalid cookie name"));
    }
    validate_value(value)?;
    if value.bytes().any(is_invalid_generated_quoted_value_byte) {
      return Err(HttpCookieParseError::new("invalid cookie value"));
    }
    let cookie = Self {
      name: name.to_owned(),
      value: value.to_owned(),
      value_quoted: cookie_value_needs_quotes(value),
      attributes: Vec::new(),
    };
    cookie.enforce_field_bound()?;
    Ok(cookie)
  }

  pub fn with_quoted_value(mut self) -> Result<Self, HttpCookieParseError> {
    self.value_quoted = true;
    self.enforce_field_bound()?;
    Ok(self)
  }

  pub fn with_expires(self, value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "Expires",
      Some(value.as_ref()),
      HttpSetCookieAttributeKind::Expires,
    )
  }

  pub fn with_max_age(self, seconds: u64) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "Max-Age",
      Some(seconds.to_string().as_str()),
      HttpSetCookieAttributeKind::MaxAge,
    )
  }

  pub fn with_domain(self, value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "Domain",
      Some(value.as_ref()),
      HttpSetCookieAttributeKind::Domain,
    )
  }

  pub fn with_path(self, value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "Path",
      Some(value.as_ref()),
      HttpSetCookieAttributeKind::Path,
    )
  }

  pub fn with_secure(self) -> Result<Self, HttpCookieParseError> {
    self.push_standard("Secure", None, HttpSetCookieAttributeKind::Secure)
  }

  pub fn with_http_only(self) -> Result<Self, HttpCookieParseError> {
    self.push_standard("HttpOnly", None, HttpSetCookieAttributeKind::HttpOnly)
  }

  pub fn with_same_site(self, value: HttpSameSite) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "SameSite",
      Some(value.as_str()),
      HttpSetCookieAttributeKind::SameSite,
    )
  }

  pub fn with_partitioned(self) -> Result<Self, HttpCookieParseError> {
    self.push_standard("Partitioned", None, HttpSetCookieAttributeKind::Partitioned)
  }

  pub fn with_priority(self, value: impl AsRef<str>) -> Result<Self, HttpCookieParseError> {
    self.push_standard(
      "Priority",
      Some(value.as_ref()),
      HttpSetCookieAttributeKind::Priority,
    )
  }

  pub fn with_extension(
    self,
    name: impl AsRef<str>,
    value: Option<&str>,
  ) -> Result<Self, HttpCookieParseError> {
    let name = name.as_ref().trim_matches([' ', '\t']);
    if !is_token(name)
      || HttpSetCookieAttributeKind::parse(name) != HttpSetCookieAttributeKind::Extension
    {
      return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
    }
    let value = match value {
      Some(value) => {
        validate_value(value)?;
        if value.bytes().any(is_invalid_generated_quoted_value_byte) {
          return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
        }
        Some(value.to_owned())
      }
      None => None,
    };
    let quoted = value.as_deref().is_some_and(cookie_value_needs_quotes);
    self.push_parsed_attribute(HttpSetCookieAttribute {
      name: name.to_owned(),
      value,
      quoted,
      kind: HttpSetCookieAttributeKind::Extension,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  pub fn is_value_quoted(&self) -> bool {
    self.value_quoted
  }

  pub fn attributes(&self) -> &[HttpSetCookieAttribute] {
    &self.attributes
  }

  pub fn extension_attributes(&self) -> impl Iterator<Item = &HttpSetCookieAttribute> {
    self
      .attributes
      .iter()
      .filter(|attribute| attribute.kind == HttpSetCookieAttributeKind::Extension)
  }

  pub fn expires(&self) -> Option<&str> {
    self.standard_value(HttpSetCookieAttributeKind::Expires)
  }

  pub fn max_age(&self) -> Option<u64> {
    self
      .standard_value(HttpSetCookieAttributeKind::MaxAge)
      .and_then(|value| value.parse().ok())
  }

  pub fn domain(&self) -> Option<&str> {
    self.standard_value(HttpSetCookieAttributeKind::Domain)
  }

  pub fn path(&self) -> Option<&str> {
    self.standard_value(HttpSetCookieAttributeKind::Path)
  }

  pub fn secure(&self) -> bool {
    self.has_kind(HttpSetCookieAttributeKind::Secure)
  }

  pub fn http_only(&self) -> bool {
    self.has_kind(HttpSetCookieAttributeKind::HttpOnly)
  }

  pub fn same_site(&self) -> Option<HttpSameSite> {
    self
      .standard_value(HttpSetCookieAttributeKind::SameSite)
      .and_then(|value| HttpSameSite::parse(value).ok())
  }

  pub fn partitioned(&self) -> bool {
    self.has_kind(HttpSetCookieAttributeKind::Partitioned)
  }

  pub fn priority(&self) -> Option<&str> {
    self.standard_value(HttpSetCookieAttributeKind::Priority)
  }

  pub fn header_value(&self) -> String {
    let mut value = if self.value_quoted {
      format!("{}=\"{}\"", self.name, self.value)
    } else {
      format!("{}={}", self.name, self.value)
    };
    for attribute in &self.attributes {
      value.push_str("; ");
      value.push_str(&attribute.header_value());
    }
    value
  }

  fn standard_value(&self, kind: HttpSetCookieAttributeKind) -> Option<&str> {
    self
      .attributes
      .iter()
      .find(|attribute| attribute.kind == kind)
      .and_then(HttpSetCookieAttribute::value)
  }

  fn has_kind(&self, kind: HttpSetCookieAttributeKind) -> bool {
    self
      .attributes
      .iter()
      .any(|attribute| attribute.kind == kind)
  }

  fn push_standard(
    self,
    name: &str,
    value: Option<&str>,
    kind: HttpSetCookieAttributeKind,
  ) -> Result<Self, HttpCookieParseError> {
    if let Some(value) = value {
      validate_value(value)?;
      validate_standard_attribute(kind, Some(value))?;
    } else {
      validate_standard_attribute(kind, None)?;
    }
    self.push_parsed_attribute(HttpSetCookieAttribute {
      name: name.to_owned(),
      value: value.map(str::to_owned),
      quoted: false,
      kind,
    })
  }

  fn push_parsed_attribute(
    mut self,
    attribute: HttpSetCookieAttribute,
  ) -> Result<Self, HttpCookieParseError> {
    if self.attributes.len() >= MAX_SET_COOKIE_ATTRIBUTES {
      return Err(HttpCookieParseError::new("too many Set-Cookie attributes"));
    }
    if self
      .attributes
      .iter()
      .any(|existing| existing.name.eq_ignore_ascii_case(&attribute.name))
    {
      return Err(HttpCookieParseError::new("duplicate Set-Cookie attribute"));
    }
    self.attributes.push(attribute);
    self.enforce_field_bound()?;
    Ok(self)
  }

  fn enforce_field_bound(&self) -> Result<(), HttpCookieParseError> {
    if self.header_value().len() > MAX_COOKIE_FIELD_BYTES {
      return Err(HttpCookieParseError::new(
        "cookie header value is too large",
      ));
    }
    Ok(())
  }
}

impl fmt::Debug for HttpSetCookie {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpSetCookie")
      .field("name", &self.name)
      .field("value", &"[REDACTED]")
      .field("value_quoted", &self.value_quoted)
      .field(
        "attribute_names",
        &self
          .attributes
          .iter()
          .map(HttpSetCookieAttribute::name)
          .collect::<Vec<_>>(),
      )
      .finish()
  }
}

/// Parsed response `Set-Cookie` metadata.
#[derive(Clone, PartialEq, Eq)]
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
    let mut total_bytes = 0usize;
    for value in values {
      if cookies.len() >= MAX_COOKIE_COUNT {
        return Err(HttpCookieParseError::new("too many Set-Cookie fields"));
      }
      total_bytes = total_bytes
        .checked_add(value.len())
        .filter(|total| *total <= MAX_SET_COOKIE_TOTAL_BYTES)
        .ok_or_else(|| HttpCookieParseError::new("Set-Cookie header list is too large"))?;
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

  pub fn len(&self) -> usize {
    self.cookies.len()
  }

  pub fn is_empty(&self) -> bool {
    self.cookies.is_empty()
  }

  pub fn header_values(&self) -> Vec<String> {
    self
      .cookies
      .iter()
      .map(HttpSetCookie::header_value)
      .collect()
  }
}

impl fmt::Debug for HttpSetCookies {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpSetCookies")
      .field("cookie_count", &self.cookies.len())
      .field(
        "names",
        &self
          .cookies
          .iter()
          .map(HttpSetCookie::name)
          .collect::<Vec<_>>(),
      )
      .finish()
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

fn parse_set_cookie_pair(value: &str) -> Result<(String, String, bool), HttpCookieParseError> {
  let value = value.trim_matches([' ', '\t']);
  let Some((name, value)) = value.split_once('=') else {
    return Err(HttpCookieParseError::new("invalid cookie pair"));
  };
  let name = name.trim_matches([' ', '\t']);
  let value = value.trim_matches([' ', '\t']);
  if !is_token(name) {
    return Err(HttpCookieParseError::new("invalid cookie name"));
  }
  let (value, quoted) = parse_maybe_quoted_value(value, "invalid cookie value")?;
  validate_value(&value)?;
  Ok((name.to_owned(), value, quoted))
}

fn parse_set_cookie_attribute(
  member: &str,
) -> Result<HttpSetCookieAttribute, HttpCookieParseError> {
  let member = member.trim_matches([' ', '\t']);
  if member.is_empty() {
    return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
  }
  let (name, raw_value) = match member.split_once('=') {
    Some((name, value)) => (
      name.trim_matches([' ', '\t']),
      Some(value.trim_matches([' ', '\t'])),
    ),
    None => (member, None),
  };
  if !is_token(name) {
    return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
  }
  let kind = HttpSetCookieAttributeKind::parse(name);
  let (value, quoted) = match raw_value {
    Some(raw_value) => {
      let (value, quoted) = parse_maybe_quoted_value(raw_value, "invalid Set-Cookie attribute")?;
      validate_value(&value)?;
      (Some(value), quoted)
    }
    None => (None, false),
  };
  validate_standard_attribute(kind, value.as_deref())?;
  Ok(HttpSetCookieAttribute {
    name: name.to_owned(),
    value,
    quoted,
    kind,
  })
}

fn validate_standard_attribute(
  kind: HttpSetCookieAttributeKind,
  value: Option<&str>,
) -> Result<(), HttpCookieParseError> {
  match kind {
    HttpSetCookieAttributeKind::Secure
    | HttpSetCookieAttributeKind::HttpOnly
    | HttpSetCookieAttributeKind::Partitioned => {
      if value.is_some() {
        return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
      }
    }
    HttpSetCookieAttributeKind::Expires
    | HttpSetCookieAttributeKind::Domain
    | HttpSetCookieAttributeKind::Path
    | HttpSetCookieAttributeKind::Priority => {
      if value.is_none_or(str::is_empty) {
        return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
      }
    }
    HttpSetCookieAttributeKind::MaxAge => {
      let Some(value) = value else {
        return Err(HttpCookieParseError::new("invalid Set-Cookie attribute"));
      };
      if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(HttpCookieParseError::new("invalid Max-Age attribute"));
      }
      if value.parse::<u64>().is_err() {
        return Err(HttpCookieParseError::new("invalid Max-Age attribute"));
      }
    }
    HttpSetCookieAttributeKind::SameSite => {
      let Some(value) = value else {
        return Err(HttpCookieParseError::new("invalid SameSite attribute"));
      };
      HttpSameSite::parse(value)?;
    }
    HttpSetCookieAttributeKind::Extension => {}
  }
  Ok(())
}

fn parse_maybe_quoted_value(
  value: &str,
  invalid: &'static str,
) -> Result<(String, bool), HttpCookieParseError> {
  if !value.starts_with('"') {
    if value.bytes().any(|byte| byte == b'"') {
      return Err(HttpCookieParseError::new(invalid));
    }
    return Ok((value.to_owned(), false));
  }
  if value.len() < 2 || !value.ends_with('"') {
    return Err(HttpCookieParseError::new(invalid));
  }
  let inner = &value[1..value.len() - 1];
  if inner.bytes().any(|byte| byte == b'"' || byte == b'\\') {
    return Err(HttpCookieParseError::new(invalid));
  }
  Ok((inner.to_owned(), true))
}

fn split_unquoted_members(value: &str) -> Result<Vec<&str>, HttpCookieParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut quoted = false;
  for (index, byte) in value.bytes().enumerate() {
    match byte {
      b'"' => quoted = !quoted,
      b';' if !quoted => {
        members.push(&value[start..index]);
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted {
    return Err(HttpCookieParseError::new("invalid cookie value"));
  }
  members.push(&value[start..]);
  Ok(members)
}

fn validate_field(value: &str) -> Result<(), HttpCookieParseError> {
  if value.len() > MAX_COOKIE_FIELD_BYTES {
    return Err(HttpCookieParseError::new(
      "cookie header value is too large",
    ));
  }
  if value.bytes().any(is_invalid_control_byte) {
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
  if value.bytes().any(is_invalid_control_byte) {
    return Err(HttpCookieParseError::new(
      "cookie header contains a control byte",
    ));
  }
  Ok(())
}

fn cookie_value_needs_quotes(value: &str) -> bool {
  !value.is_empty() && value.bytes().any(|byte| !is_cookie_octet(byte))
}

fn is_cookie_octet(byte: u8) -> bool {
  matches!(
    byte,
    0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e
  )
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_invalid_generated_quoted_value_byte(byte: u8) -> bool {
  byte == b'"' || byte == b'\\'
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

  #[test]
  fn set_cookie_builder_outputs_round_trip_through_parser() {
    let cookies = [
      HttpSetCookie::new("session", "abc def")
        .unwrap()
        .with_path("/")
        .unwrap()
        .with_http_only()
        .unwrap()
        .with_same_site(HttpSameSite::Lax)
        .unwrap(),
      HttpSetCookie::new("prefs", "a,b")
        .unwrap()
        .with_extension("Ext", Some("v w"))
        .unwrap(),
      HttpSetCookie::new("flag", "enabled")
        .unwrap()
        .with_extension("Flag", None)
        .unwrap(),
    ];

    for cookie in cookies {
      assert_eq!(cookie, HttpSetCookie::parse(cookie.header_value()).unwrap());
    }
  }

  #[test]
  fn set_cookie_builder_rejects_values_that_parser_rejects_when_quoted() {
    assert!(HttpSetCookie::new("session", "a\\b").is_err());
    assert!(HttpSetCookie::new("session", "a\"b").is_err());
    assert!(HttpSetCookie::new("session", "abc")
      .unwrap()
      .with_extension("Ext", Some("v\\w"))
      .is_err());
    assert!(HttpSetCookie::new("session", "abc")
      .unwrap()
      .with_extension("Ext", Some("v\"w"))
      .is_err());
  }
}
