//! Bounded, policy-free `Content-Security-Policy` response metadata parsing.
//!
//! This module validates the response field value as opaque metadata only.
//! Callers decide whether and how to enforce browser security policy.

use std::error::Error;
use std::fmt;

use crate::csp_policy::{
  parse_csp_policy_values, MAX_CSP_POLICY_FIELDS, MAX_CSP_POLICY_VALUE_BYTES,
};

pub const MAX_CONTENT_SECURITY_POLICY_VALUE_BYTES: usize = MAX_CSP_POLICY_VALUE_BYTES;
pub const MAX_CONTENT_SECURITY_POLICY_FIELDS: usize = MAX_CSP_POLICY_FIELDS;

/// The opaque policies declared by `Content-Security-Policy`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentSecurityPolicy(Vec<String>);

impl ContentSecurityPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentSecurityPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentSecurityPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_csp_policy_values(
      values,
      "Content-Security-Policy",
      ContentSecurityPolicyParseError::new,
    )
    .map(Self)
  }

  pub fn as_str(&self) -> &str {
    &self.0[0]
  }

  pub fn header_value(&self) -> &str {
    self.as_str()
  }

  pub fn header_values(&self) -> &[String] {
    &self.0
  }
}

impl AsRef<str> for ContentSecurityPolicy {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSecurityPolicyParseError {
  message: String,
}

impl ContentSecurityPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentSecurityPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentSecurityPolicyParseError {}
