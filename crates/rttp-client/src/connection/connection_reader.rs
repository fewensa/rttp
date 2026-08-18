use std::io;
use std::io::Read;

use rttp_protocol::content_length::HttpContentLength;
use rttp_protocol::http1::{
  is_header_value_byte, is_reason_phrase_byte, is_token as is_http_token,
  parse_chunk_size as parse_protocol_chunk_size, ChunkSizeError,
};
use url::Url;

use crate::config::DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES;
use crate::error;
use crate::response::{InformationalResponse, Response};
use crate::types::{Header, RoUrl};

const HEADER_END: &[u8] = b"\r\n\r\n";
const CRLF: &[u8] = b"\r\n";
pub(crate) const MAX_CHUNKED_RESPONSE_LINE_BYTES: usize = 8 * 1024;
pub(crate) const MAX_RESPONSE_HEAD_BYTES: usize = 64 * 1024;

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
  pub(crate) informational_responses: Vec<InformationalResponse>,
  pub(crate) content_length: Option<HttpContentLength>,
  pub(crate) connection_reusable: bool,
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
    read_response_body_to_end(
      &mut self.body,
      &mut binary,
      DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
    )?;
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
  max_buffered_response_body_bytes: usize,
}

impl<'a> ConnectionReader<'a> {
  pub fn new(
    url: &'a Url,
    reader: &'a mut dyn io::Read,
    expect_no_body: bool,
  ) -> ConnectionReader<'a> {
    Self::new_with_limit(
      url,
      reader,
      expect_no_body,
      DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
    )
  }

  pub(crate) fn new_with_limit(
    url: &'a Url,
    reader: &'a mut dyn io::Read,
    expect_no_body: bool,
    max_buffered_response_body_bytes: usize,
  ) -> ConnectionReader<'a> {
    Self {
      url,
      reader,
      expect_no_body,
      max_buffered_response_body_bytes,
    }
  }

  #[allow(dead_code)]
  pub fn binary(&mut self) -> error::Result<Vec<u8>> {
    Ok(
      read_response_parts_with_limit(
        self.reader,
        self.expect_no_body,
        self.max_buffered_response_body_bytes,
      )?
      .binary,
    )
  }

  pub(crate) fn response_parts(&mut self) -> error::Result<ResponseParts> {
    read_response_parts_with_limit(
      self.reader,
      self.expect_no_body,
      self.max_buffered_response_body_bytes,
    )
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
    let parts = self.response_parts()?;
    Response::with_trailers_and_informational_and_limit(
      RoUrl::from(self.url.clone()),
      parts.binary,
      parts.trailers,
      parts.informational_responses,
      parts.content_length,
      self.max_buffered_response_body_bytes,
    )
  }

  // todo Connection reader will read more type from io::Reader, like Chunk data, and Stream data.
}

pub(crate) fn read_response_parts_with_limit<R>(
  reader: &mut R,
  expect_no_body: bool,
  max_body_bytes: usize,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  read_response_parts_with_informational_and_limit(reader, expect_no_body, max_body_bytes)
}

pub(crate) fn read_response_parts_with_informational_and_limit<R>(
  reader: &mut R,
  expect_no_body: bool,
  max_body_bytes: usize,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  let (binary, informational_responses) = read_response_head_with_informational(reader)?;
  read_response_parts_after_header_with_informational_and_limit(
    reader,
    expect_no_body,
    binary,
    informational_responses,
    max_body_bytes,
  )
}

pub(crate) fn read_response_head<R>(reader: &mut R) -> error::Result<Vec<u8>>
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

pub(crate) fn read_response_head_with_informational<R>(
  reader: &mut R,
) -> error::Result<(Vec<u8>, Vec<InformationalResponse>)>
where
  R: Read + ?Sized,
{
  let mut informational_responses = Vec::new();
  loop {
    let header = read_response_header(reader)?;
    let status_code = response_status_code(&header)?;
    if is_skippable_informational_status(status_code) {
      informational_responses.push(parse_informational_response(&header)?);
      continue;
    }
    return Ok((header, informational_responses));
  }
}

pub(crate) fn read_response_parts_after_header<R>(
  reader: &mut R,
  expect_no_body: bool,
  binary: Vec<u8>,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  read_response_parts_after_header_with_informational_and_limit(
    reader,
    expect_no_body,
    binary,
    Vec::new(),
    DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
  )
}

pub(crate) fn read_response_parts_after_header_with_informational_and_limit<R>(
  reader: &mut R,
  expect_no_body: bool,
  mut binary: Vec<u8>,
  informational_responses: Vec<InformationalResponse>,
  max_body_bytes: usize,
) -> error::Result<ResponseParts>
where
  R: Read + ?Sized,
{
  let close_connection = response_connection_should_close(&binary)?;
  let mut trailers = Vec::new();
  let body_kind = response_body_kind(&binary, expect_no_body)?;
  let content_length = content_length_from_response_body_kind(&body_kind);
  let connection_reusable = response_connection_reusable(&binary, &body_kind)?;
  match body_kind {
    ResponseBodyKind::NoBody => {}
    ResponseBodyKind::Chunked => {
      let mut body_reader = ResponseBodyReader::new(reader, ResponseBodyKind::Chunked);
      read_response_body_to_end(&mut body_reader, &mut binary, max_body_bytes)?;
      trailers = body_reader.trailers().clone();
    }
    ResponseBodyKind::ContentLength(content_length) => {
      let mut body_reader =
        ResponseBodyReader::new(reader, ResponseBodyKind::ContentLength(content_length));
      read_response_body_to_end(&mut body_reader, &mut binary, max_body_bytes)?;
    }
    ResponseBodyKind::UntilEof => {
      read_response_body_to_end(reader, &mut binary, max_body_bytes)?;
    }
  }
  Ok(ResponseParts {
    binary,
    trailers,
    informational_responses,
    content_length,
    connection_reusable,
    close_connection,
  })
}

pub(crate) fn content_length_from_response_body_kind(
  kind: &ResponseBodyKind,
) -> Option<HttpContentLength> {
  match kind {
    ResponseBodyKind::ContentLength(length) => Some(HttpContentLength::new(*length)),
    ResponseBodyKind::NoBody | ResponseBodyKind::Chunked | ResponseBodyKind::UntilEof => None,
  }
}

fn read_response_body_to_end<R>(
  reader: &mut R,
  binary: &mut Vec<u8>,
  max_body_bytes: usize,
) -> error::Result<usize>
where
  R: Read + ?Sized,
{
  let start = binary.len();
  let mut buffer = [0u8; 8 * 1024];
  loop {
    let body_len = binary.len() - start;
    let remaining = max_body_bytes - body_len;
    let read_limit = buffer.len().min(remaining.saturating_add(1));
    let read = reader
      .read(&mut buffer[..read_limit])
      .map_err(response_body_read_error)?;
    if read == 0 {
      return Ok(body_len);
    }
    if read > remaining {
      return Err(error::body_too_large(max_body_bytes));
    }
    binary.extend_from_slice(&buffer[..read]);
  }
}

pub(crate) fn response_connection_reusable(
  header: &[u8],
  body_kind: &ResponseBodyKind,
) -> error::Result<bool> {
  Ok(!matches!(body_kind, ResponseBodyKind::UntilEof) && !response_connection_should_close(header)?)
}

pub(crate) fn response_headers(header: &[u8]) -> error::Result<Vec<Header>> {
  let (_, header_lines) = split_response_head_lines(header)?;
  let mut headers = Vec::new();
  for line in header_lines.into_iter().filter(|line| !line.is_empty()) {
    let Some(colon) = line.iter().position(|byte| *byte == b':') else {
      return Err(error::bad_response("Invalid response header"));
    };
    if matches!(line.first(), Some(b' ' | b'\t')) {
      return Err(error::bad_response("Invalid response header"));
    }
    let (name, value) = line.split_at(colon);
    let value = &value[1..];
    let name = std::str::from_utf8(name).map_err(error::response)?;
    headers.push(Header::from_http1(name, decode_http1_text(value)));
  }
  Ok(headers)
}

pub(crate) fn response_connection_should_close(header: &[u8]) -> error::Result<bool> {
  let header = decode_http1_text(header);
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
  validate_response_header_lines(header)?;

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
    if is_supported_chunked_transfer_coding_path(&transfer_codings) {
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

fn is_supported_chunked_transfer_coding_path(transfer_codings: &[&str]) -> bool {
  transfer_codings.len() == 1 && transfer_codings[0].eq_ignore_ascii_case("chunked")
}

pub(crate) fn parse_informational_response(header: &[u8]) -> error::Result<InformationalResponse> {
  if header.len() > MAX_RESPONSE_HEAD_BYTES {
    return Err(error::bad_response(
      "HTTP informational response head is too large",
    ));
  }
  let (status_line, header_lines) = split_response_head_lines(header)?;
  let (version, status_code, reason) = parse_response_status_line(status_line)?;
  if !version.eq_ignore_ascii_case("HTTP/1.1") || !(100..200).contains(&status_code) {
    return Err(error::bad_response("Invalid informational response"));
  }

  let mut headers = Vec::new();
  for line in header_lines {
    if line.is_empty() {
      continue;
    }
    let Some(colon) = line.iter().position(|byte| *byte == b':') else {
      return Err(error::bad_response("Invalid informational response header"));
    };
    let (name, value) = line.split_at(colon);
    let value = &value[1..];
    let name = std::str::from_utf8(name).map_err(error::response)?;
    if !is_http_token(name) || !value.iter().copied().all(is_header_value_byte) {
      return Err(error::bad_response("Invalid informational response header"));
    }
    if name.eq_ignore_ascii_case("Content-Length") || name.eq_ignore_ascii_case("Transfer-Encoding")
    {
      return Err(error::bad_response(
        "Informational response must not declare body framing",
      ));
    }
    headers.push(Header::from_http1(name, decode_http1_text(value)));
  }

  Ok(InformationalResponse::new(
    status_code,
    reason.to_string(),
    headers,
  ))
}

fn split_response_head_lines(header: &[u8]) -> error::Result<(&[u8], Vec<&[u8]>)> {
  let header = header
    .strip_suffix(HEADER_END)
    .ok_or_else(|| error::bad_response("Invalid informational response"))?;
  if header.is_empty() {
    return Err(error::bad_response("Invalid informational response"));
  }
  if !has_only_crlf_line_breaks(header) {
    return Err(error::bad_response("Invalid informational response"));
  }
  let mut lines = header
    .split(|byte| *byte == b'\n')
    .map(|line| line.strip_suffix(b"\r").unwrap_or(line));
  let Some(status_line) = lines.next() else {
    return Err(error::bad_response("Invalid informational response"));
  };
  let mut header_lines = Vec::new();
  for line in lines {
    header_lines.push(line);
  }
  Ok((status_line, header_lines))
}

fn has_only_crlf_line_breaks(bytes: &[u8]) -> bool {
  for (index, byte) in bytes.iter().enumerate() {
    match *byte {
      b'\r' => {
        if bytes.get(index + 1) != Some(&b'\n') {
          return false;
        }
      }
      b'\n' if index == 0 || bytes.get(index - 1) != Some(&b'\r') => {
        return false;
      }
      b'\n' => {}
      _ => {}
    }
  }
  true
}

fn decode_http1_text(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| *byte as char).collect()
}

fn parse_response_status_line(status_line: &[u8]) -> error::Result<(&str, u16, &str)> {
  if status_line.contains(&b'\r') || status_line.contains(&b'\n') {
    return Err(error::bad_response("Invalid informational response"));
  }
  let status_line = std::str::from_utf8(status_line).map_err(error::response)?;
  let mut parts = status_line.splitn(3, ' ');
  let version = parts
    .next()
    .ok_or_else(|| error::bad_response("Invalid informational response"))?;
  let code = parts
    .next()
    .ok_or_else(|| error::bad_response("Invalid informational response"))?;
  if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(error::bad_response("Invalid informational response"));
  }
  let reason = parts.next().unwrap_or_default();
  if !reason.bytes().all(is_reason_phrase_byte) {
    return Err(error::bad_response("Invalid informational response"));
  }
  let status_code = code
    .parse::<u16>()
    .map_err(|_| error::bad_response("Invalid informational response"))?;
  Ok((version, status_code, reason))
}

fn validate_response_header_lines(header: &[u8]) -> error::Result<()> {
  let header = match header
    .windows(HEADER_END.len())
    .position(|w| w == HEADER_END)
  {
    Some(header_end) => &header[..header_end],
    None => header,
  };
  let header = String::from_utf8_lossy(header);
  for line in header.lines().skip(1).filter(|line| !line.is_empty()) {
    if !line.contains(':') {
      return Err(error::bad_response("Invalid response header"));
    }
  }
  Ok(())
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
  parse_protocol_chunk_size(line).map_err(|error| match error {
    ChunkSizeError::NotUtf8 => {
      let size = line
        .strip_suffix(CRLF)
        .unwrap_or(line)
        .splitn(2, |byte| *byte == b';')
        .next()
        .expect("split always returns the first chunk size segment");
      error::response(std::str::from_utf8(size).expect_err("chunk size is not UTF-8"))
    }
    ChunkSizeError::Empty => error::bad_response("Chunk size line is empty"),
    ChunkSizeError::Invalid => error::bad_response("Invalid chunk size"),
    ChunkSizeError::InvalidExtension => error::bad_response("Invalid chunk extension"),
  })
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

  Ok(Header::from_http1(name, value))
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

#[cfg(test)]
mod tests {
  use std::error::Error as StdError;
  use std::io::{self, Cursor, Read};

  use super::{ConnectionReader, ResponseBodyKind, MAX_RESPONSE_HEAD_BYTES};

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
  fn test_transfer_encoding_without_final_chunked_is_rejected() {
    for transfer_encoding in ["gzip", "chunked, gzip"] {
      let raw = format!(
        "HTTP/1.1 200 OK\r\n\
         Transfer-Encoding: {transfer_encoding}\r\n\
         \r\n\
         unframed"
      );

      let error = super::response_body_kind(raw.as_bytes(), false)
        .expect_err("unsupported transfer coding should be rejected");

      assert!(
        error
          .to_string()
          .contains("Unsupported Transfer-Encoding response body"),
        "unexpected error for {transfer_encoding}: {error}"
      );
    }
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
  fn test_malformed_response_header_without_colon_is_rejected_before_body() {
    let raw = concat!(
      "HTTP/1.1 200 OK\r\n",
      "BrokenHeader\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let error = reader
      .response()
      .expect_err("malformed response header should be rejected");

    assert!(
      error.to_string().contains("Invalid response header"),
      "unexpected error: {error}"
    );
    assert_eq!(
      (raw.len() - "OK".len()) as u64,
      cursor.position(),
      "malformed response headers must be rejected before body bytes are consumed"
    );
  }

  #[test]
  fn response_parts_preserve_skipped_informational_heads() {
    let raw = concat!(
      "HTTP/1.1 100 Continue\r\n",
      "X-Continue: yes\r\n",
      "\r\n",
      "HTTP/1.1 103 Early Hints\r\n",
      "Link: </style.css>; rel=preload\r\n",
      "X-Trace: hint\r\n",
      "\r\n",
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let response = reader.response().unwrap();
    let informational = response.informational_responses();

    assert_eq!(200, response.code());
    assert_eq!("OK", response.body().string().unwrap());
    assert_eq!(2, informational.len());
    assert_eq!(100, informational[0].code());
    assert_eq!("Continue", informational[0].reason());
    assert_eq!(
      Some("yes"),
      informational[0]
        .header_value("X-Continue")
        .map(String::as_str)
    );
    assert_eq!(103, informational[1].code());
    assert_eq!("Early Hints", informational[1].reason());
    assert_eq!(
      Some("</style.css>; rel=preload"),
      informational[1].header_value("Link").map(String::as_str)
    );
    assert_eq!(
      Some("hint"),
      informational[1].header_value("X-Trace").map(String::as_str)
    );
  }

  #[test]
  fn malformed_informational_response_header_is_rejected() {
    let raw = concat!(
      "HTTP/1.1 103 Early Hints\r\n",
      "BrokenHeader\r\n",
      "\r\n",
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let error = reader
      .response()
      .expect_err("malformed informational header should be rejected");

    assert!(
      error.to_string().contains("Invalid informational response"),
      "unexpected error: {error}"
    );
    assert_eq!(
      "HTTP/1.1 103 Early Hints\r\nBrokenHeader\r\n\r\n".len() as u64,
      cursor.position(),
      "malformed informational heads are rejected before final response bytes are consumed"
    );
  }

  #[test]
  fn ambiguous_informational_response_framing_is_rejected() {
    for framing in ["Content-Length: 2", "Transfer-Encoding: chunked"] {
      let raw = format!(
        "HTTP/1.1 103 Early Hints\r\n{framing}\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"
      );
      let url = url::Url::parse("http://localhost").unwrap();
      let mut cursor = Cursor::new(raw.as_bytes());
      let mut reader = ConnectionReader::new(&url, &mut cursor, false);

      let error = reader
        .response()
        .expect_err("informational body framing should be rejected");

      assert!(
        error
          .to_string()
          .contains("Informational response must not declare body framing"),
        "unexpected error for {framing}: {error}"
      );
    }
  }

  #[test]
  fn malformed_informational_status_line_is_rejected() {
    for status_line in [
      "HTTP/1.1 103 Early\x7fHints",
      "HTTP/1.0 103 Early Hints",
      "HTTP/9.9 103 Early Hints",
    ] {
      let raw = format!(
        "{status_line}\r\nX-Interim: ignored\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"
      );
      let url = url::Url::parse("http://localhost").unwrap();
      let mut cursor = Cursor::new(raw.as_bytes());
      let mut reader = ConnectionReader::new(&url, &mut cursor, false);

      let error = reader
        .response()
        .expect_err("malformed informational status line should be rejected");

      assert!(
        error.to_string().contains("Invalid informational response"),
        "unexpected error for {status_line:?}: {error}"
      );
    }
  }

  #[test]
  fn oversized_informational_response_head_is_rejected() {
    let oversized = "a".repeat(MAX_RESPONSE_HEAD_BYTES);
    let raw = format!(
      "HTTP/1.1 103 Early Hints\r\nX-Large: {oversized}\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK"
    );
    let url = url::Url::parse("http://localhost").unwrap();
    let mut cursor = Cursor::new(raw.as_bytes());
    let mut reader = ConnectionReader::new(&url, &mut cursor, false);

    let error = reader
      .response()
      .expect_err("oversized informational head should be rejected");

    assert!(
      error
        .to_string()
        .contains("HTTP informational response head is too large"),
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
      assert_eq!(None, response.content_length());
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
    let content_length = response
      .content_length()
      .expect("matching fixed length should be retained");
    assert_eq!(2, content_length.len());
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
