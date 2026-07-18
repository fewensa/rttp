use super::*;
pub(crate) use rttp_protocol::http1::{
  is_header_value_byte, is_qdtext, is_quoted_pair_char, is_token as is_http_token,
  parse_chunk_size as parse_protocol_chunk_size,
};

pub struct RequestBodyReader<'a, R: BufRead> {
  pub(crate) reader: &'a mut R,
  pub(crate) kind: RequestBodyKind,
  pub(crate) remaining: usize,
  pub(crate) chunk_remaining: usize,
  pub(crate) chunk_needs_crlf: bool,
  pub(crate) body_bytes_read: usize,
  pub(crate) trailers: Vec<(String, String)>,
  pub(crate) eof: bool,
  pub(crate) normalize_timeouts: bool,
}

impl<'a, R: BufRead> RequestBodyReader<'a, R> {
  pub(crate) fn new(reader: &'a mut R, kind: RequestBodyKind, normalize_timeouts: bool) -> Self {
    let remaining = match kind {
      RequestBodyKind::ContentLength(length) => length,
      RequestBodyKind::Chunked => 0,
    };
    Self {
      reader,
      kind,
      remaining,
      chunk_remaining: 0,
      chunk_needs_crlf: false,
      body_bytes_read: 0,
      trailers: Vec::new(),
      eof: matches!(kind, RequestBodyKind::ContentLength(0)),
      normalize_timeouts,
    }
  }

  pub fn trailers(&self) -> &[(String, String)] {
    &self.trailers
  }

  pub(crate) fn read_fixed_length(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.remaining == 0 || buf.is_empty() {
      self.eof = self.remaining == 0;
      return Ok(0);
    }

    let limit = buf.len().min(self.remaining);
    let read = self
      .reader
      .read(&mut buf[..limit])
      .map_err(|err| self.normalize_error(err))?;
    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "incomplete HTTP request body",
      ));
    }
    self.remaining -= read;
    if self.remaining == 0 {
      self.eof = true;
    }
    Ok(read)
  }

  pub(crate) fn read_chunked(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.eof || buf.is_empty() {
      return Ok(0);
    }

    if self.chunk_needs_crlf {
      consume_crlf(self.reader, &mut self.body_bytes_read)
        .map_err(|err| self.normalize_error(err))?;
      self.chunk_needs_crlf = false;
    }

    while self.chunk_remaining == 0 {
      let line = read_bounded_crlf_line(self.reader, &mut self.body_bytes_read)
        .map_err(|err| self.normalize_error(err))?;
      let chunk_size = parse_chunk_size(&line)?;
      if chunk_size == 0 {
        self.trailers = read_trailers(self.reader, &mut self.body_bytes_read)
          .map_err(|err| self.normalize_error(err))?;
        self.eof = true;
        return Ok(0);
      }
      add_request_body_bytes(&mut self.body_bytes_read, chunk_size)?;
      self.chunk_remaining = chunk_size;
    }

    let limit = buf.len().min(self.chunk_remaining);
    let read = self
      .reader
      .read(&mut buf[..limit])
      .map_err(|err| self.normalize_error(err))?;
    if read == 0 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "incomplete chunked request body",
      ));
    }
    self.chunk_remaining -= read;
    if self.chunk_remaining == 0 {
      self.chunk_needs_crlf = true;
    }
    Ok(read)
  }

  pub(crate) fn normalize_error(&self, err: io::Error) -> io::Error {
    if self.normalize_timeouts && err.kind() == io::ErrorKind::WouldBlock {
      io::Error::new(io::ErrorKind::TimedOut, err)
    } else {
      err
    }
  }
}

impl<R: BufRead> Read for RequestBodyReader<'_, R> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    match self.kind {
      RequestBodyKind::ContentLength(_) => self.read_fixed_length(buf),
      RequestBodyKind::Chunked => self.read_chunked(buf),
    }
  }
}

pub(crate) fn find_header_end(raw: &[u8]) -> Option<usize> {
  raw.windows(4).position(|window| window == b"\r\n\r\n")
}

pub(crate) fn reject_oversized_request_head(length: usize) -> io::Result<()> {
  if length > MAX_REQUEST_HEAD_BYTES {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "request head is too large",
    ))
  } else {
    Ok(())
  }
}

pub(crate) fn reject_oversized_request_body(length: usize) -> io::Result<()> {
  if length > MAX_REQUEST_BODY_BYTES {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "request body is too large",
    ))
  } else {
    Ok(())
  }
}

pub(crate) fn is_authority_form_request_target(target: &str) -> bool {
  if target.is_empty()
    || target.starts_with('/')
    || target.starts_with('*')
    || target.contains("://")
    || target.contains(['/', '?', '#'])
  {
    return false;
  }

  let Some((host, port)) = target.rsplit_once(':') else {
    return false;
  };
  !host.is_empty() && !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn checked_request_message_len(
  header_end: usize,
  content_length: usize,
) -> io::Result<usize> {
  header_end
    .checked_add(4)
    .and_then(|body_start| body_start.checked_add(content_length))
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))
}

pub(crate) struct RequestHead {
  pub(crate) method: String,
  pub(crate) target: String,
  pub(crate) version: String,
  pub(crate) headers: Vec<(String, String)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequestBodyKind {
  ContentLength(usize),
  Chunked,
}

pub(crate) struct ChunkedRequestBody {
  pub(crate) body: Vec<u8>,
  pub(crate) trailers: Vec<(String, String)>,
}

pub(crate) fn parse_request_head(raw: &[u8]) -> io::Result<RequestHead> {
  let text = decode_http1_text(raw);
  let mut lines = text.split("\r\n");
  let request_line = lines
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
  let mut parts = request_line.split(' ');
  let method = parts
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request method"))?;
  let target = parts
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request target"))?;
  let version = parts
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request version"))?;

  if parts.next().is_some() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid request line",
    ));
  }
  validate_request_line(method, target, version)?;

  let headers = parse_header_lines(lines)?;
  validate_host_header(version, target, &headers)?;
  let target = normalize_request_target(target);

  Ok(RequestHead {
    method: method.to_string(),
    target,
    version: version.to_string(),
    headers,
  })
}

fn decode_http1_text(bytes: &[u8]) -> String {
  let mut text = String::new();
  let mut remaining = bytes;
  while !remaining.is_empty() {
    match std::str::from_utf8(remaining) {
      Ok(valid) => {
        text.push_str(valid);
        break;
      }
      Err(error) => {
        let valid_up_to = error.valid_up_to();
        text.push_str(std::str::from_utf8(&remaining[..valid_up_to]).expect("valid UTF-8 prefix"));
        let invalid_len = error.error_len().unwrap_or(remaining.len() - valid_up_to);
        for byte in &remaining[valid_up_to..valid_up_to + invalid_len] {
          text.push(*byte as char);
        }
        remaining = &remaining[valid_up_to + invalid_len..];
      }
    }
  }
  text
}

pub(crate) fn validate_request_line(method: &str, target: &str, version: &str) -> io::Result<()> {
  if !is_http_token(method) {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid request method",
    ));
  }
  if target.is_empty() || !target.bytes().all(is_request_target_byte) {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid request target",
    ));
  }
  if !is_valid_request_target_for_method(method, target) {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid request target",
    ));
  }
  if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid request version",
    ));
  }

  Ok(())
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequestTargetForm {
  Origin,
  Absolute,
  Asterisk,
  Authority,
}

pub(crate) fn is_valid_request_target_for_method(method: &str, target: &str) -> bool {
  let Some(form) = request_target_form(target) else {
    return false;
  };

  match form {
    RequestTargetForm::Origin | RequestTargetForm::Absolute => method != "CONNECT",
    RequestTargetForm::Asterisk => method == "OPTIONS",
    RequestTargetForm::Authority => method == "CONNECT",
  }
}

pub(crate) fn request_target_form(target: &str) -> Option<RequestTargetForm> {
  if target == "*" {
    Some(RequestTargetForm::Asterisk)
  } else if target.starts_with('/') {
    Some(RequestTargetForm::Origin)
  } else if is_absolute_form_target(target) {
    Some(RequestTargetForm::Absolute)
  } else if is_authority_form_target(target) {
    Some(RequestTargetForm::Authority)
  } else {
    None
  }
}

pub(crate) fn is_absolute_form_target(target: &str) -> bool {
  let Some((scheme, rest)) = target.split_once("://") else {
    return false;
  };
  if !is_uri_scheme(scheme) {
    return false;
  }
  if rest.contains('#') {
    return false;
  }

  let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
  is_valid_host_authority(&rest[..authority_end], false)
}

pub(crate) fn normalize_request_target(target: &str) -> String {
  if request_target_form(target) != Some(RequestTargetForm::Absolute) {
    return target.to_string();
  }

  let (_, rest) = target
    .split_once("://")
    .expect("absolute-form target must include a scheme separator");
  let path_start = rest.find(['/', '?']).unwrap_or(rest.len());
  let origin = &rest[path_start..];

  if origin.is_empty() {
    "/".to_string()
  } else if origin.starts_with('?') {
    format!("/{origin}")
  } else {
    origin.to_string()
  }
}

pub(crate) fn is_uri_scheme(scheme: &str) -> bool {
  let mut bytes = scheme.bytes();
  let Some(first) = bytes.next() else {
    return false;
  };
  first.is_ascii_alphabetic()
    && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

pub(crate) fn is_authority_form_target(target: &str) -> bool {
  is_valid_host_authority(target, true)
}

pub(crate) fn is_valid_host_authority(authority: &str, require_port: bool) -> bool {
  if authority.is_empty()
    || authority
      .bytes()
      .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'@'))
  {
    return false;
  }

  if let Some(rest) = authority.strip_prefix('[') {
    let Some(end) = rest.find(']') else {
      return false;
    };
    let host = &rest[..end];
    let suffix = &rest[end + 1..];
    if host.is_empty() || host.bytes().any(|byte| matches!(byte, b'[' | b']')) {
      return false;
    }
    return validate_host_port_suffix(suffix, require_port);
  }

  let colon_count = authority.bytes().filter(|byte| *byte == b':').count();
  match colon_count {
    0 => !require_port && is_valid_reg_name_or_ipv4(authority),
    1 => {
      let Some((host, port)) = authority.rsplit_once(':') else {
        return false;
      };
      is_valid_reg_name_or_ipv4(host) && is_valid_port(port)
    }
    _ => false,
  }
}

pub(crate) fn validate_host_port_suffix(suffix: &str, require_port: bool) -> bool {
  if suffix.is_empty() {
    return !require_port;
  }
  let Some(port) = suffix.strip_prefix(':') else {
    return false;
  };
  is_valid_port(port)
}

pub(crate) fn is_valid_reg_name_or_ipv4(host: &str) -> bool {
  !host.is_empty()
    && host
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
}

pub(crate) fn is_valid_port(port: &str) -> bool {
  !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn parse_header_lines<'a>(
  lines: impl Iterator<Item = &'a str>,
) -> io::Result<Vec<(String, String)>> {
  parse_header_lines_with_error(lines, "invalid request header")
}

pub(crate) fn parse_header_lines_with_error<'a>(
  lines: impl Iterator<Item = &'a str>,
  invalid_line_error: &'static str,
) -> io::Result<Vec<(String, String)>> {
  let mut headers = Vec::new();

  for line in lines {
    if line.is_empty() {
      continue;
    }
    if line.starts_with(' ') || line.starts_with('\t') {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        invalid_line_error,
      ));
    }
    let (name, value) = line
      .split_once(':')
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, invalid_line_error))?;
    if !is_http_token(name) || !value.bytes().all(is_header_value_byte) {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        invalid_line_error,
      ));
    }
    headers.push((name.trim().to_string(), value.trim().to_string()));
  }

  Ok(headers)
}

pub(crate) fn validate_host_header(
  version: &str,
  target: &str,
  headers: &[(String, String)],
) -> io::Result<()> {
  if version != "HTTP/1.1" {
    return Ok(());
  }

  let mut host_headers = headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Host"));
  let Some((_, host)) = host_headers.next() else {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "HTTP/1.1 request requires exactly one Host header",
    ));
  };

  if host_headers.next().is_some() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "HTTP/1.1 request requires exactly one Host header",
    ));
  }

  let host_matches_target = match request_target_form(target) {
    Some(RequestTargetForm::Origin | RequestTargetForm::Absolute | RequestTargetForm::Asterisk) => {
      true
    }
    Some(RequestTargetForm::Authority) => host == target,
    None => false,
  };

  if !host_matches_target || !is_valid_host_authority(host, false) {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid Host header",
    ));
  }

  Ok(())
}

pub(crate) fn is_request_target_byte(byte: u8) -> bool {
  byte > 0x20 && byte != 0x7f
}

pub(crate) fn optional_header_content_length(
  headers: &[(String, String)],
) -> io::Result<Option<usize>> {
  let mut length = None;

  for (_, value) in headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
  {
    for token in value.split(',') {
      let token = token.trim();
      if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "invalid Content-Length header",
        ));
      }
      let parsed = token
        .parse::<usize>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length header"))?;
      if length
        .replace(parsed)
        .is_some_and(|previous| previous != parsed)
      {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "conflicting Content-Length headers",
        ));
      }
    }
  }

  Ok(length)
}

pub(crate) fn request_body_kind(headers: &[(String, String)]) -> io::Result<RequestBodyKind> {
  let content_length = optional_header_content_length(headers)?;
  let mut transfer_codings = Vec::new();

  for (_, value) in headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Transfer-Encoding"))
  {
    for token in value.split(',').map(str::trim) {
      if token.is_empty() {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "unsupported Transfer-Encoding request body",
        ));
      }
      transfer_codings.push(token);
    }
  }

  if transfer_codings.is_empty() {
    return Ok(RequestBodyKind::ContentLength(content_length.unwrap_or(0)));
  }

  if content_length.is_some() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Transfer-Encoding conflicts with Content-Length",
    ));
  }

  if transfer_codings.len() == 1 && transfer_codings[0].eq_ignore_ascii_case("chunked") {
    Ok(RequestBodyKind::Chunked)
  } else {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "unsupported Transfer-Encoding request body",
    ))
  }
}

pub(crate) fn read_chunked_request_body<R>(reader: &mut R) -> io::Result<ChunkedRequestBody>
where
  R: BufRead,
{
  let mut body = Vec::new();
  let mut body_bytes_read = 0;

  loop {
    let line = read_bounded_crlf_line(reader, &mut body_bytes_read)?;
    let chunk_size = parse_chunk_size(&line)?;

    if chunk_size == 0 {
      let trailers = read_trailers(reader, &mut body_bytes_read)?;
      return Ok(ChunkedRequestBody { body, trailers });
    }

    add_request_body_bytes(&mut body_bytes_read, chunk_size)?;

    let copied = {
      let mut chunk_reader = reader.take(chunk_size as u64);
      io::copy(&mut chunk_reader, &mut body)?
    };

    if copied != chunk_size as u64 {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "incomplete chunked request body",
      ));
    };
    consume_crlf(reader, &mut body_bytes_read)?;
  }
}

pub(crate) fn add_request_body_bytes(total: &mut usize, length: usize) -> io::Result<()> {
  *total = total
    .checked_add(length)
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))?;
  reject_oversized_request_body(*total)
}

pub(crate) fn read_bounded_crlf_line<R>(
  reader: &mut R,
  body_bytes_read: &mut usize,
) -> io::Result<Vec<u8>>
where
  R: BufRead,
{
  let mut line = Vec::new();
  let remaining = MAX_REQUEST_BODY_BYTES
    .checked_sub(*body_bytes_read)
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))?;
  let read = {
    let mut limited_reader = reader.take(remaining.saturating_add(1) as u64);
    limited_reader.read_until(b'\n', &mut line)?
  };
  if read == 0 {
    return Err(io::Error::new(
      io::ErrorKind::UnexpectedEof,
      "incomplete chunked request body",
    ));
  }
  add_request_body_bytes(body_bytes_read, read)?;
  if line.ends_with(b"\r\n") {
    Ok(line)
  } else {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid chunked request line terminator",
    ))
  }
}

pub(crate) fn parse_chunk_size(line: &[u8]) -> io::Result<usize> {
  parse_protocol_chunk_size(line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn consume_crlf<R>(reader: &mut R, body_bytes_read: &mut usize) -> io::Result<()>
where
  R: BufRead,
{
  add_request_body_bytes(body_bytes_read, 2)?;
  let mut suffix = [0u8; 2];
  reader.read_exact(&mut suffix).map_err(|err| {
    if err.kind() == io::ErrorKind::UnexpectedEof {
      io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "incomplete chunked request body",
      )
    } else {
      err
    }
  })?;
  if suffix == *b"\r\n" {
    Ok(())
  } else {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid chunk terminator",
    ))
  }
}

pub(crate) fn read_trailers<R>(
  reader: &mut R,
  body_bytes_read: &mut usize,
) -> io::Result<Vec<(String, String)>>
where
  R: BufRead,
{
  let mut lines = Vec::new();

  loop {
    let line = read_bounded_crlf_line(reader, body_bytes_read)?;
    if line == b"\r\n" {
      return parse_trailer_lines(lines.iter().map(String::as_str));
    }
    let line = line.strip_suffix(b"\r\n").unwrap_or(&line);
    let line = std::str::from_utf8(line)
      .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "trailer line is not UTF-8"))?;
    lines.push(line.to_string());
  }
}

pub(crate) fn parse_trailer_lines<'a>(
  lines: impl Iterator<Item = &'a str>,
) -> io::Result<Vec<(String, String)>> {
  let trailers = parse_header_lines_with_error(lines, "invalid request trailer")?;
  if trailers
    .iter()
    .any(|(name, _)| is_forbidden_trailer_name(name))
  {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "forbidden request trailer",
    ));
  }
  Ok(trailers)
}

pub(crate) fn connection_header_has_token(value: Option<&str>, expected: &str) -> bool {
  value.is_some_and(|value| {
    value
      .split(',')
      .any(|token| token.trim().eq_ignore_ascii_case(expected))
  })
}

pub(crate) fn assert_valid_header_component(component: &str) {
  assert!(
    !component.contains('\r') && !component.contains('\n'),
    "response headers must not contain CR or LF"
  );
}

pub(crate) fn validate_early_hints_link_value(value: &str) -> Result<&str, HttpEarlyHintsError> {
  let value = validate_early_hints_header_value(value)?;
  if value.trim().is_empty() {
    return Err(HttpEarlyHintsError::new(
      "Early Hints Link header must not be empty",
    ));
  }
  Ok(value)
}

pub(crate) fn validate_early_hints_metadata_name(name: &str) -> Result<&str, HttpEarlyHintsError> {
  if !is_http_token(name) {
    return Err(HttpEarlyHintsError::new(
      "Early Hints metadata header name is invalid",
    ));
  }
  if name.eq_ignore_ascii_case("Link") {
    return Err(HttpEarlyHintsError::new(
      "Early Hints Link headers must be provided through the links argument",
    ));
  }
  if is_forbidden_early_hints_metadata_name(name) {
    return Err(HttpEarlyHintsError::new(
      "Early Hints metadata must not contain framing or connection fields",
    ));
  }
  Ok(name)
}

pub(crate) fn validate_early_hints_header_value(value: &str) -> Result<&str, HttpEarlyHintsError> {
  if value.len() > MAX_EARLY_HINTS_VALUE_BYTES {
    return Err(HttpEarlyHintsError::new(
      "Early Hints header value is too large",
    ));
  }
  if !value.bytes().all(is_header_value_byte) {
    return Err(HttpEarlyHintsError::new(
      "Early Hints header value contains invalid bytes",
    ));
  }
  Ok(value)
}

pub(crate) fn is_forbidden_early_hints_metadata_name(name: &str) -> bool {
  matches!(
    name.to_ascii_lowercase().as_str(),
    "connection"
      | "content-length"
      | "keep-alive"
      | "proxy-connection"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
  )
}

pub(crate) fn assert_allowed_trailer_name(name: &str) {
  assert!(
    is_http_token(name),
    "response trailers must use valid field names"
  );
  assert!(
    !is_forbidden_trailer_name(name),
    "response trailers must not contain framing or routing fields"
  );
}

pub(crate) fn is_forbidden_trailer_name(name: &str) -> bool {
  matches!(
    name.to_ascii_lowercase().as_str(),
    "authorization"
      | "cache-control"
      | "connection"
      | "content-encoding"
      | "content-length"
      | "content-range"
      | "content-type"
      | "cookie"
      | "host"
      | "keep-alive"
      | "max-forwards"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "proxy-connection"
      | "www-authenticate"
      | "set-cookie"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
  )
}

pub(crate) fn response_status_allows_body(status_code: u16) -> bool {
  !(status_code / 100 == 1 || status_code == 204 || status_code == 304)
}

pub(crate) fn is_bad_request_error(err: &io::Error) -> bool {
  matches!(
    err.kind(),
    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
  )
}

pub(crate) fn is_expectation_failed_error(err: &io::Error) -> bool {
  err
    .get_ref()
    .is_some_and(|source| source.is::<UnsupportedExpectation>())
}

pub(crate) fn bad_request_response() -> HttpResponse {
  HttpResponse::new(400, "Bad Request").body("Bad Request")
}

pub(crate) fn expectation_failed_response() -> HttpResponse {
  HttpResponse::new(417, "Expectation Failed").body("Expectation Failed")
}

#[derive(Debug)]
pub(crate) struct UnsupportedExpectation;

impl fmt::Display for UnsupportedExpectation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("unsupported Expect header")
  }
}

impl Error for UnsupportedExpectation {}
