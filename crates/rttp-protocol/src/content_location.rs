//! Bounded, policy-free `Content-Location` response metadata parsing.
//!
//! This module validates a singleton `Content-Location` field value as an
//! absolute URI or relative reference. It preserves the unresolved reference
//! string and never performs redirect, cache selection, representation
//! replacement, retry, route, or status-policy behavior.

use std::error::Error;
use std::fmt;
use std::net::Ipv6Addr;

pub const MAX_CONTENT_LOCATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Content-Location` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocation {
  value: String,
}

impl ContentLocation {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentLocationParseError> {
    let value = value.as_ref();
    if value.len() > MAX_CONTENT_LOCATION_VALUE_BYTES {
      return Err(ContentLocationParseError::new(
        "Content-Location header value is too large",
      ));
    }

    let value = trim_http_optional_whitespace(value);
    if value.is_empty() {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }
    if !is_content_location_field_value(value) {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }

    if !is_uri_reference_field_value(value) {
      return Err(ContentLocationParseError::new(
        "Invalid Content-Location value",
      ));
    }

    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Option<Self>, ContentLocationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(ContentLocationParseError::new(
        "Duplicate Content-Location header values",
      ));
    }
    Self::parse(value).map(Some)
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }

  pub fn header_value(&self) -> &str {
    &self.value
  }
}

impl AsRef<str> for ContentLocation {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocationParseError {
  message: String,
}

impl ContentLocationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentLocationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentLocationParseError {}

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn is_content_location_field_value(value: &str) -> bool {
  value.bytes().all(|byte| {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'<' && byte != b'>' && byte != b'\\'
  })
}

fn is_uri_reference_field_value(value: &str) -> bool {
  let (without_fragment, fragment) = split_once(value, b'#');
  if let Some(fragment) = fragment {
    if fragment.contains('#') || !is_fragment(fragment) {
      return false;
    }
  }

  let (without_query, query) = split_once(without_fragment, b'?');
  if let Some(query) = query {
    if !is_query(query) {
      return false;
    }
  }

  if let Some(scheme_end) = scheme_end(without_query) {
    return is_scheme(&without_query[..scheme_end])
      && is_hier_part(&without_query[scheme_end + 1..]);
  }

  is_relative_part(without_query)
}

fn split_once(value: &str, separator: u8) -> (&str, Option<&str>) {
  match value.bytes().position(|byte| byte == separator) {
    Some(index) => (&value[..index], Some(&value[index + 1..])),
    None => (value, None),
  }
}

fn scheme_end(value: &str) -> Option<usize> {
  value
    .bytes()
    .position(|byte| matches!(byte, b':' | b'/' | b'?' | b'#'))
    .filter(|&index| value.as_bytes()[index] == b':')
}

fn is_scheme(value: &str) -> bool {
  let mut bytes = value.bytes();
  let Some(first) = bytes.next() else {
    return false;
  };
  first.is_ascii_alphabetic()
    && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn is_hier_part(value: &str) -> bool {
  if let Some(after_slashes) = value.strip_prefix("//") {
    let (authority, path) = split_authority_and_path(after_slashes);
    return is_authority(authority) && is_path(path);
  }

  is_path(value)
}

fn is_relative_part(value: &str) -> bool {
  if let Some(after_slashes) = value.strip_prefix("//") {
    let (authority, path) = split_authority_and_path(after_slashes);
    return is_authority(authority) && is_path(path);
  }

  if value.starts_with('/') {
    return is_path(value);
  }

  let first_segment = value.split('/').next().unwrap_or_default();
  !first_segment.contains(':') && is_path(value)
}

fn split_authority_and_path(value: &str) -> (&str, &str) {
  match value.bytes().position(|byte| byte == b'/') {
    Some(index) => (&value[..index], &value[index..]),
    None => (value, ""),
  }
}

fn is_authority(value: &str) -> bool {
  let (userinfo, host_port) = match value.rsplit_once('@') {
    Some((userinfo, host_port)) => {
      if userinfo.contains('@') || !is_userinfo(userinfo) {
        return false;
      }
      (Some(userinfo), host_port)
    }
    None => (None, value),
  };
  let _ = userinfo;

  if let Some(host_port) = host_port.strip_prefix('[') {
    let Some(end) = host_port.bytes().position(|byte| byte == b']') else {
      return false;
    };
    let host = &host_port[..end];
    let port = &host_port[end + 1..];
    return !host.is_empty()
      && is_ip_literal(host)
      && (port.is_empty() || port.strip_prefix(':').is_some_and(is_port));
  }

  let (host, port) = match host_port.rsplit_once(':') {
    Some((host, port)) => (host, Some(port)),
    None => (host_port, None),
  };
  is_reg_name(host) && port.is_none_or(is_port)
}

fn is_userinfo(value: &str) -> bool {
  is_uri_component(value, |byte| {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte) || byte == b':'
  })
}

fn is_ip_literal(value: &str) -> bool {
  value.parse::<Ipv6Addr>().is_ok() || is_ipv_future(value)
}

fn is_ipv_future(value: &str) -> bool {
  let Some(rest) = value.strip_prefix(['v', 'V']) else {
    return false;
  };
  let Some((version, address)) = rest.split_once('.') else {
    return false;
  };

  !version.is_empty()
    && version.bytes().all(|byte| byte.is_ascii_hexdigit())
    && !address.is_empty()
    && address
      .bytes()
      .all(|byte| is_uri_unreserved(byte) || is_uri_sub_delim(byte) || byte == b':')
}

fn is_reg_name(value: &str) -> bool {
  is_uri_component(value, |byte| {
    is_uri_unreserved(byte) || is_uri_sub_delim(byte)
  })
}

fn is_port(value: &str) -> bool {
  value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_path(value: &str) -> bool {
  is_uri_component(value, is_uri_path_char)
}

fn is_query(value: &str) -> bool {
  is_uri_component(value, is_query_char)
}

fn is_fragment(value: &str) -> bool {
  is_query(value)
}

fn is_uri_component(value: &str, allowed: impl Fn(u8) -> bool) -> bool {
  let mut bytes = value.bytes();

  while let Some(byte) = bytes.next() {
    match byte {
      b'%' => {
        let Some(first) = bytes.next() else {
          return false;
        };
        let Some(second) = bytes.next() else {
          return false;
        };
        if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
          return false;
        }
      }
      _ if !allowed(byte) => {
        return false;
      }
      _ => {}
    }
  }

  true
}

fn is_uri_path_char(byte: u8) -> bool {
  is_uri_pchar(byte) || byte == b'/'
}

fn is_query_char(byte: u8) -> bool {
  is_uri_pchar(byte) || matches!(byte, b'/' | b'?')
}

fn is_uri_pchar(byte: u8) -> bool {
  is_uri_unreserved(byte) || is_uri_sub_delim(byte) || matches!(byte, b':' | b'@')
}

fn is_uri_unreserved(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn is_uri_sub_delim(byte: u8) -> bool {
  matches!(
    byte,
    b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
  )
}
