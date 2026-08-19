use std::error::Error;
use std::fmt;

pub const MAX_RANGE_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_RANGE_COUNT: usize = 32;
pub const MAX_CONTENT_RANGE_VALUE_BYTES: usize = 64 * 1024;

/// A bounded, parsed `Range` request header supporting the `bytes` range unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Range {
  ranges: Vec<ByteRangeSpec>,
}

/// One `bytes` range member from a `Range` header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ByteRangeSpec {
  FromTo { start: u64, end: Option<u64> },
  Suffix { length: u64 },
}

/// A parsed `Content-Range` response header supporting the `bytes` range unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContentRange {
  Bytes {
    start: u64,
    end: u64,
    complete_length: Option<u64>,
  },
  Unsatisfied {
    complete_length: u64,
  },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeParseError {
  message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRangeParseError {
  message: String,
}

impl RangeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl ContentRangeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for RangeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl fmt::Display for ContentRangeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for RangeParseError {}
impl Error for ContentRangeParseError {}

impl Range {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, RangeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, RangeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Err(RangeParseError::new("invalid Range header value"));
    };
    if values.next().is_some() {
      return Err(RangeParseError::new("multiple Range header values"));
    }

    let mut ranges = Vec::new();
    validate_value(value, MAX_RANGE_VALUE_BYTES, "Range").map_err(RangeParseError::new)?;
    let value = value.trim();
    let Some((unit, members)) = value.split_once('=') else {
      return Err(RangeParseError::new("invalid Range header value"));
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") || members.is_empty() {
      return Err(RangeParseError::new("invalid Range header value"));
    }
    for member in members.split(',') {
      if ranges.len() >= MAX_RANGE_COUNT {
        return Err(RangeParseError::new("too many Range members"));
      }
      ranges.push(parse_range_member(member.trim())?);
    }
    if ranges.is_empty() {
      return Err(RangeParseError::new("invalid Range header value"));
    }
    Ok(Self { ranges })
  }

  pub fn ranges(&self) -> &[ByteRangeSpec] {
    &self.ranges
  }

  pub fn header_value(&self) -> String {
    let members = self
      .ranges
      .iter()
      .map(ByteRangeSpec::header_value)
      .collect::<Vec<_>>();
    format!("bytes={}", members.join(", "))
  }
}

impl ByteRangeSpec {
  fn header_value(&self) -> String {
    match self {
      Self::FromTo { start, end } => {
        end.map_or_else(|| format!("{start}-"), |end| format!("{start}-{end}"))
      }
      Self::Suffix { length } => format!("-{length}"),
    }
  }
}

impl ContentRange {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ContentRangeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ContentRangeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Err(ContentRangeParseError::new(
        "invalid Content-Range header value",
      ));
    };
    if values.next().is_some() {
      return Err(ContentRangeParseError::new(
        "multiple Content-Range header values",
      ));
    }

    validate_value(value, MAX_CONTENT_RANGE_VALUE_BYTES, "Content-Range")
      .map_err(ContentRangeParseError::new)?;
    let value = value.trim();
    let Some((unit, range)) = value.split_once(' ') else {
      return Err(ContentRangeParseError::new(
        "invalid Content-Range header value",
      ));
    };
    if !unit.eq_ignore_ascii_case("bytes") || range.is_empty() || range.contains(' ') {
      return Err(ContentRangeParseError::new(
        "invalid Content-Range header value",
      ));
    }
    let Some((range, complete_length)) = range.split_once('/') else {
      return Err(ContentRangeParseError::new(
        "invalid Content-Range header value",
      ));
    };
    if complete_length.contains('/') {
      return Err(ContentRangeParseError::new(
        "invalid Content-Range header value",
      ));
    }
    if range == "*" {
      return Ok(Self::Unsatisfied {
        complete_length: parse_u64(complete_length).map_err(ContentRangeParseError::new)?,
      });
    }

    let (start, end) = parse_closed_range(range).map_err(ContentRangeParseError::new)?;
    let complete_length = if complete_length == "*" {
      None
    } else {
      Some(parse_u64(complete_length).map_err(ContentRangeParseError::new)?)
    };
    if complete_length.is_some_and(|length| end >= length) {
      return Err(ContentRangeParseError::new(
        "Content-Range position exceeds complete length",
      ));
    }
    Ok(Self::Bytes {
      start,
      end,
      complete_length,
    })
  }

  pub fn header_value(&self) -> String {
    match self {
      Self::Bytes {
        start,
        end,
        complete_length,
      } => format!(
        "bytes {start}-{end}/{}",
        complete_length.map_or_else(|| "*".to_string(), |length| length.to_string())
      ),
      Self::Unsatisfied { complete_length } => format!("bytes */{complete_length}"),
    }
  }
}

fn validate_value(value: &str, maximum_length: usize, name: &str) -> Result<(), String> {
  if value.len() > maximum_length {
    return Err(format!("{name} header value is too large"));
  }
  if value.bytes().any(|byte| byte.is_ascii_control()) {
    return Err(format!("invalid {name} header value"));
  }
  Ok(())
}

fn parse_range_member(value: &str) -> Result<ByteRangeSpec, RangeParseError> {
  if value.is_empty() {
    return Err(RangeParseError::new("invalid Range member"));
  }
  if let Some(suffix) = value.strip_prefix('-') {
    return Ok(ByteRangeSpec::Suffix {
      length: parse_u64(suffix).map_err(RangeParseError::new)?,
    });
  }
  let Some((start, end)) = value.split_once('-') else {
    return Err(RangeParseError::new("invalid Range member"));
  };
  if end.contains('-') {
    return Err(RangeParseError::new("invalid Range member"));
  }
  let start = parse_u64(start).map_err(RangeParseError::new)?;
  let end = if end.is_empty() {
    None
  } else {
    Some(parse_u64(end).map_err(RangeParseError::new)?)
  };
  if end.is_some_and(|end| start > end) {
    return Err(RangeParseError::new("invalid Range member"));
  }
  Ok(ByteRangeSpec::FromTo { start, end })
}

fn parse_closed_range(value: &str) -> Result<(u64, u64), String> {
  let Some((start, end)) = value.split_once('-') else {
    return Err("invalid Content-Range header value".to_string());
  };
  if end.is_empty() || end.contains('-') {
    return Err("invalid Content-Range header value".to_string());
  }
  let start = parse_u64(start)?;
  let end = parse_u64(end)?;
  if start > end {
    return Err("invalid Content-Range header value".to_string());
  }
  Ok((start, end))
}

fn parse_u64(value: &str) -> Result<u64, String> {
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err("invalid range number".to_string());
  }
  value
    .parse()
    .map_err(|_| "range number is out of bounds".to_string())
}
