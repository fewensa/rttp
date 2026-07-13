use std::error::Error;
use std::fmt;

pub const MAX_PRIORITY_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_PRIORITY_PARAMETERS: usize = 256;

/// Parsed, bounded HTTP `Priority` metadata as defined by RFC 9218.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Priority {
  urgency: Option<u8>,
  incremental: Option<bool>,
  extensions: Vec<PriorityExtension>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorityExtension {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorityParseError {
  message: String,
}

impl PriorityParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for PriorityParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for PriorityParseError {}

impl Priority {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, PriorityParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, PriorityParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut priority = Self::default();
    let mut parameter_count = 0usize;
    for value in values {
      if value.len() > MAX_PRIORITY_VALUE_BYTES {
        return Err(PriorityParseError::new(
          "Priority header value is too large",
        ));
      }
      if value.trim().is_empty() {
        return Err(PriorityParseError::new("invalid Priority parameter"));
      }
      for member in split_members(value)? {
        parameter_count += 1;
        if parameter_count > MAX_PRIORITY_PARAMETERS {
          return Err(PriorityParseError::new("too many Priority parameters"));
        }
        priority.apply_member(member)?;
      }
    }
    if parameter_count == 0 {
      return Err(PriorityParseError::new("invalid Priority parameter"));
    }
    Ok(priority)
  }

  pub fn urgency(&self) -> Option<u8> {
    self.urgency
  }

  pub fn incremental(&self) -> bool {
    self.incremental.unwrap_or(false)
  }

  pub fn extensions(&self) -> &[PriorityExtension] {
    &self.extensions
  }

  pub fn header_value(&self) -> String {
    let mut members = Vec::new();
    if let Some(urgency) = self.urgency {
      members.push(format!("u={urgency}"));
    }
    if self.incremental() {
      members.push("i".to_string());
    }
    members.extend(self.extensions.iter().map(PriorityExtension::header_value));
    members.join(", ")
  }

  fn apply_member(&mut self, member: &str) -> Result<(), PriorityParseError> {
    let (name, value) = match member.split_once('=') {
      Some((name, value)) => (name.trim(), Some(value.trim())),
      None => (member.trim(), None),
    };
    if !is_key(name) || value.is_some_and(|value| !is_bare_item(value)) {
      return Err(PriorityParseError::new("invalid Priority parameter"));
    }
    if self.has_name(name) {
      return Err(PriorityParseError::new("duplicate Priority parameter"));
    }
    match name {
      "u" => {
        let Some(value) = value else {
          return Err(PriorityParseError::new("invalid Priority urgency"));
        };
        let urgency = value
          .parse::<u8>()
          .ok()
          .filter(|urgency| *urgency <= 7)
          .ok_or_else(|| PriorityParseError::new("invalid Priority urgency"))?;
        self.urgency = Some(urgency);
      }
      "i" => {
        self.incremental = Some(match value {
          None | Some("?1") => true,
          Some("?0") => false,
          _ => {
            return Err(PriorityParseError::new(
              "invalid Priority incremental value",
            ))
          }
        });
      }
      _ => self.extensions.push(PriorityExtension {
        name: name.to_string(),
        value: value.map(ToString::to_string),
      }),
    }
    Ok(())
  }

  fn has_name(&self, name: &str) -> bool {
    match name {
      "u" => self.urgency.is_some(),
      "i" => self.incremental.is_some(),
      _ => self
        .extensions
        .iter()
        .any(|extension| extension.name == name),
    }
  }
}

impl PriorityExtension {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }

  fn header_value(&self) -> String {
    self.value.as_ref().map_or_else(
      || self.name.clone(),
      |value| format!("{}={value}", self.name),
    )
  }
}

fn split_members(value: &str) -> Result<Vec<&str>, PriorityParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
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
      let member = value[start..index].trim();
      if member.is_empty() {
        return Err(PriorityParseError::new("invalid Priority parameter"));
      }
      members.push(member);
      start = index + 1;
    }
  }
  if quoted || escaped {
    return Err(PriorityParseError::new("invalid Priority parameter"));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(PriorityParseError::new("invalid Priority parameter"));
  }
  members.push(member);
  Ok(members)
}

fn is_key(value: &str) -> bool {
  let mut bytes = value.bytes();
  matches!(bytes.next(), Some(b'a'..=b'z' | b'*'))
    && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'*'))
}

fn is_bare_item(value: &str) -> bool {
  is_boolean(value)
    || is_integer(value)
    || is_decimal(value)
    || is_string(value)
    || is_token(value)
    || is_byte_sequence(value)
}

fn is_boolean(value: &str) -> bool {
  matches!(value, "?0" | "?1")
}

fn is_integer(value: &str) -> bool {
  let digits = value.strip_prefix('-').unwrap_or(value);
  !digits.is_empty() && digits.len() <= 15 && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_decimal(value: &str) -> bool {
  let value = value.strip_prefix('-').unwrap_or(value);
  let Some((whole, fraction)) = value.split_once('.') else {
    return false;
  };
  !whole.is_empty()
    && whole.len() <= 12
    && whole.bytes().all(|byte| byte.is_ascii_digit())
    && (1..=3).contains(&fraction.len())
    && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_string(value: &str) -> bool {
  let Some(inner) = value
    .strip_prefix('"')
    .and_then(|value| value.strip_suffix('"'))
  else {
    return false;
  };
  let mut escaped = false;
  for byte in inner.bytes() {
    if escaped {
      if !matches!(byte, b'"' | b'\\') {
        return false;
      }
      escaped = false;
    } else if byte == b'\\' {
      escaped = true;
    } else if !(0x20..=0x7e).contains(&byte) || byte == b'"' {
      return false;
    }
  }
  !escaped
}

fn is_token(value: &str) -> bool {
  let mut bytes = value.bytes();
  matches!(bytes.next(), Some(b'*' | b'a'..=b'z'))
    && bytes.all(|byte| matches!(byte, b'*' | b'a'..=b'z' | b'0'..=b'9' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b':' | b'/'))
}

fn is_byte_sequence(value: &str) -> bool {
  let Some(inner) = value
    .strip_prefix(':')
    .and_then(|value| value.strip_suffix(':'))
  else {
    return false;
  };
  !inner.is_empty()
    && inner
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
}

#[cfg(test)]
mod tests {
  use super::Priority;

  #[test]
  fn priority_round_trips_known_and_extension_members() {
    let priority = Priority::parse("u=1, i, x=token").expect("Priority should parse");

    assert_eq!(Some(1), priority.urgency());
    assert!(priority.incremental());
    assert_eq!(1, priority.extensions().len());
    assert_eq!("x", priority.extensions()[0].name());
    assert_eq!(Some("token"), priority.extensions()[0].value());
    assert_eq!("u=1, i, x=token", priority.header_value());
  }

  #[test]
  fn priority_rejects_oversized_parameter_sets() {
    let too_many = (0..257)
      .map(|index| format!("x{index}=?1"))
      .collect::<Vec<_>>()
      .join(", ");

    assert!(Priority::parse(too_many).is_err());
  }
}
