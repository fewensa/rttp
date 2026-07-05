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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
  status: u16,
  reason: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl HttpResponse {
  pub fn new(status: u16, reason: impl Into<String>, body: impl AsRef<[u8]>) -> Self {
    Self {
      status,
      reason: reason.into(),
      headers: Vec::new(),
      body: body.as_ref().to_vec(),
    }
  }

  pub fn ok(body: impl AsRef<[u8]>) -> Self {
    Self::new(200, "OK", body)
  }

  pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
    self.headers.push((name.into(), value.into()));
    self
  }

  pub fn write_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    write!(writer, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;

    for (name, value) in &self.headers {
      write!(writer, "{}: {}\r\n", name, value)?;
    }

    if !self.has_header("Content-Length") {
      write!(writer, "Content-Length: {}\r\n", self.body.len())?;
    }
    if !self.has_header("Connection") {
      writer.write_all(b"Connection: close\r\n")?;
    }

    writer.write_all(b"\r\n")?;
    writer.write_all(&self.body)?;
    writer.flush()
  }

  fn has_header(&self, name: &str) -> bool {
    self
      .headers
      .iter()
      .any(|(key, _)| key.eq_ignore_ascii_case(name))
  }
}

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
