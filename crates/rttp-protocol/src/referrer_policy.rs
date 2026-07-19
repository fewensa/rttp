//! Bounded, policy-free `Referrer-Policy` response metadata parsing.
//!
//! This module validates declared policy tokens only. Callers decide whether
//! and how to apply referrer behavior.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

pub const MAX_REFERRER_POLICY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_REFERRER_POLICY_TOKENS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferrerPolicy(Vec<ReferrerPolicyToken>);

impl ReferrerPolicy {
  pub fn new(policies: impl IntoIterator<Item = ReferrerPolicyToken>) -> Self {
    Self(policies.into_iter().collect())
  }

  pub fn parse(value: impl AsRef<str>) -> Result<Self, ReferrerPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ReferrerPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut policies = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
      if value.len() > MAX_REFERRER_POLICY_VALUE_BYTES {
        return Err(ReferrerPolicyParseError::new(
          "Referrer-Policy header value is too large",
        ));
      }

      for token in value.split(',') {
        let token = token.trim_matches([' ', '\t']);
        if token.is_empty() {
          return Err(invalid_value());
        }
        let policy = ReferrerPolicyToken::parse(token).ok_or_else(invalid_value)?;
        if policies.len() >= MAX_REFERRER_POLICY_TOKENS {
          return Err(ReferrerPolicyParseError::new(
            "too many Referrer-Policy tokens",
          ));
        }
        if !seen.insert(policy) {
          return Err(ReferrerPolicyParseError::new(
            "duplicate Referrer-Policy declarations",
          ));
        }
        policies.push(policy);
      }
    }

    if policies.is_empty() {
      return Err(invalid_value());
    }

    Ok(Self(policies))
  }

  pub fn policies(&self) -> &[ReferrerPolicyToken] {
    &self.0
  }

  pub fn header_value(&self) -> String {
    self
      .0
      .iter()
      .map(|policy| policy.as_str())
      .collect::<Vec<_>>()
      .join(", ")
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReferrerPolicyToken {
  NoReferrer,
  NoReferrerWhenDowngrade,
  Origin,
  OriginWhenCrossOrigin,
  SameOrigin,
  StrictOrigin,
  StrictOriginWhenCrossOrigin,
  UnsafeUrl,
}

impl ReferrerPolicyToken {
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "no-referrer" => Some(Self::NoReferrer),
      "no-referrer-when-downgrade" => Some(Self::NoReferrerWhenDowngrade),
      "origin" => Some(Self::Origin),
      "origin-when-cross-origin" => Some(Self::OriginWhenCrossOrigin),
      "same-origin" => Some(Self::SameOrigin),
      "strict-origin" => Some(Self::StrictOrigin),
      "strict-origin-when-cross-origin" => Some(Self::StrictOriginWhenCrossOrigin),
      "unsafe-url" => Some(Self::UnsafeUrl),
      _ => None,
    }
  }

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::NoReferrer => "no-referrer",
      Self::NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
      Self::Origin => "origin",
      Self::OriginWhenCrossOrigin => "origin-when-cross-origin",
      Self::SameOrigin => "same-origin",
      Self::StrictOrigin => "strict-origin",
      Self::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
      Self::UnsafeUrl => "unsafe-url",
    }
  }

  pub const fn header_value(self) -> &'static str {
    self.as_str()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferrerPolicyParseError {
  message: String,
}

impl ReferrerPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ReferrerPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ReferrerPolicyParseError {}

fn invalid_value() -> ReferrerPolicyParseError {
  ReferrerPolicyParseError::new("invalid Referrer-Policy header value")
}
