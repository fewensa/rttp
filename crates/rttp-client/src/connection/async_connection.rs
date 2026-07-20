use std::collections::HashSet;
use std::net::TcpStream;

use futures::io::{AllowStdIo, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use rttp_protocol::http1::{parse_chunk_size as parse_protocol_chunk_size, ChunkSizeError};
use socks::{Socks4Stream, Socks5Stream};
use std::io::{self, Write};
use url::Url;

#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use crate::connection::connection::{
  connect_tcp_stream, prepend_informational_responses, read_proxy_connect_response,
  request_expects_continue, Connection, ExpectContinueResult,
};
use crate::connection::connection_reader::{
  is_skippable_informational_status, parse_informational_response, response_body_kind,
  response_connection_reusable, response_connection_should_close, response_headers,
  response_status_code, validate_response_trailer_header, ResponseBodyKind, ResponseParts,
  MAX_CHUNKED_RESPONSE_LINE_BYTES,
};
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Header, Proxy, ProxyType};
const CRLF: &[u8] = b"\r\n";

pub struct AsyncStreamingResponse<'a, S: AsyncRead + Unpin + ?Sized> {
  head: Vec<u8>,
  body: AsyncResponseBodyReader<'a, S>,
}

impl<'a, S: AsyncRead + Unpin + ?Sized> AsyncStreamingResponse<'a, S> {
  pub fn code(&self) -> error::Result<u16> {
    response_status_code(&self.head)
  }

  pub fn headers(&self) -> error::Result<Vec<Header>> {
    response_headers(&self.head)
  }

  pub fn head(&self) -> &[u8] {
    &self.head
  }

  pub fn body_mut(&mut self) -> &mut AsyncResponseBodyReader<'a, S> {
    &mut self.body
  }

  pub fn trailers(&self) -> &Vec<Header> {
    self.body.trailers()
  }

  pub fn trailer<SName: AsRef<str>>(&self, name: SName) -> Option<&Header> {
    self
      .trailers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn trailer_value<SName: AsRef<str>>(&self, name: SName) -> Option<&String> {
    self.trailer(name).map(|header| header.value())
  }

  async fn read_to_parts(mut self) -> error::Result<ResponseParts> {
    let close_connection = response_connection_should_close(&self.head)?;
    let connection_reusable = response_connection_reusable(&self.head, &self.body.kind)?;
    let mut binary = self.head;
    self.body.read_to_end(&mut binary).await?;
    Ok(ResponseParts {
      binary,
      trailers: self.body.trailers().clone(),
      informational_responses: Vec::new(),
      connection_reusable,
      close_connection,
    })
  }
}

pub struct AsyncResponseBodyReader<'a, S: AsyncRead + Unpin + ?Sized> {
  stream: &'a mut S,
  kind: ResponseBodyKind,
  remaining: usize,
  chunk_remaining: usize,
  chunk_needs_crlf: bool,
  trailers: Vec<Header>,
  eof: bool,
}

impl<'a, S: AsyncRead + Unpin + ?Sized> AsyncResponseBodyReader<'a, S> {
  fn new(stream: &'a mut S, kind: ResponseBodyKind) -> Self {
    let remaining = match kind {
      ResponseBodyKind::ContentLength(length) => length,
      _ => 0,
    };
    let eof = matches!(kind, ResponseBodyKind::NoBody);
    Self {
      stream,
      kind,
      remaining,
      chunk_remaining: 0,
      chunk_needs_crlf: false,
      trailers: Vec::new(),
      eof,
    }
  }

  pub fn trailers(&self) -> &Vec<Header> {
    &self.trailers
  }

  pub async fn read(&mut self, buf: &mut [u8]) -> error::Result<usize> {
    match self.kind {
      ResponseBodyKind::NoBody => Ok(0),
      ResponseBodyKind::ContentLength(_) => self.read_fixed_length(buf).await,
      ResponseBodyKind::Chunked => self.read_chunked(buf).await,
      ResponseBodyKind::UntilEof => {
        let read = self.stream.read(buf).await.map_err(error::request)?;
        if read == 0 {
          self.eof = true;
        }
        Ok(read)
      }
    }
  }

  pub async fn read_to_end(&mut self, body: &mut Vec<u8>) -> error::Result<usize> {
    let start = body.len();
    let mut buf = [0u8; 8 * 1024];
    loop {
      let read = self.read(&mut buf).await?;
      if read == 0 {
        return Ok(body.len() - start);
      }
      body.extend_from_slice(&buf[..read]);
    }
  }

  async fn read_fixed_length(&mut self, buf: &mut [u8]) -> error::Result<usize> {
    if self.remaining == 0 || buf.is_empty() {
      self.eof = self.remaining == 0;
      return Ok(0);
    }

    let limit = buf.len().min(self.remaining);
    let read = self
      .stream
      .read(&mut buf[..limit])
      .await
      .map_err(error::request)?;
    if read == 0 {
      return Err(error::request(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "failed to fill whole buffer",
      )));
    }
    self.remaining -= read;
    if self.remaining == 0 {
      self.eof = true;
    }
    Ok(read)
  }

  async fn read_chunked(&mut self, buf: &mut [u8]) -> error::Result<usize> {
    if self.eof || buf.is_empty() {
      return Ok(0);
    }

    if self.chunk_needs_crlf {
      async_consume_crlf(self.stream).await?;
      self.chunk_needs_crlf = false;
    }

    while self.chunk_remaining == 0 {
      let line = async_read_bounded_crlf_line(self.stream, MAX_CHUNKED_RESPONSE_LINE_BYTES).await?;
      let chunk_size = parse_chunk_size(&line)?;
      if chunk_size == 0 {
        self.trailers = async_read_trailers(self.stream).await?;
        self.eof = true;
        return Ok(0);
      }
      self.chunk_remaining = chunk_size;
    }

    let limit = buf.len().min(self.chunk_remaining);
    let read = self
      .stream
      .read(&mut buf[..limit])
      .await
      .map_err(error::request)?;
    if read == 0 {
      return Err(error::bad_response("Unexpected end of chunked body"));
    }
    self.chunk_remaining -= read;
    if self.chunk_remaining == 0 {
      self.chunk_needs_crlf = true;
    }
    Ok(read)
  }
}

pub struct AsyncConnection<'a> {
  conn: Connection<'a>,
}

pub(crate) enum AsyncStreamingRequestBody<'a> {
  Fixed {
    reader: &'a mut (dyn AsyncRead + Unpin),
    content_length: u64,
  },
  Chunked {
    reader: &'a mut (dyn AsyncRead + Unpin),
    trailers: &'a [Header],
  },
}

impl<'a> AsyncConnection<'a> {
  pub fn new(request: RawRequest<'a>) -> AsyncConnection<'a> {
    Self {
      conn: Connection::new(request),
    }
  }

  pub async fn async_call(mut self) -> error::Result<Response> {
    let mut visited_urls = HashSet::new();
    let mut reusable_stream: Option<AllowStdIo<TcpStream>> = None;

    loop {
      let url = self.conn.url().map_err(error::builder)?;
      visited_urls.insert((
        self.conn.request().origin().method().to_uppercase(),
        url.clone(),
      ));
      let proxy = self.conn.proxy().clone();
      let parts = if let Some(proxy) = proxy.as_ref() {
        reusable_stream = None;
        self.call_with_proxy(&url, proxy).await?
      } else if url.scheme() == "http" {
        let mut stream = match reusable_stream.take() {
          Some(stream) => stream,
          None => {
            let addr = self.conn.addr(&url)?;
            AllowStdIo::new(self.async_tcp_stream(&addr).await?)
          }
        };
        let parts = self.async_send_http_parts(&url, &mut stream).await?;
        if parts.connection_reusable {
          reusable_stream = Some(stream);
        }
        parts
      } else {
        reusable_stream = None;
        self.async_send_parts(&url).await?
      };

      let close_connection = parts.close_connection;
      let response = Response::with_trailers_and_informational(
        self.conn.rourl().clone(),
        parts.binary,
        parts.trailers,
        parts.informational_responses,
      )?;
      let config = self.conn.config().clone();

      if response.is_redirect() {
        let Some(location) = response.location() else {
          self.conn.closed_set(close_connection);
          return Ok(response);
        };
        if config.auto_redirect() {
          let count = self.conn.count();
          if count > config.max_redirect() {
            return Err(error::too_many_redirects(url));
          }

          let redirect_url = self.conn.resolve_redirect_url(&url, location)?;
          let strip_sensitive_headers = !self.conn.is_same_origin_url(&url, &redirect_url.url);
          if !self.conn.is_same_origin_url(&url, &redirect_url.url)
            || redirect_url.url.scheme() != "http"
          {
            reusable_stream = None;
          }
          self.conn.request_mut().redirect_status_set(response.code());
          let next_visit = (
            self.conn.request().origin().method().to_uppercase(),
            redirect_url.url.clone(),
          );
          if visited_urls.contains(&next_visit) {
            return Err(error::loop_detected(url));
          }
          self.conn.request_mut().redirect_url_set(
            redirect_url.url.to_string(),
            strip_sensitive_headers,
            Some(&redirect_url.request_target),
          )?;
          self.conn.request_mut().origin_mut().count_set(count + 1);
          continue;
        }
      }

      self.conn.closed_set(close_connection);
      return Ok(response);
    }
  }

  pub async fn async_call_streaming_body(
    mut self,
    body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<Response> {
    if self.conn.proxy().is_some() {
      return Err(error::builder_with_message(
        "streaming request bodies do not support proxies",
      ));
    }

    let url = self.conn.url().map_err(error::builder)?;
    let parts = self.async_send_streaming_parts(&url, body).await?;
    let close_connection = parts.close_connection;
    let response = Response::with_trailers_and_informational(
      self.conn.rourl().clone(),
      parts.binary,
      parts.trailers,
      parts.informational_responses,
    )?;
    self.conn.closed_set(close_connection);
    Ok(response)
  }
}

impl<'a> AsyncConnection<'a> {
  async fn async_tcp_stream(&self, addr: &str) -> error::Result<TcpStream> {
    connect_tcp_stream(addr, self.conn.config())
  }

  async fn async_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    self.async_write_request(stream, self.conn.header()).await
  }

  async fn async_write_request<S>(&self, stream: &mut S, header: &str) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    let body = self.conn.body();

    stream
      .write_all(header.as_bytes())
      .await
      .map_err(error::request)?;
    if let Some(body) = body {
      stream
        .write_all(body.bytes())
        .await
        .map_err(error::request)?;
    }
    stream.flush().await.map_err(error::request)?;

    Ok(())
  }

  async fn async_write_request_header_with<S>(
    &self,
    stream: &mut S,
    header: &str,
  ) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    stream
      .write_all(header.as_bytes())
      .await
      .map_err(error::request)?;
    stream.flush().await.map_err(error::request)
  }

  async fn async_write_request_body<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    if let Some(body) = self.conn.body() {
      stream
        .write_all(body.bytes())
        .await
        .map_err(error::request)?;
    }
    stream.flush().await.map_err(error::request)
  }

  async fn async_write_streaming_request<S>(
    &self,
    stream: &mut S,
    mut body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    stream
      .write_all(self.conn.header().as_bytes())
      .await
      .map_err(error::request)?;
    match &mut body {
      AsyncStreamingRequestBody::Fixed {
        reader,
        content_length,
      } => async_write_fixed_streaming_body(stream, *reader, *content_length).await?,
      AsyncStreamingRequestBody::Chunked { reader, trailers } => {
        async_write_chunked_streaming_body(stream, *reader, trailers).await?
      }
    }
    stream.flush().await.map_err(error::request)
  }

  async fn async_read_stream_parts<S>(
    &self,
    _url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: AsyncRead + Unpin,
  {
    let (binary, informational_responses) =
      async_read_response_head_with_informational(stream).await?;
    self
      .async_read_stream_parts_after_header_with_informational(
        stream,
        binary,
        informational_responses,
      )
      .await
  }

  async fn async_read_stream_parts_after_header_with_informational<S>(
    &self,
    stream: &mut S,
    binary: Vec<u8>,
    informational_responses: Vec<crate::response::InformationalResponse>,
  ) -> error::Result<ResponseParts>
  where
    S: AsyncRead + Unpin,
  {
    let mut parts =
      async_streaming_response_after_header(stream, self.conn.expect_no_response_body(), binary)
        .await?
        .read_to_parts()
        .await?;
    parts.informational_responses = informational_responses;
    Ok(parts)
  }

  async fn async_send_expect_continue_parts<S>(
    &self,
    stream: &mut S,
  ) -> error::Result<ExpectContinueResult>
  where
    S: AsyncRead + AsyncWrite + Unpin,
  {
    self
      .async_send_expect_continue_parts_with_header(stream, self.conn.header())
      .await
  }

  async fn async_send_expect_continue_parts_with_header<S>(
    &self,
    stream: &mut S,
    header: &str,
  ) -> error::Result<ExpectContinueResult>
  where
    S: AsyncRead + AsyncWrite + Unpin,
  {
    if !request_expects_continue(header, self.conn.body().as_ref()) {
      return Ok(ExpectContinueResult::NotUsed);
    }

    self.async_write_request_header_with(stream, header).await?;
    let mut informational_responses = Vec::new();
    loop {
      let header = async_read_response_header(stream).await?;
      let status_code = response_status_code(&header)?;
      if status_code == 100 {
        informational_responses.push(parse_informational_response(&header)?);
        self.async_write_request_body(stream).await?;
        return Ok(ExpectContinueResult::BodySent(informational_responses));
      }
      if is_skippable_informational_status(status_code) {
        informational_responses.push(parse_informational_response(&header)?);
        continue;
      }
      return self
        .async_read_stream_parts_after_header_with_informational(
          stream,
          header,
          informational_responses,
        )
        .await
        .map(ExpectContinueResult::Final);
    }
  }
}

#[cfg(test)]
async fn async_read_response_head<S>(stream: &mut S) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin + ?Sized,
{
  loop {
    let header = async_read_response_header(stream).await?;
    let status_code = response_status_code(&header)?;
    if is_skippable_informational_status(status_code) {
      continue;
    }
    return Ok(header);
  }
}

async fn async_read_response_head_with_informational<S>(
  stream: &mut S,
) -> error::Result<(Vec<u8>, Vec<crate::response::InformationalResponse>)>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let mut informational_responses = Vec::new();
  loop {
    let header = async_read_response_header(stream).await?;
    let status_code = response_status_code(&header)?;
    if is_skippable_informational_status(status_code) {
      informational_responses.push(parse_informational_response(&header)?);
      continue;
    }
    return Ok((header, informational_responses));
  }
}

pub async fn async_streaming_response_after_header<S>(
  stream: &mut S,
  expect_no_body: bool,
  head: Vec<u8>,
) -> error::Result<AsyncStreamingResponse<'_, S>>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let kind = response_body_kind(&head, expect_no_body)?;
  Ok(AsyncStreamingResponse {
    head,
    body: AsyncResponseBodyReader::new(stream, kind),
  })
}

async fn async_read_response_header<S>(stream: &mut S) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let mut header = Vec::new();
  let mut byte = [0u8; 1];

  loop {
    let read = stream.read(&mut byte).await.map_err(error::request)?;
    if read == 0 {
      if header.is_empty() {
        return Ok(header);
      }
      return Err(error::bad_response("Incomplete http response headers"));
    }

    header.push(byte[0]);
    if header.ends_with(b"\r\n\r\n") {
      return Ok(header);
    }
  }
}

async fn async_read_bounded_crlf_line<S>(stream: &mut S, max_len: usize) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let mut line = Vec::new();
  let mut byte = [0u8; 1];

  loop {
    let read = stream.read(&mut byte).await.map_err(error::request)?;
    if read == 0 {
      return Err(error::bad_response("Unexpected end of chunked body"));
    }

    if line.len() == max_len {
      return Err(error::bad_response("chunked response line is too large"));
    }

    line.push(byte[0]);
    if line.ends_with(CRLF) {
      return Ok(line);
    }
  }
}

fn parse_chunk_size(line: &[u8]) -> error::Result<usize> {
  parse_protocol_chunk_size(line).map_err(|error| match error {
    ChunkSizeError::NotUtf8 => {
      let size = line
        .strip_suffix(CRLF)
        .unwrap_or(line)
        .splitn(2, |byte| *byte == b';')
        .next()
        .expect("split always returns the first chunk size segment");
      error::response(std::str::from_utf8(size).expect_err("chunk size is not UTF-8"))
    }
    ChunkSizeError::Empty => error::bad_response("Chunk size line is empty"),
    ChunkSizeError::Invalid => error::bad_response("Invalid chunk size"),
    ChunkSizeError::InvalidExtension => error::bad_response("Invalid chunk extension"),
  })
}

async fn async_consume_crlf<S>(stream: &mut S) -> error::Result<()>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let mut suffix = [0u8; 2];
  stream.read_exact(&mut suffix).await.map_err(|err| {
    if err.kind() == io::ErrorKind::UnexpectedEof {
      error::bad_response("Unexpected end of chunked body")
    } else {
      error::request(err)
    }
  })?;
  if suffix == *CRLF {
    Ok(())
  } else {
    Err(error::bad_response("Invalid chunk terminator"))
  }
}

async fn async_read_trailers<S>(stream: &mut S) -> error::Result<Vec<Header>>
where
  S: AsyncRead + Unpin + ?Sized,
{
  let mut trailers = Vec::new();
  loop {
    let line = async_read_bounded_crlf_line(stream, MAX_CHUNKED_RESPONSE_LINE_BYTES).await?;
    if line == CRLF {
      return Ok(trailers);
    }

    trailers.push(parse_trailer_line(&line)?);
  }
}

fn parse_trailer_line(line: &[u8]) -> error::Result<Header> {
  let line = std::str::from_utf8(line).map_err(error::response)?;
  let line = line.trim_end_matches("\r\n");
  let (name, value) = line
    .split_once(':')
    .ok_or_else(|| error::bad_response("Invalid trailer header"))?;
  validate_response_trailer_header(name, value)?;

  Ok(Header::new(name, value))
}

// connection send
impl<'a> AsyncConnection<'a> {
  async fn async_send_parts(&self, url: &Url) -> error::Result<ResponseParts> {
    let addr = self.conn.addr(url)?;
    let stream = self.async_tcp_stream(&addr).await?;

    self.async_send_with_stream_parts(url, stream).await
  }

  async fn async_send_streaming_parts(
    &self,
    url: &Url,
    body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts> {
    let addr = self.conn.addr(url)?;
    let stream = self.async_tcp_stream(&addr).await?;

    self
      .async_send_streaming_with_stream_parts(url, stream, body)
      .await
  }

  async fn async_send_streaming_with_stream_parts(
    &self,
    url: &Url,
    stream: TcpStream,
    body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts> {
    match url.scheme() {
      "http" => {
        let mut stream = AllowStdIo::new(stream);
        self
          .async_write_streaming_request(&mut stream, body)
          .await?;
        self.async_read_stream_parts(url, &mut stream).await
      }
      "https" => {
        self
          .async_send_https_streaming_parts(url, stream, body)
          .await
      }
      _ => Err(error::url_bad_scheme(url.clone())),
    }
  }

  async fn async_send_with_stream_parts(
    &self,
    url: &Url,
    stream: TcpStream,
  ) -> error::Result<ResponseParts> {
    match url.scheme() {
      "http" => {
        let mut stream = AllowStdIo::new(stream);
        self.async_send_http_parts(url, &mut stream).await
      }
      "https" => self.async_send_https_parts(url, stream).await,
      _ => Err(error::url_bad_scheme(url.clone())),
    }
  }

  async fn async_send_http_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: AsyncRead + AsyncWrite + Unpin,
  {
    match self.async_send_expect_continue_parts(stream).await? {
      ExpectContinueResult::NotUsed => self.async_write_stream(stream).await?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .async_read_stream_parts(url, stream)
          .await
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.async_read_stream_parts(url, stream).await
  }

  async fn async_send_https_parts(
    &self,
    url: &Url,
    stream: TcpStream,
  ) -> error::Result<ResponseParts> {
    #[cfg(feature = "tls-rustls")]
    {
      return self.async_send_https_rustls_parts(url, stream).await;
    }

    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      let mut stream = stream;
      return self.conn.block_send_https_parts(url, &mut stream);
    }

    #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
    {
      let _ = url;
      let _ = stream;
      return Err(error::no_request_features(
        "Not have any tls features, Can't request a https url",
      ));
    }
  }

  async fn async_send_https_streaming_parts(
    &self,
    url: &Url,
    stream: TcpStream,
    body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts> {
    #[cfg(feature = "tls-rustls")]
    {
      return self
        .async_send_https_rustls_streaming_parts(url, stream, body)
        .await;
    }

    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      let _ = url;
      let _ = stream;
      let _ = body;
      return Err(error::no_request_features(
        "Async streaming HTTPS request bodies require the tls-rustls feature",
      ));
    }

    #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
    {
      let _ = url;
      let _ = stream;
      let _ = body;
      return Err(error::no_request_features(
        "Not have any tls features, Can't request a https url",
      ));
    }
  }

  #[cfg(feature = "tls-rustls")]
  async fn async_send_https_rustls_parts(
    &self,
    url: &Url,
    stream: TcpStream,
  ) -> error::Result<ResponseParts> {
    use futures_rustls::TlsConnector;
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, RootCertStore};

    use crate::connection::connection::{NoCertificateVerification, NoHostnameVerification};

    let config = self.conn.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if !config.verify_ssl_cert() {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
    } else if !config.verify_ssl_hostname() {
      let verifier = rustls::client::WebPkiServerVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(error::bad_ssl)?;
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoHostnameVerification::new(verifier)))
        .with_no_client_auth()
    } else {
      builder
        .with_root_certificates(root_store)
        .with_no_client_auth()
    };

    let host = self.conn.host(url)?;
    let server_name: ServerName<'static> = match host.parse::<std::net::IpAddr>() {
      Ok(ip) => ServerName::IpAddress(ip.into()),
      Err(_) => ServerName::try_from(host.as_str())
        .map_err(|_| error::bad_ssl(format!("Invalid server name: {}", host)))?
        .to_owned(),
    };

    let connector = TlsConnector::from(Arc::new(rustls_config));
    let async_tcp = AllowStdIo::new(stream);
    let mut tls_stream = connector
      .connect(server_name, async_tcp)
      .await
      .map_err(error::bad_ssl)?;

    match self
      .async_send_expect_continue_parts(&mut tls_stream)
      .await?
    {
      ExpectContinueResult::NotUsed => self.async_write_stream(&mut tls_stream).await?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .async_read_stream_parts(url, &mut tls_stream)
          .await
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.async_read_stream_parts(url, &mut tls_stream).await
  }

  #[cfg(feature = "tls-rustls")]
  async fn async_send_https_rustls_streaming_parts(
    &self,
    url: &Url,
    stream: TcpStream,
    body: AsyncStreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts> {
    use futures_rustls::TlsConnector;
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, RootCertStore};

    use crate::connection::connection::{NoCertificateVerification, NoHostnameVerification};

    let config = self.conn.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if !config.verify_ssl_cert() {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
    } else if !config.verify_ssl_hostname() {
      let verifier = rustls::client::WebPkiServerVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(error::bad_ssl)?;
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoHostnameVerification::new(verifier)))
        .with_no_client_auth()
    } else {
      builder
        .with_root_certificates(root_store)
        .with_no_client_auth()
    };

    let host = self.conn.host(url)?;
    let server_name: ServerName<'static> = match host.parse::<std::net::IpAddr>() {
      Ok(ip) => ServerName::IpAddress(ip.into()),
      Err(_) => ServerName::try_from(host.as_str())
        .map_err(|_| error::bad_ssl(format!("Invalid server name: {}", host)))?
        .to_owned(),
    };

    let connector = TlsConnector::from(Arc::new(rustls_config));
    let async_tcp = AllowStdIo::new(stream);
    let mut tls_stream = connector
      .connect(server_name, async_tcp)
      .await
      .map_err(error::bad_ssl)?;

    self
      .async_write_streaming_request(&mut tls_stream, body)
      .await?;
    self.async_read_stream_parts(url, &mut tls_stream).await
  }
}

async fn async_write_fixed_streaming_body<S>(
  writer: &mut S,
  reader: &mut (dyn AsyncRead + Unpin),
  content_length: u64,
) -> error::Result<()>
where
  S: AsyncWrite + Unpin,
{
  let mut remaining = content_length;
  let mut buffer = [0u8; 8 * 1024];
  while remaining > 0 {
    let limit = buffer.len().min(remaining as usize);
    let read = reader
      .read(&mut buffer[..limit])
      .await
      .map_err(error::request)?;
    if read == 0 {
      return Err(error::request(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "streaming request body ended before Content-Length",
      )));
    }
    writer
      .write_all(&buffer[..read])
      .await
      .map_err(error::request)?;
    remaining -= read as u64;
  }
  Ok(())
}

async fn async_write_chunked_streaming_body<S>(
  writer: &mut S,
  reader: &mut (dyn AsyncRead + Unpin),
  trailers: &[Header],
) -> error::Result<()>
where
  S: AsyncWrite + Unpin,
{
  let mut buffer = [0u8; 8 * 1024];
  loop {
    let read = reader.read(&mut buffer).await.map_err(error::request)?;
    if read == 0 {
      async_write_chunked_trailers(writer, trailers).await?;
      return Ok(());
    }
    writer
      .write_all(format!("{:x}\r\n", read).as_bytes())
      .await
      .map_err(error::request)?;
    writer
      .write_all(&buffer[..read])
      .await
      .map_err(error::request)?;
    writer.write_all(CRLF).await.map_err(error::request)?;
  }
}

async fn async_write_chunked_trailers<S>(writer: &mut S, trailers: &[Header]) -> error::Result<()>
where
  S: AsyncWrite + Unpin,
{
  writer.write_all(b"0\r\n").await.map_err(error::request)?;
  for trailer in trailers {
    writer
      .write_all(format!("{}: {}\r\n", trailer.name(), trailer.value()).as_bytes())
      .await
      .map_err(error::request)?;
  }
  writer.write_all(CRLF).await.map_err(error::request)
}

// proxy connection
impl<'a> AsyncConnection<'a> {
  async fn call_with_proxy(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    match proxy.type_() {
      ProxyType::HTTP => {
        if url.scheme() == "http" {
          self.call_with_proxy_http(url, proxy).await
        } else {
          self.call_with_proxy_https(url, proxy).await
        }
      }
      ProxyType::HTTPS => self.call_with_proxy_https(url, proxy).await,
      ProxyType::SOCKS4 => self.call_with_proxy_socks4(url, proxy).await,
      ProxyType::SOCKS5 => self.call_with_proxy_socks5(url, proxy).await,
    }
  }

  async fn call_with_proxy_http(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let stream = self.async_tcp_stream(&addr).await?;
    let mut stream = AllowStdIo::new(stream);
    let header = self.conn.proxy_http_header(url, proxy);

    match self
      .async_send_expect_continue_parts_with_header(&mut stream, &header)
      .await?
    {
      ExpectContinueResult::NotUsed => self.async_write_request(&mut stream, &header).await?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .async_read_stream_parts(url, &mut stream)
          .await
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.async_read_stream_parts(url, &mut stream).await
  }

  async fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.async_tcp_stream(&addr).await?;

    stream
      .write_all(connect_header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)?;
    read_proxy_connect_response(&mut stream)?;

    self.async_send_with_stream_parts(url, stream).await
  }

  async fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    // The SOCKS crate is sync-only, but its established stream still plugs into the existing
    // response path. Keeping it avoids duplicating the handshake state machine here.
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.conn.addr(url)?;
    let user = if let Some(u) = proxy.username() {
      u.to_string()
    } else {
      "".to_string()
    };
    let mut stream = Socks4Stream::connect(&addr_proxy[..], &addr_target[..], &user[..])
      .map_err(error::request)?;
    self.conn.block_send_with_stream_parts(url, &mut stream)
  }

  async fn call_with_proxy_socks5(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    // socket2 already covers the direct TCP path; the SOCKS exception stays isolated to the
    // proxy handshake and keeps the async API surface unchanged.
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.conn.addr(url)?;
    let mut stream = if let Some(u) = proxy.username() {
      if let Some(p) = proxy.password() {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], &p[..])
      } else {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], "")
      }
    } else {
      Socks5Stream::connect(&addr_proxy[..], &addr_target[..])
    }
    .map_err(error::request)?;
    self.conn.block_send_with_stream_parts(url, &mut stream)
  }
}

#[cfg(test)]
mod tests {
  use std::io::Cursor;

  use futures::executor::block_on;
  use futures::io::AllowStdIo;

  use super::{async_read_response_head, async_streaming_response_after_header};

  #[test]
  fn async_streaming_response_reads_fixed_length_body_incrementally() {
    block_on(async {
      let raw = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 5\r\n",
        "X-Trace: head\r\n",
        "\r\n",
        "hello",
        "next"
      );
      let mut cursor = AllowStdIo::new(Cursor::new(raw.as_bytes()));
      let head = async_read_response_head(&mut cursor).await.unwrap();
      let mut response = async_streaming_response_after_header(&mut cursor, false, head)
        .await
        .unwrap();
      let mut buf = [0; 2];

      assert_eq!(200, response.code().unwrap());
      assert_eq!(
        Some("head"),
        response
          .headers()
          .unwrap()
          .iter()
          .find(|header| header.name().eq_ignore_ascii_case("x-trace"))
          .map(|header| header.value().as_str())
      );
      assert_eq!(2, response.body_mut().read(&mut buf).await.unwrap());
      assert_eq!(b"he", &buf);
      assert_eq!(2, response.body_mut().read(&mut buf).await.unwrap());
      assert_eq!(b"ll", &buf);
      assert_eq!(1, response.body_mut().read(&mut buf).await.unwrap());
      assert_eq!(b"o", &buf[..1]);
      assert_eq!(0, response.body_mut().read(&mut buf).await.unwrap());
      assert!(response.trailers().is_empty());
    });
  }

  #[test]
  fn async_streaming_response_reads_chunked_body_and_exposes_trailers_after_eof() {
    block_on(async {
      let raw = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "2\r\nhe\r\n",
        "3\r\nllo\r\n",
        "0\r\n",
        "X-Trace: abc\r\n",
        "\r\n"
      );
      let mut cursor = AllowStdIo::new(Cursor::new(raw.as_bytes()));
      let head = async_read_response_head(&mut cursor).await.unwrap();
      let mut response = async_streaming_response_after_header(&mut cursor, false, head)
        .await
        .unwrap();
      let mut body = Vec::new();

      response.body_mut().read_to_end(&mut body).await.unwrap();

      assert_eq!(b"hello", body.as_slice());
      assert_eq!(
        Some("abc"),
        response.trailer_value("x-trace").map(String::as_str)
      );
    });
  }

  #[test]
  fn async_streaming_response_rejects_malformed_header_without_colon() {
    block_on(async {
      let raw = concat!(
        "HTTP/1.1 200 OK\r\n",
        "BrokenHeader\r\n",
        "Content-Length: 2\r\n",
        "\r\n",
        "OK"
      );
      let mut cursor = AllowStdIo::new(Cursor::new(raw.as_bytes()));
      let head = async_read_response_head(&mut cursor).await.unwrap();

      let error = match async_streaming_response_after_header(&mut cursor, false, head).await {
        Ok(_) => panic!("malformed response header should be rejected"),
        Err(error) => error,
      };

      assert!(
        error.to_string().contains("Invalid response header"),
        "unexpected error: {error}"
      );
      assert_eq!(
        (raw.len() - "OK".len()) as u64,
        cursor.get_ref().position(),
        "malformed response headers must be rejected before body bytes are consumed"
      );
    });
  }
}
