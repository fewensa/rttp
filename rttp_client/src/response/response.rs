use std::fmt;

use url::Url;

use crate::error;
use crate::response::raw_response::RawResponse;
use crate::types::{Cookie, Header, RoUrl};

const MAX_CACHE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
const MAX_CACHE_CONTROL_DIRECTIVES: usize = 256;

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

  pub fn is_not_modified(&self) -> bool {
    self.code() == 304
  }

  pub fn is_precondition_failed(&self) -> bool {
    self.code() == 412
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

  pub fn etag(&self) -> Option<&String> {
    self.header_value("etag")
  }

  pub fn last_modified(&self) -> Option<&String> {
    self.header_value("last-modified")
  }

  pub fn content_range(&self) -> Option<ContentRange> {
    self
      .header_value("content-range")
      .and_then(ContentRange::parse)
  }

  pub fn cache_control(&self) -> error::Result<Option<CacheControl>> {
    let values = self.header_values("cache-control");
    if values.is_empty() {
      return Ok(None);
    }
    CacheControl::parse_values(values.into_iter().map(String::as_str)).map(Some)
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheControl {
  no_cache: bool,
  no_cache_fields: Vec<String>,
  no_store: bool,
  max_age: Option<u64>,
  s_maxage: Option<u64>,
  private: bool,
  private_fields: Vec<String>,
  public: bool,
  must_revalidate: bool,
  proxy_revalidate: bool,
  immutable: bool,
  stale_while_revalidate: Option<u64>,
  stale_if_error: Option<u64>,
  extensions: Vec<CacheControlExtension>,
}

impl CacheControl {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut cache_control = Self::default();
    let mut directive_count = 0usize;
    for value in values {
      for directive in split_cache_control_directives(value)? {
        directive_count += 1;
        if directive_count > MAX_CACHE_CONTROL_DIRECTIVES {
          return Err(error::bad_response("Too many Cache-Control directives"));
        }
        cache_control.apply_directive(&directive)?;
      }
    }
    Ok(cache_control)
  }

  fn apply_directive(&mut self, directive: &str) -> error::Result<()> {
    let (name, value, value_was_quoted) = match directive.split_once('=') {
      Some((name, value)) => {
        let value = value.trim();
        (
          name.trim(),
          Some(parse_directive_value(value)?),
          value.starts_with('"'),
        )
      }
      None => (directive.trim(), None, false),
    };
    if !is_token(name) {
      return Err(error::bad_response("Invalid Cache-Control directive"));
    }

    match name.to_ascii_lowercase().as_str() {
      "no-cache" => {
        self.no_cache = true;
        if let Some(value) = value {
          self.no_cache_fields = split_field_names(&value);
        }
      }
      "no-store" => self.no_store = true,
      "max-age" => {
        self.max_age = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "s-maxage" => {
        self.s_maxage = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "private" => {
        self.private = true;
        if let Some(value) = value {
          self.private_fields = split_field_names(&value);
        }
      }
      "public" => self.public = true,
      "must-revalidate" => self.must_revalidate = true,
      "proxy-revalidate" => self.proxy_revalidate = true,
      "immutable" => self.immutable = true,
      "stale-while-revalidate" => {
        self.stale_while_revalidate = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "stale-if-error" => {
        self.stale_if_error = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      _ => self
        .extensions
        .push(CacheControlExtension::new(name, value.as_deref())),
    }
    Ok(())
  }

  pub fn no_cache(&self) -> bool {
    self.no_cache
  }

  pub fn no_cache_fields(&self) -> Vec<&str> {
    self.no_cache_fields.iter().map(String::as_str).collect()
  }

  pub fn no_store(&self) -> bool {
    self.no_store
  }

  pub fn max_age(&self) -> Option<u64> {
    self.max_age
  }

  pub fn s_maxage(&self) -> Option<u64> {
    self.s_maxage
  }

  pub fn private(&self) -> bool {
    self.private
  }

  pub fn private_fields(&self) -> Vec<&str> {
    self.private_fields.iter().map(String::as_str).collect()
  }

  pub fn public(&self) -> bool {
    self.public
  }

  pub fn must_revalidate(&self) -> bool {
    self.must_revalidate
  }

  pub fn proxy_revalidate(&self) -> bool {
    self.proxy_revalidate
  }

  pub fn immutable(&self) -> bool {
    self.immutable
  }

  pub fn stale_while_revalidate(&self) -> Option<u64> {
    self.stale_while_revalidate
  }

  pub fn stale_if_error(&self) -> Option<u64> {
    self.stale_if_error
  }

  pub fn extensions(&self) -> &[CacheControlExtension] {
    &self.extensions
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheControlExtension {
  name: String,
  value: Option<String>,
}

impl CacheControlExtension {
  fn new(name: &str, value: Option<&str>) -> Self {
    Self {
      name: name.to_string(),
      value: value.map(ToString::to_string),
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

fn split_cache_control_directives(value: &str) -> error::Result<Vec<String>> {
  if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
    return Err(error::bad_response(
      "Cache-Control header value is too large",
    ));
  }

  let mut directives = Vec::new();
  let mut current = String::new();
  let mut in_quote = false;
  let mut escaped = false;

  for ch in value.chars() {
    if escaped {
      current.push(ch);
      escaped = false;
      continue;
    }

    match ch {
      '\\' if in_quote => {
        current.push(ch);
        escaped = true;
      }
      '"' => {
        current.push(ch);
        in_quote = !in_quote;
      }
      ',' if !in_quote => {
        push_directive(&mut directives, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(error::bad_response("Malformed Cache-Control quoted-string"));
  }
  push_directive(&mut directives, &current)?;
  Ok(directives)
}

fn push_directive(directives: &mut Vec<String>, directive: &str) -> error::Result<()> {
  let directive = directive.trim();
  if directive.is_empty() {
    return Err(error::bad_response("Invalid Cache-Control directive"));
  }
  if directives.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
    return Err(error::bad_response("Too many Cache-Control directives"));
  }
  directives.push(directive.to_string());
  Ok(())
}

fn parse_directive_value(value: &str) -> error::Result<String> {
  if let Some(value) = value.strip_prefix('"') {
    return parse_quoted_string(value);
  }
  if value.contains('"') || value.is_empty() {
    return Err(error::bad_response("Invalid Cache-Control directive value"));
  }
  Ok(value.to_string())
}

fn parse_quoted_string(value: &str) -> error::Result<String> {
  let mut chars = value.chars();
  let mut parsed = String::new();
  let mut closed = false;

  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(error::bad_response("Malformed Cache-Control quoted-string"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(error::bad_response("Malformed Cache-Control quoted-string"));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => return Err(error::bad_response("Malformed Cache-Control quoted-string")),
    }
  }

  if !closed || chars.any(|ch| !ch.is_ascii_whitespace()) {
    return Err(error::bad_response("Malformed Cache-Control quoted-string"));
  }
  Ok(parsed)
}

fn parse_delta_seconds(
  name: &str,
  value: Option<&str>,
  value_was_quoted: bool,
) -> error::Result<u64> {
  let Some(value) = value else {
    return Err(error::bad_response(format!(
      "Missing Cache-Control {name} delta-seconds"
    )));
  };
  if value_was_quoted || value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
    return Err(error::bad_response(format!(
      "Invalid Cache-Control {name} delta-seconds"
    )));
  }
  value
    .parse::<u64>()
    .map_err(|_| error::bad_response(format!("Invalid Cache-Control {name} delta-seconds")))
}

fn split_field_names(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|field| !field.is_empty())
    .map(ToString::to_string)
    .collect()
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'!' | b'#'
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
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

fn is_qdtext(ch: char) -> bool {
  matches!(ch, '\t' | ' ' | '!' | '#'..='[' | ']'..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn is_quoted_pair_char(ch: char) -> bool {
  matches!(ch, '\t' | ' '..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
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
