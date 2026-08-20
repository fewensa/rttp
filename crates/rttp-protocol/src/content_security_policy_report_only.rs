//! Bounded, policy-free `Content-Security-Policy-Report-Only` response metadata parsing.
//!
//! This module validates the response field value as opaque metadata only.
//! Callers decide whether and how to consume report-only browser policy metadata.

use std::error::Error;
use std::fmt;

use crate::csp_policy::{
  parse_csp_policy_values, MAX_CSP_POLICY_FIELDS, MAX_CSP_POLICY_VALUE_BYTES,
};

pub const MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_VALUE_BYTES: usize = MAX_CSP_POLICY_VALUE_BYTES;
pub const MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS: usize = MAX_CSP_POLICY_FIELDS;

/// The opaque policies declared by `Content-Security-Policy-Report-Only`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentSecurityPolicyReportOnly(Vec<String>);

impl ContentSecurityPolicyReportOnly {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentSecurityPolicyReportOnlyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentSecurityPolicyReportOnlyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_csp_policy_values(
      values,
      "Content-Security-Policy-Report-Only",
      ContentSecurityPolicyReportOnlyParseError::new,
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

impl AsRef<str> for ContentSecurityPolicyReportOnly {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSecurityPolicyReportOnlyParseError {
  message: String,
}

impl ContentSecurityPolicyReportOnlyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ContentSecurityPolicyReportOnlyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ContentSecurityPolicyReportOnlyParseError {}
