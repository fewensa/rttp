use std::error::Error;
use std::fmt;

pub const MAX_CLIENT_HINT_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_CLIENT_HINT_NAMES: usize = 256;
pub const MAX_DPR_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `DPR` request Client Hint metadata.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Dpr {
  value: String,
}

/// Parsed, bounded `Accept-CH` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptCh {
  client_hints: Vec<String>,
}

/// Parsed, bounded `Critical-CH` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CriticalCh {
  client_hints: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientHintsParseError {
  message: String,
}

pub type AcceptChParseError = ClientHintsParseError;
pub type CriticalChParseError = ClientHintsParseError;
pub type DprParseError = ClientHintsParseError;

impl ClientHintsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ClientHintsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ClientHintsParseError {}

impl Dpr {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, DprParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, DprParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let value = parse_dpr_singleton(values)?;
    let value = value.trim_matches([' ', '\t']);
    parse_dpr_ratio(value)?;
    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn ratio(&self) -> f64 {
    parse_dpr_ratio(&self.value).expect("DPR values are validated at construction")
  }

  pub fn header_value(&self) -> String {
    self.value.clone()
  }
}

impl AcceptCh {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, AcceptChParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, AcceptChParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      client_hints: parse_client_hints(values, "Accept-CH")?,
    })
  }

  pub fn client_hints(&self) -> &[String] {
    &self.client_hints
  }

  pub fn len(&self) -> usize {
    self.client_hints.len()
  }

  pub fn is_empty(&self) -> bool {
    self.client_hints.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.client_hints.join(", ")
  }
}

impl CriticalCh {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, CriticalChParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, CriticalChParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      client_hints: parse_client_hints(values, "Critical-CH")?,
    })
  }

  pub fn client_hints(&self) -> &[String] {
    &self.client_hints
  }

  pub fn len(&self) -> usize {
    self.client_hints.len()
  }

  pub fn is_empty(&self) -> bool {
    self.client_hints.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.client_hints.join(", ")
  }
}

fn parse_client_hints<'a, I>(
  values: I,
  header_name: &str,
) -> Result<Vec<String>, ClientHintsParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut client_hints = Vec::new();
  for value in values {
    if value.len() > MAX_CLIENT_HINT_VALUE_BYTES {
      return Err(ClientHintsParseError::new(format!(
        "{header_name} header value is too large"
      )));
    }
    for member in value.split(',') {
      let client_hint = member.trim();
      if !is_structured_token(client_hint) {
        return Err(ClientHintsParseError::new(format!(
          "invalid {header_name} client hint"
        )));
      }
      if client_hints.len() >= MAX_CLIENT_HINT_NAMES {
        return Err(ClientHintsParseError::new(format!(
          "too many {header_name} client hints"
        )));
      }
      client_hints.push(client_hint.to_string());
    }
  }
  if client_hints.is_empty() {
    return Err(ClientHintsParseError::new(format!(
      "invalid {header_name} client hint"
    )));
  }
  Ok(client_hints)
}

fn parse_dpr_singleton<'a, I>(values: I) -> Result<&'a str, DprParseError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let value = values.next().ok_or_else(invalid_dpr_value)?;
  validate_bounded_dpr_value(value)?;
  let mut has_duplicate = false;
  for value in values {
    has_duplicate = true;
    validate_bounded_dpr_value(value)?;
  }
  if has_duplicate {
    return Err(ClientHintsParseError::new("duplicate DPR header fields"));
  }
  Ok(value)
}

fn validate_bounded_dpr_value(value: &str) -> Result<(), DprParseError> {
  if value.len() > MAX_DPR_VALUE_BYTES {
    return Err(ClientHintsParseError::new("DPR header value is too large"));
  }
  if value
    .bytes()
    .any(|byte| byte.is_ascii_control() && byte != b'\t')
  {
    return Err(ClientHintsParseError::new("invalid DPR control byte"));
  }
  Ok(())
}

fn parse_dpr_ratio(value: &str) -> Result<f64, DprParseError> {
  if !matches_dpr_grammar(value) {
    return Err(invalid_dpr_value());
  }
  let ratio: f64 = value.parse().map_err(|_| invalid_dpr_value())?;
  if !ratio.is_finite() || ratio <= 0.0 {
    return Err(invalid_dpr_value());
  }
  Ok(ratio)
}

fn matches_dpr_grammar(value: &str) -> bool {
  let bytes = value.as_bytes();
  if bytes.is_empty() || !bytes[0].is_ascii_digit() {
    return false;
  }

  let mut index = 0;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  if index == bytes.len() {
    return true;
  }
  if bytes[index] != b'.' {
    return false;
  }
  index += 1;
  let fraction_start = index;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  index > fraction_start && index == bytes.len()
}

fn invalid_dpr_value() -> DprParseError {
  ClientHintsParseError::new("invalid DPR header value")
}

fn is_structured_token(value: &str) -> bool {
  let mut bytes = value.bytes();
  matches!(bytes.next(), Some(b'*' | b'a'..=b'z' | b'A'..=b'Z'))
    && bytes.all(|byte| {
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
            | b':'
            | b'/'
        )
    })
}
