//! Bounded, policy-free `Sec-Required-Document-Policy` request metadata parsing.
//!
//! This module validates required Document Policy request metadata through the
//! same directive model and bounds as `Document-Policy`. It reports declared
//! metadata only: callers decide whether and how to use it. This parser does
//! not enforce document policy, compare required policies with
//! `Document-Policy`, block document loads, disable browser features, or echo
//! response fields.

use std::error::Error;
use std::fmt;

use crate::document_policy::{
  format_document_policy_directives, parse_document_policy_values, DocumentPolicyDirective,
};
pub use crate::document_policy::{
  DocumentPolicyDirective as SecRequiredDocumentPolicyDirective,
  DocumentPolicyValue as SecRequiredDocumentPolicyValue, MAX_DOCUMENT_POLICY_DIRECTIVES,
  MAX_DOCUMENT_POLICY_TOTAL_BYTES, MAX_DOCUMENT_POLICY_VALUE_BYTES,
};

/// Parsed, bounded `Sec-Required-Document-Policy` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecRequiredDocumentPolicy {
  directives: Vec<DocumentPolicyDirective>,
}

/// An error returned when `Sec-Required-Document-Policy` metadata is malformed
/// or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecRequiredDocumentPolicyParseError {
  message: String,
}

impl SecRequiredDocumentPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SecRequiredDocumentPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SecRequiredDocumentPolicyParseError {}

impl SecRequiredDocumentPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SecRequiredDocumentPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SecRequiredDocumentPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    parse_document_policy_values("Sec-Required-Document-Policy", values)
      .map(|directives| Self { directives })
      .map_err(|error| SecRequiredDocumentPolicyParseError::new(error.message()))
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
