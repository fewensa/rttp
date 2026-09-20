//! Bounded, policy-free `Permissions-Policy-Report-Only` response metadata parsing.
//!
//! This module validates report-only Permissions Policy response metadata
//! through the same directive model and bounds as `Permissions-Policy`. It
//! reports declared metadata only: callers decide whether and how to use it.
//! This parser does not enforce browser permissions, compare origins, resolve
//! `self`, enable or disable APIs, or deliver reports.

use std::error::Error;
use std::fmt;

use crate::permissions_policy::{
  format_permissions_policy_directives, parse_permissions_policy_values, PermissionsPolicyDirective,
};
pub use crate::permissions_policy::{
  PermissionsPolicyAllowlist as PermissionsPolicyReportOnlyAllowlist,
  PermissionsPolicyAllowlistMember as PermissionsPolicyReportOnlyAllowlistMember,
  PermissionsPolicyDirective as PermissionsPolicyReportOnlyDirective,
  MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS, MAX_PERMISSIONS_POLICY_DIRECTIVES,
  MAX_PERMISSIONS_POLICY_VALUE_BYTES,
};

/// Parsed, bounded `Permissions-Policy-Report-Only` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionsPolicyReportOnly {
  directives: Vec<PermissionsPolicyDirective>,
}

/// An error returned when `Permissions-Policy-Report-Only` metadata is malformed
/// or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionsPolicyReportOnlyParseError {
  message: String,
}

impl PermissionsPolicyReportOnlyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for PermissionsPolicyReportOnlyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for PermissionsPolicyReportOnlyParseError {}

impl PermissionsPolicyReportOnly {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PermissionsPolicyReportOnlyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PermissionsPolicyReportOnlyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_permissions_policy_values("Permissions-Policy-Report-Only", values)
      .map(|directives| Self { directives })
      .map_err(|error| PermissionsPolicyReportOnlyParseError::new(error.message()))
  }

  pub fn directives(&self) -> &[PermissionsPolicyDirective] {
    &self.directives
  }

  pub fn directive(&self, name: impl AsRef<str>) -> Option<&PermissionsPolicyDirective> {
    self
      .directives
      .iter()
      .find(|directive| directive.feature() == name.as_ref())
  }

  pub fn len(&self) -> usize {
    self.directives.len()
  }

  pub fn is_empty(&self) -> bool {
    self.directives.is_empty()
  }

  pub fn header_value(&self) -> String {
    format_permissions_policy_directives(&self.directives)
  }
}
