//! Bounded, policy-free W3C Network Error Logging (NEL) response metadata parsing.
//!
//! This module validates one `NEL` response field as a bounded JSON object and
//! exposes the policy members `report_to`, `max_age`, `include_subdomains`,
//! `success_fraction`, and `failure_fraction` with checked types. Malformed
//! JSON, invalid member types, non-finite or out-of-range fractions, duplicate
//! singleton members, oversized input, and duplicate header fields are errors.
//! Unknown JSON members are preserved verbatim as raw metadata without
//! assigning them policy semantics.
//!
//! The W3C NEL defaults apply when an optional member is absent:
//! `include_subdomains` is `false`, `success_fraction` is `0.0`, and
//! `failure_fraction` is `1.0`. This parser keeps those members optional so
//! re-serialization is faithful; callers that need the spec defaults apply
//! them on access.
//!
//! `max_age` is required and must be a non-negative JSON integer literal that
//! fits in `u64`; fraction and exponent forms such as `1.0` or `1e3` are
//! rejected for this member. Fractions must parse as finite `f64` values in
//! the inclusive range `[0.0, 1.0]`.
//!
//! Parsing is policy-free: no reports are sent, no policy is persisted, and no
//! Reporting endpoint group is configured. Callers own report delivery,
//! policy storage, and expiry.
//!
//! ```
//! use rttp_protocol::nel::Nel;
//!
//! let nel = Nel::parse(
//!   r#"{"report_to":"network-errors","max_age":2592000,"include_subdomains":true,"success_fraction":0.1,"failure_fraction":1.0}"#,
//! )
//! .expect("valid NEL policy");
//! assert_eq!(nel.max_age(), 2592000);
//! assert_eq!(nel.report_to(), Some("network-errors"));
//! assert_eq!(nel.include_subdomains(), Some(true));
//! assert_eq!(nel.success_fraction(), Some(0.1));
//! assert_eq!(nel.failure_fraction(), Some(1.0));
//! ```

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

/// Maximum bytes accepted in a `NEL` field value.
pub const MAX_NEL_VALUE_BYTES: usize = 64 * 1024;
/// Maximum JSON members accepted in one object, including the policy object.
pub const MAX_NEL_MEMBERS: usize = 256;
/// Maximum decoded bytes accepted in a single JSON string.
pub const MAX_NEL_STRING_BYTES: usize = 64 * 1024;
/// Maximum JSON nesting depth accepted inside the policy object.
pub const MAX_NEL_DEPTH: usize = 64;

/// Parsed, bounded W3C Network Error Logging response metadata.
///
/// Absent optional members keep their W3C defaults (`include_subdomains`
/// `false`, `success_fraction` `0.0`, `failure_fraction` `1.0`) but remain
/// `None` here so re-serialization is faithful.
#[derive(Clone, Debug, PartialEq)]
pub struct Nel {
  report_to: Option<String>,
  max_age: u64,
  include_subdomains: Option<bool>,
  success_fraction: Option<f64>,
  failure_fraction: Option<f64>,
  unknown_members: Vec<NelUnknownMember>,
}

/// A JSON member preserved verbatim without policy semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NelUnknownMember {
  name: String,
  value: String,
}

impl NelUnknownMember {
  pub fn name(&self) -> &str {
    &self.name
  }

  /// The raw JSON text of the member value, preserved verbatim.
  pub fn value(&self) -> &str {
    &self.value
  }
}

impl Nel {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, NelParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, NelParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_singleton(values)?;
    parse_policy(value)
  }

  pub fn report_to(&self) -> Option<&str> {
    self.report_to.as_deref()
  }

  pub fn max_age(&self) -> u64 {
    self.max_age
  }

  pub fn include_subdomains(&self) -> Option<bool> {
    self.include_subdomains
  }

  pub fn success_fraction(&self) -> Option<f64> {
    self.success_fraction
  }

  pub fn failure_fraction(&self) -> Option<f64> {
    self.failure_fraction
  }

  pub fn unknown_members(&self) -> &[NelUnknownMember] {
    &self.unknown_members
  }

  pub fn header_value(&self) -> String {
    let mut out = String::new();
    out.push('{');
    out.push_str("\"max_age\":");
    out.push_str(&self.max_age.to_string());
    if let Some(report_to) = &self.report_to {
      out.push_str(",\"report_to\":");
      push_json_string(&mut out, report_to);
    }
    if let Some(include_subdomains) = self.include_subdomains {
      out.push_str(",\"include_subdomains\":");
      out.push_str(if include_subdomains { "true" } else { "false" });
    }
    if let Some(success_fraction) = self.success_fraction {
      out.push_str(",\"success_fraction\":");
      out.push_str(&format_fraction(success_fraction));
    }
    if let Some(failure_fraction) = self.failure_fraction {
      out.push_str(",\"failure_fraction\":");
      out.push_str(&format_fraction(failure_fraction));
    }
    for member in &self.unknown_members {
      out.push(',');
      push_json_string(&mut out, &member.name);
      out.push(':');
      out.push_str(&member.value);
    }
    out.push('}');
    out
  }
}

/// An error returned when `NEL` metadata is malformed or exceeds bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NelParseError {
  message: String,
}

impl NelParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for NelParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for NelParseError {}

fn parse_singleton<'a, I>(values: I) -> Result<&'a str, NelParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_value)?;
  validate_value_bound(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_value_bound(value)?;
  }
  if has_duplicate {
    return Err(NelParseError::new("duplicate NEL header fields"));
  }
  Ok(value)
}

fn validate_value_bound(value: &str) -> Result<(), NelParseError> {
  if value.len() > MAX_NEL_VALUE_BYTES {
    return Err(NelParseError::new("NEL header value is too large"));
  }
  if value.bytes().any(is_invalid_control_byte) {
    return Err(NelParseError::new(
      "NEL header value contains an invalid control byte",
    ));
  }
  Ok(())
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn parse_policy(value: &str) -> Result<Nel, NelParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ws(bytes, &mut position);
  if bytes.get(position) != Some(&b'{') {
    return Err(NelParseError::new("NEL policy must be a JSON object"));
  }
  position += 1;
  let mut nel = Nel {
    report_to: None,
    max_age: 0,
    include_subdomains: None,
    success_fraction: None,
    failure_fraction: None,
    unknown_members: Vec::new(),
  };
  let mut seen = HashSet::new();
  parse_members(value, bytes, &mut position, &mut nel, &mut seen)?;
  skip_ws(bytes, &mut position);
  if position != bytes.len() {
    return Err(invalid_json());
  }
  if !seen.contains("max_age") {
    return Err(NelParseError::new("NEL policy is missing max_age"));
  }
  Ok(nel)
}

fn parse_members(
  value: &str,
  bytes: &[u8],
  position: &mut usize,
  nel: &mut Nel,
  seen: &mut HashSet<&'static str>,
) -> Result<(), NelParseError> {
  skip_ws(bytes, position);
  if bytes.get(*position) == Some(&b'}') {
    *position += 1;
    return Ok(());
  }
  let mut member_count = 0;
  loop {
    member_count += 1;
    if member_count > MAX_NEL_MEMBERS {
      return Err(NelParseError::new("NEL policy has too many members"));
    }
    skip_ws(bytes, position);
    let name = parse_string(bytes, position)?;
    skip_ws(bytes, position);
    if bytes.get(*position) != Some(&b':') {
      return Err(invalid_json());
    }
    *position += 1;
    skip_ws(bytes, position);
    match name.as_str() {
      "report_to" => {
        if !seen.insert("report_to") {
          return Err(NelParseError::new("duplicate NEL member report_to"));
        }
        if bytes.get(*position) != Some(&b'"') {
          return Err(NelParseError::new("NEL member report_to must be a string"));
        }
        nel.report_to = Some(parse_string(bytes, position)?);
      }
      "max_age" => {
        if !seen.insert("max_age") {
          return Err(NelParseError::new("duplicate NEL member max_age"));
        }
        let raw = parse_number(value, bytes, position)?;
        if !raw.bytes().all(|byte| byte.is_ascii_digit()) {
          return Err(NelParseError::new(
            "NEL member max_age must be a non-negative integer",
          ));
        }
        nel.max_age = raw
          .parse::<u64>()
          .map_err(|_| NelParseError::new("NEL member max_age is out of range"))?;
      }
      "include_subdomains" => {
        if !seen.insert("include_subdomains") {
          return Err(NelParseError::new(
            "duplicate NEL member include_subdomains",
          ));
        }
        nel.include_subdomains = Some(parse_bool(bytes, position)?);
      }
      "success_fraction" => {
        if !seen.insert("success_fraction") {
          return Err(NelParseError::new("duplicate NEL member success_fraction"));
        }
        let raw = parse_number(value, bytes, position)?;
        nel.success_fraction = Some(parse_fraction(raw, "success_fraction")?);
      }
      "failure_fraction" => {
        if !seen.insert("failure_fraction") {
          return Err(NelParseError::new("duplicate NEL member failure_fraction"));
        }
        let raw = parse_number(value, bytes, position)?;
        nel.failure_fraction = Some(parse_fraction(raw, "failure_fraction")?);
      }
      _ => {
        let start = *position;
        validate_value(value, bytes, position, 1)?;
        nel.unknown_members.push(NelUnknownMember {
          name,
          value: value[start..*position].to_string(),
        });
      }
    }
    skip_ws(bytes, position);
    match bytes.get(*position) {
      Some(b',') => {
        *position += 1;
      }
      Some(b'}') => {
        *position += 1;
        return Ok(());
      }
      _ => return Err(invalid_json()),
    }
  }
}

fn parse_bool(bytes: &[u8], position: &mut usize) -> Result<bool, NelParseError> {
  if bytes.get(*position..*position + 4) == Some(b"true".as_slice()) {
    *position += 4;
    Ok(true)
  } else if bytes.get(*position..*position + 5) == Some(b"false".as_slice()) {
    *position += 5;
    Ok(false)
  } else {
    Err(NelParseError::new(
      "NEL member include_subdomains must be a boolean",
    ))
  }
}

fn parse_fraction(raw: &str, member: &str) -> Result<f64, NelParseError> {
  let fraction = raw
    .parse::<f64>()
    .map_err(|_| NelParseError::new(format!("NEL member {member} must be a number")))?;
  if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
    return Err(NelParseError::new(format!(
      "NEL member {member} must be in the inclusive range 0.0 to 1.0"
    )));
  }
  Ok(fraction)
}

fn validate_value(
  value: &str,
  bytes: &[u8],
  position: &mut usize,
  depth: usize,
) -> Result<(), NelParseError> {
  if depth > MAX_NEL_DEPTH {
    return Err(NelParseError::new("NEL JSON is too deeply nested"));
  }
  match bytes.get(*position) {
    Some(b'{') => {
      *position += 1;
      validate_object(value, bytes, position, depth)?;
    }
    Some(b'[') => {
      *position += 1;
      validate_array(value, bytes, position, depth)?;
    }
    Some(b'"') => {
      parse_string(bytes, position)?;
    }
    Some(b't') => {
      if bytes.get(*position..*position + 4) != Some(b"true".as_slice()) {
        return Err(invalid_json());
      }
      *position += 4;
    }
    Some(b'f') => {
      if bytes.get(*position..*position + 5) != Some(b"false".as_slice()) {
        return Err(invalid_json());
      }
      *position += 5;
    }
    Some(b'n') => {
      if bytes.get(*position..*position + 4) != Some(b"null".as_slice()) {
        return Err(invalid_json());
      }
      *position += 4;
    }
    Some(b'-' | b'0'..=b'9') => {
      parse_number(value, bytes, position)?;
    }
    _ => return Err(invalid_json()),
  }
  Ok(())
}

fn validate_object(
  value: &str,
  bytes: &[u8],
  position: &mut usize,
  depth: usize,
) -> Result<(), NelParseError> {
  skip_ws(bytes, position);
  if bytes.get(*position) == Some(&b'}') {
    *position += 1;
    return Ok(());
  }
  let mut member_count = 0;
  loop {
    member_count += 1;
    if member_count > MAX_NEL_MEMBERS {
      return Err(NelParseError::new("NEL JSON object has too many members"));
    }
    skip_ws(bytes, position);
    parse_string(bytes, position)?;
    skip_ws(bytes, position);
    if bytes.get(*position) != Some(&b':') {
      return Err(invalid_json());
    }
    *position += 1;
    skip_ws(bytes, position);
    validate_value(value, bytes, position, depth + 1)?;
    skip_ws(bytes, position);
    match bytes.get(*position) {
      Some(b',') => {
        *position += 1;
      }
      Some(b'}') => {
        *position += 1;
        return Ok(());
      }
      _ => return Err(invalid_json()),
    }
  }
}

fn validate_array(
  value: &str,
  bytes: &[u8],
  position: &mut usize,
  depth: usize,
) -> Result<(), NelParseError> {
  skip_ws(bytes, position);
  if bytes.get(*position) == Some(&b']') {
    *position += 1;
    return Ok(());
  }
  let mut member_count = 0;
  loop {
    member_count += 1;
    if member_count > MAX_NEL_MEMBERS {
      return Err(NelParseError::new("NEL JSON array has too many members"));
    }
    skip_ws(bytes, position);
    validate_value(value, bytes, position, depth + 1)?;
    skip_ws(bytes, position);
    match bytes.get(*position) {
      Some(b',') => {
        *position += 1;
      }
      Some(b']') => {
        *position += 1;
        return Ok(());
      }
      _ => return Err(invalid_json()),
    }
  }
}

fn parse_number<'a>(
  value: &'a str,
  bytes: &[u8],
  position: &mut usize,
) -> Result<&'a str, NelParseError> {
  let start = *position;
  if bytes.get(*position) == Some(&b'-') {
    *position += 1;
  }
  match bytes.get(*position) {
    Some(b'0') => {
      *position += 1;
      if matches!(bytes.get(*position), Some(b'0'..=b'9')) {
        return Err(invalid_json());
      }
    }
    Some(b'1'..=b'9') => {
      while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
        *position += 1;
      }
    }
    _ => return Err(invalid_json()),
  }
  if bytes.get(*position) == Some(&b'.') {
    *position += 1;
    if !matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      return Err(invalid_json());
    }
    while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      *position += 1;
    }
  }
  if matches!(bytes.get(*position), Some(b'e' | b'E')) {
    *position += 1;
    if matches!(bytes.get(*position), Some(b'+' | b'-')) {
      *position += 1;
    }
    if !matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      return Err(invalid_json());
    }
    while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
      *position += 1;
    }
  }
  Ok(&value[start..*position])
}

fn parse_string(bytes: &[u8], position: &mut usize) -> Result<String, NelParseError> {
  if bytes.get(*position) != Some(&b'"') {
    return Err(invalid_json());
  }
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = bytes.get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        if parsed.len() > MAX_NEL_STRING_BYTES {
          return Err(NelParseError::new("NEL string is too large"));
        }
        return String::from_utf8(parsed).map_err(|_| invalid_json());
      }
      b'\\' => {
        let Some(&escaped) = bytes.get(*position) else {
          return Err(invalid_json());
        };
        *position += 1;
        match escaped {
          b'"' => parsed.push(b'"'),
          b'\\' => parsed.push(b'\\'),
          b'/' => parsed.push(b'/'),
          b'b' => parsed.push(0x08),
          b'f' => parsed.push(0x0C),
          b'n' => parsed.push(b'\n'),
          b'r' => parsed.push(b'\r'),
          b't' => parsed.push(b'\t'),
          b'u' => parse_unicode_escape(bytes, position, &mut parsed)?,
          _ => return Err(invalid_json()),
        }
        if parsed.len() > MAX_NEL_STRING_BYTES {
          return Err(NelParseError::new("NEL string is too large"));
        }
      }
      byte if byte < 0x20 => return Err(invalid_json()),
      _ => parsed.push(byte),
    }
  }
  Err(invalid_json())
}

fn parse_unicode_escape(
  bytes: &[u8],
  position: &mut usize,
  parsed: &mut Vec<u8>,
) -> Result<(), NelParseError> {
  let first = parse_hex4(bytes, position)?;
  if (0xD800..=0xDBFF).contains(&first) {
    if bytes.get(*position) != Some(&b'\\') || bytes.get(*position + 1) != Some(&b'u') {
      return Err(invalid_json());
    }
    *position += 2;
    let second = parse_hex4(bytes, position)?;
    if !(0xDC00..=0xDFFF).contains(&second) {
      return Err(invalid_json());
    }
    let codepoint = 0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00);
    push_codepoint(codepoint, parsed);
    Ok(())
  } else if (0xDC00..=0xDFFF).contains(&first) {
    Err(invalid_json())
  } else {
    push_codepoint(first, parsed);
    Ok(())
  }
}

fn parse_hex4(bytes: &[u8], position: &mut usize) -> Result<u32, NelParseError> {
  let mut value = 0u32;
  for _ in 0..4 {
    let Some(&byte) = bytes.get(*position) else {
      return Err(invalid_json());
    };
    *position += 1;
    let digit = match byte {
      b'0'..=b'9' => byte - b'0',
      b'a'..=b'f' => byte - b'a' + 10,
      b'A'..=b'F' => byte - b'A' + 10,
      _ => return Err(invalid_json()),
    };
    value = value * 16 + u32::from(digit);
  }
  Ok(value)
}

fn push_codepoint(codepoint: u32, parsed: &mut Vec<u8>) {
  if codepoint < 0x80 {
    parsed.push(codepoint as u8);
  } else if codepoint < 0x800 {
    parsed.push(0xC0 | (codepoint >> 6) as u8);
    parsed.push(0x80 | (codepoint & 0x3F) as u8);
  } else if codepoint < 0x1_0000 {
    parsed.push(0xE0 | (codepoint >> 12) as u8);
    parsed.push(0x80 | ((codepoint >> 6) & 0x3F) as u8);
    parsed.push(0x80 | (codepoint & 0x3F) as u8);
  } else {
    parsed.push(0xF0 | (codepoint >> 18) as u8);
    parsed.push(0x80 | ((codepoint >> 12) & 0x3F) as u8);
    parsed.push(0x80 | ((codepoint >> 6) & 0x3F) as u8);
    parsed.push(0x80 | (codepoint & 0x3F) as u8);
  }
}

fn skip_ws(bytes: &[u8], position: &mut usize) {
  while matches!(bytes.get(*position), Some(b' ' | b'\t' | b'\n' | b'\r')) {
    *position += 1;
  }
}

fn push_json_string(out: &mut String, value: &str) {
  out.push('"');
  for ch in value.chars() {
    match ch {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '\u{08}' => out.push_str("\\b"),
      '\u{0C}' => out.push_str("\\f"),
      ch if (ch as u32) < 0x20 || ch == '\u{7f}' => {
        out.push_str(&format!("\\u{:04x}", ch as u32));
      }
      ch => out.push(ch),
    }
  }
  out.push('"');
}

fn format_fraction(fraction: f64) -> String {
  format!("{fraction}")
}

fn invalid_json() -> NelParseError {
  NelParseError::new("invalid NEL JSON")
}

fn invalid_value() -> NelParseError {
  NelParseError::new("invalid NEL header value")
}

#[cfg(test)]
mod tests {
  use super::parse_string;
  use super::Nel;
  use super::NelParseError;
  use super::MAX_NEL_STRING_BYTES;

  #[test]
  fn string_bound_applies_independently_of_value_bound() {
    let mut position = 0;
    let oversized = format!("\"{}\"", "x".repeat(MAX_NEL_STRING_BYTES + 1));
    let result: Result<String, NelParseError> = parse_string(oversized.as_bytes(), &mut position);
    assert!(result.is_err(), "oversized string must be rejected");
  }

  #[test]
  fn absent_optional_members_keep_spec_defaults_without_reemission() {
    let nel = Nel::parse(r#"{"max_age": 2592000}"#).expect("minimal policy should parse");
    assert_eq!(nel.include_subdomains(), None);
    assert_eq!(nel.success_fraction(), None);
    assert_eq!(nel.failure_fraction(), None);
    assert_eq!(nel.header_value(), r#"{"max_age":2592000}"#);
  }
}
