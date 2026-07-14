use std::error::Error;
use std::fmt;

pub const MAX_TRAILER_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_TRAILER_FIELD_NAMES: usize = 32;

/// Bounded field names declared by an HTTP `Trailer` header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trailer {
  field_names: Vec<String>,
}

impl Trailer {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, TrailerParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, TrailerParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut field_names = Vec::new();
    let mut field_count = 0usize;

    for value in values {
      if value.len() > MAX_TRAILER_VALUE_BYTES {
        return Err(TrailerParseError::new("Trailer header value is too large"));
      }
      for field_name in value.split(',') {
        let field_name = field_name.trim();
        if !is_http_token(field_name) {
          return Err(TrailerParseError::new("invalid Trailer field name"));
        }
        if is_forbidden_trailer_field_name(field_name) {
          return Err(TrailerParseError::new("forbidden Trailer field name"));
        }
        field_count += 1;
        if field_count > MAX_TRAILER_FIELD_NAMES {
          return Err(TrailerParseError::new("too many Trailer field names"));
        }
        let normalized = field_name.to_ascii_lowercase();
        if !field_names.contains(&normalized) {
          field_names.push(normalized);
        }
      }
    }

    if field_names.is_empty() {
      return Err(TrailerParseError::new("invalid Trailer field name"));
    }
    Ok(Self { field_names })
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
    self.field_names.join(", ")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrailerParseError {
  message: String,
}

impl TrailerParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TrailerParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for TrailerParseError {}

pub fn is_forbidden_trailer_field_name(name: &str) -> bool {
  matches!(
    name.to_ascii_lowercase().as_str(),
    "authorization"
      | "cache-control"
      | "connection"
      | "content-encoding"
      | "content-length"
      | "content-range"
      | "content-type"
      | "cookie"
      | "host"
      | "keep-alive"
      | "max-forwards"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "proxy-connection"
      | "set-cookie"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
      | "www-authenticate"
  )
}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
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
