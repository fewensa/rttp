use std::io;
use std::io::Read;

use url::Url;

use crate::error;
use crate::response::Response;
use crate::types::RoUrl;

const HEADER_END: &[u8] = b"\r\n\r\n";
const CRLF: &[u8] = b"\r\n";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResponseBodyKind {
  NoBody,
  Chunked,
  ContentLength(usize),
  UntilEof,
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

  pub fn binary(&mut self) -> error::Result<Vec<u8>> {
    read_response_binary(self.reader, self.expect_no_body)
  }

  #[allow(dead_code)]
  pub fn response(&mut self) -> error::Result<Response> {
    Response::new(RoUrl::from(self.url.clone()), self.binary()?)
  }

  // todo Connection reader will read more type from io::Reader, like Chunk data, and Stream data.
}

fn read_response_binary<R>(reader: &mut R, expect_no_body: bool) -> error::Result<Vec<u8>>
where
  R: Read + ?Sized,
{
  let mut binary = read_response_header(reader)?;
  match response_body_kind(&binary, expect_no_body)? {
    ResponseBodyKind::NoBody => {}
    ResponseBodyKind::Chunked => {
      binary.extend_from_slice(&read_chunked_body(reader)?);
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
  Ok(binary)
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

pub(crate) fn response_body_kind(
  header: &[u8],
  expect_no_body: bool,
) -> error::Result<ResponseBodyKind> {
  if expect_no_body {
    return Ok(ResponseBodyKind::NoBody);
  }

  let header = String::from_utf8_lossy(header);
  let mut lines = header.lines();
  let status_line = lines
    .next()
    .ok_or_else(|| error::bad_response("Response not have status line"))?;
  let status_code = status_line
    .split_whitespace()
    .nth(1)
    .ok_or_else(|| error::bad_response("Response status not have code"))?
    .parse::<u16>()
    .map_err(|_| error::bad_response("Response status code is not a number"))?;

  if (100..200).contains(&status_code) || status_code == 204 || status_code == 304 {
    return Ok(ResponseBodyKind::NoBody);
  }

  let mut content_length = None;
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
      content_length = value.trim().parse::<usize>().ok();
    }
  }

  if chunked {
    Ok(ResponseBodyKind::Chunked)
  } else if let Some(content_length) = content_length {
    Ok(ResponseBodyKind::ContentLength(content_length))
  } else {
    Ok(ResponseBodyKind::UntilEof)
  }
}

fn read_chunked_body<R>(reader: &mut R) -> error::Result<Vec<u8>>
where
  R: Read + ?Sized,
{
  let mut body = Vec::new();

  loop {
    let line = read_crlf_line(reader)?;
    let chunk_size = parse_chunk_size(&line)?;

    if chunk_size == 0 {
      consume_trailers(reader)?;
      return Ok(body);
    }

    let current_len = body.len();
    body.resize(current_len + chunk_size, 0);
    reader
      .read_exact(&mut body[current_len..])
      .map_err(error::request)?;
    consume_crlf(reader)?;
  }
}

fn read_crlf_line<R>(reader: &mut R) -> error::Result<Vec<u8>>
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
  reader.read_exact(&mut suffix).map_err(error::request)?;
  if suffix == *CRLF {
    Ok(())
  } else {
    Err(error::bad_response("Invalid chunk terminator"))
  }
}

fn consume_trailers<R>(reader: &mut R) -> error::Result<()>
where
  R: Read + ?Sized,
{
  loop {
    let line = read_crlf_line(reader)?;
    if line == CRLF {
      return Ok(());
    }
  }
}

#[cfg(test)]
mod tests {
  use std::io::Cursor;

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
  fn test_chunked_extensions_and_trailers_are_ignored() {
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
}
