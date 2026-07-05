use std::error::Error;
use std::fmt;

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
    let body = raw[(header_end + 4)..].to_vec();

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

  pub fn header<N: AsRef<str>, V: AsRef<str>>(mut self, name: N, value: V) -> Self {
    self.headers.push(HttpHeader::new(name, value));
    self
  }

  pub fn body<B: AsRef<[u8]>>(mut self, body: B) -> Self {
    self.body = body.as_ref().to_vec();
    self
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
      format!("{} {} {}\r\n", self.version, self.status_code, self.reason).as_bytes(),
    );

    for header in &self.headers {
      if !header.name.eq_ignore_ascii_case("Content-Length") {
        bytes.extend_from_slice(format!("{}: {}\r\n", header.name, header.value).as_bytes());
      }
    }

    bytes.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(&self.body);
    bytes
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
