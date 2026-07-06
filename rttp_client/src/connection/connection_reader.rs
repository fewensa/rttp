use std::io;
use std::io::Read;

use url::Url;

use crate::error;
use crate::response::Response;
use crate::types::{Header, RoUrl};

const HEADER_END: &[u8] = b"\r\n\r\n";
const CRLF: &[u8] = b"\r\n";
pub(crate) const MAX_CHUNKED_RESPONSE_LINE_BYTES: usize = 8 * 1024;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResponseBodyKind {
  NoBody,
  Chunked,
  ContentLength(usize),
  UntilEof,
}

pub(crate) struct ResponseParts {
  pub(crate) binary: Vec<u8>,
  pub(crate) trailers: Vec<Header>,
}

#[allow(dead_code)]
pub struct ConnectionReader<'a> {
  url: &'a Url,
  reader: &'a mut dyn io::Read,
  expect_no_body: bool,
}

impl<'a> ConnectionReader<'a> {
  pub fn new(
    url: &'a Url,
    reader: &'a mut dyn io::Read,
    expect_no_body: bool,
  ) -> ConnectionReader<'a> {
    Self {
      url,
      reader,
      expect_no_body,
    }
  }

  #[allow(dead_code)]
  pub fn binary(&mut self) -> error::Result<Vec<u8>> {
    Ok(read_response_parts(self.reader, self.expect_no_body)?.binary)
  }

  pub(crate) fn response_parts(&mut self) -> error::Result<ResponseParts> {
    read_response_parts(self.reader, self.expect_no_body)
  }

  #[allow(dead_code)]
  pub fn response(&mut self) -> error::Result<Response> {
    let parts = self.response_parts()?;
    Response::with_trailers(RoUrl::from(self.url.clone()), parts.binary, parts.trailers)
  }

  // todo Connection reader will read more type from io::Reader, like Chunk data, and Stream data.
}

pub(crate) fn read_response_parts<R>(
  reader: &mut R,
  expect_no_body: bool,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  let mut binary = loop {
    let header = read_response_header(reader)?;
    if response_status_code(&header)? == 100 {
      continue;
    }
    break header;
  };
  let mut trailers = Vec::new();
  match response_body_kind(&binary, expect_no_body)? {
    ResponseBodyKind::NoBody => {}
    ResponseBodyKind::Chunked => {
      let chunked = read_chunked_response_body(reader)?;
      binary.extend_from_slice(&chunked.body);
      trailers = chunked.trailers;
    }
    ResponseBodyKind::ContentLength(content_length) => {
      let current_len = binary.len();
      binary.resize(current_len + content_length, 0);
      reader
        .read_exact(&mut binary[current_len..])
        .map_err(error::request)?;
    }
    ResponseBodyKind::UntilEof => {
      reader.read_to_end(&mut binary).map_err(error::request)?;
    }
  }
  Ok(ResponseParts { binary, trailers })
}

fn read_response_header<R>(reader: &mut R) -> error::Result<Vec<u8>>
where
  R: Read + ?Sized,
{
  let mut header = Vec::new();
  let mut byte = [0u8; 1];

  loop {
    let read = reader.read(&mut byte).map_err(error::request)?;
    if read == 0 {
      if header.is_empty() {
        return Ok(header);
      }
      return Err(error::bad_response("Incomplete http response headers"));
    }

    header.push(byte[0]);
    if header.ends_with(HEADER_END) {
      return Ok(header);
    }
  }
}

fn response_status_code(header: &[u8]) -> error::Result<u16> {
  let header = String::from_utf8_lossy(header);
  let status_line = header
    .lines()
    .next()
    .ok_or_else(|| error::bad_response("Response not have status line"))?;
  status_line
    .split_whitespace()
    .nth(1)
    .ok_or_else(|| error::bad_response("Response status not have code"))?
    .parse::<u16>()
    .map_err(|_| error::bad_response("Response status code is not a number"))
}

pub(crate) fn response_body_kind(
  header: &[u8],
  expect_no_body: bool,
) -> error::Result<ResponseBodyKind> {
  if expect_no_body {
    return Ok(ResponseBodyKind::NoBody);
  }

  let status_code = response_status_code(header)?;

  if (100..200).contains(&status_code) || status_code == 204 || status_code == 304 {
    return Ok(ResponseBodyKind::NoBody);
  }

  let header = String::from_utf8_lossy(header);
  let lines = header.lines().skip(1);
  let mut content_length = None;
  let mut invalid_content_length = false;
  let mut conflicting_content_length = false;
  let mut chunked = false;

  for line in lines {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };

    if name.eq_ignore_ascii_case("Transfer-Encoding") {
      chunked = value
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case("chunked"));
    }

    if name.eq_ignore_ascii_case("Content-Length") {
      for token in value.split(',') {
        let Ok(length) = token.trim().parse::<usize>() else {
          invalid_content_length = true;
          continue;
        };

        if content_length.is_some_and(|existing| existing != length) {
          conflicting_content_length = true;
        } else {
          content_length = Some(length);
        }
      }
    }
  }

  if chunked {
    Ok(ResponseBodyKind::Chunked)
  } else if conflicting_content_length {
    Err(error::bad_response("Conflicting Content-Length headers"))
  } else if invalid_content_length {
    Err(error::bad_response("Invalid Content-Length header"))
  } else if let Some(content_length) = content_length {
    Ok(ResponseBodyKind::ContentLength(content_length))
  } else {
    Ok(ResponseBodyKind::UntilEof)
  }
}

#[derive(Debug)]
struct ChunkedResponseBody {
  body: Vec<u8>,
  trailers: Vec<Header>,
}

fn read_chunked_response_body<R>(reader: &mut R) -> error::Result<ChunkedResponseBody>
where
  R: Read + ?Sized,
{
  let mut body = Vec::new();

  loop {
    let line = read_bounded_crlf_line(reader, MAX_CHUNKED_RESPONSE_LINE_BYTES)?;
    let chunk_size = parse_chunk_size(&line)?;

    if chunk_size == 0 {
      let trailers = read_trailers(reader)?;
      return Ok(ChunkedResponseBody { body, trailers });
    }

    let current_len = body.len();
    body.resize(current_len + chunk_size, 0);
    reader
      .read_exact(&mut body[current_len..])
      .map_err(error::request)?;
    consume_crlf(reader)?;
  }
}

fn read_bounded_crlf_line<R>(reader: &mut R, max_len: usize) -> error::Result<Vec<u8>>
where
  R: Read + ?Sized,
{
  let mut line = Vec::new();
  let mut byte = [0u8; 1];

  loop {
    let read = reader.read(&mut byte).map_err(error::request)?;
    if read == 0 {
      return Err(error::bad_response("Unexpected end of chunked body"));
    }

    if line.len() == max_len {
      return Err(error::bad_response("chunked response line is too large"));
    }

    line.push(byte[0]);
    if line.ends_with(CRLF) {
      return Ok(line);
    }
  }
}

fn parse_chunk_size(line: &[u8]) -> error::Result<usize> {
  let line = std::str::from_utf8(line).map_err(error::response)?;
  let size = line
    .trim_end_matches("\r\n")
    .split(';')
    .next()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| error::bad_response("Chunk size line is empty"))?;

  usize::from_str_radix(size, 16).map_err(|_| error::bad_response("Invalid chunk size"))
}

fn consume_crlf<R>(reader: &mut R) -> error::Result<()>
where
  R: Read + ?Sized,
{
  let mut suffix = [0u8; 2];
  reader.read_exact(&mut suffix).map_err(|err| {
    if err.kind() == io::ErrorKind::UnexpectedEof {
      error::bad_response("Unexpected end of chunked body")
    } else {
      error::request(err)
    }
  })?;
  if suffix == *CRLF {
    Ok(())
  } else {
    Err(error::bad_response("Invalid chunk terminator"))
  }
}

fn read_trailers<R>(reader: &mut R) -> error::Result<Vec<Header>>
where
  R: Read + ?Sized,
{
  let mut trailers = Vec::new();
  loop {
    let line = read_bounded_crlf_line(reader, MAX_CHUNKED_RESPONSE_LINE_BYTES)?;
    if line == CRLF {
      return Ok(trailers);
    }

    trailers.push(parse_trailer_line(&line)?);
  }
}

fn parse_trailer_line(line: &[u8]) -> error::Result<Header> {
  let line = std::str::from_utf8(line).map_err(error::response)?;
  let line = line.trim_end_matches("\r\n");
  let (name, value) = line
    .split_once(':')
    .ok_or_else(|| error::bad_response("Invalid trailer header"))?;

  Ok(Header::new(name, value))
}

#[cfg(test)]
mod tests {
  use std::error::Error as StdError;
  use std::io::{self, Cursor, Read};

  use super::{ConnectionReader, ResponseBodyKind};

  #[test]
  fn test_chunked_binary_is_decoded() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Connection: close\r\n",
      "\r\n",
      "4\r\nWiki\r\n",
      "5\r\npedia\r\n",
      "0\r\n\r\n"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let binary = reader.binary().unwrap();
    let text = String::from_utf8(binary).unwrap();

    assert!(text.ends_with("\r\n\r\nWikipedia"));
  }

  #[test]
  fn test_chunked_extensions_and_trailers_are_preserved() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: gzip, chunked\r\n",
      "\r\n",
      "7;foo=bar\r\nchunked\r\n",
      "6\r\n body!\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "\r\n"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);
    let response = reader.response().unwrap();

    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(
      Some(&"gzip, chunked".to_string()),
      response.header_value("Transfer-Encoding")
    );
    assert_eq!(
      Some("abc"),
      response.trailer_value("x-trace").map(String::as_str)
    );
  }

  #[test]
  fn test_chunked_terminator_read_error_is_preserved() {
    struct FailingTerminator {
      bytes: Cursor<&'static [u8]>,
    }

    impl Read for FailingTerminator {
      fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.bytes.read(buf)?;
        if read == 0 {
          Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "terminator read timed out",
          ))
        } else {
          Ok(read)
        }
      }
    }

    let mut reader = FailingTerminator {
      bytes: Cursor::new(b"4\r\nWiki"),
    };

    let err = super::read_chunked_response_body(&mut reader).unwrap_err();

    assert!(
      err.to_string().contains("terminator read timed out"),
      "unexpected error: {err}"
    );
    assert!(
      err
        .source()
        .and_then(|source| source.downcast_ref::<io::Error>())
        .is_some_and(|io_error| io_error.kind() == io::ErrorKind::TimedOut),
      "expected timed out io source: {err:?}"
    );
  }

  #[test]
  fn test_content_length_response_does_not_require_eof() {
    let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
    let kind = super::response_body_kind(raw.as_bytes(), false).unwrap();

    assert_eq!(ResponseBodyKind::ContentLength(2), kind);
  }

  #[test]
  fn test_head_response_has_no_body() {
    let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n");
    let kind = super::response_body_kind(raw.as_bytes(), true).unwrap();

    assert_eq!(ResponseBodyKind::NoBody, kind);
  }

  #[test]
  fn test_head_response_body_bytes_are_not_consumed() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 7\r\n",
      "\r\n",
      "ignored"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, true);

    let response = reader.response().unwrap();

    assert_eq!("", response.body().string().unwrap());
    assert_eq!(
      (raw.len() - "ignored".len()) as u64,
      cursor.position(),
      "HEAD responses are framed by the header block only"
    );
  }

  #[test]
  fn test_no_body_status_codes_ignore_framing_headers() {
    for status_line in [
      "HTTP/1.1 101 Switching Protocols",
      "HTTP/1.1 204 No Content",
      "HTTP/1.1 304 Not Modified",
    ] {
      let raw = format!("{status_line}\r\nContent-Length: 7\r\n\r\nignored");
      let url = url::Url::parse("http://localhost").unwrap();
      let mut cursor = Cursor::new(raw.as_bytes());
      let mut reader = ConnectionReader::new(&url, &mut cursor, false);

      let response = reader.response().unwrap();

      assert_eq!("", response.body().string().unwrap());
      assert_eq!(
        (raw.len() - "ignored".len()) as u64,
        cursor.position(),
        "{status_line} responses must not consume body bytes"
      );
    }
  }

  #[test]
  fn test_duplicate_content_length_with_same_value_is_allowed() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let response = reader.response().unwrap();

    assert_eq!("OK", response.body().string().unwrap());
  }

  #[test]
  fn test_duplicate_content_length_with_different_values_is_rejected() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "Content-Length: 3\r\n",
      "\r\n",
      "OK!"
    );

    let err = super::response_body_kind(raw.as_bytes(), false).unwrap_err();

    assert!(
      err.to_string().contains("Conflicting Content-Length"),
      "unexpected error: {err}"
    );
  }

  #[test]
  fn test_transfer_encoding_chunked_takes_precedence_over_content_length() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 999\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\nOK\r\n",
      "0\r\n\r\n"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let response = reader.response().unwrap();

    assert_eq!("OK", response.body().string().unwrap());
  }
}
