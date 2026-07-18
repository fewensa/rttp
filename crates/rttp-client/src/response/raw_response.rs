use std::fmt;
use std::io::Read;

use crate::error;
use crate::response::ResponseBody;
use crate::types::{Cookie, Header, RoUrl, ToUrl};
use url::Url;

static CR: u8 = b'\r';
static LF: u8 = b'\n';

#[derive(Clone)]
pub struct RawResponse {
  _url: Url,
  binary: Vec<u8>,
  code: u32,
  version: String,
  reason: String,
  headers: Vec<Header>,
  trailers: Vec<Header>,
  cookies: Vec<Cookie>,
  body: ResponseBody,
}

impl RawResponse {
  pub fn new(url: RoUrl, binary: Vec<u8>) -> error::Result<Self> {
    Self::with_trailers(url, binary, Vec::new())
  }

  pub(crate) fn with_trailers(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
  ) -> error::Result<Self> {
    let _url = url.to_url().map_err(error::builder)?;
    let mut response = RawResponse {
      _url,
      binary: vec![],
      code: 0,
      version: "".to_string(),
      reason: "".to_string(),
      headers: vec![],
      trailers,
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
  pub fn trailers_get(&self) -> &Vec<Header> {
    &self.trailers
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
        position = i;
        break;
      }
    }
    if position == 0 {
      return Err(error::bad_response("No http response"));
    }
    let header = &self.binary[..position];
    let body = self.binary[position + 4..].to_owned();

    self.parse_header(response, header)?;
    self.parse_body(response, body)?;

    response.binary = self.binary;
    Ok(())
  }

  fn parse_header(&self, response: &mut RawResponse, text: &[u8]) -> error::Result<()> {
    if !has_only_crlf_line_breaks(text) {
      return Err(error::bad_response("Invalid response header"));
    }
    let mut lines = text
      .split(|byte| *byte == LF)
      .map(|line| line.strip_suffix(&[CR]).unwrap_or(line));
    let status_line = lines
      .next()
      .ok_or(error::bad_response("Response not have status line"))?;
    let status_line = std::str::from_utf8(status_line).map_err(error::response)?;
    let status_parts: Vec<&str> = status_line.splitn(3, " ").collect();

    let http_version = status_parts
      .first()
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

    let mut headers = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
      if matches!(line.first(), Some(b' ' | b'\t')) {
        return Err(error::bad_response("Invalid response header"));
      }
      let Some(colon) = line.iter().position(|byte| *byte == b':') else {
        return Err(error::bad_response("Invalid response header"));
      };
      let (name, value) = line.split_at(colon);
      let value = &value[1..];
      headers.push(Header::new(
        decode_http1_text(name),
        decode_http1_text(value),
      ));
    }

    let cookies: Vec<Cookie> = headers
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case("set-cookie"))
      .filter_map(|header| Cookie::parse(header.value()).ok())
      .collect();

    response.headers(headers);
    response.cookies(cookies);
    Ok(())
  }

  fn parse_body(&self, response: &mut RawResponse, binary: Vec<u8>) -> error::Result<()> {
    if response_status_has_no_body(response.code_get()) {
      return Ok(());
    }

    if binary.is_empty() {
      return Ok(());
    }

    if has_single_gzip_content_encoding(response.headers_get()) {
      let mut decoder = flate2::read::GzDecoder::new(binary.as_slice());
      let mut buffer = Vec::new();
      decoder.read_to_end(&mut buffer).map_err(error::decode)?;
      let body = ResponseBody::new(buffer);
      response.body(body);
      return Ok(());
    }

    let body = ResponseBody::new(binary);
    response.body(body);
    Ok(())
  }
}

fn has_only_crlf_line_breaks(bytes: &[u8]) -> bool {
  bytes
    .iter()
    .enumerate()
    .all(|(index, byte)| *byte != LF || (index > 0 && bytes.get(index - 1) == Some(&CR)))
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

fn response_status_has_no_body(status_code: u32) -> bool {
  (100..200).contains(&status_code) || status_code == 204 || status_code == 304
}

fn has_single_gzip_content_encoding(headers: &[Header]) -> bool {
  let mut values = headers
    .iter()
    .filter(|header| header.name().eq_ignore_ascii_case("Content-Encoding"));
  let Some(header) = values.next() else {
    return false;
  };
  if values.next().is_some() {
    return false;
  }

  let mut codings = header.value().split(',').map(str::trim);
  let Some(coding) = codings.next() else {
    return false;
  };

  !coding.is_empty() && coding.eq_ignore_ascii_case("gzip") && codings.next().is_none()
}
