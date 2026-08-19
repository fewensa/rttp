use std::fmt;

use crate::types::{FormData, Header, Para, Proxy, RoUrl, ToRoUrl};
use crate::{error, Config};

pub(crate) fn is_sensitive_redirect_header(name: &str) -> bool {
  name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("cookie")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("traceparent")
    || name.eq_ignore_ascii_case("tracestate")
}

#[derive(Clone)]
pub struct Request {
  closed: bool,
  count: u32,
  config: Config,
  url: Option<RoUrl>,
  method: String,
  paths: Vec<String>,
  paras: Vec<Para>,
  formdatas: Vec<FormData>,
  headers: Vec<Header>,
  trailers: Vec<Header>,
  traditional: bool,
  encode: bool,
  raw: Option<String>,
  binary: Vec<u8>,
  proxy: Option<Proxy>,
  http2_extended_connect_protocol: Option<String>,
}

impl fmt::Debug for Request {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Request")
      .field("closed", &self.closed)
      .field("count", &self.count)
      .field("config", &self.config)
      .field("url", &self.url)
      .field("method", &self.method)
      .field("paths", &self.paths)
      .field("paras", &self.paras)
      .field("formdatas", &self.formdatas)
      .field("headers", &self.headers)
      .field("trailers", &self.trailers)
      .field("traditional", &self.traditional)
      .field("encode", &self.encode)
      .field("raw", &debug_raw_request(&self.raw))
      .field("binary", &self.binary)
      .field("proxy", &self.proxy)
      .field(
        "http2_extended_connect_protocol",
        &self.http2_extended_connect_protocol,
      )
      .finish()
  }
}

fn debug_raw_request(raw: &Option<String>) -> DebugRawRequest<'_> {
  match raw {
    Some(value) if raw_request_has_sensitive_header(value) => DebugRawRequest::Redacted,
    Some(value) => DebugRawRequest::Visible(value),
    None => DebugRawRequest::None,
  }
}

enum DebugRawRequest<'a> {
  None,
  Redacted,
  Visible(&'a str),
}

impl fmt::Debug for DebugRawRequest<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::None => formatter.write_str("None"),
      Self::Redacted => formatter.write_str("Some(\"[REDACTED]\")"),
      Self::Visible(value) => formatter.debug_tuple("Some").field(value).finish(),
    }
  }
}

fn raw_request_has_sensitive_header(raw: &str) -> bool {
  raw.lines().any(|line| {
    let Some((name, _)) = line.split_once(':') else {
      return false;
    };
    is_sensitive_redirect_header(name.trim()) || name.trim().eq_ignore_ascii_case("set-cookie")
  })
}

#[allow(dead_code)]
impl Request {
  pub fn new() -> Self {
    Self {
      closed: false,
      count: 1,
      config: Default::default(),
      url: None,
      method: "GET".to_string(),
      paths: vec![],
      paras: vec![],
      formdatas: vec![],
      headers: vec![],
      trailers: vec![],
      traditional: true,
      encode: true,
      raw: None,
      binary: vec![],
      proxy: None,
      http2_extended_connect_protocol: None,
    }
  }

  pub fn closed(&self) -> bool {
    self.closed
  }
  pub fn config(&self) -> &Config {
    &self.config
  }
  pub fn count(&self) -> u32 {
    self.count
  }
  pub fn url(&self) -> &Option<RoUrl> {
    &self.url
  }
  pub fn method(&self) -> &String {
    &self.method
  }
  pub fn paths(&self) -> &Vec<String> {
    &self.paths
  }
  pub fn paras(&self) -> &Vec<Para> {
    &self.paras
  }
  pub fn formdatas(&self) -> &Vec<FormData> {
    &self.formdatas
  }
  pub fn headers(&self) -> &Vec<Header> {
    &self.headers
  }
  pub fn trailers(&self) -> &Vec<Header> {
    &self.trailers
  }
  pub fn traditional(&self) -> bool {
    self.traditional
  }
  pub fn encode(&self) -> bool {
    self.encode
  }
  pub fn raw(&self) -> &Option<String> {
    &self.raw
  }
  pub fn binary(&self) -> &Vec<u8> {
    &self.binary
  }
  pub fn proxy(&self) -> &Option<Proxy> {
    &self.proxy
  }
  pub fn http2_extended_connect_protocol(&self) -> &Option<String> {
    &self.http2_extended_connect_protocol
  }

  pub(crate) fn closed_mut(&mut self) -> &mut bool {
    &mut self.closed
  }
  pub(crate) fn config_mut(&mut self) -> &mut Config {
    &mut self.config
  }
  pub(crate) fn count_mut(&mut self) -> &mut u32 {
    &mut self.count
  }
  pub(crate) fn url_mut(&mut self) -> &mut Option<RoUrl> {
    &mut self.url
  }
  pub(crate) fn method_mut(&mut self) -> &mut String {
    &mut self.method
  }
  pub(crate) fn paths_mut(&mut self) -> &mut Vec<String> {
    &mut self.paths
  }
  pub(crate) fn paras_mut(&mut self) -> &mut Vec<Para> {
    &mut self.paras
  }
  pub(crate) fn formdatas_mut(&mut self) -> &mut Vec<FormData> {
    &mut self.formdatas
  }
  pub(crate) fn headers_mut(&mut self) -> &mut Vec<Header> {
    &mut self.headers
  }
  pub(crate) fn trailers_mut(&mut self) -> &mut Vec<Header> {
    &mut self.trailers
  }
  pub(crate) fn traditional_mut(&mut self) -> &mut bool {
    &mut self.traditional
  }
  pub(crate) fn encode_mut(&mut self) -> &mut bool {
    &mut self.encode
  }
  pub(crate) fn raw_mut(&mut self) -> &mut Option<String> {
    &mut self.raw
  }
  pub(crate) fn binary_mut(&mut self) -> &mut Vec<u8> {
    &mut self.binary
  }
  pub(crate) fn proxy_mut(&mut self) -> &mut Option<Proxy> {
    &mut self.proxy
  }
  pub(crate) fn http2_extended_connect_protocol_mut(&mut self) -> &mut Option<String> {
    &mut self.http2_extended_connect_protocol
  }

  pub(crate) fn closed_set(&mut self, closed: bool) -> &mut Self {
    self.closed = closed;
    self
  }
  pub(crate) fn config_set<C: AsRef<Config>>(&mut self, config: C) -> &mut Self {
    self.config = config.as_ref().clone();
    self
  }
  pub(crate) fn count_set(&mut self, count: u32) -> &mut Self {
    self.count = count;
    self
  }
  pub(crate) fn url_set<S: AsRef<RoUrl>>(&mut self, rourl: S) -> &mut Self {
    self.url = Some(rourl.as_ref().to_rourl());
    self
  }
  pub(crate) fn method_set<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.method = method.as_ref().into();
    self
  }
  pub(crate) fn paths_set(&mut self, paths: Vec<String>) -> &mut Self {
    self.paths = paths;
    self
  }
  pub(crate) fn paras_set(&mut self, paras: Vec<Para>) -> &mut Self {
    self.paras = paras;
    self
  }
  pub(crate) fn formdatas_set(&mut self, formdatas: Vec<FormData>) -> &mut Self {
    self.formdatas = formdatas;
    self
  }
  pub(crate) fn headers_set(&mut self, headers: Vec<Header>) -> &mut Self {
    self.headers = headers;
    self
  }
  pub(crate) fn trailers_set(&mut self, trailers: Vec<Header>) -> &mut Self {
    self.trailers = trailers;
    self
  }
  pub(crate) fn traditional_set(&mut self, traditional: bool) -> &mut Self {
    self.traditional = traditional;
    self
  }
  pub(crate) fn encode_set(&mut self, encode: bool) -> &mut Self {
    self.encode = encode;
    self
  }
  pub(crate) fn raw_set<S: AsRef<str>>(&mut self, raw: S) -> &mut Self {
    self.raw = Some(raw.as_ref().into());
    self
  }
  pub(crate) fn binary_set(&mut self, binary: Vec<u8>) -> &mut Self {
    self.binary = binary;
    self
  }
  pub(crate) fn proxy_set(&mut self, proxy: Proxy) -> &mut Self {
    self.proxy = Some(proxy);
    self
  }
  pub(crate) fn http2_extended_connect_protocol_set<S: AsRef<str>>(
    &mut self,
    protocol: S,
  ) -> &mut Self {
    self.http2_extended_connect_protocol = Some(protocol.as_ref().into());
    self
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<String> {
    self
      .headers
      .iter()
      .find(|h| h.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|h| h.value().clone())
  }

  pub(crate) fn redirect_status_set(&mut self, status_code: u32) -> &mut Self {
    if self.redirect_rewrites_to_get(status_code) {
      self.method_set("GET");
      self.paras.clear();
      self.formdatas.clear();
      self.raw = None;
      self.binary.clear();
      self.headers.retain(|header| {
        !header.name().eq_ignore_ascii_case("content-length")
          && !header.name().eq_ignore_ascii_case("content-type")
          && !header.name().eq_ignore_ascii_case("transfer-encoding")
      });
    }
    self
  }

  pub(crate) fn redirect_rewrites_to_get(&self, status_code: u32) -> bool {
    // Compatibility behavior: 301/302 rewrite POST requests to GET; 303
    // rewrites any non-HEAD request. 307 and 308 preserve method/body.
    match status_code {
      301 | 302 => self.method.eq_ignore_ascii_case("post"),
      303 => !self.method.eq_ignore_ascii_case("head"),
      _ => false,
    }
  }

  pub(crate) fn remove_sensitive_redirect_headers(&mut self) {
    self
      .headers
      .retain(|header| !is_sensitive_redirect_header(header.name()));
  }

  pub(crate) fn has_configured_body(&self) -> bool {
    self.raw.is_some() || !self.binary.is_empty() || !self.formdatas.is_empty()
  }

  pub(crate) fn prepare_streaming_fixed_body(&mut self, content_length: u64) {
    self.headers.retain(|header| {
      !header.name().eq_ignore_ascii_case("content-length")
        && !header.name().eq_ignore_ascii_case("transfer-encoding")
    });
    self
      .headers
      .push(Header::new("Content-Length", content_length.to_string()));
  }

  pub(crate) fn prepare_streaming_chunked_body(&mut self) {
    let has_trailers = !self.trailers.is_empty();
    self.headers.retain(|header| {
      !header.name().eq_ignore_ascii_case("content-length")
        && !header.name().eq_ignore_ascii_case("transfer-encoding")
        && (!has_trailers || !header.name().eq_ignore_ascii_case("trailer"))
    });
    self
      .headers
      .push(Header::new("Transfer-Encoding", "chunked"));
    if has_trailers {
      let names = self
        .trailers
        .iter()
        .map(|header| header.name().as_str())
        .collect::<Vec<_>>()
        .join(", ");
      self.headers.push(Header::new("Trailer", names));
    }
  }

  pub(crate) fn clear_streaming_body_headers(&mut self) {
    self.headers.retain(|header| {
      !header.name().eq_ignore_ascii_case("content-length")
        && !header.name().eq_ignore_ascii_case("transfer-encoding")
    });
  }

  pub(crate) fn clear_streaming_chunked_body_headers(&mut self) {
    self.clear_streaming_body_headers();
    if !self.trailers.is_empty() {
      self
        .headers
        .retain(|header| !header.name().eq_ignore_ascii_case("trailer"));
    }
  }
}

#[derive(Clone)]
pub struct RequestBody {
  binary: Vec<u8>,
}

impl RequestBody {
  pub fn with_vec(vec: Vec<u8>) -> Self {
    Self { binary: vec }
  }

  pub fn with_text<S: AsRef<str>>(text: S) -> Self {
    Self::with_slice(text.as_ref().to_owned().as_bytes())
  }

  pub fn with_slice(slice: &[u8]) -> Self {
    Self::with_vec(slice.to_vec())
  }

  pub fn bytes(&self) -> &[u8] {
    self.binary.as_slice()
  }

  pub fn string(&self) -> error::Result<String> {
    String::from_utf8(self.binary.clone()).map_err(error::request)
  }

  pub fn len(&self) -> usize {
    self.binary.len()
  }
}

impl fmt::Display for RequestBody {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let text = self.string().unwrap_or_default();
    fmt::Display::fmt(&text, formatter)
  }
}

impl fmt::Debug for RequestBody {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let text = self.string().unwrap_or_default();
    fmt::Debug::fmt(&text, formatter)
  }
}

#[cfg(test)]
mod tests {
  use super::Request;
  use crate::types::Header;

  #[test]
  fn request_debug_redacts_sensitive_headers_and_raw_request() {
    let mut request = Request::new();
    request
      .headers_mut()
      .push(Header::new("Authorization", "Bearer origin-token"));
    request
      .headers_mut()
      .push(Header::new("Cookie", "session=private"));
    request
      .headers_mut()
      .push(Header::new("Idempotency-Key", "charge-2026-08-19-9f3c"));
    request
      .trailers_mut()
      .push(Header::new("Proxy-Authorization", "Basic cHJveHk6c2VjcmV0"));
    request
      .raw_set("GET / HTTP/1.1\r\nAuthorization: Bearer raw-token\r\nHost: example.test\r\n\r\n");

    let debug = format!("{request:?}");
    assert!(debug.contains("[REDACTED]"));
    for secret in [
      "origin-token",
      "session=private",
      "cHJveHk6c2VjcmV0",
      "raw-token",
      "charge-2026-08-19-9f3c",
    ] {
      assert!(!debug.contains(secret));
    }
  }
}
