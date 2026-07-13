use std::error::Error;
use std::fmt;

pub const MAX_SERVER_TIMING_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_SERVER_TIMING_METRICS: usize = 256;
pub const MAX_SERVER_TIMING_PARAMETERS: usize = 256;
pub const MAX_SERVER_TIMING_PARAMETER_VALUE_BYTES: usize = 64 * 1024;

/// Parsed, bounded `Server-Timing` response metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerTiming {
  metrics: Vec<ServerTimingMetric>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerTimingMetric {
  name: String,
  duration: Option<f64>,
  description: Option<String>,
  parameters: Vec<ServerTimingParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerTimingParameter {
  name: String,
  value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerTimingParseError {
  message: String,
}

impl ServerTimingParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for ServerTimingParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for ServerTimingParseError {}

impl ServerTiming {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, ServerTimingParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, ServerTimingParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut metrics = Vec::new();
    for value in values {
      if value.len() > MAX_SERVER_TIMING_VALUE_BYTES {
        return Err(ServerTimingParseError::new(
          "Server-Timing header value is too large",
        ));
      }
      parse_field(value, &mut metrics)?;
    }
    if metrics.is_empty() {
      return Err(ServerTimingParseError::new("invalid Server-Timing metric"));
    }
    Ok(Self { metrics })
  }

  pub fn metrics(&self) -> &[ServerTimingMetric] {
    &self.metrics
  }
  pub fn len(&self) -> usize {
    self.metrics.len()
  }
  pub fn is_empty(&self) -> bool {
    self.metrics.is_empty()
  }

  pub fn header_value(&self) -> String {
    self
      .metrics
      .iter()
      .map(ServerTimingMetric::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl ServerTimingMetric {
  pub fn name(&self) -> &str {
    &self.name
  }
  pub fn duration(&self) -> Option<f64> {
    self.duration
  }
  pub fn description(&self) -> Option<&str> {
    self.description.as_deref()
  }
  pub fn parameters(&self) -> &[ServerTimingParameter] {
    &self.parameters
  }

  fn header_value(&self) -> String {
    let mut value = self.name.clone();
    if let Some(duration) = self.duration {
      value.push_str("; dur=");
      value.push_str(&duration.to_string());
    }
    if let Some(description) = &self.description {
      value.push_str("; desc=\"");
      value.push_str(&escape_quoted(description));
      value.push('"');
    }
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(parameter.name());
      if let Some(parameter_value) = parameter.value() {
        value.push('=');
        if is_token(parameter_value) {
          value.push_str(parameter_value);
        } else {
          value.push('"');
          value.push_str(&escape_quoted(parameter_value));
          value.push('"');
        }
      }
    }
    value
  }
}

impl ServerTimingParameter {
  pub fn name(&self) -> &str {
    &self.name
  }
  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

fn parse_field(
  value: &str,
  metrics: &mut Vec<ServerTimingMetric>,
) -> Result<(), ServerTimingParseError> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(ServerTimingParseError::new("invalid Server-Timing metric"));
  }
  while position < bytes.len() {
    if metrics.len() >= MAX_SERVER_TIMING_METRICS {
      return Err(ServerTimingParseError::new(
        "too many Server-Timing metrics",
      ));
    }
    let name = parse_token(value, &mut position, "invalid Server-Timing metric name")?.to_string();
    let mut metric = ServerTimingMetric {
      name,
      duration: None,
      description: None,
      parameters: Vec::new(),
    };
    loop {
      skip_ows(bytes, &mut position);
      if position == bytes.len() || bytes[position] == b',' {
        break;
      }
      if bytes[position] != b';' {
        return Err(ServerTimingParseError::new("invalid Server-Timing metric"));
      }
      position += 1;
      skip_ows(bytes, &mut position);
      parse_parameter(value, &mut position, &mut metric)?;
    }
    metrics.push(metric);
    if position == bytes.len() {
      break;
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Err(ServerTimingParseError::new("invalid Server-Timing metric"));
    }
  }
  Ok(())
}

fn parse_parameter(
  value: &str,
  position: &mut usize,
  metric: &mut ServerTimingMetric,
) -> Result<(), ServerTimingParseError> {
  let name =
    parse_token(value, position, "invalid Server-Timing parameter name")?.to_ascii_lowercase();
  skip_ows(value.as_bytes(), position);
  let parameter_value = if value.as_bytes().get(*position) == Some(&b'=') {
    *position += 1;
    skip_ows(value.as_bytes(), position);
    Some(parse_parameter_value(value, position)?)
  } else {
    None
  };
  if parameter_value
    .as_ref()
    .is_some_and(|parameter| parameter.len() > MAX_SERVER_TIMING_PARAMETER_VALUE_BYTES)
  {
    return Err(ServerTimingParseError::new(
      "Server-Timing parameter value is too large",
    ));
  }
  if name == "dur" {
    let duration = parameter_value
      .ok_or_else(|| ServerTimingParseError::new("invalid Server-Timing dur parameter"))?;
    let duration = duration
      .parse::<f64>()
      .map_err(|_| ServerTimingParseError::new("invalid Server-Timing dur parameter"))?;
    if !duration.is_finite() || duration < 0.0 || metric.duration.replace(duration).is_some() {
      return Err(ServerTimingParseError::new(
        "invalid Server-Timing dur parameter",
      ));
    }
  } else if name == "desc" {
    let description = parameter_value
      .ok_or_else(|| ServerTimingParseError::new("invalid Server-Timing desc parameter"))?;
    if metric.description.replace(description).is_some() {
      return Err(ServerTimingParseError::new(
        "duplicate Server-Timing desc parameter",
      ));
    }
  } else {
    if metric.parameters.len() >= MAX_SERVER_TIMING_PARAMETERS {
      return Err(ServerTimingParseError::new(
        "too many Server-Timing parameters",
      ));
    }
    if metric
      .parameters
      .iter()
      .any(|parameter| parameter.name.eq_ignore_ascii_case(&name))
    {
      return Err(ServerTimingParseError::new(
        "duplicate Server-Timing parameter",
      ));
    }
    metric.parameters.push(ServerTimingParameter {
      name,
      value: parameter_value,
    });
  }
  Ok(())
}

fn parse_parameter_value(
  value: &str,
  position: &mut usize,
) -> Result<String, ServerTimingParseError> {
  if value.as_bytes().get(*position) == Some(&b'"') {
    parse_quoted_string(value, position)
  } else {
    Ok(parse_token(value, position, "invalid Server-Timing parameter value")?.to_string())
  }
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
) -> Result<String, ServerTimingParseError> {
  let bytes = value.as_bytes();
  *position += 1;
  let mut parsed = String::new();
  let mut unescaped_start = *position;
  let mut escaped = false;
  while *position < bytes.len() {
    let byte = bytes[*position];
    if escaped {
      *position += 1;
      if !(byte == b'\t' || (0x20..=0x7e).contains(&byte)) {
        return Err(ServerTimingParseError::new(
          "invalid Server-Timing quoted-string",
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
      return Err(ServerTimingParseError::new(
        "invalid Server-Timing quoted-string",
      ));
    } else {
      *position += 1;
    }
  }
  Err(ServerTimingParseError::new(
    "invalid Server-Timing quoted-string",
  ))
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  message: &str,
) -> Result<&'a str, ServerTimingParseError> {
  let start = *position;
  while *position < value.len() && is_token_byte(value.as_bytes()[*position]) {
    *position += 1;
  }
  if *position == start {
    Err(ServerTimingParseError::new(message))
  } else {
    Ok(&value[start..*position])
  }
}

fn skip_ows(bytes: &[u8], position: &mut usize) {
  while *position < bytes.len() && matches!(bytes[*position], b' ' | b'\t') {
    *position += 1;
  }
}
fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}
fn is_token_byte(byte: u8) -> bool {
  matches!(byte, b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z')
}
fn escape_quoted(value: &str) -> String {
  value.replace('\\', "\\\\").replace('"', "\\\"")
}
