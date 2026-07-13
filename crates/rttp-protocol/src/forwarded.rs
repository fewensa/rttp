use std::error::Error;
use std::fmt;

pub const MAX_FORWARDED_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_FORWARDED_ELEMENTS: usize = 256;
pub const MAX_FORWARDED_PARAMETERS: usize = 32;

/// Parsed, bounded RFC 7239 `Forwarded` request metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Forwarded {
  elements: Vec<ForwardedElement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardedElement {
  parameters: Vec<ForwardedParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardedParameter {
  name: String,
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardedParseError {
  message: String,
}

impl ForwardedParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ForwardedParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ForwardedParseError {}

impl Forwarded {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ForwardedParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ForwardedParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut elements = Vec::new();
    for value in values {
      if value.len() > MAX_FORWARDED_VALUE_BYTES {
        return Err(ForwardedParseError::new(
          "Forwarded header value is too large",
        ));
      }
      parse_field(value, &mut elements)?;
    }
    if elements.is_empty() {
      return Err(ForwardedParseError::new("invalid Forwarded element"));
    }
    Ok(Self { elements })
  }

  pub fn elements(&self) -> &[ForwardedElement] {
    &self.elements
  }

  pub fn len(&self) -> usize {
    self.elements.len()
  }

  pub fn is_empty(&self) -> bool {
    self.elements.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .elements
      .iter()
      .map(ForwardedElement::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ForwardedElement {
  pub fn parameters(&self) -> &[ForwardedParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|parameter| parameter.value.as_str())
  }

  pub fn for_value(&self) -> Option<&str> {
    self.parameter("for")
  }

  pub fn by(&self) -> Option<&str> {
    self.parameter("by")
  }

  pub fn host(&self) -> Option<&str> {
    self.parameter("host")
  }

  pub fn proto(&self) -> Option<&str> {
    self.parameter("proto")
  }

  fn header_value(&self) -> String {
    self
      .parameters
      .iter()
      .map(ForwardedParameter::header_value)
      .collect::<Vec<_>>()
      .join("; ")
  }
}

impl ForwardedParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }

  fn header_value(&self) -> String {
    if is_token(&self.value) {
      format!("{}={}", self.name, self.value)
    } else {
      format!(
        "{}=\"{}\"",
        self.name,
        self.value.replace('\\', "\\\\").replace('"', "\\\"")
      )
    }
  }
}

fn parse_field(
  value: &str,
  elements: &mut Vec<ForwardedElement>,
) -> Result<(), ForwardedParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ForwardedParseError::new("invalid Forwarded element"));
  }

  loop {
    if elements.len() >= MAX_FORWARDED_ELEMENTS {
      return Err(ForwardedParseError::new("too many Forwarded elements"));
    }
    let mut parameters = Vec::new();
    parse_element(value, &mut position, &mut parameters)?;
    elements.push(ForwardedElement { parameters });
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(ForwardedParseError::new("invalid Forwarded element"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ForwardedParseError::new("invalid Forwarded element"));
    }
  }
}

fn parse_element(
  value: &str,
  position: &mut usize,
  parameters: &mut Vec<ForwardedParameter>,
) -> Result<(), ForwardedParseError> {
  loop {
    let name =
      parse_token(value, position, "invalid Forwarded parameter name")?.to_ascii_lowercase();
    skip_ows(value.as_bytes(), position);
    if value.as_bytes().get(*position) != Some(&b'=') {
      return Err(ForwardedParseError::new("invalid Forwarded parameter"));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    let parameter_value = parse_value(value, position)?;
    if parameters
      .iter()
      .any(|known: &ForwardedParameter| known.name.eq_ignore_ascii_case(&name))
    {
      return Err(ForwardedParseError::new("duplicate Forwarded parameter"));
    }
    if parameters.len() >= MAX_FORWARDED_PARAMETERS {
      return Err(ForwardedParseError::new("too many Forwarded parameters"));
    }
    parameters.push(ForwardedParameter {
      name,
      value: parameter_value,
    });
    skip_ows(value.as_bytes(), position);
    match value.as_bytes().get(*position) {
      Some(b';') => {
        *position += 1;
        skip_ows(value.as_bytes(), position);
        if *position == value.len() {
          return Err(ForwardedParseError::new("invalid Forwarded parameter"));
        }
      }
      Some(b',') | None => return Ok(()),
      _ => return Err(ForwardedParseError::new("invalid Forwarded parameter")),
    }
  }
}

fn parse_value(value: &str, position: &mut usize) -> Result<String, ForwardedParseError> {
  if value.as_bytes().get(*position) != Some(&b'"') {
    return Ok(parse_token(value, position, "invalid Forwarded parameter value")?.to_string());
  }

  *position += 1;
  let mut parsed = String::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => return Ok(parsed),
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(ForwardedParseError::new("invalid Forwarded quoted-string"));
        };
        if !(escaped == b'\t' || (0x20..=0x7e).contains(&escaped)) {
          return Err(ForwardedParseError::new("invalid Forwarded quoted-string"));
        }
        *position += 1;
        parsed.push(escaped as char);
      }
      b'\t' | 0x20..=0x7e => parsed.push(byte as char),
      _ => return Err(ForwardedParseError::new("invalid Forwarded quoted-string")),
    }
  }
  Err(ForwardedParseError::new("invalid Forwarded quoted-string"))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, ForwardedParseError> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(ForwardedParseError::new(message))
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
