#[cfg(test)]
mod tests {
  use super::{
    CacheControl, MAX_CACHE_CONTROL_DIRECTIVES, MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES,
    MAX_CACHE_CONTROL_VALUE_BYTES,
  };

  #[test]
  fn parses_token_and_quoted_directives_and_preserves_extensions() {
    let cache_control =
      CacheControl::parse("max-age=60, community=token, example=\"quoted, value\\\"\" , immutable")
        .expect("Cache-Control should parse");

    assert_eq!(cache_control.len(), 4);
    assert_eq!(cache_control.directives()[0].name(), "max-age");
    assert_eq!(cache_control.directives()[0].value(), Some("60"));
    assert_eq!(cache_control.directives()[1].name(), "community");
    assert_eq!(cache_control.directives()[1].value(), Some("token"));
    assert_eq!(cache_control.directives()[2].name(), "example");
    assert_eq!(
      cache_control.directives()[2].value(),
      Some("quoted, value\"")
    );
    assert_eq!(cache_control.directives()[3].name(), "immutable");
    assert_eq!(cache_control.directives()[3].value(), None);
    assert_eq!(
      cache_control.header_value(),
      "max-age=60, community=token, example=\"quoted, value\\\"\", immutable"
    );
  }

  #[test]
  fn combines_multiple_field_values() {
    let cache_control = CacheControl::parse_values(["no-store", "custom=enabled"])
      .expect("Cache-Control should parse");

    assert_eq!(cache_control.len(), 2);
    assert_eq!(cache_control.directives()[1].name(), "custom");
  }

  #[test]
  fn ignores_empty_list_members() {
    let cache_control = CacheControl::parse("public,, max-age=60,")
      .expect("Cache-Control should ignore empty list members");

    assert_eq!(cache_control.len(), 2);
    assert_eq!(cache_control.directives()[0].name(), "public");
    assert_eq!(cache_control.directives()[1].name(), "max-age");
    assert_eq!(cache_control.directives()[1].value(), Some("60"));
  }

  #[test]
  fn rejects_invalid_syntax_and_control_bytes() {
    for value in [
      "max-age=",
      "max-age=not a token",
      "custom=\"unterminated",
      "custom=\"invalid\\\x01\"",
      "max-age=60\r\nno-store",
    ] {
      assert!(CacheControl::parse(value).is_err(), "{value:?} should fail");
    }
  }

  #[test]
  fn enforces_value_directive_and_directive_value_limits() {
    assert!(CacheControl::parse("x".repeat(MAX_CACHE_CONTROL_VALUE_BYTES + 1)).is_err());
    assert!(CacheControl::parse(format!(
      "x={}",
      "x".repeat(MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES + 1)
    ))
    .is_err());
    assert!(CacheControl::parse(
      std::iter::repeat_n("x", MAX_CACHE_CONTROL_DIRECTIVES + 1)
        .collect::<Vec<_>>()
        .join(","),
    )
    .is_err());
  }
}

use std::error::Error;
use std::fmt;

pub const MAX_CACHE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CACHE_CONTROL_DIRECTIVES: usize = 256;
pub const MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded HTTP `Cache-Control` metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheControl {
  directives: Vec<CacheControlDirective>,
}

/// A `Cache-Control` directive, including extension directives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheControlDirective {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheControlParseError {
  message: String,
}

impl CacheControlParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CacheControlParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CacheControlParseError {}

impl CacheControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CacheControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CacheControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    for value in values {
      if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
        return Err(CacheControlParseError::new(
          "Cache-Control header value is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(CacheControlParseError::new(
          "invalid Cache-Control control byte",
        ));
      }
      parse_field(value, &mut directives)?;
    }
    if directives.is_empty() {
      return Err(CacheControlParseError::new(
        "invalid Cache-Control directive",
      ));
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[CacheControlDirective] {
    &self.directives
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
      .map(CacheControlDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl CacheControlDirective {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  fn header_value(&self) -> String {
    match &self.value {
      None => self.name.clone(),
      Some(value) if is_token(value) => format!("{}={value}", self.name),
      Some(value) => format!("{}=\"{}\"", self.name, escape_quoted(value)),
    }
  }
}

fn parse_field(
  value: &str,
  directives: &mut Vec<CacheControlDirective>,
) -> Result<(), CacheControlParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(CacheControlParseError::new(
      "invalid Cache-Control directive",
    ));
  }

  loop {
    skip_ows(bytes, &mut position);
    while bytes.get(position) == Some(&b',') {
      position += 1;
      skip_ows(bytes, &mut position);
    }
    if position == bytes.len() {
      return Ok(());
    }
    if directives.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
      return Err(CacheControlParseError::new(
        "too many Cache-Control directives",
      ));
    }
    directives.push(parse_directive(value, &mut position)?);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(CacheControlParseError::new(
        "invalid Cache-Control directive",
      ));
    }
  }
}

fn parse_directive(
  value: &str,
  position: &mut usize,
) -> Result<CacheControlDirective, CacheControlParseError> {
  let name = parse_token(value, position, "invalid Cache-Control directive")?.to_string();
  skip_ows(value.as_bytes(), position);
  let value = if value.as_bytes().get(*position) == Some(&b'=') {
    *position += 1;
    skip_ows(value.as_bytes(), position);
    Some(parse_directive_value(value, position)?)
  } else {
    None
  };
  Ok(CacheControlDirective { name, value })
}

fn parse_directive_value(
  value: &str,
  position: &mut usize,
) -> Result<String, CacheControlParseError> {
  let parsed = if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)?
  } else {
    parse_token(value, position, "invalid Cache-Control directive value")?.to_string()
  };
  if parsed.len() > MAX_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES {
    return Err(CacheControlParseError::new(
      "Cache-Control directive value is too large",
    ));
  }
  Ok(parsed)
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
) -> Result<String, CacheControlParseError> {
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed)
          .map_err(|_| CacheControlParseError::new("malformed Cache-Control quoted-string"));
      }
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(CacheControlParseError::new(
            "malformed Cache-Control quoted-string",
          ));
        };
        if !is_quoted_pair_byte(escaped) {
          return Err(CacheControlParseError::new(
            "malformed Cache-Control quoted-string",
          ));
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_quoted_text_byte(byte) => parsed.push(byte),
      _ => {
        return Err(CacheControlParseError::new(
          "malformed Cache-Control quoted-string",
        ))
      }
    }
  }
  Err(CacheControlParseError::new(
    "malformed Cache-Control quoted-string",
  ))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, CacheControlParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(CacheControlParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while bytes
    .get(*position)
    .is_some_and(|byte| matches!(*byte, b' ' | b'\t'))
  {
    *position += 1;
  }
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!'
        | b'#'
        | b'$'
        | b'%'
        | b'&'
        | b'\''
        | b'*'
        | b'+'
        | b'-'
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~'
    )
}

fn is_quoted_text_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
}

fn is_quoted_pair_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x7e | 0x80..=0xff)
}

fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}
