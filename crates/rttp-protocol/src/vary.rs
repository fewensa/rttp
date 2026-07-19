use std::error::Error;
use std::fmt;

pub const MAX_VARY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_VARY_FIELD_NAMES: usize = 256;

/// Parsed, bounded `Vary` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vary {
  any: bool,
  field_names: Vec<String>,
}

impl Vary {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, VaryParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, VaryParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut any = false;
    let mut field_names = Vec::new();
    let mut field_count = 0usize;

    for value in values {
      if value.len() > MAX_VARY_VALUE_BYTES {
        return Err(VaryParseError::new("Vary header value is too large"));
      }
      if value.bytes().any(is_invalid_control_byte) {
        return Err(VaryParseError::new("invalid Vary control byte"));
      }
      for member in value.split(',') {
        let field_name = member.trim_matches([' ', '\t']);
        if field_name == "*" {
          if any || field_count != 0 {
            return Err(VaryParseError::new("invalid Vary field name"));
          }
          any = true;
          continue;
        }
        if any || !is_http_token(field_name) {
          return Err(VaryParseError::new("invalid Vary field name"));
        }
        field_count += 1;
        if field_count > MAX_VARY_FIELD_NAMES {
          return Err(VaryParseError::new("too many Vary field names"));
        }
        let normalized = field_name.to_ascii_lowercase();
        if !field_names.contains(&normalized) {
          field_names.push(normalized);
        }
      }
    }

    if !any && field_names.is_empty() {
      return Err(VaryParseError::new("invalid Vary field name"));
    }
    Ok(Self { any, field_names })
  }

  pub fn is_any(&self) -> bool {
    self.any
  }

  pub fn field_names(&self) -> Vec<&str> {
    self.field_names.iter().map(String::as_str).collect()
  }

  pub fn len(&self) -> usize {
    self.field_names.len()
  }

  pub fn is_empty(&self) -> bool {
    self.field_names.is_empty()
  }

  pub fn header_value(&self) -> String {
    if self.any {
      "*".to_owned()
    } else {
      self.field_names.join(", ")
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaryParseError {
  message: String,
}

impl VaryParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for VaryParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for VaryParseError {}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_invalid_control_byte(byte: u8) -> bool {
  byte != b'\t' && (byte <= 0x1f || byte == 0x7f)
}

fn is_http_token_byte(byte: u8) -> bool {
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
