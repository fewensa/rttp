use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

const MAX_REQUEST_HEAD_BYTES: usize = 64 * 1024;
const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;

pub struct HttpServer {
  listener: TcpListener,
  read_timeout: Option<Duration>,
  write_timeout: Option<Duration>,
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
        read_timeout: None,
        write_timeout: None,
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

  /// Sets the read timeout applied to each accepted connection before parsing requests.
  pub fn with_read_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.read_timeout = timeout;
    self
  }

  /// Sets the write timeout applied to each accepted connection before writing responses.
  pub fn with_write_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.write_timeout = timeout;
    self
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
      self.configure_stream(&stream)?;
      let handled = self.handle_connection(stream, request_count - served, &mut handler)?;
      served += handled.max(1);
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
      let request =
        match self.normalize_connection_error(Request::read_next_from_with_continue(&mut reader)) {
          Ok(Some(request)) => request,
          Ok(None) => break,
          Err(err) if is_bad_request_error(&err) => {
            self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()))?;
            served += 1;
            break;
          }
          Err(err) => return Err(err),
        };
      let request_closes_connection = request.closes_connection();
      let request_uses_http10_defaults = request.version() == "HTTP/1.0";
      let request_is_head = request.method() == "HEAD";
      let response = handler(request);
      let response_closes_connection = response.closes_connection();
      served += 1;

      let close_after_response =
        request_closes_connection || response_closes_connection || served == request_limit;
      let default_connection = if close_after_response {
        DefaultConnectionHeader::ForceClose
      } else if request_uses_http10_defaults {
        DefaultConnectionHeader::KeepAlive
      } else {
        DefaultConnectionHeader::Omit
      };
      self.normalize_connection_error(response.write_to_with_default_connection_and_body(
        reader.get_mut(),
        default_connection,
        !request_is_head,
      ))?;

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
    let (stream, _) = self.listener.accept()?;
    self.configure_stream(&stream)?;
    let mut reader = BufReader::new(stream);
    let request = match self.normalize_connection_error(
      Request::read_next_from_with_continue(&mut reader).and_then(|request| {
        request.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))
      }),
    ) {
      Ok(request) => request,
      Err(err) if is_bad_request_error(&err) => {
        return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
      }
      Err(err) => return Err(err),
    };
    let request_is_head = request.method() == "HEAD";
    let response = handler(request);
    self.normalize_connection_error(response.write_to_with_default_connection_and_body(
      reader.get_mut(),
      DefaultConnectionHeader::ForceClose,
      !request_is_head,
    ))
  }

  fn configure_stream(&self, stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(self.read_timeout)?;
    stream.set_write_timeout(self.write_timeout)
  }

  fn normalize_connection_error<T>(&self, result: io::Result<T>) -> io::Result<T> {
    result.map_err(|err| {
      if err.kind() == io::ErrorKind::WouldBlock
        && (self.read_timeout.is_some() || self.write_timeout.is_some())
      {
        io::Error::new(io::ErrorKind::TimedOut, err)
      } else {
        err
      }
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  method: String,
  target: String,
  version: String,
  headers: Vec<(String, String)>,
  trailers: Vec<(String, String)>,
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

  pub fn trailers(&self) -> &[(String, String)] {
    &self.trailers
  }

  pub fn trailer(&self, name: &str) -> Option<&str> {
    self
      .trailers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  pub fn body(&self) -> &[u8] {
    &self.body
  }

  pub fn closes_connection(&self) -> bool {
    if self.connection_header_has_token("close") {
      return true;
    }

    self.version == "HTTP/1.0" && !self.connection_header_has_token("keep-alive")
  }

  fn connection_header_has_token(&self, token: &str) -> bool {
    self
      .headers
      .iter()
      .filter(|(name, _)| name.eq_ignore_ascii_case("Connection"))
      .any(|(_, value)| connection_header_has_token(Some(value), token))
  }

  #[cfg(test)]
  fn read_next_from<R>(reader: &mut R) -> io::Result<Option<Self>>
  where
    R: BufRead,
  {
    Self::read_next_from_without_continue(reader)
  }

  fn read_next_from_with_continue<S>(reader: &mut BufReader<S>) -> io::Result<Option<Self>>
  where
    S: Read + Write,
  {
    let mut raw = Vec::new();
    let mut body_kind: Option<RequestBodyKind> = None;

    loop {
      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        if raw.len() == message_len {
          return Ok(Some(Self::from_raw_frame(&raw)?));
        }
      }

      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
          (find_header_end(&raw), body_kind)
        {
          let body_start = header_end + 4;
          let body_end = checked_request_message_len(header_end, content_length)?;
          if raw.len() < body_end || body_end < body_start {
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

      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
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
          reject_oversized_request_head(header_end + 4)?;
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          let parsed_body_kind = request_body_kind(&head.headers)?;
          if request_needs_continue(&head.headers, parsed_body_kind)? {
            write_continue_response(reader.get_mut())?;
          }
          match parsed_body_kind {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_and_body(head, Vec::new())));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader)?;
              return Ok(Some(Self::from_head_body_and_trailers(
                head,
                chunked.body,
                chunked.trailers,
              )));
            }
          }
        }
        None => {
          let take = available.len();
          reject_oversized_request_head(raw.len().saturating_add(take))?;
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  #[cfg(test)]
  fn read_next_from_without_continue<R>(reader: &mut R) -> io::Result<Option<Self>>
  where
    R: BufRead,
  {
    let mut raw = Vec::new();
    let mut body_kind: Option<RequestBodyKind> = None;

    loop {
      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        if raw.len() == message_len {
          return Ok(Some(Self::from_raw_frame(&raw)?));
        }
      }

      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
          (find_header_end(&raw), body_kind)
        {
          let body_start = header_end + 4;
          let body_end = checked_request_message_len(header_end, content_length)?;
          if raw.len() < body_end || body_end < body_start {
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

      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
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
          reject_oversized_request_head(header_end + 4)?;
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          match request_body_kind(&head.headers)? {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_and_body(head, Vec::new())));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader)?;
              return Ok(Some(Self::from_head_body_and_trailers(
                head,
                chunked.body,
                chunked.trailers,
              )));
            }
          }
        }
        None => {
          let take = available.len();
          reject_oversized_request_head(raw.len().saturating_add(take))?;
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  fn from_raw_frame(raw: &[u8]) -> io::Result<Self> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))?;
    reject_oversized_request_head(header_end + 4)?;
    let head = parse_request_head(&raw[..header_end])?;
    let body_start = header_end + 4;
    let body = match request_body_kind(&head.headers)? {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length)?;
        let body_end = checked_request_message_len(header_end, content_length)?;

        if raw.len() < body_end {
          return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete HTTP request body",
          ));
        }

        raw[body_start..body_end].to_vec()
      }
      RequestBodyKind::Chunked => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "chunked request body requires streaming reader",
        ));
      }
    };

    Ok(Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers: Vec::new(),
      body,
    })
  }

  fn from_head_and_body(head: RequestHead, body: Vec<u8>) -> Self {
    Self::from_head_body_and_trailers(head, body, Vec::new())
  }

  fn from_head_body_and_trailers(
    head: RequestHead,
    body: Vec<u8>,
    trailers: Vec<(String, String)>,
  ) -> Self {
    Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers,
      body,
    }
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
    let header_end = find_header_end(raw)
      .ok_or_else(|| HttpParseError::new("request is missing header terminator"))?;
    reject_oversized_request_head(header_end + 4).map_err(HttpParseError::from_io_error)?;
    let head = parse_request_head(&raw[..header_end]).map_err(HttpParseError::from_io_error)?;
    let body_bytes = &raw[(header_end + 4)..];

    let (path, query) = match head.target.split_once('?') {
      Some((path, query)) => (path.to_string(), Some(query.to_string())),
      None => (head.target.clone(), None),
    };

    let body = match request_body_kind(&head.headers).map_err(HttpParseError::from_io_error)? {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length).map_err(HttpParseError::from_io_error)?;
        if body_bytes.len() != content_length {
          return Err(HttpParseError::new(
            "request body length does not match Content-Length",
          ));
        }
        body_bytes.to_vec()
      }
      RequestBodyKind::Chunked => {
        return Err(HttpParseError::new(
          "chunked request body requires streaming reader",
        ));
      }
    };
    let headers = head
      .headers
      .into_iter()
      .map(|(name, value)| HttpHeader::new(name, value))
      .collect();

    Ok(Self {
      method: head.method,
      path,
      query,
      version: head.version,
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
  trailers: Vec<HttpHeader>,
  body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DefaultConnectionHeader {
  Close,
  ForceClose,
  KeepAlive,
  Omit,
}

impl HttpResponse {
  pub fn new<S: AsRef<str>>(status_code: u16, reason: S) -> Self {
    Self {
      version: "HTTP/1.1".to_string(),
      status_code,
      reason: reason.as_ref().to_string(),
      headers: Vec::new(),
      trailers: Vec::new(),
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

  pub fn trailer<N: AsRef<str>, V: AsRef<str>>(mut self, name: N, value: V) -> Self {
    let name = name.as_ref();
    let value = value.as_ref();
    assert_valid_header_component(name);
    assert_valid_header_component(value);
    assert_allowed_trailer_name(name);
    self.trailers.push(HttpHeader::new(name, value));
    self
  }

  pub fn trailers(&self) -> &[HttpHeader] {
    &self.trailers
  }

  pub fn trailer_value<S: AsRef<str>>(&self, name: S) -> Option<&str> {
    self
      .trailers
      .iter()
      .find(|trailer| trailer.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|trailer| trailer.value.as_str())
  }

  pub fn body<B: AsRef<[u8]>>(mut self, body: B) -> Self {
    self.body = body.as_ref().to_vec();
    self
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    self
      .write_head_to(&mut bytes, DefaultConnectionHeader::Omit)
      .expect("write to Vec cannot fail");
    if self.allows_body() {
      self
        .write_body_to(&mut bytes)
        .expect("write to Vec cannot fail");
    }
    bytes
  }

  pub fn write_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    self.write_to_with_default_connection(writer, DefaultConnectionHeader::Close)
  }

  fn write_to_with_default_connection<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
  ) -> io::Result<()>
  where
    W: Write,
  {
    self.write_to_with_default_connection_and_body(writer, default_connection, true)
  }

  fn write_to_with_default_connection_and_body<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
    write_body: bool,
  ) -> io::Result<()>
  where
    W: Write,
  {
    self.write_head_to(writer, default_connection)?;
    if write_body && self.allows_body() {
      self.write_body_to(writer)?;
    }
    writer.flush()
  }

  fn write_head_to<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
  ) -> io::Result<()>
  where
    W: Write,
  {
    write!(
      writer,
      "{} {} {}\r\n",
      self.version, self.status_code, self.reason
    )?;

    let connection_header_index = self.connection_header_index();
    for (index, header) in self.headers.iter().enumerate() {
      if !header.name.eq_ignore_ascii_case("Content-Length")
        && (self.allows_body() || !header.name.eq_ignore_ascii_case("Transfer-Encoding"))
        && (!header.name.eq_ignore_ascii_case("Connection")
          || (default_connection != DefaultConnectionHeader::ForceClose
            && Some(index) == connection_header_index))
      {
        write!(writer, "{}: {}\r\n", header.name, header.value)?;
      }
    }

    if self.allows_body() && !self.uses_chunked_transfer_encoding() {
      write!(writer, "Content-Length: {}\r\n", self.body.len())?;
    }
    if default_connection == DefaultConnectionHeader::ForceClose
      || connection_header_index.is_none()
    {
      match default_connection {
        DefaultConnectionHeader::Close | DefaultConnectionHeader::ForceClose => {
          writer.write_all(b"Connection: close\r\n")?
        }
        DefaultConnectionHeader::KeepAlive => writer.write_all(b"Connection: keep-alive\r\n")?,
        DefaultConnectionHeader::Omit => {}
      }
    }

    writer.write_all(b"\r\n")
  }

  fn write_body_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    if self.uses_chunked_transfer_encoding() {
      write!(writer, "{:x}\r\n", self.body.len())?;
      writer.write_all(&self.body)?;
      writer.write_all(b"\r\n0\r\n")?;
      for trailer in &self.trailers {
        write!(writer, "{}: {}\r\n", trailer.name, trailer.value)?;
      }
      writer.write_all(b"\r\n")
    } else {
      writer.write_all(&self.body)
    }
  }

  fn allows_body(&self) -> bool {
    response_status_allows_body(self.status_code)
  }

  fn uses_chunked_transfer_encoding(&self) -> bool {
    self.headers.iter().any(|header| {
      header.name.eq_ignore_ascii_case("Transfer-Encoding")
        && header
          .value
          .split(',')
          .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
    })
  }

  fn connection_header_index(&self) -> Option<usize> {
    self
      .headers
      .iter()
      .rposition(|header| header.name.eq_ignore_ascii_case("Connection"))
  }

  fn closes_connection(&self) -> bool {
    self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Connection"))
      .any(|header| connection_header_has_token(Some(header.value.as_str()), "close"))
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

  fn from_io_error(error: io::Error) -> Self {
    Self::new(error.to_string())
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

fn reject_oversized_request_head(length: usize) -> io::Result<()> {
  if length > MAX_REQUEST_HEAD_BYTES {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "request head is too large",
    ))
  } else {
    Ok(())
  }
}

fn reject_oversized_request_body(length: usize) -> io::Result<()> {
  if length > MAX_REQUEST_BODY_BYTES {
    Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "request body is too large",
    ))
  } else {
    Ok(())
  }
}

fn checked_request_message_len(header_end: usize, content_length: usize) -> io::Result<usize> {
  header_end
    .checked_add(4)
    .and_then(|body_start| body_start.checked_add(content_length))
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))
}

struct RequestHead {
  method: String,
  target: String,
  version: String,
  headers: Vec<(String, String)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RequestBodyKind {
  ContentLength(usize),
  Chunked,
}

struct ChunkedRequestBody {
  body: Vec<u8>,
  trailers: Vec<(String, String)>,
}

fn parse_request_head(raw: &[u8]) -> io::Result<RequestHead> {
  let text = std::str::from_utf8(raw)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request head is not UTF-8"))?;
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

  Ok(RequestHead {
    method: method.to_string(),
    target: target.to_string(),
    version: version.to_string(),
    headers,
  })
}

fn validate_request_line(method: &str, target: &str, version: &str) -> io::Result<()> {
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
enum RequestTargetForm {
  Origin,
  Absolute,
  Asterisk,
  Authority,
}

fn is_valid_request_target_for_method(method: &str, target: &str) -> bool {
  let Some(form) = request_target_form(target) else {
    return false;
  };

  match form {
    RequestTargetForm::Origin | RequestTargetForm::Absolute => method != "CONNECT",
    RequestTargetForm::Asterisk => method == "OPTIONS",
    RequestTargetForm::Authority => method == "CONNECT",
  }
}

fn request_target_form(target: &str) -> Option<RequestTargetForm> {
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

fn is_absolute_form_target(target: &str) -> bool {
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

fn is_uri_scheme(scheme: &str) -> bool {
  let mut bytes = scheme.bytes();
  let Some(first) = bytes.next() else {
    return false;
  };
  first.is_ascii_alphabetic()
    && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn is_authority_form_target(target: &str) -> bool {
  is_valid_host_authority(target, true)
}

fn is_valid_host_authority(authority: &str, require_port: bool) -> bool {
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

fn validate_host_port_suffix(suffix: &str, require_port: bool) -> bool {
  if suffix.is_empty() {
    return !require_port;
  }
  let Some(port) = suffix.strip_prefix(':') else {
    return false;
  };
  is_valid_port(port)
}

fn is_valid_reg_name_or_ipv4(host: &str) -> bool {
  !host.is_empty()
    && host
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
}

fn is_valid_port(port: &str) -> bool {
  !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

fn parse_header_lines<'a>(
  lines: impl Iterator<Item = &'a str>,
) -> io::Result<Vec<(String, String)>> {
  let mut headers = Vec::new();

  for line in lines {
    if line.is_empty() {
      continue;
    }
    if line.starts_with(' ') || line.starts_with('\t') {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid request header",
      ));
    }
    let (name, value) = line
      .split_once(':')
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid request header"))?;
    if !is_http_token(name) || !value.bytes().all(is_header_value_byte) {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid request header",
      ));
    }
    headers.push((name.trim().to_string(), value.trim().to_string()));
  }

  Ok(headers)
}

fn validate_host_header(
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

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
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

fn is_request_target_byte(byte: u8) -> bool {
  byte > 0x20 && byte != 0x7f
}

fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}

fn optional_header_content_length(headers: &[(String, String)]) -> io::Result<Option<usize>> {
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

  Ok(length)
}

fn request_body_kind(headers: &[(String, String)]) -> io::Result<RequestBodyKind> {
  let content_length = optional_header_content_length(headers)?;
  let mut transfer_codings = Vec::new();

  for (_, value) in headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Transfer-Encoding"))
  {
    transfer_codings.extend(
      value
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty()),
    );
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

fn request_needs_continue(
  headers: &[(String, String)],
  body_kind: RequestBodyKind,
) -> io::Result<bool> {
  let mut has_expect = false;

  for (_, value) in headers
    .iter()
    .filter(|(name, _)| name.eq_ignore_ascii_case("Expect"))
  {
    has_expect = true;
    if !value.eq_ignore_ascii_case("100-continue") {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "unsupported Expect header",
      ));
    }
  }

  Ok(
    has_expect
      && (matches!(
        body_kind,
        RequestBodyKind::ContentLength(content_length) if content_length > 0
      ) || body_kind == RequestBodyKind::Chunked),
  )
}

fn write_continue_response<W>(writer: &mut W) -> io::Result<()>
where
  W: Write,
{
  writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
  writer.flush()
}

fn read_chunked_request_body<R>(reader: &mut R) -> io::Result<ChunkedRequestBody>
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

fn add_request_body_bytes(total: &mut usize, length: usize) -> io::Result<()> {
  *total = total
    .checked_add(length)
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))?;
  reject_oversized_request_body(*total)
}

fn read_bounded_crlf_line<R>(reader: &mut R, body_bytes_read: &mut usize) -> io::Result<Vec<u8>>
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

fn parse_chunk_size(line: &[u8]) -> io::Result<usize> {
  let line = std::str::from_utf8(line)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "chunk size is not UTF-8"))?;
  let size = line
    .trim_end_matches("\r\n")
    .split(';')
    .next()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty chunk size"))?;

  usize::from_str_radix(size, 16)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid chunk size"))
}

fn consume_crlf<R>(reader: &mut R, body_bytes_read: &mut usize) -> io::Result<()>
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

fn read_trailers<R>(
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
      return parse_header_lines(lines.iter().map(String::as_str));
    }
    let line = line.strip_suffix(b"\r\n").unwrap_or(&line);
    let line = std::str::from_utf8(line)
      .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "trailer line is not UTF-8"))?;
    lines.push(line.to_string());
  }
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

fn assert_allowed_trailer_name(name: &str) {
  assert!(
    !is_forbidden_trailer_name(name),
    "response trailers must not contain framing or routing fields"
  );
}

fn is_forbidden_trailer_name(name: &str) -> bool {
  matches!(
    name.to_ascii_lowercase().as_str(),
    "connection" | "content-length" | "host" | "te" | "trailer" | "transfer-encoding" | "upgrade"
  )
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
  fn read_next_from_consumes_one_chunked_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "\r\n",
      "GET /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
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
    assert_eq!("GET", second.method());
    assert_eq!("/second", second.target());
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
  fn chunk_extension_bytes_count_toward_request_body_limit() {
    let chunk_extension = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0;{}\r\n",
        "\r\n"
      ),
      chunk_extension
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk extension should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn chunk_trailer_bytes_count_toward_request_body_limit() {
    let trailer_value = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0\r\n",
        "X-Trace: {}\r\n",
        "\r\n"
      ),
      trailer_value
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk trailer should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
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
