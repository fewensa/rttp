use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};

use socket2::{Domain, Protocol, Socket, Type};

pub struct HttpServer {
  listener: TcpListener,
}

impl HttpServer {
  pub fn bind<A>(addr: A) -> io::Result<Self>
  where
    A: ToSocketAddrs,
  {
    let mut last_err = None;

    for addr in addr.to_socket_addrs()? {
      let socket = match Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP)) {
        Ok(socket) => socket,
        Err(err) => {
          last_err = Some(err);
          continue;
        }
      };

      if let Err(err) = socket.set_reuse_address(true) {
        last_err = Some(err);
        continue;
      }
      if let Err(err) = socket.bind(&addr.into()) {
        last_err = Some(err);
        continue;
      }
      if let Err(err) = socket.listen(128) {
        last_err = Some(err);
        continue;
      }

      return Ok(Self {
        listener: TcpListener::from(socket),
      });
    }

    Err(
      last_err
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "address did not resolve")),
    )
  }

  pub fn local_addr(&self) -> io::Result<SocketAddr> {
    self.listener.local_addr()
  }

  pub fn accept_one<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpResponse,
  {
    self.handle_next_connection(handler)
  }

  pub fn serve_requests<F>(&self, request_count: usize, mut handler: F) -> io::Result<()>
  where
    F: FnMut(Request) -> HttpResponse,
  {
    let mut served = 0;

    while served < request_count {
      let (stream, _) = self.listener.accept()?;
      served += self.handle_connection(stream, request_count - served, &mut handler)?;
    }

    Ok(())
  }

  fn handle_connection<F>(
    &self,
    stream: TcpStream,
    request_limit: usize,
    handler: &mut F,
  ) -> io::Result<usize>
  where
    F: FnMut(Request) -> HttpResponse,
  {
    let mut reader = BufReader::new(stream);
    let mut served = 0;

    while served < request_limit {
      let request = match Request::read_next_from(&mut reader) {
        Ok(Some(request)) => request,
        Ok(None) => break,
        Err(err) if is_bad_request_error(&err) => {
          bad_request_response().write_to(reader.get_mut())?;
          break;
        }
        Err(err) => return Err(err),
      };
      let request_closes_connection = request.closes_connection();
      let request_keeps_connection_alive = request.keeps_connection_alive();
      let response = handler(request);
      let response_closes_connection = response.closes_connection();
      served += 1;

      let close_after_response = request_closes_connection
        || response_closes_connection
        || !request_keeps_connection_alive
        || served == request_limit;
      response.write_to_with_default_connection(reader.get_mut(), close_after_response)?;

      if close_after_response {
        break;
      }
    }

    Ok(served)
  }

  fn handle_next_connection<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpResponse,
  {
    let (mut stream, _) = self.listener.accept()?;
    let request = match Request::read_from(&mut stream) {
      Ok(request) => request,
      Err(err) if is_bad_request_error(&err) => {
        return bad_request_response().write_to(&mut stream);
      }
      Err(err) => return Err(err),
    };
    let response = handler(request);
    response.write_to(&mut stream)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  method: String,
  target: String,
  version: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Request {
  pub fn method(&self) -> &str {
    &self.method
  }

  pub fn target(&self) -> &str {
    &self.target
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  pub fn body(&self) -> &[u8] {
    &self.body
  }

  pub fn closes_connection(&self) -> bool {
    connection_header_has_token(self.header("Connection"), "close")
  }

  fn keeps_connection_alive(&self) -> bool {
    connection_header_has_token(self.header("Connection"), "keep-alive")
  }

  fn read_from<R>(reader: &mut R) -> io::Result<Self>
  where
    R: Read,
  {
    let mut reader = BufReader::new(reader);
    Self::read_next_from(&mut reader)?
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))
  }

  fn read_next_from<R>(reader: &mut R) -> io::Result<Option<Self>>
  where
    R: BufRead,
  {
    let mut raw = Vec::new();
    let mut content_length: Option<usize> = None;

    loop {
      if let (Some(header_end), Some(content_length)) = (find_header_end(&raw), content_length) {
        let message_len = header_end + 4 + content_length;
        if raw.len() == message_len {
          return Ok(Some(Self::from_raw_frame(&raw)?));
        }
      }

      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        if let (Some(header_end), Some(content_length)) = (find_header_end(&raw), content_length) {
          let body_start = header_end + 4;
          if raw.len() < body_start + content_length {
            return Err(io::Error::new(
              io::ErrorKind::UnexpectedEof,
              "incomplete HTTP request body",
            ));
          }
        }
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "incomplete HTTP request",
        ));
      }

      if let (Some(header_end), Some(content_length)) = (find_header_end(&raw), content_length) {
        let message_len = header_end + 4 + content_length;
        let take = (message_len - raw.len()).min(available.len());
        raw.extend_from_slice(&available[..take]);
        reader.consume(take);
        continue;
      }

      let mut combined = raw.clone();
      combined.extend_from_slice(available);
      match find_header_end(&combined) {
        Some(header_end) => {
          let take = header_end + 4 - raw.len();
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          reject_transfer_encoding(&head.headers)?;
          content_length = Some(header_content_length(&head.headers)?);
        }
        None => {
          let take = available.len();
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  fn from_raw_frame(raw: &[u8]) -> io::Result<Self> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))?;
    let head = parse_request_head(&raw[..header_end])?;
    reject_transfer_encoding(&head.headers)?;
    let body_start = header_end + 4;
    let content_length = header_content_length(&head.headers)?;
    let body_end = body_start + content_length;

    if raw.len() < body_end {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "incomplete HTTP request body",
      ));
    }

    Ok(Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      body: raw[body_start..body_end].to_vec(),
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
  method: String,
  path: String,
  query: Option<String>,
  version: String,
  headers: Vec<HttpHeader>,
  body: Vec<u8>,
}

impl HttpRequest {
  pub fn parse(raw: &[u8]) -> Result<Self, HttpParseError> {
    let header_end = raw
      .windows(4)
      .position(|window| window == b"\r\n\r\n")
      .ok_or_else(|| HttpParseError::new("request is missing header terminator"))?;
    let head = std::str::from_utf8(&raw[..header_end])
      .map_err(|_| HttpParseError::new("request headers are not valid UTF-8"))?;
    let body_bytes = &raw[(header_end + 4)..];

    let mut lines = head.split("\r\n");
    let request_line = lines
      .next()
      .ok_or_else(|| HttpParseError::new("request line is missing"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
      .next()
      .ok_or_else(|| HttpParseError::new("request method is missing"))?;
    let target = request_parts
      .next()
      .ok_or_else(|| HttpParseError::new("request target is missing"))?;
    let version = request_parts
      .next()
      .ok_or_else(|| HttpParseError::new("request version is missing"))?;

    if request_parts.next().is_some() {
      return Err(HttpParseError::new("request line has too many parts"));
    }

    let (path, query) = match target.split_once('?') {
      Some((path, query)) => (path.to_string(), Some(query.to_string())),
      None => (target.to_string(), None),
    };

    let mut headers = Vec::new();
    for line in lines {
      let (name, value) = line
        .split_once(':')
        .ok_or_else(|| HttpParseError::new("header line is missing ':'"))?;
      headers.push(HttpHeader::new(name.trim(), value.trim()));
    }

    if headers
      .iter()
      .any(|header| header.name.eq_ignore_ascii_case("Transfer-Encoding"))
    {
      return Err(HttpParseError::new(
        "Transfer-Encoding request bodies are not supported",
      ));
    }

    let body = match headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Content-Length"))
    {
      Some(header) => {
        let content_length = header
          .value
          .parse::<usize>()
          .map_err(|_| HttpParseError::new("Content-Length is not a valid length"))?;
        if body_bytes.len() != content_length {
          return Err(HttpParseError::new(
            "request body length does not match Content-Length",
          ));
        }
        body_bytes.to_vec()
      }
      None => body_bytes.to_vec(),
    };

    Ok(Self {
      method: method.to_string(),
      path,
      query,
      version: version.to_string(),
      headers,
      body,
    })
  }

  pub fn method(&self) -> &str {
    &self.method
  }

  pub fn path(&self) -> &str {
    &self.path
  }

  pub fn query(&self) -> Option<&str> {
    self.query.as_deref()
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn headers(&self) -> &[HttpHeader] {
    &self.headers
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value.as_str())
  }

  pub fn body(&self) -> &[u8] {
    &self.body
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
  version: String,
  status_code: u16,
  reason: String,
  headers: Vec<HttpHeader>,
  body: Vec<u8>,
}

impl HttpResponse {
  pub fn new<S: AsRef<str>>(status_code: u16, reason: S) -> Self {
    Self {
      version: "HTTP/1.1".to_string(),
      status_code,
      reason: reason.as_ref().to_string(),
      headers: Vec::new(),
      body: Vec::new(),
    }
  }

  pub fn ok(body: impl AsRef<[u8]>) -> Self {
    Self::new(200, "OK").body(body)
  }

  pub fn header<N: AsRef<str>, V: AsRef<str>>(mut self, name: N, value: V) -> Self {
    let name = name.as_ref();
    let value = value.as_ref();
    assert_valid_header_component(name);
    assert_valid_header_component(value);
    self.headers.push(HttpHeader::new(name, value));
    self
  }

  pub fn body<B: AsRef<[u8]>>(mut self, body: B) -> Self {
    self.body = body.as_ref().to_vec();
    self
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    self
      .write_head_to(&mut bytes, false)
      .expect("write to Vec cannot fail");
    if self.allows_body() {
      bytes.extend_from_slice(&self.body);
    }
    bytes
  }

  pub fn write_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    self.write_to_with_default_connection(writer, true)
  }

  fn write_to_with_default_connection<W>(
    &self,
    writer: &mut W,
    close_by_default: bool,
  ) -> io::Result<()>
  where
    W: Write,
  {
    self.write_head_to(writer, close_by_default)?;
    if self.allows_body() {
      writer.write_all(&self.body)?;
    }
    writer.flush()
  }

  fn write_head_to<W>(&self, writer: &mut W, include_default_connection: bool) -> io::Result<()>
  where
    W: Write,
  {
    write!(
      writer,
      "{} {} {}\r\n",
      self.version, self.status_code, self.reason
    )?;

    for header in &self.headers {
      if !header.name.eq_ignore_ascii_case("Content-Length") {
        write!(writer, "{}: {}\r\n", header.name, header.value)?;
      }
    }

    if self.allows_body() {
      write!(writer, "Content-Length: {}\r\n", self.body.len())?;
    }
    if include_default_connection && !self.has_header("Connection") {
      writer.write_all(b"Connection: close\r\n")?;
    }

    writer.write_all(b"\r\n")
  }

  fn allows_body(&self) -> bool {
    response_status_allows_body(self.status_code)
  }

  fn has_header(&self, name: &str) -> bool {
    self
      .headers
      .iter()
      .any(|header| header.name.eq_ignore_ascii_case(name))
  }

  fn closes_connection(&self) -> bool {
    let connection = self
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Connection"))
      .map(|header| header.value.as_str());
    connection_header_has_token(connection, "close")
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpHeader {
  name: String,
  value: String,
}

impl HttpHeader {
  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().to_string(),
      value: value.as_ref().to_string(),
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpParseError {
  message: String,
}

impl HttpParseError {
  fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpParseError {}

fn find_header_end(raw: &[u8]) -> Option<usize> {
  raw.windows(4).position(|window| window == b"\r\n\r\n")
}

struct RequestHead {
  method: String,
  target: String,
  version: String,
  headers: Vec<(String, String)>,
}

fn parse_request_head(raw: &[u8]) -> io::Result<RequestHead> {
  let text = std::str::from_utf8(raw)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request head is not UTF-8"))?;
  let mut lines = text.split("\r\n");
  let request_line = lines
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
  let mut parts = request_line.split_whitespace();
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

  Ok(RequestHead {
    method: method.to_string(),
    target: target.to_string(),
    version: version.to_string(),
    headers: parse_header_lines(lines)?,
  })
}

fn parse_header_lines<'a>(
  lines: impl Iterator<Item = &'a str>,
) -> io::Result<Vec<(String, String)>> {
  let mut headers = Vec::new();

  for line in lines {
    if line.is_empty() {
      continue;
    }
    let (name, value) = line
      .split_once(':')
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid request header"))?;
    headers.push((name.trim().to_string(), value.trim().to_string()));
  }

  Ok(headers)
}

fn header_content_length(headers: &[(String, String)]) -> io::Result<usize> {
  let mut length = None;

  for (_, value) in headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
  {
    let parsed = value
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

  Ok(length.unwrap_or(0))
}

fn reject_transfer_encoding(headers: &[(String, String)]) -> io::Result<()> {
  if headers
    .iter()
    .any(|(name, _)| name.eq_ignore_ascii_case("Transfer-Encoding"))
  {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Transfer-Encoding request bodies are not supported",
    ));
  }

  Ok(())
}

fn connection_header_has_token(value: Option<&str>, expected: &str) -> bool {
  value.is_some_and(|value| {
    value
      .split(',')
      .any(|token| token.trim().eq_ignore_ascii_case(expected))
  })
}

fn assert_valid_header_component(component: &str) {
  assert!(
    !component.contains('\r') && !component.contains('\n'),
    "response headers must not contain CR or LF"
  );
}

fn response_status_allows_body(status_code: u16) -> bool {
  !(status_code / 100 == 1 || status_code == 204 || status_code == 304)
}

fn is_bad_request_error(err: &io::Error) -> bool {
  matches!(
    err.kind(),
    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
  )
}

fn bad_request_response() -> HttpResponse {
  HttpResponse::new(400, "Bad Request").body("Bad Request")
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{BufRead, BufReader, Cursor};

  #[test]
  fn read_next_from_consumes_one_fully_framed_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello",
      "POST /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "world"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let second = Request::read_next_from(&mut reader)
      .expect("second frame should parse")
      .expect("second request should be present");

    assert_eq!("POST", first.method());
    assert_eq!("/first", first.target());
    assert_eq!(b"hello", first.body());
    assert_eq!("POST", second.method());
    assert_eq!("/second", second.target());
    assert_eq!(b"world", second.body());
    assert!(reader.fill_buf().expect("remaining bytes").is_empty());
  }

  #[test]
  fn connection_close_request_marks_keep_alive_loop_terminal() {
    let raw = concat!(
      "POST /final HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: close\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "done",
      "GET /ignored HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("request frame should parse")
      .expect("request should be present");

    assert_eq!("/final", request.target());
    assert_eq!(b"done", request.body());
    assert!(request.closes_connection());
    assert!(reader
      .fill_buf()
      .expect("remaining bytes")
      .starts_with(b"GET /ignored"));
  }

  #[test]
  fn partial_second_request_returns_unexpected_eof_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "POST /partial HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "he"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::UnexpectedEof, error.kind());
    assert_eq!("incomplete HTTP request body", error.to_string());
  }

  #[test]
  fn malformed_second_request_returns_invalid_data_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "GET /broken HTTP/1.1\r\n",
      "Host example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid request header", error.to_string());
  }
}
