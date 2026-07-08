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
