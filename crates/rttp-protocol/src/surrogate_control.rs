use std::collections::HashSet;
use std::error::Error;
use std::fmt;

pub const MAX_SURROGATE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SURROGATE_CONTROL_AGGREGATE_BYTES: usize = 64 * 1024;
pub const MAX_SURROGATE_CONTROL_DIRECTIVES: usize = 256;
pub const MAX_SURROGATE_CONTROL_DIRECTIVE_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Surrogate-Control` response metadata.
///
/// This exposes surrogate cache directives without applying CDN cache policy or
/// translating directives into `Cache-Control`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurrogateControl {
  directives: Vec<SurrogateControlDirective>,
}

/// A `Surrogate-Control` directive, including extension directives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurrogateControlDirective {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurrogateControlParseError {
  message: String,
}

impl SurrogateControlParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for SurrogateControlParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for SurrogateControlParseError {}

impl SurrogateControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, SurrogateControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, SurrogateControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut directives = Vec::new();
    let mut names = HashSet::new();
    let mut aggregate_size = 0usize;
    for value in values {
      if value.len() > MAX_SURROGATE_CONTROL_VALUE_BYTES {
        return Err(SurrogateControlParseError::new(
          "Surrogate-Control header value is too large",
        ));
      }
      aggregate_size = aggregate_size.checked_add(value.len()).ok_or_else(|| {
        SurrogateControlParseError::new("Surrogate-Control header set is too large")
      })?;
      if aggregate_size > MAX_SURROGATE_CONTROL_AGGREGATE_BYTES {
        return Err(SurrogateControlParseError::new(
          "Surrogate-Control header set is too large",
        ));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(SurrogateControlParseError::new(
          "invalid Surrogate-Control control byte",
        ));
      }
      parse_field(value, &mut directives, &mut names)?;
    }
    if directives.is_empty() {
      return Err(SurrogateControlParseError::new(
        "invalid Surrogate-Control directive",
      ));
    }
    Ok(Self { directives })
  }

  pub fn directives(&self) -> &[SurrogateControlDirective] {
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
      .map(SurrogateControlDirective::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl SurrogateControlDirective {
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
  directives: &mut Vec<SurrogateControlDirective>,
  names: &mut HashSet<String>,
) -> Result<(), SurrogateControlParseError> {
  let bytes = value.as_bytes();
  let mut position = 0usize;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(SurrogateControlParseError::new(
      "invalid Surrogate-Control directive",
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
    if directives.len() >= MAX_SURROGATE_CONTROL_DIRECTIVES {
      return Err(SurrogateControlParseError::new(
        "too many Surrogate-Control directives",
      ));
    }
    let directive = parse_directive(value, &mut position)?;
    let normalized_name = directive.name.to_ascii_lowercase();
    if !names.insert(normalized_name) {
      return Err(SurrogateControlParseError::new(
        "duplicate Surrogate-Control directive",
      ));
    }
    directives.push(directive);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(SurrogateControlParseError::new(
        "invalid Surrogate-Control directive",
      ));
    }
  }
}

fn parse_directive(
  value: &str,
  position: &mut usize,
) -> Result<SurrogateControlDirective, SurrogateControlParseError> {
  let name = parse_token(value, position, "invalid Surrogate-Control directive")?.to_string();
  skip_ows(value.as_bytes(), position);
  let value = if value.as_bytes().get(*position) == Some(&b'=') {
    *position += 1;
    skip_ows(value.as_bytes(), position);
    Some(parse_directive_value(value, position)?)
  } else {
    None
  };
  Ok(SurrogateControlDirective { name, value })
}

fn parse_directive_value(
  value: &str,
  position: &mut usize,
) -> Result<String, SurrogateControlParseError> {
  let parsed = if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)?
  } else {
    parse_token(value, position, "invalid Surrogate-Control directive value")?.to_string()
  };
  if parsed.len() > MAX_SURROGATE_CONTROL_DIRECTIVE_VALUE_BYTES {
    return Err(SurrogateControlParseError::new(
      "Surrogate-Control directive value is too large",
    ));
  }
  Ok(parsed)
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
) -> Result<String, SurrogateControlParseError> {
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed).map_err(|_| {
          SurrogateControlParseError::new("malformed Surrogate-Control quoted-string")
        });
      }
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(SurrogateControlParseError::new(
            "malformed Surrogate-Control quoted-string",
          ));
        };
        if !is_quoted_pair_byte(escaped) {
          return Err(SurrogateControlParseError::new(
            "malformed Surrogate-Control quoted-string",
          ));
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_quoted_text_byte(byte) => parsed.push(byte),
      _ => {
        return Err(SurrogateControlParseError::new(
          "malformed Surrogate-Control quoted-string",
        ))
      }
    }
  }
  Err(SurrogateControlParseError::new(
    "malformed Surrogate-Control quoted-string",
  ))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, SurrogateControlParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(SurrogateControlParseError::new(message))
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
