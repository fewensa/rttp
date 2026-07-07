use std::io;
use std::io::Read;

use url::Url;

use crate::error;
use crate::response::Response;
use crate::types::{Header, IntoHeader, RoUrl};

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
  pub(crate) close_connection: bool,
}

pub struct StreamingResponse<'a, R: Read + ?Sized> {
  url: RoUrl,
  head: Vec<u8>,
  body: ResponseBodyReader<'a, R>,
}

impl<'a, R: Read + ?Sized> StreamingResponse<'a, R> {
  pub fn code(&self) -> error::Result<u16> {
    response_status_code(&self.head)
  }

  pub fn headers(&self) -> error::Result<Vec<Header>> {
    response_headers(&self.head)
  }

  pub fn head(&self) -> &[u8] {
    &self.head
  }

  pub fn body_mut(&mut self) -> &mut ResponseBodyReader<'a, R> {
    &mut self.body
  }

  pub fn trailers(&self) -> &Vec<Header> {
    self.body.trailers()
  }

  pub fn trailer<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .trailers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn trailer_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.trailer(name).map(|header| header.value())
  }

  pub fn read_to_response(mut self) -> error::Result<Response> {
    let mut binary = self.head.clone();
    self.body.read_to_end(&mut binary).map_err(error::request)?;
    Response::with_trailers(self.url, binary, self.body.trailers().clone())
  }
}

pub struct ResponseBodyReader<'a, R: Read + ?Sized> {
  reader: &'a mut R,
  kind: ResponseBodyKind,
  remaining: usize,
  chunk_remaining: usize,
  chunk_needs_crlf: bool,
  trailers: Vec<Header>,
  eof: bool,
}

impl<'a, R: Read + ?Sized> ResponseBodyReader<'a, R> {
  fn new(reader: &'a mut R, kind: ResponseBodyKind) -> Self {
    let remaining = match kind {
      ResponseBodyKind::ContentLength(length) => length,
      _ => 0,
    };
    let eof = matches!(kind, ResponseBodyKind::NoBody);
    Self {
      reader,
      kind,
      remaining,
      chunk_remaining: 0,
      chunk_needs_crlf: false,
      trailers: Vec::new(),
      eof,
    }
  }

  pub fn trailers(&self) -> &Vec<Header> {
    &self.trailers
  }

  fn read_fixed_length(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.remaining == 0 || buf.is_empty() {
      self.eof = self.remaining == 0;
      return Ok(0);
    }

    let limit = buf.len().min(self.remaining);
    let read = self.reader.read(&mut buf[..limit])?;
    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "failed to fill whole buffer",
      ));
    }
    self.remaining -= read;
    if self.remaining == 0 {
      self.eof = true;
    }
    Ok(read)
  }

  fn read_chunked(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.eof || buf.is_empty() {
      return Ok(0);
    }

    if self.chunk_needs_crlf {
      consume_crlf(self.reader).map_err(to_io_error)?;
      self.chunk_needs_crlf = false;
    }

    while self.chunk_remaining == 0 {
      let line = read_bounded_crlf_line(self.reader, MAX_CHUNKED_RESPONSE_LINE_BYTES)
        .map_err(to_io_error)?;
      let chunk_size = parse_chunk_size(&line).map_err(to_io_error)?;
      if chunk_size == 0 {
        self.trailers = read_trailers(self.reader).map_err(to_io_error)?;
        self.eof = true;
        return Ok(0);
      }
      self.chunk_remaining = chunk_size;
    }

    let limit = buf.len().min(self.chunk_remaining);
    let read = self.reader.read(&mut buf[..limit])?;
    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "Unexpected end of chunked body",
      ));
    }
    self.chunk_remaining -= read;
    if self.chunk_remaining == 0 {
      self.chunk_needs_crlf = true;
    }
    Ok(read)
  }
}

impl<R: Read + ?Sized> Read for ResponseBodyReader<'_, R> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    match self.kind {
      ResponseBodyKind::NoBody => Ok(0),
      ResponseBodyKind::ContentLength(_) => self.read_fixed_length(buf),
      ResponseBodyKind::Chunked => self.read_chunked(buf),
      ResponseBodyKind::UntilEof => {
        let read = self.reader.read(buf)?;
        if read == 0 {
          self.eof = true;
        }
        Ok(read)
      }
    }
  }
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

  pub fn streaming_response(&mut self) -> error::Result<StreamingResponse<'_, dyn io::Read + '_>> {
    let head = read_response_head(self.reader)?;
    let kind = response_body_kind(&head, self.expect_no_body)?;
    Ok(StreamingResponse {
      url: RoUrl::from(self.url.clone()),
      head,
      body: ResponseBodyReader::new(self.reader, kind),
    })
  }

  #[allow(dead_code)]
  pub fn response(&mut self) -> error::Result<Response> {
    self.streaming_response()?.read_to_response()
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
  let binary = read_response_head(reader)?;
  read_response_parts_after_header(reader, expect_no_body, binary)
}

fn read_response_head<R>(reader: &mut R) -> error::Result<Vec<u8>>
where
  R: Read + ?Sized,
{
  loop {
    let header = read_response_header(reader)?;
    let status_code = response_status_code(&header)?;
    if is_skippable_informational_status(status_code) {
      continue;
    }
    return Ok(header);
  }
}

pub(crate) fn read_response_parts_after_header<R>(
  reader: &mut R,
  expect_no_body: bool,
  mut binary: Vec<u8>,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  let close_connection = response_connection_should_close(&binary)?;
  let mut trailers = Vec::new();
  match response_body_kind(&binary, expect_no_body)? {
    ResponseBodyKind::NoBody => {}
    ResponseBodyKind::Chunked => {
      let mut body_reader = ResponseBodyReader::new(reader, ResponseBodyKind::Chunked);
      body_reader
        .read_to_end(&mut binary)
        .map_err(response_body_read_error)?;
      trailers = body_reader.trailers().clone();
    }
    ResponseBodyKind::ContentLength(content_length) => {
      let mut body_reader =
        ResponseBodyReader::new(reader, ResponseBodyKind::ContentLength(content_length));
      body_reader
        .read_to_end(&mut binary)
        .map_err(response_body_read_error)?;
    }
    ResponseBodyKind::UntilEof => {
      reader.read_to_end(&mut binary).map_err(error::request)?;
    }
  }
  Ok(ResponseParts {
    binary,
    trailers,
    close_connection,
  })
}

pub(crate) fn response_headers(header: &[u8]) -> error::Result<Vec<Header>> {
  let header = String::from_utf8(header.to_vec()).map_err(error::response)?;
  Ok(
    header
      .lines()
      .skip(1)
      .filter(|line| !line.is_empty())
      .flat_map(|line| line.into_headers())
      .collect(),
  )
}

pub(crate) fn response_connection_should_close(header: &[u8]) -> error::Result<bool> {
  let header = String::from_utf8(header.to_vec()).map_err(error::response)?;
  let version = header
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().next())
    .unwrap_or_default();
  let mut has_keep_alive = false;

  for line in header.lines().skip(1) {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };
    if !name.eq_ignore_ascii_case("Connection") {
      continue;
    }

    for token in value.split(',').map(str::trim) {
      if token.eq_ignore_ascii_case("close") {
        return Ok(true);
      }
      if token.eq_ignore_ascii_case("keep-alive") {
        has_keep_alive = true;
      }
    }
  }

  Ok(version.eq_ignore_ascii_case("HTTP/1.0") && !has_keep_alive)
}

pub(crate) fn read_response_header<R>(reader: &mut R) -> error::Result<Vec<u8>>
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

pub(crate) fn is_skippable_informational_status(status_code: u16) -> bool {
  status_code == 100 || (102..200).contains(&status_code)
}

pub(crate) fn response_status_code(header: &[u8]) -> error::Result<u16> {
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
  let mut has_content_length = false;
  let mut invalid_content_length = false;
  let mut conflicting_content_length = false;
  let mut transfer_codings = Vec::new();

  for line in lines {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };

    if name.eq_ignore_ascii_case("Transfer-Encoding") {
      for token in value.split(',').map(str::trim) {
        if token.is_empty() {
          return Err(error::bad_response(
            "Unsupported Transfer-Encoding response body",
          ));
        }
        transfer_codings.push(token);
      }
    }

    if name.eq_ignore_ascii_case("Content-Length") {
      has_content_length = true;
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

  if !transfer_codings.is_empty() {
    if has_content_length {
      return Err(error::bad_response(
        "Transfer-Encoding conflicts with Content-Length",
      ));
    }
    if transfer_codings.len() == 1 && transfer_codings[0].eq_ignore_ascii_case("chunked") {
      return Ok(ResponseBodyKind::Chunked);
    }
    return Err(error::bad_response(
      "Unsupported Transfer-Encoding response body",
    ));
  }

  if conflicting_content_length {
    Err(error::bad_response("Conflicting Content-Length headers"))
  } else if invalid_content_length {
    Err(error::bad_response("Invalid Content-Length header"))
  } else if let Some(content_length) = content_length {
    Ok(ResponseBodyKind::ContentLength(content_length))
  } else {
    Ok(ResponseBodyKind::UntilEof)
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
  let line = line.strip_suffix(CRLF).unwrap_or(line);
  let (size, extensions) = line
    .iter()
    .position(|byte| *byte == b';')
    .map_or((line, None), |index| {
      (&line[..index], Some(&line[index + 1..]))
    });
  let size = std::str::from_utf8(size).map_err(error::response)?.trim();
  if size.is_empty() {
    return Err(error::bad_response("Chunk size line is empty"));
  }
  if let Some(extensions) = extensions {
    validate_chunk_extensions(extensions)?;
  }

  usize::from_str_radix(size, 16).map_err(|_| error::bad_response("Invalid chunk size"))
}

fn validate_chunk_extensions(mut bytes: &[u8]) -> error::Result<()> {
  loop {
    bytes = trim_bws(bytes);
    let token_len = bytes
      .iter()
      .position(|byte| !is_tchar(*byte))
      .unwrap_or(bytes.len());
    if token_len == 0 {
      return Err(error::bad_response("Invalid chunk extension"));
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
          return Err(error::bad_response("Invalid chunk extension"));
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
      return Err(error::bad_response("Invalid chunk extension"));
    }
  }
}

fn parse_quoted_chunk_extension(mut bytes: &[u8]) -> error::Result<&[u8]> {
  loop {
    let Some((&byte, rest)) = bytes.split_first() else {
      return Err(error::bad_response("Invalid chunk extension"));
    };
    match byte {
      b'"' => return Ok(rest),
      b'\\' => {
        let Some((&escaped, rest)) = rest.split_first() else {
          return Err(error::bad_response("Invalid chunk extension"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(error::bad_response("Invalid chunk extension"));
        }
        bytes = rest;
      }
      byte if is_qdtext(byte) => bytes = rest,
      _ => return Err(error::bad_response("Invalid chunk extension")),
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

fn is_tchar(byte: u8) -> bool {
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

fn is_qdtext(byte: u8) -> bool {
  matches!(byte, b'\t' | b' ' | b'!' | 0x23..=0x5b | 0x5d..=0x7e | 0x80..=0xff)
}

fn is_quoted_pair_char(byte: u8) -> bool {
  matches!(byte, b'\t' | b' ' | 0x21..=0x7e | 0x80..=0xff)
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
  validate_response_trailer_header(name, value)?;

  Ok(Header::new(name, value))
}

fn to_io_error(err: error::Error) -> io::Error {
  if let Some(kind) = std::error::Error::source(&err)
    .and_then(|source| source.downcast_ref::<io::Error>())
    .map(|source| source.kind())
  {
    io::Error::new(kind, err)
  } else if let Some(source) = std::error::Error::source(&err) {
    io::Error::new(io::ErrorKind::InvalidData, source.to_string())
  } else {
    io::Error::new(io::ErrorKind::InvalidData, err)
  }
}

fn response_body_read_error(err: io::Error) -> error::Error {
  match err.kind() {
    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
      error::bad_response(err.to_string())
    }
    _ => error::request(err),
  }
}

pub(crate) fn validate_response_trailer_header(name: &str, value: &str) -> error::Result<()> {
  if !is_http_token(name) || !value.bytes().all(is_header_value_byte) {
    return Err(error::bad_response("Invalid trailer header"));
  }
  if is_forbidden_response_trailer_name(name) {
    return Err(error::bad_response("Forbidden trailer header"));
  }
  Ok(())
}

fn is_forbidden_response_trailer_name(name: &str) -> bool {
  matches!(
    name.trim().to_ascii_lowercase().as_str(),
    "authorization"
      | "connection"
      | "content-length"
      | "cookie"
      | "host"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "www-authenticate"
      | "set-cookie"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
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

fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
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
  fn streaming_response_reads_fixed_length_body_incrementally() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 5\r\n",
      "X-Trace: head\r\n",
      "\r\n",
      "hello",
      "next"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let mut response = reader.streaming_response().unwrap();
    let mut buf = [0; 2];

    assert_eq!(200, response.code().unwrap());
    assert_eq!(
      Some("head"),
      response
        .headers()
        .unwrap()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("x-trace"))
        .map(|header| header.value().as_str())
    );
    assert_eq!(2, response.body_mut().read(&mut buf).unwrap());
    assert_eq!(b"he", &buf);
    assert_eq!(2, response.body_mut().read(&mut buf).unwrap());
    assert_eq!(b"ll", &buf);
    assert_eq!(1, response.body_mut().read(&mut buf).unwrap());
    assert_eq!(b"o", &buf[..1]);
    assert_eq!(0, response.body_mut().read(&mut buf).unwrap());
    assert!(response.trailers().is_empty());
    drop(response);
    assert_eq!((raw.len() - "next".len()) as u64, cursor.position());
  }

  #[test]
  fn streaming_response_reads_chunked_body_and_exposes_trailers_after_eof() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\nhe\r\n",
      "3\r\nllo\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "\r\n"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let mut response = reader.streaming_response().unwrap();
    let mut body = Vec::new();
    response.body_mut().read_to_end(&mut body).unwrap();

    assert_eq!(b"hello", body.as_slice());
    assert_eq!(
      Some("abc"),
      response.trailer_value("x-trace").map(String::as_str)
    );
  }

  #[test]
  fn test_non_chunked_transfer_coding_before_chunked_is_rejected() {
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

    let error = reader
      .response()
      .expect_err("unsupported transfer coding should be rejected");

    assert!(
      error
        .to_string()
        .contains("Unsupported Transfer-Encoding response body"),
      "unexpected error: {error}"
    );
  }

  #[test]
  fn test_forbidden_chunked_response_trailer_is_rejected() {
    for name in [
      "Transfer-Encoding",
      "Content-Length",
      "Host",
      "Authorization",
      "Proxy-Authorization",
      "WWW-Authenticate",
      "Proxy-Authenticate",
      "Cookie",
      "Connection",
      "TE",
      "Trailer",
      "Set-Cookie",
      "Upgrade",
    ] {
      let raw = format!(
        "HTTP/1.1 200 OK\r\n\
         Transfer-Encoding: chunked\r\n\
         \r\n\
         2\r\n\
         OK\r\n\
         0\r\n\
         {name}: unsafe\r\n\
         \r\n"
      );
      let url = url::Url::parse("http://localhost").unwrap();
      let mut cursor = Cursor::new(raw.as_bytes());
      let mut reader = ConnectionReader::new(&url, &mut cursor, false);

      let error = reader
        .response()
        .expect_err("forbidden response trailer should be rejected");

      assert!(
        error.to_string().contains("Forbidden trailer header"),
        "unexpected error for {name}: {error}"
      );
    }
  }

  #[test]
  fn test_malformed_chunked_response_trailer_is_rejected() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "2\r\n",
      "OK\r\n",
      "0\r\n",
      "Bad Name: unsafe\r\n",
      "\r\n"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let error = reader
      .response()
      .expect_err("malformed response trailer should be rejected");

    assert!(
      error.to_string().contains("Invalid trailer header"),
      "unexpected error: {error}"
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

    let mut body_reader = super::ResponseBodyReader::new(&mut reader, ResponseBodyKind::Chunked);
    let mut body = Vec::new();

    let err = body_reader.read_to_end(&mut body).unwrap_err();

    assert_eq!(io::ErrorKind::TimedOut, err.kind());
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
  fn test_transfer_encoding_chunked_with_content_length_is_rejected() {
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

    let error = reader
      .response()
      .expect_err("Transfer-Encoding with Content-Length should be rejected");

    assert!(
      error
        .to_string()
        .contains("Transfer-Encoding conflicts with Content-Length"),
      "unexpected error: {error}"
    );
  }
}
