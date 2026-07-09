#[cfg(feature = "async")]
use crate::connection::{AsyncConnection, AsyncStreamingRequestBody};
use crate::connection::{BlockConnection, HandoffConnection, StreamingRequestBody};
use crate::request::{RawRequest, Request};
use crate::response::Response;
use crate::types::{Auth, Header, IntoHeader, IntoPara, Proxy, ToFormData, ToRoUrl};
use crate::{error, Config};
#[cfg(feature = "async")]
use futures::io::AsyncRead;
use std::io;

#[derive(Debug)]
pub struct HttpClient {
  request: Request,
}

impl Default for HttpClient {
  fn default() -> Self {
    Self {
      request: Request::new(),
    }
  }
}

impl HttpClient {
  /// Create a `HttpClient` object.
  /// # Examples
  /// ```rust
  /// use rttp_client::HttpClient;
  /// let client = HttpClient::new();
  /// ```
  pub fn new() -> Self {
    Default::default()
  }

  /// Reset this request, The request only use once, This function can reset request.
  pub fn reset(&mut self) -> &mut Self {
    self.request = Request::new();
    self
  }

  /// Set get request
  pub fn get(&mut self) -> &mut Self {
    self.method("GET")
  }

  /// Set post request
  pub fn post(&mut self) -> &mut Self {
    self.method("POST")
  }

  /// Set put request
  pub fn put(&mut self) -> &mut Self {
    self.method("PUT")
  }

  /// Set delete request
  pub fn delete(&mut self) -> &mut Self {
    self.method("DELETE")
  }

  /// Set options request
  pub fn options(&mut self) -> &mut Self {
    self.method("OPTIONS")
  }

  /// Set head request
  pub fn head(&mut self) -> &mut Self {
    self.method("HEAD")
  }

  /// Set trace request
  pub fn trace(&mut self) -> &mut Self {
    self.method("TRACE")
  }

  /// Set request by method
  pub fn method<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.request.method_set(method);
    self
  }

  /// Set request url.
  pub fn url<U: ToRoUrl>(&mut self, url: U) -> &mut Self {
    self.request.url_set(url.to_rourl());
    self
  }

  /// Set request config
  pub fn config<C: AsRef<Config>>(&mut self, config: C) -> &mut Self {
    self.request.config_set(config);
    self
  }

  /// Whether traditional request, if false, the same para name will be add []
  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.request.traditional_set(traditional);
    self
  }

  /// Add url path
  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.request.paths_mut().push(path.as_ref().into());
    self
  }

  /// Whether encode para
  pub fn encode(&mut self, encode: bool) -> &mut Self {
    self.request.encode_set(encode);
    self
  }

  /// Set proxy request
  pub fn proxy<P: AsRef<Proxy>>(&mut self, proxy: P) -> &mut Self {
    self.request.proxy_set(proxy.as_ref().clone());
    self
  }

  /// Use RFC 8441 extended CONNECT on the bounded prior-knowledge h2c path.
  ///
  /// This is only honored by `emit_http2_prior_knowledge` with the `http2`
  /// feature enabled. The request is emitted as `:method CONNECT` and includes
  /// the configured `:protocol` pseudo-header.
  #[cfg(feature = "http2")]
  pub fn http2_extended_connect<S: AsRef<str>>(&mut self, protocol: S) -> &mut Self {
    self
      .request
      .http2_extended_connect_protocol_set(protocol.as_ref());
    self
  }

  /// Set HTTP authentication. Supports Basic Auth and Bearer Token.
  ///
  /// # Examples
  ///
  /// ```rust
  /// use rttp_client::HttpClient;
  /// use rttp_client::types::Auth;
  ///
  /// let mut client = HttpClient::new();
  /// client.auth(Auth::basic("user", "secret"));
  /// client.auth(Auth::bearer("my-token"));
  /// ```
  pub fn auth<A: AsRef<Auth>>(&mut self, auth: A) -> &mut Self {
    self.header(("Authorization", auth.as_ref().header_value().as_str()))
  }

  ///  Add request header
  pub fn header<P: IntoHeader>(&mut self, header: P) -> &mut Self {
    let headers = self.request.headers_mut();
    for h in header.into_headers() {
      let exit = headers
        .iter_mut()
        .find(|d| d.name().eq_ignore_ascii_case(h.name()));

      if let Some(eh) = exit {
        if h.name().eq_ignore_ascii_case("cookie") {
          let new_cookie_value = format!("{};{}", eh.value(), h.value());
          eh.replace(Header::new("Cookie", new_cookie_value));
          continue;
        }

        eh.replace(h);
        continue;
      }
      headers.push(h);
    }
    self
  }

  /// Add a request trailer field for chunked streaming uploads.
  pub fn trailer<P: IntoHeader>(&mut self, trailer: P) -> error::Result<&mut Self> {
    let trailers = trailer.into_headers();
    for h in &trailers {
      validate_request_trailer_header(h.name(), h.value())?;
    }
    for h in trailers {
      let trailers = self.request.trailers_mut();
      if let Some(existing) = trailers
        .iter_mut()
        .find(|d| d.name().eq_ignore_ascii_case(h.name()))
      {
        existing.replace(h);
      } else {
        trailers.push(h);
      }
    }
    Ok(self)
  }

  /// Add request cookie
  pub fn cookie<S: AsRef<str>>(&mut self, cookie: S) -> &mut Self {
    self.header(("Cookie", cookie.as_ref()))
  }

  /// Set a single bounded byte range request header, `Range: bytes=start-end`.
  pub fn range(&mut self, start: u64, end: u64) -> error::Result<&mut Self> {
    if start > end {
      return Err(error::builder_with_message(
        "byte range start cannot be greater than end",
      ));
    }
    Ok(self.header(("Range", format!("bytes={}-{}", start, end).as_str())))
  }

  /// Set a single open-ended byte range request header, `Range: bytes=start-`.
  pub fn range_from(&mut self, start: u64) -> &mut Self {
    self.header(("Range", format!("bytes={}-", start).as_str()))
  }

  /// Set a single suffix byte range request header, `Range: bytes=-suffix`.
  pub fn range_suffix(&mut self, suffix: u64) -> error::Result<&mut Self> {
    if suffix == 0 {
      return Err(error::builder_with_message(
        "byte range suffix length must be greater than zero",
      ));
    }
    Ok(self.header(("Range", format!("bytes=-{}", suffix).as_str())))
  }

  /// Set a single entity-tag validator, `If-None-Match: <etag>`.
  ///
  /// Accepts `*`, a strong entity tag such as `"abc"`, or a weak entity tag
  /// such as `W/"abc"`. Use `header` directly for multiple validators.
  pub fn if_none_match<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-None-Match", etag)))
  }

  /// Set a single entity-tag validator, `If-Match: <etag>`.
  ///
  /// Accepts `*`, a strong entity tag such as `"abc"`, or a weak entity tag
  /// such as `W/"abc"`. Use `header` directly for multiple validators.
  pub fn if_match<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-Match", etag)))
  }

  /// Set an HTTP-date modification validator, `If-Modified-Since: <http-date>`.
  pub fn if_modified_since<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let http_date = validate_http_date(http_date.as_ref())?;
    Ok(self.header(Header::new("If-Modified-Since", http_date)))
  }

  /// Set an HTTP-date modification validator, `If-Unmodified-Since: <http-date>`.
  pub fn if_unmodified_since<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let http_date = validate_http_date(http_date.as_ref())?;
    Ok(self.header(Header::new("If-Unmodified-Since", http_date)))
  }

  /// Set request content type
  pub fn content_type<S: AsRef<str>>(&mut self, content_type: S) -> &mut Self {
    self.header(("Content-Type", content_type.as_ref()))
  }

  /// Add request para
  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
    let paras = para.into_paras();
    self.request.paras_mut().extend(paras);
    self
  }

  /// Add request form data. include file
  pub fn form<S: ToFormData>(&mut self, formdata: S) -> &mut Self {
    let formdatas = formdata.to_formdatas();
    self.request.formdatas_mut().extend(formdatas);
    self
  }

  /// Set request raw data
  pub fn raw<S: AsRef<str>>(&mut self, raw: S) -> &mut Self {
    self.request.raw_set(raw);
    self
  }

  /// Set binary data
  pub fn binary(&mut self, binary: Vec<u8>) -> &mut Self {
    self.request.binary_set(binary);
    self
  }

  /// emit a request
  ///
  /// # Examples
  /// ```rust
  /// # use rttp_client::HttpClient;
  /// HttpClient::new()
  ///   .url("http://httpbin.org.get")
  ///   .emit();
  /// ```
  pub fn emit(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call()
  }

  #[cfg(feature = "http2")]
  pub fn emit_http2_prior_knowledge(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.proxy().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 prior-knowledge client does not support proxies",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    crate::http2::PriorKnowledgeClient::new(request).get()
  }

  #[cfg(feature = "http2")]
  pub fn emit_http2_upgrade(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.proxy().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 h2c upgrade client does not support proxies",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    crate::http2::UpgradeClient::new(request).get()
  }

  pub fn emit_streaming_fixed<R>(
    &mut self,
    mut reader: R,
    content_length: u64,
  ) -> error::Result<Response>
  where
    R: io::Read,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_fixed_body(content_length);
    let result = (|| {
      let request = RawRequest::block_new(&mut self.request)?;
      BlockConnection::new(request).call_streaming_body(StreamingRequestBody::Fixed {
        reader: &mut reader,
        content_length,
      })
    })();
    self.request.clear_streaming_body_headers();
    result
  }

  pub fn emit_streaming_chunked<R>(&mut self, mut reader: R) -> error::Result<Response>
  where
    R: io::Read,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_chunked_body();
    let trailers = self.request.trailers().clone();
    let result = (|| {
      let request = RawRequest::block_new(&mut self.request)?;
      BlockConnection::new(request).call_streaming_body(StreamingRequestBody::Chunked {
        reader: &mut reader,
        trailers: &trailers,
      })
    })();
    self.request.clear_streaming_chunked_body_headers();
    result
  }

  pub fn connect(&mut self) -> error::Result<HandoffConnection> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "CONNECT socket handoff cannot be combined with a request body",
      ));
    }
    self.request.method_set("CONNECT");
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call_connect_handoff()
  }

  pub fn upgrade(&mut self) -> error::Result<HandoffConnection> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "Upgrade socket handoff cannot be combined with a request body",
      ));
    }
    let request = RawRequest::block_new(&mut self.request)?;
    BlockConnection::new(request).call_upgrade_handoff()
  }

  /// Async request emit
  ///
  /// # Examples
  ///
  /// ```rust
  /// # use rttp_client::HttpClient;
  /// # #[cfg(feature = "async")]
  /// # async fn test_async() {
  /// HttpClient::new()
  ///   .url("http://httpbin.org.get")
  ///   .rasync()
  ///   .await;
  /// # }
  /// ```
  #[cfg(feature = "async")]
  pub async fn rasync(&mut self) -> error::Result<Response> {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    let request = RawRequest::async_new(&mut self.request).await?;
    AsyncConnection::new(request).async_call().await
  }

  #[cfg(feature = "async")]
  pub async fn rasync_streaming_fixed<R>(
    &mut self,
    mut reader: R,
    content_length: u64,
  ) -> error::Result<Response>
  where
    R: AsyncRead + Unpin,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_fixed_body(content_length);
    let result = async {
      let request = RawRequest::async_new(&mut self.request).await?;
      AsyncConnection::new(request)
        .async_call_streaming_body(AsyncStreamingRequestBody::Fixed {
          reader: &mut reader,
          content_length,
        })
        .await
    }
    .await;
    self.request.clear_streaming_body_headers();
    result
  }

  #[cfg(feature = "async")]
  pub async fn rasync_streaming_chunked<R>(&mut self, mut reader: R) -> error::Result<Response>
  where
    R: AsyncRead + Unpin,
  {
    if self.request.closed() {
      return Err(error::connection_closed());
    }
    if self.request.has_configured_body() {
      return Err(error::builder_with_message(
        "streaming request body cannot be combined with buffered body fields",
      ));
    }
    self.request.prepare_streaming_chunked_body();
    let trailers = self.request.trailers().clone();
    let result = async {
      let request = RawRequest::async_new(&mut self.request).await?;
      AsyncConnection::new(request)
        .async_call_streaming_body(AsyncStreamingRequestBody::Chunked {
          reader: &mut reader,
          trailers: &trailers,
        })
        .await
    }
    .await;
    self.request.clear_streaming_chunked_body_headers();
    result
  }
}

fn validate_request_trailer_header(name: &str, value: &str) -> error::Result<()> {
  if !is_http_token(name) || !value.bytes().all(is_header_value_byte) {
    return Err(error::builder_with_message(
      "Invalid request trailer header",
    ));
  }
  if is_forbidden_request_trailer_name(name) {
    return Err(error::builder_with_message(
      "Forbidden request trailer header",
    ));
  }
  Ok(())
}

fn validate_single_etag(etag: &str) -> error::Result<&str> {
  let etag = etag.trim();
  if etag == "*" {
    return Ok(etag);
  }
  if etag.contains(',') {
    return Err(error::builder_with_message(
      "conditional entity-tag helper accepts one validator; use header() for lists",
    ));
  }

  let opaque_tag = etag.strip_prefix("W/").unwrap_or(etag);
  let Some(inner) = opaque_tag
    .strip_prefix('"')
    .and_then(|value| value.strip_suffix('"'))
  else {
    return Err(error::builder_with_message(
      "conditional entity-tag must be *, \"tag\", or W/\"tag\"",
    ));
  };

  if inner
    .as_bytes()
    .iter()
    .any(|byte| matches!(*byte, b'"' | b'\r' | b'\n') || *byte < 0x21 || *byte == 0x7f)
  {
    return Err(error::builder_with_message(
      "conditional entity-tag contains invalid characters",
    ));
  }

  Ok(etag)
}

fn validate_http_date(http_date: &str) -> error::Result<&str> {
  let http_date = http_date.trim();
  httpdate::parse_http_date(http_date).map_err(|_| {
    error::builder_with_message("conditional modification time must be a valid HTTP-date")
  })?;
  Ok(http_date)
}

fn is_forbidden_request_trailer_name(name: &str) -> bool {
  matches!(
    name.trim().to_ascii_lowercase().as_str(),
    "connection"
      | "content-length"
      | "host"
      | "keep-alive"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "proxy-connection"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
  )
}

fn is_http_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_http_token_byte)
}

fn is_http_token_byte(byte: u8) -> bool {
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

fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}
