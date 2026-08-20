//! Bounded, policy-free `Permissions-Policy` response metadata parsing.
//!
//! This module validates the response field value as a W3C Permissions Policy
//! Structured Fields dictionary. It reports declared metadata only: callers
//! decide whether and how to enforce browser permissions or origin policy.
//! This parser does not compare origins, resolve `self`, look up known browser
//! features, or enable or disable any API.
//!
//! Member values are one of the token `*`, the token `self`, a quoted
//! serialized HTTP(S) origin, or an inner list of `self` and quoted origins,
//! including the empty inner list `()`. `*` is the whole allowlist and `()`
//! disables the feature; mixing `*` with other members is rejected. The
//! HTML-attribute tokens `src` and `'none'` are not part of the HTTP
//! structured-header value set and are rejected. A well-formed `report-to`
//! string parameter is accepted as syntax and dropped; other parameters are
//! rejected. Unparsable input is an error; this parser never fails open to an
//! empty policy.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use sfv::{BareItem, Dictionary, ListEntry, Parser};
use url::Url;

pub const MAX_PERMISSIONS_POLICY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PERMISSIONS_POLICY_DIRECTIVES: usize = 256;
pub const MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS: usize = 256;

/// Parsed, bounded `Permissions-Policy` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionsPolicy {
  directives: Vec<PermissionsPolicyDirective>,
}

/// One feature directive from a `Permissions-Policy` dictionary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionsPolicyDirective {
  feature: String,
  allowlist: PermissionsPolicyAllowlist,
}

/// The allowlist declared for one feature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionsPolicyAllowlist {
  /// The `*` token: every origin is in the allowlist.
  AllOrigins,
  /// The declared members, which may be empty when the feature is disabled by
  /// `()`.
  Members(Vec<PermissionsPolicyAllowlistMember>),
}

/// One allowlist member: the document origin or a declared origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PermissionsPolicyAllowlistMember {
  /// The `self` token: the document origin.
  SelfToken,
  /// A quoted serialized HTTP(S) origin.
  Origin(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionsPolicyParseError {
  message: String,
}

impl PermissionsPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for PermissionsPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for PermissionsPolicyParseError {}

impl PermissionsPolicy {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PermissionsPolicyParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PermissionsPolicyParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    for value in values {
      if value.len() > MAX_PERMISSIONS_POLICY_VALUE_BYTES {
        return Err(PermissionsPolicyParseError::new(
          "Permissions-Policy header value is too large",
        ));
      }
      parse_field(value, &mut directives)?;
    }
    if directives.is_empty() {
      return Err(PermissionsPolicyParseError::new(
        "Permissions-Policy field must contain a directive",
      ));
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[PermissionsPolicyDirective] {
    &self.directives
  }

  pub fn directive(&self, name: impl AsRef<str>) -> Option<&PermissionsPolicyDirective> {
    self
      .directives
      .iter()
      .find(|directive| directive.feature == name.as_ref())
  }

  pub fn len(&self) -> usize {
    self.directives.len()
  }

  pub fn is_empty(&self) -> bool {
    self.directives.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .directives
      .iter()
      .map(PermissionsPolicyDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl PermissionsPolicyDirective {
  /// Returns the feature name token with its wire spelling.
  pub fn feature(&self) -> &str {
    &self.feature
  }

  pub fn allowlist(&self) -> &PermissionsPolicyAllowlist {
    &self.allowlist
  }

  fn header_value(&self) -> String {
    format!("{}={}", self.feature, self.allowlist.header_value())
  }
}

impl PermissionsPolicyAllowlist {
  pub const fn is_all_origins(&self) -> bool {
    matches!(self, Self::AllOrigins)
  }

  pub fn members(&self) -> &[PermissionsPolicyAllowlistMember] {
    match self {
      Self::AllOrigins => &[],
      Self::Members(members) => members,
    }
  }

  pub fn is_empty(&self) -> bool {
    matches!(self, Self::Members(members) if members.is_empty())
  }

  fn header_value(&self) -> String {
    match self {
      Self::AllOrigins => "*".to_string(),
      Self::Members(members) => {
        if members.is_empty() {
          return "()".to_string();
        }
        if members.len() == 1 && matches!(members[0], PermissionsPolicyAllowlistMember::SelfToken) {
          return "self".to_string();
        }
        let inner = members
          .iter()
          .map(PermissionsPolicyAllowlistMember::header_value)
          .collect::<Vec<_>>()
          .join(" ");
        format!("({inner})")
      }
    }
  }
}

impl PermissionsPolicyAllowlistMember {
  pub const fn is_self(&self) -> bool {
    matches!(self, Self::SelfToken)
  }

  pub fn origin(&self) -> Option<&str> {
    match self {
      Self::SelfToken => None,
      Self::Origin(origin) => Some(origin),
    }
  }

  fn header_value(&self) -> String {
    match self {
      Self::SelfToken => "self".to_string(),
      Self::Origin(origin) => format!("\"{origin}\""),
    }
  }
}

fn parse_field(
  value: &str,
  directives: &mut Vec<PermissionsPolicyDirective>,
) -> Result<(), PermissionsPolicyParseError> {
  let dictionary = Parser::new(value)
    .parse::<Dictionary>()
    .map_err(|_| invalid_member())?;
  if dictionary.is_empty() {
    return Err(PermissionsPolicyParseError::new(
      "Permissions-Policy field must contain a directive",
    ));
  }
  if top_level_member_count(value) != dictionary.len() {
    return Err(PermissionsPolicyParseError::new(
      "duplicate Permissions-Policy feature key",
    ));
  }

  for (key, member) in dictionary {
    let feature = key.as_str().to_owned();
    if directives
      .iter()
      .any(|directive| directive.feature == feature)
    {
      return Err(PermissionsPolicyParseError::new(
        "duplicate Permissions-Policy feature key",
      ));
    }
    if directives.len() >= MAX_PERMISSIONS_POLICY_DIRECTIVES {
      return Err(PermissionsPolicyParseError::new(
        "too many Permissions-Policy directives",
      ));
    }
    let allowlist = parse_allowlist(member)?;
    directives.push(PermissionsPolicyDirective { feature, allowlist });
  }
  Ok(())
}

fn parse_allowlist(
  member: ListEntry,
) -> Result<PermissionsPolicyAllowlist, PermissionsPolicyParseError> {
  match member {
    ListEntry::Item(item) => {
      validate_parameters(&item.params)?;
      match item.bare_item {
        BareItem::Token(token) => match token.as_str() {
          "*" => Ok(PermissionsPolicyAllowlist::AllOrigins),
          "self" => Ok(PermissionsPolicyAllowlist::Members(vec![
            PermissionsPolicyAllowlistMember::SelfToken,
          ])),
          _ => Err(invalid_member()),
        },
        BareItem::String(string) => {
          if string.as_str() == "'none'" {
            return Err(invalid_member());
          }
          let origin = parse_serialized_origin(string.as_str())?;
          Ok(PermissionsPolicyAllowlist::Members(vec![
            PermissionsPolicyAllowlistMember::Origin(origin),
          ]))
        }
        _ => Err(invalid_member()),
      }
    }
    ListEntry::InnerList(inner_list) => {
      validate_parameters(&inner_list.params)?;
      if inner_list.items.len() > MAX_PERMISSIONS_POLICY_ALLOWLIST_MEMBERS {
        return Err(PermissionsPolicyParseError::new(
          "too many Permissions-Policy allowlist members",
        ));
      }
      let mut members = Vec::with_capacity(inner_list.items.len());
      let mut seen = HashSet::new();
      for item in inner_list.items {
        validate_parameters(&item.params)?;
        let member = match item.bare_item {
          BareItem::Token(token) if token.as_str() == "self" => {
            PermissionsPolicyAllowlistMember::SelfToken
          }
          BareItem::String(string) => {
            if string.as_str() == "'none'" {
              return Err(invalid_member());
            }
            let origin = parse_serialized_origin(string.as_str())?;
            PermissionsPolicyAllowlistMember::Origin(origin)
          }
          _ => return Err(invalid_member()),
        };
        if !seen.insert(member_identity(&member)) {
          return Err(PermissionsPolicyParseError::new(
            "duplicate Permissions-Policy allowlist member",
          ));
        }
        members.push(member);
      }
      Ok(PermissionsPolicyAllowlist::Members(members))
    }
  }
}

fn member_identity(member: &PermissionsPolicyAllowlistMember) -> String {
  match member {
    PermissionsPolicyAllowlistMember::SelfToken => "self".to_string(),
    PermissionsPolicyAllowlistMember::Origin(origin) => origin.clone(),
  }
}

fn validate_parameters(params: &sfv::Parameters) -> Result<(), PermissionsPolicyParseError> {
  for (name, value) in params {
    if name.as_str() != "report-to" {
      return Err(invalid_member());
    }
    if !matches!(value, BareItem::String(_)) {
      return Err(invalid_member());
    }
  }
  Ok(())
}

fn parse_serialized_origin(value: &str) -> Result<String, PermissionsPolicyParseError> {
  let url = Url::parse(value).map_err(|_| invalid_member())?;
  if url.cannot_be_a_base() || !matches!(url.scheme(), "http" | "https") {
    return Err(invalid_member());
  }
  let origin = url.origin().ascii_serialization();
  if origin == "null" || value != origin {
    return Err(invalid_member());
  }
  Ok(origin)
}

fn top_level_member_count(value: &str) -> usize {
  let mut count = 0;
  let mut start = 0;
  let mut quoted = false;
  let mut escaped = false;
  for (index, byte) in value.bytes().enumerate() {
    if quoted {
      if escaped {
        escaped = false;
      } else if byte == b'\\' {
        escaped = true;
      } else if byte == b'"' {
        quoted = false;
      }
    } else if byte == b'"' {
      quoted = true;
    } else if byte == b',' {
      if !value[start..index].trim_matches([' ', '\t']).is_empty() {
        count += 1;
      }
      start = index + 1;
    }
  }
  if !value[start..].trim_matches([' ', '\t']).is_empty() {
    count += 1;
  }
  count
}

fn invalid_member() -> PermissionsPolicyParseError {
  PermissionsPolicyParseError::new("invalid Permissions-Policy dictionary member")
}
