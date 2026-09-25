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
  let size = std::str::from_utf8(trim_bws(size)).map_err(|_| ChunkSizeError::NotUtf8)?;
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

/// Split an HTTP/1 status-line on literal ASCII SP separators only.
///
/// Returns `(HTTP-version, status-code, reason-phrase)`. HTAB, vertical tab,
/// form feed, Unicode whitespace, and other non-SP whitespace anywhere in the
/// line are rejected. A missing reason-phrase is returned as `""`.
pub fn split_status_line(status_line: &str) -> Option<(&str, &str, &str)> {
  if status_line
    .chars()
    .any(|character| character != ' ' && character.is_whitespace())
  {
    return None;
  }

  let (version, rest) = status_line.split_once(' ')?;
  if version.is_empty() || version.chars().any(char::is_whitespace) {
    return None;
  }

  let (code, reason) = match rest.split_once(' ') {
    Some((code, reason)) => (code, reason),
    None => (rest, ""),
  };
  if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }

  Some((version, code, reason))
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
  let end = bytes
    .iter()
    .rposition(|byte| *byte != b' ' && *byte != b'\t')
    .map_or(start, |index| index + 1);
  &bytes[start..end]
}

#[cfg(test)]
mod tests {
  use super::{
    is_header_value_byte, is_token, parse_chunk_size, split_status_line, ChunkSizeError,
  };

  #[test]
  fn parses_plain_chunk_sizes_and_valid_extensions() {
    assert_eq!(parse_chunk_size(b"4\r\n"), Ok(4));
    assert_eq!(parse_chunk_size(b"  4\t\r\n"), Ok(4));
    assert_eq!(parse_chunk_size(b"4;foo=bar;flag\r\n"), Ok(4));
    assert_eq!(parse_chunk_size(b"A;foo=bar;quoted=\"a\\\"b\"\r\n"), Ok(10));
  }

  #[test]
  fn rejects_non_sp_htab_whitespace_in_chunk_size() {
    for line in [
      b"\r4\r\n".as_slice(),
      b"4\r\r\n",
      b"\n4\r\n",
      b"4\n\r\n",
      b"4\x0b\r\n",
      b"4\x0c\r\n",
      "4\u{00a0}\r\n".as_bytes(),
      "\u{00a0}4\r\n".as_bytes(),
      "4\u{2003}\r\n".as_bytes(),
      "\u{00a0}\r\n".as_bytes(),
    ] {
      assert_eq!(parse_chunk_size(line), Err(ChunkSizeError::Invalid));
    }

    assert_eq!(parse_chunk_size(b"  \t  \r\n"), Err(ChunkSizeError::Empty));
  }

  #[test]
  fn rejects_invalid_chunk_extensions() {
    for line in [
      b"A;foo=\"unterminated\r\n".as_slice(),
      b"A;=bar\r\n",
      b"A;foo=\r\n",
    ] {
      assert_eq!(
        parse_chunk_size(line),
        Err(ChunkSizeError::InvalidExtension)
      );
    }
  }

  #[test]
  fn validates_http_field_syntax_bytes() {
    assert!(is_token("X-Request_Id"));
    assert!(!is_token("bad name"));
    assert!(is_header_value_byte(0x80));
    assert!(!is_header_value_byte(b'\n'));
  }

  #[test]
  fn split_status_line_accepts_ascii_sp_separators() {
    assert_eq!(
      split_status_line("HTTP/1.1 200 OK"),
      Some(("HTTP/1.1", "200", "OK"))
    );
    assert_eq!(
      split_status_line("HTTP/1.1 200"),
      Some(("HTTP/1.1", "200", ""))
    );
    assert_eq!(
      split_status_line("HTTP/1.1 103 Early Hints"),
      Some(("HTTP/1.1", "103", "Early Hints"))
    );
  }

  #[test]
  fn split_status_line_rejects_non_sp_whitespace() {
    for status_line in [
      "HTTP/1.1\t200 OK",
      "HTTP/1.1\u{000b}200 OK",
      "HTTP/1.1\u{000c}200 OK",
      "HTTP/1.1\u{00a0}200 OK",
      "HTTP/1.1\u{2003}200 OK",
      "HTTP/1.1200 OK",
      "HTTP/1.1-200 OK",
      "HTTP/1.1/200 OK",
      "HTTP/1.1 200 Connection\tEstablished",
      "HTTP/1.1 200 Connection\u{00a0}Established",
      "HTTP/1.1 200 Connection\u{2003}Established",
    ] {
      assert_eq!(
        split_status_line(status_line),
        None,
        "expected rejection for {status_line:?}"
      );
    }
  }
}
