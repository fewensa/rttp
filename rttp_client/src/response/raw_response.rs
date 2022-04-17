use std::fmt;
use std::io::Read;

use crate::error;
use crate::response::ResponseBody;
use crate::types::{Cookie, Header, IntoHeader, RoUrl, ToUrl};
use url::Url;

static CR: u8 = b'\r';
static LF: u8 = b'\n';
static CRLF: &str = "\r\n";

#[derive(Clone)]
pub struct RawResponse {
  _url: Url,
  url: RoUrl,
  binary: Vec<u8>,
  code: u32,
  version: String,
  reason: String,
  headers: Vec<Header>,
  cookies: Vec<Cookie>,
  body: ResponseBody,
}

impl RawResponse {
  pub fn new(url: RoUrl, binary: Vec<u8>) -> error::Result<Self> {
    let _url = url.to_url().map_err(error::builder)?;
    let mut response = RawResponse {
      _url,
      url,
      binary: vec![],
      code: 0,
      version: "".to_string(),
      reason: "".to_string(),
      headers: vec![],
      cookies: vec![],
      body: ResponseBody::new(vec![]),
    };
    Parser::new(binary).parse(&mut response)?;
    Ok(response)
  }

  #[allow(dead_code)]
  pub fn binary(&mut self, binary: Vec<u8>) -> &mut Self {
    self.binary = binary;
    self
  }
  pub fn code(&mut self, code: u32) -> &mut Self {
    self.code = code;
    self
  }
  pub fn version<S: AsRef<str>>(&mut self, version: S) -> &mut Self {
    self.version = version.as_ref().to_owned();
    self
  }
  pub fn reason<S: AsRef<str>>(&mut self, reason: S) -> &mut Self {
    self.reason = reason.as_ref().to_owned();
    self
  }
  pub fn headers(&mut self, headers: Vec<Header>) -> &mut Self {
    self.headers = headers;
    self
  }
  pub fn body(&mut self, body: ResponseBody) -> &mut Self {
    self.body = body;
    self
  }
  pub fn cookies(&mut self, cookies: Vec<Cookie>) -> &mut Self {
    self.cookies = cookies;
    self
  }

  pub(crate) fn url_get(&self) -> &Url {
    &self._url
  }
  pub fn binary_get(&self) -> &[u8] {
    self.binary.as_slice()
  }
  pub fn code_get(&self) -> u32 {
    self.code
  }
  pub fn version_get(&self) -> &String {
    &self.version
  }
  pub fn reason_get(&self) -> &String {
    &self.reason
  }
  pub fn headers_get(&self) -> &Vec<Header> {
    &self.headers
  }
  pub fn body_get(&self) -> &ResponseBody {
    &self.body
  }
  pub fn cookies_get(&self) -> &Vec<Cookie> {
    &self.cookies
  }

  pub fn string(&self) -> error::Result<String> {
    let mut text = String::new();
    text.push_str(&format!(
      "{} {} {}\r\n",
      self.version, self.code, self.reason
    ));
    self.headers.iter().for_each(|h| {
      text.push_str(&format!("{}: {}\r\n", h.name(), h.value()));
    });
    text.push_str("\r\n");
    text.push_str(&self.body.string()?);
    Ok(text)
  }
}

impl fmt::Debug for RawResponse {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    match self.string() {
      Ok(text) => fmt::Debug::fmt(&text, formatter),
      Err(e) => fmt::Debug::fmt(&e, formatter),
    }
  }
}

impl fmt::Display for RawResponse {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    match self.string() {
      Ok(text) => fmt::Display::fmt(&text, formatter),
      Err(e) => fmt::Display::fmt(&e, formatter),
    }
  }
}

struct Parser {
  binary: Vec<u8>,
}

impl Parser {
  pub fn new(binary: Vec<u8>) -> Self {
    Self { binary }
  }

  pub fn parse(self, response: &mut RawResponse) -> error::Result<()> {
    if self.binary.is_empty() {
      return Ok(());
    }
    // find \r\n\r\n position
    let mut position: usize = 0;
    for i in 0..self.binary.len() - 1 {
      if self.binary.get(i) == Some(&CR)
        && self.binary.get(i + 1) == Some(&LF)
        && self.binary.get(i + 2) == Some(&CR)
        && self.binary.get(i + 3) == Some(&LF)
      {
        position = i + 3;
        break;
      }
      //      if self.binary[i] == CR && self.binary[i + 1] == LF && self.binary[i + 2] == CR && self.binary[i + 3] == LF {
      //        position = i + 3;
      //        break;
      //      }
    }
    if position == 0 {
      return Err(error::bad_response("No http response"));
    }
    let (header_b, body_b): (&[u8], &[u8]) = self.binary.split_at(position);

    let header = String::from_utf8(header_b.to_vec()).map_err(error::response)?;
    let body = body_b[1..].to_owned();

    self.parse_header(response, header)?;
    self.parse_body(response, body)?;

    response.binary = self.binary;
    Ok(())
  }

  fn parse_header(&self, response: &mut RawResponse, text: String) -> error::Result<()> {
    let parts: Vec<&str> = text.split(CRLF).collect();
    let status_line = parts
      .get(0)
      .ok_or(error::bad_response("Response not have status line"))?;
    let status_parts: Vec<&str> = status_line.splitn(3, " ").collect();

    let http_version = status_parts
      .get(0)
      .ok_or(error::bad_response("Response status not have http version"))?;
    let status_code: u32 = match status_parts
      .get(1)
      .ok_or(error::bad_response("Response status not have code"))?
      .parse()
    {
      Ok(c) => c,
      Err(_) => return Err(error::bad_response("Response status code is not a number")),
    };
    let reason = status_parts.get(2).unwrap_or(&"");
    response
      .version(http_version)
      .code(status_code)
      .reason(reason);

    let headers = parts
      .iter()
      .enumerate()
      .filter(|(ix, _)| *ix > 0)
      .filter(|(_, v)| !v.is_empty())
      .map(|(_, v)| v.into_headers())
      .filter(|hs| !hs.is_empty())
      .map(|hs| match hs.get(0) {
        Some(h) => Some(h.clone()),
        None => None,
      })
      .filter(Option::is_some)
      .map(|h| h.unwrap())
      .collect::<Vec<Header>>();

    let cookies: Vec<Cookie> = headers
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case("set-cookie"))
      .map(|header| Cookie::parse(header.value()).ok())
      .filter(|ck| ck.is_some())
      .map(|ck| ck.unwrap())
      .collect();

    response.headers(headers);
    response.cookies(cookies);
    Ok(())
  }

  fn parse_body(&self, response: &mut RawResponse, binary: Vec<u8>) -> error::Result<()> {
    if binary.is_empty() {
      return Ok(());
    }

    let content_encoding = response
      .headers_get()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case("Content-Encoding"));

    if let Some(header) = content_encoding {
      if header.value().eq_ignore_ascii_case("gzip") {
        let mut decoder = flate2::read::GzDecoder::new(binary.as_slice());
        let mut buffer = Vec::new();
        decoder.read_to_end(&mut buffer).unwrap();
        let body = ResponseBody::new(buffer);
        response.body(body);
        return Ok(());
      }
    }

    let body = ResponseBody::new(binary);
    response.body(body);
    Ok(())
  }
}
