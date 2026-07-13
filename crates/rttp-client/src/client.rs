#[cfg(feature = "async")]
use crate::connection::{AsyncConnection, AsyncStreamingRequestBody};
use crate::connection::{BlockConnection, HandoffConnection, StreamingRequestBody};
use crate::request::{RawRequest, Request};
use crate::response::Response;
use crate::types::{Auth, Header, IntoHeader, IntoPara, Proxy, ToFormData, ToRoUrl};
use crate::{error, Config, H2cClientPolicy};
#[cfg(feature = "async")]
use futures::io::AsyncRead;
use rttp_protocol::forwarded::{Forwarded, MAX_FORWARDED_VALUE_BYTES};
use rttp_protocol::priority::Priority;
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

  /// Configure local settings for the bounded prior-knowledge h2c client path.
  ///
  /// This is honored only by `emit_http2_prior_knowledge` and does not enable
  /// pooling, retries, server push, or multiplexing. Invalid HTTP/2 settings
  /// are rejected before the client opens its TCP socket.
  pub fn h2c_policy(&mut self, policy: H2cClientPolicy) -> &mut Self {
    self.request.config_mut().h2c_policy_set(policy);
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
  /// feature enabled. The client opens a direct `socket2` h2c TCP connection,
  /// advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1`, emits `:method
  /// CONNECT`, and includes the configured `:protocol` pseudo-header. It
  /// returns the peer's HTTP/2 response through the normal `Response` API; it
  /// does not hand an upgraded socket to the caller.
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

  /// Set bounded `Authorization` request metadata from an authentication
  /// scheme and opaque credentials.
  ///
  /// The scheme must be an HTTP token and credentials must be a non-empty,
  /// bounded header value. RTTP does not interpret credentials or implement
  /// scheme-specific authentication behavior. Use [`Self::header`] when an
  /// application needs to send a custom Authorization syntax.
  pub fn authorization<S: AsRef<str>, C: AsRef<str>>(
    &mut self,
    scheme: S,
    credentials: C,
  ) -> error::Result<&mut Self> {
    let scheme = scheme.as_ref().trim();
    let credentials = credentials.as_ref();
    if !is_http_token(scheme) {
      return Err(error::builder_with_message(
        "invalid Authorization authentication scheme",
      ));
    }
    if credentials.is_empty()
      || credentials.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
      || !credentials.bytes().all(is_header_value_byte)
    {
      return Err(error::builder_with_message(
        "invalid Authorization credentials",
      ));
    }
    let value = format!("{scheme} {credentials}");
    if value.len() > MAX_AUTHORIZATION_VALUE_BYTES {
      return Err(error::builder_with_message(
        "Authorization header value is too large",
      ));
    }
    Ok(self.header(Header::new("Authorization", value)))
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

  /// Set bounded `Accept-Language` request metadata.
  ///
  /// Each supplied item is a language range, optionally followed by a `q`
  /// weight such as `fr-CA; q=0.8`. This validates metadata only; it does not
  /// perform locale matching or choose a response language.
  pub fn accept_language<I, L>(&mut self, ranges: I) -> error::Result<&mut Self>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    let value = build_accept_language_value(ranges)?;
    Ok(self.header(Header::new("Accept-Language", value)))
  }

  /// Set bounded HTTP `Priority` request metadata.
  ///
  /// This validates RFC 9218 urgency, incremental, and extension parameters
  /// before connecting. It only declares request metadata; it does not change
  /// transport scheduling.
  pub fn priority<V: AsRef<str>>(&mut self, value: V) -> error::Result<&mut Self> {
    let priority = Priority::parse(value)
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    Ok(self.header(Header::new("Priority", priority.header_value())))
  }

  /// Append bounded RFC 7239 `Forwarded` request metadata.
  ///
  /// This validates and preserves forwarding elements such as `for`, `by`,
  /// `host`, and `proto`; it does not select a proxy, establish trust, or
  /// rewrite any address.
  pub fn forwarded<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let forwarded = Forwarded::parse(value.as_ref())
      .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
    let headers = self.request.headers_mut();
    if let Some(header) = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Forwarded"))
    {
      let combined = Forwarded::parse_values([header.value().as_str(), value.as_ref()])
        .map_err(|parse_error| error::builder_with_message(parse_error.to_string()))?;
      let value = bounded_forwarded_header_value(combined)?;
      header.replace(Header::new("Forwarded", value));
    } else {
      headers.push(Header::new(
        "Forwarded",
        bounded_forwarded_header_value(forwarded)?,
      ));
    }
    Ok(self)
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

  /// Set a bounded `Max-Forwards` request header for TRACE or OPTIONS diagnostics.
  ///
  /// The value must be at most ten ASCII decimal digits and fit in the `u32`
  /// range (`0` through `4294967295`). This only emits the header; it does not
  /// route through proxies, decrement the value, retry requests, or select a
  /// TRACE or OPTIONS policy. Use `header` directly for unusual values.
  pub fn max_forwards<S: AsRef<str>>(&mut self, value: S) -> error::Result<&mut Self> {
    let value = validate_max_forwards(value.as_ref())?;
    Ok(self.header(Header::new("Max-Forwards", value)))
  }

  /// Append a validated `Accept-Encoding` coding with the default quality of
  /// `1`. This declares request metadata only; it does not enable compression
  /// or decompression.
  pub fn accept_encoding<S: AsRef<str>>(&mut self, coding: S) -> error::Result<&mut Self> {
    self.accept_encoding_member(coding.as_ref(), None)
  }

  /// Append a validated `Accept-Encoding` coding with an HTTP q-value.
  ///
  /// The q-value must be between `0` and `1` with at most three fractional
  /// digits. This declares request metadata only; it does not enable
  /// compression or decompression.
  pub fn accept_encoding_with_q<C: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    coding: C,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.accept_encoding_member(coding.as_ref(), Some(qvalue.as_ref()))
  }

  /// Append `gzip` to `Accept-Encoding`.
  pub fn accept_gzip(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("gzip")
  }

  /// Append `gzip` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_gzip_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("gzip", qvalue)
  }

  /// Append `deflate` to `Accept-Encoding`.
  pub fn accept_deflate(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("deflate")
  }

  /// Append `deflate` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_deflate_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("deflate", qvalue)
  }

  /// Append `br` to `Accept-Encoding`.
  pub fn accept_br(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("br")
  }

  /// Append `br` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_br_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("br", qvalue)
  }

  /// Append `identity` to `Accept-Encoding`.
  pub fn accept_identity(&mut self) -> error::Result<&mut Self> {
    self.accept_encoding("identity")
  }

  /// Append `identity` to `Accept-Encoding` with an HTTP q-value.
  pub fn accept_identity_with_q<Q: AsRef<str>>(&mut self, qvalue: Q) -> error::Result<&mut Self> {
    self.accept_encoding_with_q("identity", qvalue)
  }

  /// Append bounded `TE` request metadata without enabling transfer codings.
  pub fn te<S: AsRef<str>>(&mut self, coding: S) -> error::Result<&mut Self> {
    self.te_member(coding.as_ref(), None)
  }

  /// Append bounded `TE` request metadata with an HTTP q-value.
  pub fn te_with_q<C: AsRef<str>, Q: AsRef<str>>(
    &mut self,
    coding: C,
    qvalue: Q,
  ) -> error::Result<&mut Self> {
    self.te_member(coding.as_ref(), Some(qvalue.as_ref()))
  }

  /// Declare support for request trailers through bounded `TE` metadata.
  pub fn te_trailers(&mut self) -> error::Result<&mut Self> {
    self.te("trailers")
  }

  /// Append a token-only `Prefer` request metadata item.
  ///
  /// This records application preference metadata only; it does not schedule
  /// asynchronous work or alter response handling.
  pub fn prefer<S: AsRef<str>>(&mut self, name: S) -> error::Result<&mut Self> {
    self.prefer_member(name.as_ref(), None)
  }

  /// Append a token-valued `Prefer` request metadata item.
  ///
  /// This records application preference metadata only; it does not apply
  /// response preference policy.
  pub fn prefer_with_value<N: AsRef<str>, V: AsRef<str>>(
    &mut self,
    name: N,
    value: V,
  ) -> error::Result<&mut Self> {
    self.prefer_member(name.as_ref(), Some(value.as_ref()))
  }

  /// Set an `If-Range` validator with a single strong entity tag.
  ///
  /// `If-Range` only permits strong entity-tag validators. Use `header`
  /// directly for manual values that intentionally bypass this helper.
  pub fn if_range_etag<S: AsRef<str>>(&mut self, etag: S) -> error::Result<&mut Self> {
    let etag = validate_single_strong_etag(etag.as_ref())?;
    Ok(self.header(Header::new("If-Range", etag)))
  }

  /// Set an `If-Range` validator with an HTTP-date.
  pub fn if_range_date<S: AsRef<str>>(&mut self, http_date: S) -> error::Result<&mut Self> {
    let http_date = validate_http_date(http_date.as_ref())?;
    Ok(self.header(Header::new("If-Range", http_date)))
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

  fn accept_encoding_member(
    &mut self,
    coding: &str,
    qvalue: Option<&str>,
  ) -> error::Result<&mut Self> {
    let coding = coding.trim();
    if !is_http_token(coding) {
      return Err(error::builder_with_message(
        "invalid Accept-Encoding coding",
      ));
    }
    let qvalue = qvalue.map(validate_accept_encoding_qvalue).transpose()?;
    let member = qvalue.map_or_else(
      || coding.to_string(),
      |qvalue| format!("{coding};q={qvalue}"),
    );
    if member.len() > MAX_ACCEPT_ENCODING_VALUE_BYTES {
      return Err(error::builder_with_message(
        "Accept-Encoding header value is too large",
      ));
    }

    let headers = self.request.headers_mut();
    let existing = headers
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("Accept-Encoding"));
    if let Some(header) = existing {
      let existing_codings = parse_accept_encoding_codings(header.value())?;
      if existing_codings
        .iter()
        .any(|known| known.eq_ignore_ascii_case(coding))
      {
        return Err(error::builder_with_message(
          "duplicate Accept-Encoding coding",
        ));
      }
      if existing_codings.len() >= MAX_ACCEPT_ENCODINGS {
        return Err(error::builder_with_message(
          "too many Accept-Encoding codings",
        ));
      }
      let value = format!("{}, {member}", header.value());
      if value.len() > MAX_ACCEPT_ENCODING_VALUE_BYTES {
        return Err(error::builder_with_message(
          "Accept-Encoding header value is too large",
        ));
      }
      header.replace(Header::new("Accept-Encoding", value));
    } else {
      headers.push(Header::new("Accept-Encoding", member));
    }
    Ok(self)
  }

  fn te_member(&mut self, coding: &str, qvalue: Option<&str>) -> error::Result<&mut Self> {
    let coding = coding.trim();
    if !is_http_token(coding) || coding.eq_ignore_ascii_case("chunked") {
      return Err(error::builder_with_message("invalid TE coding"));
    }
    let qvalue = qvalue.map(validate_accept_encoding_qvalue).transpose()?;
    let member = qvalue.map_or_else(
      || coding.to_string(),
      |qvalue| format!("{coding};q={qvalue}"),
    );
    append_unique_metadata_member(
      self.request.headers_mut(),
      "TE",
      coding,
      member,
      "invalid TE coding",
      "duplicate TE coding",
      "too many TE codings",
      "TE header value is too large",
      parse_te_codings,
    )?;
    Ok(self)
  }

  fn prefer_member(&mut self, name: &str, value: Option<&str>) -> error::Result<&mut Self> {
    let name = name.trim();
    if !is_http_token(name) || !value.is_none_or(|value| is_http_token(value.trim())) {
      return Err(error::builder_with_message("invalid Prefer preference"));
    }
    let member = value.map_or_else(
      || name.to_string(),
      |value| format!("{name}={}", value.trim()),
    );
    append_unique_metadata_member(
      self.request.headers_mut(),
      "Prefer",
      name,
      member,
      "invalid Prefer preference",
      "duplicate Prefer preference",
      "too many Prefer preferences",
      "Prefer header value is too large",
      parse_prefer_names,
    )?;
    Ok(self)
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
    if self.request.http2_extended_connect_protocol().is_some() {
      return Err(error::builder_with_message(
        "HTTP/2 extended CONNECT is only supported by the prior-knowledge h2c client",
      ));
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

fn bounded_forwarded_header_value(forwarded: Forwarded) -> error::Result<String> {
  let value = forwarded.header_value();
  if value.len() > MAX_FORWARDED_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Forwarded header value is too large",
    ));
  }
  Ok(value)
}

fn validate_max_forwards(value: &str) -> error::Result<&str> {
  if value.is_empty()
    || value.len() > MAX_MAX_FORWARDS_VALUE_BYTES
    || !value.bytes().all(|byte| byte.is_ascii_digit())
  {
    return Err(error::builder_with_message(
      "Max-Forwards must be a non-empty decimal u32",
    ));
  }
  value.parse::<u32>().map_err(|_| {
    error::builder_with_message("Max-Forwards must be a decimal value no greater than u32::MAX")
  })?;
  Ok(value)
}

const MAX_MAX_FORWARDS_VALUE_BYTES: usize = 10;
const MAX_ACCEPT_ENCODING_VALUE_BYTES: usize = 64 * 1024;
const MAX_ACCEPT_ENCODINGS: usize = 32;
const MAX_AUTHORIZATION_VALUE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_METADATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_METADATA_MEMBERS: usize = 32;

fn append_unique_metadata_member(
  headers: &mut Vec<Header>,
  header_name: &str,
  key: &str,
  member: String,
  invalid_error: &str,
  duplicate_error: &str,
  count_error: &str,
  size_error: &str,
  parse_keys: fn(&str) -> error::Result<Vec<&str>>,
) -> error::Result<()> {
  if member.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
    return Err(error::builder_with_message(size_error));
  }
  if let Some(header) = headers
    .iter_mut()
    .find(|header| header.name().eq_ignore_ascii_case(header_name))
  {
    let known =
      parse_keys(header.value()).map_err(|_| error::builder_with_message(invalid_error))?;
    if known.iter().any(|known| known.eq_ignore_ascii_case(key)) {
      return Err(error::builder_with_message(duplicate_error));
    }
    if known.len() >= MAX_REQUEST_METADATA_MEMBERS {
      return Err(error::builder_with_message(count_error));
    }
    let value = format!("{}, {member}", header.value());
    if value.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
      return Err(error::builder_with_message(size_error));
    }
    header.replace(Header::new(header_name, value));
  } else {
    headers.push(Header::new(header_name, member));
  }
  Ok(())
}

fn parse_te_codings(value: &str) -> error::Result<Vec<&str>> {
  parse_metadata_members(value, "invalid TE coding", |member| {
    let mut parts = member.split(';');
    let coding = parts.next().unwrap_or_default().trim();
    if !is_http_token(coding) || coding.eq_ignore_ascii_case("chunked") {
      return None;
    }
    match parts.next() {
      None => Some(coding),
      Some(parameter) if parts.next().is_none() => {
        let (name, value) = parameter.trim().split_once('=')?;
        (name.trim().eq_ignore_ascii_case("q")
          && validate_accept_encoding_qvalue(value.trim()).is_ok())
        .then_some(coding)
      }
      Some(_) => None,
    }
  })
}

fn parse_prefer_names(value: &str) -> error::Result<Vec<&str>> {
  parse_metadata_members(value, "invalid Prefer preference", |member| {
    let (name, value) = member
      .split_once('=')
      .map_or((member, None), |(name, value)| (name, Some(value)));
    let name = name.trim();
    (is_http_token(name) && value.is_none_or(|value| is_http_token(value.trim()))).then_some(name)
  })
}

fn parse_metadata_members<'a>(
  value: &'a str,
  error_message: &str,
  parse: impl Fn(&'a str) -> Option<&'a str>,
) -> error::Result<Vec<&'a str>> {
  if value.len() > MAX_REQUEST_METADATA_VALUE_BYTES {
    return Err(error::builder_with_message(error_message));
  }
  let mut members = Vec::new();
  for member in value.split(',') {
    let Some(key) = parse(member.trim()) else {
      return Err(error::builder_with_message(error_message));
    };
    if members
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(key))
      || members.len() >= MAX_REQUEST_METADATA_MEMBERS
    {
      return Err(error::builder_with_message(error_message));
    }
    members.push(key);
  }
  Ok(members)
}

fn parse_accept_encoding_codings(value: &str) -> error::Result<Vec<&str>> {
  if value.len() > MAX_ACCEPT_ENCODING_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Accept-Encoding header value is too large",
    ));
  }

  let mut codings = Vec::new();
  for member in value.split(',') {
    let (coding, _) = split_accept_encoding_member(member)?;
    if codings
      .iter()
      .any(|known: &&str| known.eq_ignore_ascii_case(coding))
    {
      return Err(error::builder_with_message(
        "duplicate Accept-Encoding coding",
      ));
    }
    if codings.len() >= MAX_ACCEPT_ENCODINGS {
      return Err(error::builder_with_message(
        "too many Accept-Encoding codings",
      ));
    }
    codings.push(coding);
  }
  Ok(codings)
}

fn split_accept_encoding_member(member: &str) -> error::Result<(&str, Option<&str>)> {
  let mut parts = member.split(';');
  let coding = parts.next().unwrap_or_default().trim();
  if !is_http_token(coding) {
    return Err(error::builder_with_message(
      "invalid Accept-Encoding coding",
    ));
  }
  let Some(parameter) = parts.next() else {
    return Ok((coding, None));
  };
  if parts.next().is_some() {
    return Err(error::builder_with_message(
      "invalid Accept-Encoding q-value",
    ));
  }
  let Some((name, qvalue)) = parameter.trim().split_once('=') else {
    return Err(error::builder_with_message(
      "invalid Accept-Encoding q-value",
    ));
  };
  if !name.trim().eq_ignore_ascii_case("q") {
    return Err(error::builder_with_message(
      "invalid Accept-Encoding q-value",
    ));
  }
  let qvalue = validate_accept_encoding_qvalue(qvalue.trim())?;
  Ok((coding, Some(qvalue)))
}

fn validate_accept_encoding_qvalue(qvalue: &str) -> error::Result<&str> {
  let valid = match qvalue.split_once('.') {
    Some((whole, fraction)) => {
      fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(whole, "0" | "1")
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => matches!(qvalue, "0" | "1"),
  };
  if valid {
    Ok(qvalue)
  } else {
    Err(error::builder_with_message(
      "invalid Accept-Encoding q-value",
    ))
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

fn validate_single_strong_etag(etag: &str) -> error::Result<&str> {
  let etag = validate_single_etag(etag)?;
  if etag == "*" || etag.starts_with("W/") {
    return Err(error::builder_with_message(
      "If-Range entity-tag helper accepts only a single strong entity tag",
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

const MAX_ACCEPT_LANGUAGE_VALUE_BYTES: usize = 64 * 1024;
const MAX_ACCEPT_LANGUAGE_RANGES: usize = 32;

fn build_accept_language_value<I, L>(ranges: I) -> error::Result<String>
where
  I: IntoIterator<Item = L>,
  L: AsRef<str>,
{
  let mut parsed = Vec::new();

  for value in ranges {
    if value.as_ref().len() > MAX_ACCEPT_LANGUAGE_VALUE_BYTES {
      return Err(error::builder_with_message(
        "Accept-Language header value is too large",
      ));
    }
    for range in value.as_ref().split(',') {
      let range = range.trim();
      let (language_range, quality) = parse_accept_language_item(range)?;
      if parsed.len() >= MAX_ACCEPT_LANGUAGE_RANGES {
        return Err(error::builder_with_message(
          "too many Accept-Language ranges",
        ));
      }
      if parsed
        .iter()
        .any(|(known, _): &(String, Option<String>)| known.eq_ignore_ascii_case(language_range))
      {
        return Err(error::builder_with_message(
          "duplicate Accept-Language range",
        ));
      }
      parsed.push((language_range.to_string(), quality.map(ToString::to_string)));
    }
  }

  if parsed.is_empty() {
    return Err(error::builder_with_message("invalid Accept-Language range"));
  }

  let value = parsed
    .into_iter()
    .map(|(range, quality)| match quality {
      Some(quality) => format!("{range}; q={quality}"),
      None => range,
    })
    .collect::<Vec<_>>()
    .join(", ");
  if value.len() > MAX_ACCEPT_LANGUAGE_VALUE_BYTES {
    return Err(error::builder_with_message(
      "Accept-Language header value is too large",
    ));
  }
  Ok(value)
}

fn parse_accept_language_item(value: &str) -> error::Result<(&str, Option<&str>)> {
  let mut parts = value.split(';');
  let range = parts.next().unwrap_or_default().trim();
  if !is_language_range(range) {
    return Err(error::builder_with_message("invalid Accept-Language range"));
  }

  let Some(parameter) = parts.next() else {
    return Ok((range, None));
  };
  if parts.next().is_some() {
    return Err(error::builder_with_message(
      "invalid Accept-Language q-value",
    ));
  }
  let Some((name, quality)) = parameter.trim().split_once('=') else {
    return Err(error::builder_with_message(
      "invalid Accept-Language q-value",
    ));
  };
  let quality = quality.trim();
  if !name.trim().eq_ignore_ascii_case("q") || !is_qvalue(quality) {
    return Err(error::builder_with_message(
      "invalid Accept-Language q-value",
    ));
  }
  Ok((range, Some(quality)))
}

fn is_language_range(value: &str) -> bool {
  if value == "*" {
    return true;
  }

  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };
  (1..=8).contains(&primary.len())
    && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
    && subtags.all(|subtag| {
      (1..=8).contains(&subtag.len()) && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn is_qvalue(value: &str) -> bool {
  match value.split_once('.') {
    Some((whole, fraction)) => {
      (whole == "0" || whole == "1")
        && fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => value == "0" || value == "1",
  }
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
