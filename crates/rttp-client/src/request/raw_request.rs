use crate::error;
use crate::request::builder::RawBuilder;
use crate::request::is_sensitive_redirect_header;
use crate::request::{Request, RequestBody};
use crate::types::Header;
use crate::types::RoUrl;
use crate::types::{ToRoUrl, ToUrl};
use std::fmt;

pub struct RawRequest<'a> {
  pub(crate) origin: &'a mut Request,
  pub(crate) url: RoUrl,
  pub(crate) header: String,
  pub(crate) body: Option<RequestBody>,
}

impl<'a> RawRequest<'a> {
  pub fn block_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).raw_request_block()
  }

  #[cfg(feature = "async")]
  pub async fn async_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).raw_request_async().await
  }

  pub fn origin(&self) -> &Request {
    self.origin
  }

  pub fn url(&self) -> &RoUrl {
    &self.url
  }

  pub fn header(&self) -> &String {
    &self.header
  }

  pub(crate) fn request_target(&self) -> Option<&str> {
    self
      .header
      .lines()
      .next()
      .and_then(|line| line.split_whitespace().nth(1))
  }

  pub fn body(&self) -> &Option<RequestBody> {
    &self.body
  }

  pub fn content_type(&self) -> Option<String> {
    self.origin.header("content-type")
  }

  pub(crate) fn origin_mut(&mut self) -> &mut Request {
    self.origin
  }

  pub(crate) fn redirect_status_set(&mut self, status_code: u32) {
    let rewrite_to_get = self.origin.redirect_rewrites_to_get(status_code);
    self.origin.redirect_status_set(status_code);
    if rewrite_to_get {
      self.body = None;
      self.header = Self::redirect_body_headers_remove(&self.header);
    }
  }

  pub(crate) fn redirect_url_set<S: ToRoUrl>(
    &mut self,
    rourl: S,
    strip_sensitive_headers: bool,
    request_target: Option<&str>,
  ) -> error::Result<()> {
    let rourl = rourl.to_rourl();
    let url = rourl.to_url()?;
    let host_header = Self::redirect_host_header(&url)?;
    let request_target = request_target.map_or_else(
      || {
        let mut request_target = url.path().to_string();
        if let Some(query) = url.query() {
          request_target.push_str(&format!("?{}", query));
        }
        request_target
      },
      str::to_string,
    );

    self.origin.url_set(&rourl);
    if strip_sensitive_headers {
      self.origin.remove_sensitive_redirect_headers();
    }
    self.redirect_host_set(host_header.clone());
    self.url = rourl;

    if let Some((_, rest)) = self.header.split_once("\r\n") {
      let rest = Self::redirect_header_host_set(rest, &host_header);
      let rest = if strip_sensitive_headers {
        Self::redirect_sensitive_headers_strip(&rest)
      } else {
        rest
      };
      self.header = format!(
        "{} {} HTTP/1.1\r\n{}",
        self.origin.method().to_uppercase(),
        request_target,
        rest
      );
    }

    Ok(())
  }

  fn redirect_host_header(url: &url::Url) -> error::Result<Header> {
    let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
    Ok(match url.port() {
      Some(port) => Header::new("Host", format!("{}:{}", host, port)),
      None => Header::new("Host", host),
    })
  }

  fn redirect_host_set(&mut self, header: Header) {
    if let Some(origin_header) = self
      .origin
      .headers_mut()
      .iter_mut()
      .find(|item| item.name().eq_ignore_ascii_case("host"))
    {
      origin_header.replace(header);
    } else {
      self.origin.headers_mut().push(header);
    }
  }

  fn redirect_header_host_set(rest: &str, header: &Header) -> String {
    let mut rewritten = String::new();
    let mut replaced = false;

    for line in rest.split_inclusive("\r\n") {
      let header_name = line
        .trim_end_matches("\r\n")
        .split_once(':')
        .map(|(name, _)| name);

      if header_name.is_some_and(|name| name.eq_ignore_ascii_case("host")) {
        rewritten.push_str(&format!("{}: {}\r\n", header.name(), header.value()));
        replaced = true;
      } else {
        rewritten.push_str(line);
      }
    }

    if replaced {
      rewritten
    } else {
      format!("{}: {}\r\n{}", header.name(), header.value(), rewritten)
    }
  }

  fn redirect_sensitive_headers_strip(rest: &str) -> String {
    let mut rewritten = String::new();

    for line in rest.split_inclusive("\r\n") {
      let header_name = line
        .trim_end_matches("\r\n")
        .split_once(':')
        .map(|(name, _)| name);

      if header_name.is_some_and(is_sensitive_redirect_header) {
        continue;
      }

      rewritten.push_str(line);
    }

    rewritten
  }

  fn redirect_body_headers_remove(header: &str) -> String {
    let Some((request_line, rest)) = header.split_once("\r\n") else {
      return header.to_string();
    };
    let mut rewritten = format!("{}\r\n", request_line);

    for line in rest.split_inclusive("\r\n") {
      let header_name = line
        .trim_end_matches("\r\n")
        .split_once(':')
        .map(|(name, _)| name);

      if header_name.is_some_and(|name| {
        name.eq_ignore_ascii_case("content-length")
          || name.eq_ignore_ascii_case("content-type")
          || name.eq_ignore_ascii_case("transfer-encoding")
      }) {
        continue;
      }

      rewritten.push_str(line);
    }

    rewritten
  }
}

impl fmt::Debug for RawRequest<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("RawRequest")
      .field("origin", &self.origin)
      .field("url", &self.url)
      .field("header", &RedactedHeaderBlock(&self.header))
      .field("body", &self.body)
      .finish()
  }
}

struct RedactedHeaderBlock<'a>(&'a str);

impl fmt::Debug for RedactedHeaderBlock<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut redacted = String::with_capacity(self.0.len());
    for line in self.0.split_inclusive("\r\n") {
      let trimmed = line.trim_end_matches("\r\n");
      if let Some((name, _)) = trimmed.split_once(':') {
        if is_sensitive_debug_header(name) {
          redacted.push_str(name);
          redacted.push_str(": [REDACTED]");
          if line.ends_with("\r\n") {
            redacted.push_str("\r\n");
          }
          continue;
        }
      }
      redacted.push_str(line);
    }
    fmt::Debug::fmt(&redacted, formatter)
  }
}

fn is_sensitive_debug_header(name: &str) -> bool {
  name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("cookie")
    || name.eq_ignore_ascii_case("idempotency-key")
    || name.eq_ignore_ascii_case("lock-token")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("sec-websocket-accept")
    || name.eq_ignore_ascii_case("sec-websocket-key")
    || name.eq_ignore_ascii_case("set-cookie")
    || name.eq_ignore_ascii_case("traceparent")
    || name.eq_ignore_ascii_case("tracestate")
    || name.eq_ignore_ascii_case("baggage")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn raw_request_debug_redacts_sensitive_header_values() {
    let mut request = Request::new();
    request.url_set("http://example.test/asset".to_rourl());
    request
      .headers_mut()
      .push(Header::new("Authorization", "Bearer origin-secret-token"));
    request
      .headers_mut()
      .push(Header::new("Proxy-Authorization", "Basic cHJveHktc2VjcmV0"));
    request
      .headers_mut()
      .push(Header::new("Idempotency-Key", "charge-2026-08-19-9f3c"));
    request.headers_mut().push(Header::new(
      "Lock-Token",
      "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    ));
    request
      .headers_mut()
      .push(Header::new("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ=="));
    request.headers_mut().push(Header::new(
      "traceparent",
      "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
    ));
    request.headers_mut().push(Header::new(
      "tracestate",
      "rojo=00f067aa0ba902b7,congo=t61rcWkgMzE",
    ));
    request
      .headers_mut()
      .push(Header::new("baggage", "tenant=acme-secret;source=gateway"));

    let raw_request = RawRequest::block_new(&mut request).expect("raw request should build");
    let debug = format!("{raw_request:?}");

    assert!(debug.contains("Authorization"));
    assert!(debug.contains("Proxy-Authorization"));
    assert!(debug.contains("Idempotency-Key"));
    assert!(debug.contains("Lock-Token"));
    assert!(debug.contains("Sec-WebSocket-Key"));
    assert!(debug.contains("traceparent"));
    assert!(debug.contains("tracestate"));
    assert!(debug.contains("baggage"));
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("origin-secret-token"));
    assert!(!debug.contains("cHJveHktc2VjcmV0"));
    assert!(!debug.contains("charge-2026-08-19-9f3c"));
    assert!(!debug.contains("550e8400-e29b-41d4-a716-446655440000"));
    assert!(!debug.contains("dGhlIHNhbXBsZSBub25jZQ=="));
    assert!(!debug.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    assert!(!debug.contains("00f067aa0ba902b7"));
    assert!(!debug.contains("t61rcWkgMzE"));
    assert!(!debug.contains("acme-secret"));
    assert!(!debug.contains("gateway"));
  }
}
