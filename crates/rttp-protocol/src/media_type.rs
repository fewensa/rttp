use std::fmt::Write;

pub(crate) const MAX_MEDIA_TYPE_PARAMETERS: usize = 256;

/// A media type and its optional parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaType {
  type_: String,
  subtype: String,
  parameters: Vec<MediaTypeParameter>,
}

/// One parameter from a parsed media type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaTypeParameter {
  name: String,
  value: String,
}

impl MediaType {
  pub fn type_(&self) -> &str {
    &self.type_
  }

  pub fn subtype(&self) -> &str {
    &self.subtype
  }

  pub fn parameters(&self) -> &[MediaTypeParameter] {
    &self.parameters
  }

  pub(crate) fn header_value(&self) -> String {
    let mut value = format!("{}/{}", self.type_, self.subtype);
    for parameter in &self.parameters {
      value.push_str("; ");
      value.push_str(parameter.name());
      value.push('=');
      if is_token(parameter.value()) {
        value.push_str(parameter.value());
      } else {
        value.push('"');
        value.push_str(&escape_quoted(parameter.value()));
        value.push('"');
      }
    }
    value
  }
}

impl MediaTypeParameter {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

pub(crate) fn parse_values<'a, I>(
  values: I,
  header_name: &str,
  maximum_value_bytes: usize,
  maximum_media_types: usize,
) -> Result<Vec<MediaType>, String>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut media_types = Vec::new();
  for value in values {
    if value.len() > maximum_value_bytes {
      return Err(format!("{header_name} header value is too large"));
    }
    if value.bytes().any(is_invalid_control_byte) {
      return Err(format!("invalid {header_name} header value"));
    }
    parse_field(value, header_name, maximum_media_types, &mut media_types)?;
  }
  if media_types.is_empty() {
    return Err(format!("invalid {header_name} media type"));
  }
  Ok(media_types)
}

fn parse_field(
  value: &str,
  header_name: &str,
  maximum_media_types: usize,
  media_types: &mut Vec<MediaType>,
) -> Result<(), String> {
  let bytes = value.as_bytes();
  let mut position = 0;
  skip_ows(bytes, &mut position);
  if position == bytes.len() {
    return Err(format!("invalid {header_name} media type"));
  }

  loop {
    if media_types.len() >= maximum_media_types {
      return Err(format!("too many {header_name} media types"));
    }
    media_types.push(parse_media_type(value, &mut position, header_name)?);
    skip_ows(bytes, &mut position);
    if position == bytes.len() {
      return Ok(());
    }
    if bytes[position] != b',' {
      return Err(format!("invalid {header_name} media type"));
    }
    position += 1;
    skip_ows(bytes, &mut position);
    if position == bytes.len() || bytes[position] == b',' {
      return Err(format!("invalid {header_name} media type"));
    }
  }
}

fn parse_media_type(
  value: &str,
  position: &mut usize,
  header_name: &str,
) -> Result<MediaType, String> {
  let type_ = parse_token(value, position, header_name)?.to_string();
  if value.as_bytes().get(*position) != Some(&b'/') {
    return Err(format!("invalid {header_name} media type"));
  }
  *position += 1;
  let subtype = parse_token(value, position, header_name)?.to_string();
  let mut parameters = Vec::new();

  loop {
    skip_ows(value.as_bytes(), position);
    if value.as_bytes().get(*position) != Some(&b';') {
      break;
    }
    if parameters.len() >= MAX_MEDIA_TYPE_PARAMETERS {
      return Err(format!("too many {header_name} media type parameters"));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    let name = parse_token(value, position, header_name)?.to_string();
    skip_ows(value.as_bytes(), position);
    if value.as_bytes().get(*position) != Some(&b'=') {
      return Err(format!("invalid {header_name} media type parameter"));
    }
    *position += 1;
    skip_ows(value.as_bytes(), position);
    let parameter_value = if value.as_bytes().get(*position) == Some(&b'"') {
      parse_quoted_string(value, position, header_name)?
    } else {
      parse_token(value, position, header_name)?.to_string()
    };
    parameters.push(MediaTypeParameter {
      name,
      value: parameter_value,
    });
  }

  Ok(MediaType {
    type_,
    subtype,
    parameters,
  })
}

fn parse_token<'a>(
  value: &'a str,
  position: &mut usize,
  header_name: &str,
) -> Result<&'a str, String> {
  let start = *position;
  while value
    .as_bytes()
    .get(*position)
    .is_some_and(|byte| is_token_byte(*byte))
  {
    *position += 1;
  }
  if start == *position {
    Err(format!("invalid {header_name} media type"))
  } else {
    Ok(&value[start..*position])
  }
}

fn parse_quoted_string(
  value: &str,
  position: &mut usize,
  header_name: &str,
) -> Result<String, String> {
  *position += 1;
  let mut parsed = Vec::new();
  while let Some(&byte) = value.as_bytes().get(*position) {
    *position += 1;
    match byte {
      b'"' => {
        return String::from_utf8(parsed)
          .map_err(|_| format!("invalid {header_name} media type parameter"));
      }
      b'\\' => {
        let Some(&escaped) = value.as_bytes().get(*position) else {
          return Err(format!("invalid {header_name} media type parameter"));
        };
        if !is_quoted_pair_byte(escaped) {
          return Err(format!("invalid {header_name} media type parameter"));
        }
        *position += 1;
        parsed.push(escaped);
      }
      _ if is_quoted_text_byte(byte) => parsed.push(byte),
      _ => return Err(format!("invalid {header_name} media type parameter")),
    }
  }
  Err(format!("invalid {header_name} media type parameter"))
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
  byte == b'\t'
    || byte == b' '
    || (0x21..=0x7e).contains(&byte) && byte != b'"' && byte != b'\\'
    || byte >= 0x80
}

fn is_quoted_pair_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}

fn escape_quoted(value: &str) -> String {
  let mut escaped = String::with_capacity(value.len());
  for character in value.chars() {
    if matches!(character, '"' | '\\') {
      escaped.push('\\');
    }
    escaped
      .write_char(character)
      .expect("writing to String cannot fail");
  }
  escaped
}
