//! Bounded, policy-free `Referrer-Policy` response metadata parsing.
//!
//! This module validates declared policy tokens only. Callers decide whether
//! and how to apply referrer behavior.

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
    let mut token_count = 0usize;

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
        if contains_forbidden_token_byte(token) {
          return Err(invalid_value());
        }
        if token_count >= MAX_REFERRER_POLICY_TOKENS {
          return Err(ReferrerPolicyParseError::new(
            "too many Referrer-Policy tokens",
          ));
        }
        token_count += 1;
        let Some(policy) = ReferrerPolicyToken::parse(token) else {
          continue;
        };
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
    if value.eq_ignore_ascii_case("no-referrer") {
      Some(Self::NoReferrer)
    } else if value.eq_ignore_ascii_case("no-referrer-when-downgrade") {
      Some(Self::NoReferrerWhenDowngrade)
    } else if value.eq_ignore_ascii_case("origin") {
      Some(Self::Origin)
    } else if value.eq_ignore_ascii_case("origin-when-cross-origin") {
      Some(Self::OriginWhenCrossOrigin)
    } else if value.eq_ignore_ascii_case("same-origin") {
      Some(Self::SameOrigin)
    } else if value.eq_ignore_ascii_case("strict-origin") {
      Some(Self::StrictOrigin)
    } else if value.eq_ignore_ascii_case("strict-origin-when-cross-origin") {
      Some(Self::StrictOriginWhenCrossOrigin)
    } else if value.eq_ignore_ascii_case("unsafe-url") {
      Some(Self::UnsafeUrl)
    } else {
      None
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

fn contains_forbidden_token_byte(token: &str) -> bool {
  token.bytes().any(|byte| byte <= 0x1f || byte == 0x7f)
}
