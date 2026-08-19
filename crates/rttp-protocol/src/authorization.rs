//! Bounded, policy-free request authorization metadata.
//!
//! This module validates the shared request header shape for `Authorization`
//! and `Proxy-Authorization`: an HTTP token authentication scheme, one or more
//! SP/HTAB separators, and a non-empty opaque credential value. Callers own
//! authentication policy, credential storage, forwarding, and retry behavior.

use crate::http1::{is_header_value_byte, is_token};
use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a serialized request authorization field value.
pub const MAX_AUTHORIZATION_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Authorization` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct Authorization {
  value: RequestAuthorization,
}

/// Parsed, bounded `Proxy-Authorization` request metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct ProxyAuthorization {
  value: RequestAuthorization,
}

#[derive(Clone, Eq, PartialEq)]
struct RequestAuthorization {
  scheme: String,
  credentials: String,
}

/// An error returned when request authorization metadata is malformed or too large.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationParseError {
  message: String,
}

impl Authorization {
  pub fn new<S: AsRef<str>, C: AsRef<str>>(
    scheme: S,
    credentials: C,
  ) -> Result<Self, AuthorizationParseError> {
    Ok(Self {
      value: RequestAuthorization::new(
        "Authorization",
        scheme.as_ref().trim(),
        credentials.as_ref(),
      )?,
    })
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, AuthorizationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AuthorizationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      value: RequestAuthorization::parse_singleton("Authorization", values)?,
    })
  }

  pub fn scheme(&self) -> &str {
    self.value.scheme()
  }

  pub fn credentials(&self) -> &str {
    self.value.credentials()
  }

  pub fn header_value(&self) -> String {
    self.value.header_value()
  }
}

impl ProxyAuthorization {
  pub fn new<S: AsRef<str>, C: AsRef<str>>(
    scheme: S,
    credentials: C,
  ) -> Result<Self, AuthorizationParseError> {
    Ok(Self {
      value: RequestAuthorization::new(
        "Proxy-Authorization",
        scheme.as_ref().trim(),
        credentials.as_ref(),
      )?,
    })
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, AuthorizationParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AuthorizationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      value: RequestAuthorization::parse_singleton("Proxy-Authorization", values)?,
    })
  }

  pub fn scheme(&self) -> &str {
    self.value.scheme()
  }

  pub fn credentials(&self) -> &str {
    self.value.credentials()
  }

  pub fn header_value(&self) -> String {
    self.value.header_value()
  }
}

impl RequestAuthorization {
  fn new(
    header_name: &str,
    scheme: &str,
    credentials: &str,
  ) -> Result<Self, AuthorizationParseError> {
    validate_parts(header_name, scheme, credentials)?;
    Ok(Self {
      scheme: scheme.to_string(),
      credentials: credentials.to_string(),
    })
  }

  fn parse_singleton<'a, I>(header_name: &str, values: I) -> Result<Self, AuthorizationParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let value = values.next().ok_or_else(|| {
      AuthorizationParseError::new(format!("{header_name} header requires credentials"))
    })?;
    if values.next().is_some() {
      return Err(AuthorizationParseError::new(format!(
        "duplicate {header_name} headers"
      )));
    }
    parse_value(header_name, value)
  }

  fn scheme(&self) -> &str {
    &self.scheme
  }

  fn credentials(&self) -> &str {
    &self.credentials
  }

  fn header_value(&self) -> String {
    format!("{} {}", self.scheme, self.credentials)
  }
}

impl fmt::Debug for Authorization {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter
      .debug_struct("Authorization")
      .field("scheme", &self.scheme())
      .field("credentials", &"[REDACTED]")
      .finish()
  }
}

impl fmt::Debug for ProxyAuthorization {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter
      .debug_struct("ProxyAuthorization")
      .field("scheme", &self.scheme())
      .field("credentials", &"[REDACTED]")
      .finish()
  }
}

impl AuthorizationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for AuthorizationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for AuthorizationParseError {}

fn parse_value(
  header_name: &str,
  value: &str,
) -> Result<RequestAuthorization, AuthorizationParseError> {
  if value.len() > MAX_AUTHORIZATION_VALUE_BYTES {
    return Err(AuthorizationParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  let Some(separator) = value.bytes().position(|byte| byte == b' ' || byte == b'\t') else {
    return Err(AuthorizationParseError::new(format!(
      "{header_name} header requires credentials"
    )));
  };
  let scheme = &value[..separator];
  let credentials = value[separator..].trim_matches([' ', '\t']);
  validate_parts(header_name, scheme, credentials)?;
  Ok(RequestAuthorization {
    scheme: scheme.to_string(),
    credentials: credentials.to_string(),
  })
}

fn validate_parts(
  header_name: &str,
  scheme: &str,
  credentials: &str,
) -> Result<(), AuthorizationParseError> {
  if !is_token(scheme) {
    return Err(AuthorizationParseError::new(format!(
      "invalid {header_name} authentication scheme"
    )));
  }
  if credentials.is_empty()
    || credentials.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
    || !credentials.bytes().all(is_header_value_byte)
  {
    return Err(AuthorizationParseError::new(format!(
      "invalid {header_name} credentials"
    )));
  }
  if scheme.len() + 1 + credentials.len() > MAX_AUTHORIZATION_VALUE_BYTES {
    return Err(AuthorizationParseError::new(format!(
      "{header_name} header value is too large"
    )));
  }
  Ok(())
}
