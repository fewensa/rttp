//! HTTP/1.x syntax primitives that are independent of endpoint policy.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkSizeError {
  NotUtf8,
  Empty,
  Invalid,
  InvalidExtension,
}

impl fmt::Display for ChunkSizeError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(match self {
      Self::NotUtf8 => "chunk size is not UTF-8",
      Self::Empty => "empty chunk size",
      Self::Invalid => "invalid chunk size",
      Self::InvalidExtension => "invalid chunk extension",
    })
  }
}

impl std::error::Error for ChunkSizeError {}

pub fn parse_chunk_size(line: &[u8]) -> Result<usize, ChunkSizeError> {
  let line = line.strip_suffix(b"\r\n").unwrap_or(line);
  let (size, extensions) = line
    .iter()
    .position(|byte| *byte == b';')
    .map_or((line, None), |index| {
      (&line[..index], Some(&line[index + 1..]))
    });
  let size = std::str::from_utf8(size)
    .map_err(|_| ChunkSizeError::NotUtf8)?
    .trim();
  if size.is_empty() {
    return Err(ChunkSizeError::Empty);
  }
  if let Some(extensions) = extensions {
    validate_chunk_extensions(extensions)?;
  }

  usize::from_str_radix(size, 16).map_err(|_| ChunkSizeError::Invalid)
}

pub fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

pub fn is_token_byte(byte: u8) -> bool {
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

pub fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}

pub fn is_reason_phrase_byte(byte: u8) -> bool {
  is_header_value_byte(byte)
}

pub fn is_tchar(byte: u8) -> bool {
  is_token_byte(byte)
}

pub fn is_qdtext(byte: u8) -> bool {
  matches!(byte, b'\t' | b' ' | b'!' | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
}

pub fn is_quoted_pair_char(byte: u8) -> bool {
  matches!(byte, b'\t' | b' ' | 0x21..=0x7e | 0x80..=0xff)
}

fn validate_chunk_extensions(mut bytes: &[u8]) -> Result<(), ChunkSizeError> {
  loop {
    bytes = trim_bws(bytes);
    let token_len = bytes
      .iter()
      .position(|byte| !is_tchar(*byte))
      .unwrap_or(bytes.len());
    if token_len == 0 {
      return Err(ChunkSizeError::InvalidExtension);
    }
    bytes = trim_bws(&bytes[token_len..]);

    if let Some(rest) = bytes.strip_prefix(b"=") {
      bytes = trim_bws(rest);
      if let Some(rest) = bytes.strip_prefix(b"\"") {
        bytes = parse_quoted_chunk_extension(rest)?;
      } else {
        let value_len = bytes
          .iter()
          .position(|byte| !is_tchar(*byte))
          .unwrap_or(bytes.len());
        if value_len == 0 {
          return Err(ChunkSizeError::InvalidExtension);
        }
        bytes = &bytes[value_len..];
      }
      bytes = trim_bws(bytes);
    }

    if bytes.is_empty() {
      return Ok(());
    }
    if let Some(rest) = bytes.strip_prefix(b";") {
      bytes = rest;
    } else {
      return Err(ChunkSizeError::InvalidExtension);
    }
  }
}

fn parse_quoted_chunk_extension(mut bytes: &[u8]) -> Result<&[u8], ChunkSizeError> {
  loop {
    let Some((&byte, rest)) = bytes.split_first() else {
      return Err(ChunkSizeError::InvalidExtension);
    };
    match byte {
      b'\"' => return Ok(rest),
      b'\\' => {
        let Some((&escaped, rest)) = rest.split_first() else {
          return Err(ChunkSizeError::InvalidExtension);
        };
        if !is_quoted_pair_char(escaped) {
          return Err(ChunkSizeError::InvalidExtension);
        }
        bytes = rest;
      }
      byte if is_qdtext(byte) => bytes = rest,
      _ => return Err(ChunkSizeError::InvalidExtension),
    }
  }
}

fn trim_bws(bytes: &[u8]) -> &[u8] {
  let start = bytes
    .iter()
    .position(|byte| *byte != b' ' && *byte != b'\t')
    .unwrap_or(bytes.len());
  &bytes[start..]
}

#[cfg(test)]
mod tests {
  use super::{is_header_value_byte, is_token, parse_chunk_size, ChunkSizeError};

  #[test]
  fn parses_chunk_sizes_with_valid_extensions() {
    assert_eq!(parse_chunk_size(b"A;foo=bar;quoted=\"a\\\"b\"\r\n"), Ok(10));
  }

  #[test]
  fn rejects_invalid_chunk_extensions() {
    assert_eq!(
      parse_chunk_size(b"A;foo=\"unterminated\r\n"),
      Err(ChunkSizeError::InvalidExtension)
    );
  }

  #[test]
  fn validates_http_field_syntax_bytes() {
    assert!(is_token("X-Request_Id"));
    assert!(!is_token("bad name"));
    assert!(is_header_value_byte(0x80));
    assert!(!is_header_value_byte(b'\n'));
  }
}
