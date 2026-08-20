//! Bounded, policy-free `Document-Policy-Report-Only` response metadata parsing.
//!
//! This module validates report-only Document Policy response metadata through
//! the same directive model and bounds as `Document-Policy`. It reports declared
//! metadata only: callers decide whether and how to use it. This parser does not
//! enforce document policy, block document loads, disable browser features,
//! deliver reports, or attach `Sec-Required-Document-Policy`.

use std::error::Error;
use std::fmt;

use crate::document_policy::{
  format_document_policy_directives, parse_document_policy_values, DocumentPolicyDirective,
};
pub use crate::document_policy::{
  DocumentPolicyDirective as DocumentPolicyReportOnlyDirective,
  DocumentPolicyValue as DocumentPolicyReportOnlyValue, MAX_DOCUMENT_POLICY_DIRECTIVES,
  MAX_DOCUMENT_POLICY_TOTAL_BYTES, MAX_DOCUMENT_POLICY_VALUE_BYTES,
};

/// Parsed, bounded `Document-Policy-Report-Only` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPolicyReportOnly {
  directives: Vec<DocumentPolicyDirective>,
}

/// An error returned when `Document-Policy-Report-Only` metadata is malformed or
/// exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPolicyReportOnlyParseError {
  message: String,
}

impl DocumentPolicyReportOnlyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for DocumentPolicyReportOnlyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for DocumentPolicyReportOnlyParseError {}

impl DocumentPolicyReportOnly {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DocumentPolicyReportOnlyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DocumentPolicyReportOnlyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_document_policy_values("Document-Policy-Report-Only", values)
      .map(|directives| Self { directives })
      .map_err(|error| DocumentPolicyReportOnlyParseError::new(error.message()))
  }

  pub fn directives(&self) -> &[DocumentPolicyDirective] {
    &self.directives
  }

  pub fn directive(&self, name: impl AsRef<str>) -> Option<&DocumentPolicyDirective> {
    self
      .directives
      .iter()
      .find(|directive| directive.name() == name.as_ref())
  }

  pub fn len(&self) -> usize {
    self.directives.len()
  }

  pub fn is_empty(&self) -> bool {
    self.directives.is_empty()
  }

  pub fn header_value(&self) -> String {
    format_document_policy_directives(&self.directives)
  }
}
