use std::fmt;

use url::Url;

use crate::error;
use crate::response::raw_response::RawResponse;
use crate::types::{Cookie, Header, RoUrl};

#[derive(Clone)]
pub struct Response {
  raw: RawResponse,
}

impl Response {
  pub fn new(url: RoUrl, binary: Vec<u8>) -> error::Result<Self> {
    Ok(Self {
      raw: RawResponse::new(url, binary)?,
    })
  }

  pub(crate) fn with_trailers(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
  ) -> error::Result<Self> {
    Ok(Self {
      raw: RawResponse::with_trailers(url, binary, trailers)?,
    })
  }
}

impl Response {
  pub fn ok(&self) -> bool {
    self.code() == 200
  }

  pub fn is_partial_content(&self) -> bool {
    self.code() == 206
  }

  pub fn is_range_not_satisfiable(&self) -> bool {
    self.code() == 416
  }

  pub fn is_redirect(&self) -> bool {
    matches!(self.code(), 301 | 302 | 303 | 307 | 308)
  }

  pub fn code(&self) -> u32 {
    self.raw.code_get()
  }

  pub fn version(&self) -> &String {
    self.raw.version_get()
  }

  pub fn reason(&self) -> &String {
    self.raw.reason_get()
  }

  fn url(&self) -> &Url {
    self.raw.url_get()
  }

  pub fn host(&self) -> &str {
    self.url().host_str().unwrap_or_default()
  }

  pub fn body(&self) -> &ResponseBody {
    self.raw.body_get()
  }

  pub fn binary(&self) -> &[u8] {
    self.raw.binary_get()
  }

  pub fn location(&self) -> Option<&String> {
    self.header_value("location")
  }

  pub fn content_range(&self) -> Option<ContentRange> {
    self
      .header_value("content-range")
      .and_then(ContentRange::parse)
  }

  pub fn headers(&self) -> &Vec<Header> {
    self.raw.headers_get()
  }

  pub fn trailers(&self) -> &Vec<Header> {
    self.raw.trailers_get()
  }

  pub fn headers_of_name<S: AsRef<str>>(&self, name: S) -> Vec<&Header> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .collect()
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .headers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn header_values<S: AsRef<str>>(&self, name: S) -> Vec<&String> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value())
      .collect()
  }

  pub fn header_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.header(name).map(|header| header.value())
  }

  pub fn trailers_of_name<S: AsRef<str>>(&self, name: S) -> Vec<&Header> {
    self
      .trailers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .collect()
  }

  pub fn trailer<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .trailers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn trailer_values<S: AsRef<str>>(&self, name: S) -> Vec<&String> {
    self
      .trailers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value())
      .collect()
  }

  pub fn trailer_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.trailer(name).map(|header| header.value())
  }

  pub fn cookies(&self) -> &Vec<Cookie> {
    self.raw.cookies_get()
  }

  pub fn cookie<S: AsRef<str>>(&self, name: S) -> Option<&Cookie> {
    self
      .cookies()
      .iter()
      .find(|cookie| cookie.name().eq_ignore_ascii_case(name.as_ref()))
  }
}

impl fmt::Debug for Response {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&self.raw, formatter)
  }
}

impl fmt::Display for Response {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.raw, formatter)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRange {
  unit: String,
  start: Option<u64>,
  end: Option<u64>,
  complete_length: Option<u64>,
}

impl ContentRange {
  pub fn parse(value: impl AsRef<str>) -> Option<Self> {
    let value = value.as_ref().trim();
    let (unit, range_and_length) = value.split_once(' ')?;
    if unit.is_empty() {
      return None;
    }

    let (range, complete_length) = range_and_length.split_once('/')?;
    let complete_length = parse_complete_length(complete_length)?;
    if range == "*" {
      return Some(Self {
        unit: unit.to_string(),
        start: None,
        end: None,
        complete_length,
      });
    }

    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    if start > end {
      return None;
    }

    Some(Self {
      unit: unit.to_string(),
      start: Some(start),
      end: Some(end),
      complete_length,
    })
  }

  pub fn unit(&self) -> &str {
    &self.unit
  }

  pub fn start(&self) -> Option<u64> {
    self.start
  }

  pub fn end(&self) -> Option<u64> {
    self.end
  }

  pub fn complete_length(&self) -> Option<u64> {
    self.complete_length
  }

  pub fn is_unsatisfied(&self) -> bool {
    self.start.is_none() && self.end.is_none()
  }
}

fn parse_complete_length(value: &str) -> Option<Option<u64>> {
  if value == "*" {
    return Some(None);
  }
  value.parse::<u64>().ok().map(Some)
}

#[derive(Clone)]
pub struct ResponseBody {
  binary: Vec<u8>,
}

impl ResponseBody {
  pub fn new(binary: Vec<u8>) -> Self {
    Self { binary }
  }

  pub fn binary(&self) -> &[u8] {
    self.binary.as_slice()
  }

  pub fn string(&self) -> error::Result<String> {
    String::from_utf8(self.binary.clone()).map_err(error::body)
  }
}

impl fmt::Debug for ResponseBody {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match self.string() {
      Ok(text) => fmt::Debug::fmt(&text, formatter),
      Err(e) => fmt::Debug::fmt(&e, formatter),
    }
  }
}

impl fmt::Display for ResponseBody {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match self.string() {
      Ok(text) => fmt::Display::fmt(&text, formatter),
      Err(e) => fmt::Display::fmt(&e, formatter),
    }
  }
}
