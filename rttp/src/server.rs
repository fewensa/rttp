use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};

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
    let (mut stream, _) = self.listener.accept()?;
    let request = Request::read_from(&mut stream)?;
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

  fn read_from<R>(reader: &mut R) -> io::Result<Self>
  where
    R: Read,
  {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let mut content_length = None;

    loop {
      let read = reader.read(&mut buf)?;
      if read == 0 {
        break;
      }

      raw.extend_from_slice(&buf[..read]);
      let header_end = find_header_end(&raw);

      if content_length.is_none() {
        if let Some(header_end) = header_end {
          let headers = parse_headers(&raw[..header_end])?;
          reject_transfer_encoding(&headers)?;
          content_length = Some(header_content_length(&headers)?);
        }
      }

      if let (Some(header_end), Some(content_length)) = (header_end, content_length) {
        let message_len = header_end + 4 + content_length;
        if raw.len() >= message_len {
          break;
        }
      }
    }

    let header_end = find_header_end(&raw)
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
    self.write_head_to(writer, true)?;
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

fn parse_headers(raw: &[u8]) -> io::Result<Vec<(String, String)>> {
  let text = std::str::from_utf8(raw)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request head is not UTF-8"))?;
  parse_header_lines(text.split("\r\n").skip(1))
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

fn assert_valid_header_component(component: &str) {
  assert!(
    !component.contains('\r') && !component.contains('\n'),
    "response headers must not contain CR or LF"
  );
}

fn response_status_allows_body(status_code: u16) -> bool {
  !(status_code / 100 == 1 || status_code == 204 || status_code == 304)
}
