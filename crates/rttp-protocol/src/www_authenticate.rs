use std::error::Error;
use std::fmt;

pub const MAX_WWW_AUTHENTICATE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_WWW_AUTHENTICATE_CHALLENGES: usize = 256;
pub const MAX_WWW_AUTHENTICATE_PARAMETERS: usize = 256;
pub const MAX_WWW_AUTHENTICATE_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `WWW-Authenticate` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WwwAuthenticate {
  challenges: Vec<WwwAuthenticateChallenge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WwwAuthenticateChallenge {
  scheme: String,
  token68: Option<String>,
  parameters: Vec<WwwAuthenticateParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WwwAuthenticateParameter {
  name: String,
  value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WwwAuthenticateParseError {
  message: String,
}

impl WwwAuthenticateParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for WwwAuthenticateParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for WwwAuthenticateParseError {}

impl WwwAuthenticate {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, WwwAuthenticateParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, WwwAuthenticateParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut challenges = Vec::new();
    for value in values {
      if value.len() > MAX_WWW_AUTHENTICATE_VALUE_BYTES {
        return Err(WwwAuthenticateParseError::new(
          "WWW-Authenticate header value is too large",
        ));
      }
      parse_field(value, &mut challenges)?;
    }
    if challenges.is_empty() {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate challenge",
      ));
    }
    Ok(Self { challenges })
  }

  pub fn challenges(&self) -> &[WwwAuthenticateChallenge] {
    &self.challenges
  }

  pub fn len(&self) -> usize {
    self.challenges.len()
  }

  pub fn is_empty(&self) -> bool {
    self.challenges.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .challenges
      .iter()
      .map(WwwAuthenticateChallenge::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl WwwAuthenticateChallenge {
  pub fn scheme(&self) -> &str {
    &self.scheme
  }
  pub fn token68(&self) -> Option<&str> {
    self.token68.as_deref()
  }
  pub fn parameters(&self) -> &[WwwAuthenticateParameter] {
    &self.parameters
  }
  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|parameter| parameter.value.as_str())
  }
  fn header_value(&self) -> String {
    let mut value = self.scheme.clone();
    if let Some(token68) = &self.token68 {
      value.push(' ');
      value.push_str(token68);
    } else if !self.parameters.is_empty() {
      value.push(' ');
      value.push_str(
        &self
          .parameters
          .iter()
          .map(WwwAuthenticateParameter::header_value)
          .collect::<Vec<_>>()
          .join(", "),
      );
    }
    value
  }
}

impl WwwAuthenticateParameter {
  pub fn name(&self) -> &str {
    &self.name
  }
  pub fn value(&self) -> &str {
    &self.value
  }
  fn header_value(&self) -> String {
    if !self.name.eq_ignore_ascii_case("realm") && is_token(&self.value) {
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
  challenges: &mut Vec<WwwAuthenticateChallenge>,
) -> Result<(), WwwAuthenticateParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(WwwAuthenticateParseError::new(
      "invalid WWW-Authenticate challenge",
    ));
  }
  while position < bytes.len() {
    let scheme = parse_token(value, &mut position, "invalid WWW-Authenticate scheme")?.to_string();
    if challenges.len() >= MAX_WWW_AUTHENTICATE_CHALLENGES {
      return Err(WwwAuthenticateParseError::new(
        "too many WWW-Authenticate challenges",
      ));
    }
    let before_whitespace = position;
    skip_ows(bytes, &mut position);
    let mut challenge = WwwAuthenticateChallenge {
      scheme,
      token68: None,
      parameters: Vec::new(),
    };
    if position < bytes.len() && bytes[position] != b',' {
      if position == before_whitespace {
        return Err(WwwAuthenticateParseError::new(
          "invalid WWW-Authenticate challenge",
        ));
      }
      if let Some(token68_end) = token68_end(value, position) {
        let token68 = &value[position..token68_end];
        challenge.token68 = Some(token68.to_string());
        position = token68_end;
        skip_ows(bytes, &mut position);
      } else if looks_like_parameter(value, position) {
        parse_parameters(value, &mut position, &mut challenge.parameters)?;
      } else {
        let start = position;
        while position < bytes.len() && bytes[position] != b',' && !is_ows(bytes[position]) {
          position += 1;
        }
        let token68 = &value[start..position];
        if !is_token68(token68) {
          return Err(WwwAuthenticateParseError::new(
            "invalid WWW-Authenticate token68",
          ));
        }
        challenge.token68 = Some(token68.to_string());
        skip_ows(bytes, &mut position);
      }
    }
    challenges.push(challenge);
    if position == bytes.len() {
      break;
    }
    if bytes[position] != b',' {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate challenge",
      ));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate challenge",
      ));
    }
  }
  Ok(())
}

fn parse_parameters(
  value: &str,
  position: &mut usize,
  parameters: &mut Vec<WwwAuthenticateParameter>,
) -> Result<(), WwwAuthenticateParseError> {
  loop {
    let name =
      parse_token(value, position, "invalid WWW-Authenticate parameter name")?.to_ascii_lowercase();
    skip_ows(value.as_bytes(), position);
    if value.as_bytes().get(*position) != Some(&b'=') {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate parameter",
      ));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    let parameter_value = parse_parameter_value(value, position)?;
    if parameter_value.len() > MAX_WWW_AUTHENTICATE_PARAMETER_VALUE_BYTES {
      return Err(WwwAuthenticateParseError::new(
        "WWW-Authenticate parameter value is too large",
      ));
    }
    if parameters
      .iter()
      .any(|known| known.name.eq_ignore_ascii_case(&name))
    {
      return Err(WwwAuthenticateParseError::new(
        "duplicate WWW-Authenticate parameter",
      ));
    }
    if parameters.len() >= MAX_WWW_AUTHENTICATE_PARAMETERS {
      return Err(WwwAuthenticateParseError::new(
        "too many WWW-Authenticate parameters",
      ));
    }
    parameters.push(WwwAuthenticateParameter {
      name,
      value: parameter_value,
    });
    skip_ows(value.as_bytes(), position);
    if *position == value.len() {
      return Ok(());
    }
    if value.as_bytes()[*position] != b',' {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate parameter",
      ));
    }
    let comma = *position;
    *position += 1;
    skip_ows(value.as_bytes(), position);
    if *position == value.len() {
      return Err(WwwAuthenticateParseError::new(
        "invalid WWW-Authenticate parameter",
      ));
    }
    if !looks_like_parameter(value, *position) {
      *position = comma;
      return Ok(());
    }
  }
}

fn token68_end(value: &str, mut position: usize) -> Option<usize> {
  let bytes = value.as_bytes();
  let start = position;
  while position < bytes.len() && bytes[position] != b',' && !is_ows(bytes[position]) {
    position += 1;
  }
  is_token68(&value[start..position]).then_some(position)
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<String, WwwAuthenticateParseError> {
  let bytes = value.as_bytes();
  if bytes.get(*position) == Some(&b'"') {
    *position += 1;
    let mut parsed = String::new();
    let mut unescaped_start = *position;
    let mut escaped = false;
    while *position < bytes.len() {
      let byte = bytes[*position];
      if escaped {
        *position += 1;
        if !(byte == b'\t' || (0x20..=0x7e).contains(&byte)) {
          return Err(WwwAuthenticateParseError::new(
            "invalid WWW-Authenticate quoted-string",
          ));
        }
        parsed.push(byte as char);
        escaped = false;
        unescaped_start = *position;
      } else if byte == b'\\' {
        parsed.push_str(&value[unescaped_start..*position]);
        *position += 1;
        escaped = true;
      } else if byte == b'"' {
        parsed.push_str(&value[unescaped_start..*position]);
        *position += 1;
        return Ok(parsed);
      } else if !(byte == b'\t'
        || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff))
      {
        return Err(WwwAuthenticateParseError::new(
          "invalid WWW-Authenticate quoted-string",
        ));
      } else {
        *position += 1;
      }
    }
    Err(WwwAuthenticateParseError::new(
      "invalid WWW-Authenticate quoted-string",
    ))
  } else {
    Ok(parse_token(value, position, "invalid WWW-Authenticate parameter value")?.to_string())
  }
}

fn looks_like_parameter(value: &str, mut position: usize) -> bool {
  let bytes = value.as_bytes();
  let start = position;
  while position < bytes.len() && is_token_byte(bytes[position]) {
    position += 1;
  }
  position > start && {
    skip_ows(bytes, &mut position);
    bytes.get(position) == Some(&b'=')
  }
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, WwwAuthenticateParseError> {
  let start = *position;
  let bytes = value.as_bytes();
  while *position < bytes.len() && is_token_byte(bytes[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(WwwAuthenticateParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while *position < bytes.len() && is_ows(bytes[*position]) {
    *position += 1;
  }
}
fn is_ows(byte: u8) -> bool {
  matches!(byte, b' ' | b'\t')
}
fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}
fn is_token68(value: &str) -> bool {
  let value = value.as_bytes();
  let mut base = 0;
  while base < value.len()
    && matches!(value[base], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'+' | b'/')
  {
    base += 1;
  }
  base > 0 && value[base..].iter().all(|byte| *byte == b'=')
}
fn is_token_byte(byte: u8) -> bool {
  matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}
