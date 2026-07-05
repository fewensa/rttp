use std::net::TcpStream;

use futures::io::{AllowStdIo, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use socks::{Socks4Stream, Socks5Stream};
use std::io::Write;
use url::Url;

#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use crate::connection::connection::{connect_tcp_stream, read_proxy_connect_response, Connection};
use crate::connection::connection_reader::{response_body_kind, ResponseBodyKind};
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType, RoUrl};
const CRLF: &[u8] = b"\r\n";

pub struct AsyncConnection<'a> {
  conn: Connection<'a>,
}

impl<'a> AsyncConnection<'a> {
  pub fn new(request: RawRequest<'a>) -> AsyncConnection<'a> {
    Self {
      conn: Connection::new(request),
    }
  }

  pub async fn async_call(mut self) -> error::Result<Response> {
    loop {
      let url = self.conn.url().map_err(error::builder)?;
      let proxy = self.conn.proxy().clone();
      let binary = if let Some(proxy) = proxy.as_ref() {
        self.call_with_proxy(&url, proxy).await?
      } else {
        self.async_send(&url).await?
      };

      let response = Response::new(self.conn.rourl().clone(), binary)?;
      let config = self.conn.config().clone();

      if let Some(location) = response.location() {
        if url.as_str() == location {
          return Err(error::loop_detected(url));
        }

        if config.auto_redirect() {
          let count = self.conn.count();
          if count > config.max_redirect() {
            return Err(error::too_many_redirects(url));
          }

          let redirect_url = self.conn.redirect_url(&url, location)?;
          self
            .conn
            .request_mut()
            .origin_mut()
            .url_set(RoUrl::with(redirect_url));
          self.conn.request_mut().origin_mut().count_set(count + 1);
          continue;
        }
      }

      self.conn.closed_set(true);
      return Ok(response);
    }
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

  async fn async_read_stream<S>(&self, _url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: AsyncRead + Unpin,
  {
    let mut binary = async_read_response_header(stream).await?;
    match response_body_kind(&binary, self.conn.expect_no_response_body())? {
      ResponseBodyKind::NoBody => {}
      ResponseBodyKind::Chunked => {
        binary.extend_from_slice(&async_read_chunked_body(stream).await?);
      }
      ResponseBodyKind::ContentLength(content_length) => {
        let current_len = binary.len();
        binary.resize(current_len + content_length, 0);
        stream
          .read_exact(&mut binary[current_len..])
          .await
          .map_err(error::request)?;
      }
      ResponseBodyKind::UntilEof => {
        stream
          .read_to_end(&mut binary)
          .await
          .map_err(error::request)?;
      }
    }
    Ok(binary)
  }
}

async fn async_read_response_header<S>(stream: &mut S) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin,
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

async fn async_read_chunked_body<S>(stream: &mut S) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin,
{
  let mut body = Vec::new();

  loop {
    let line = async_read_crlf_line(stream).await?;
    let chunk_size = parse_chunk_size(&line)?;

    if chunk_size == 0 {
      async_consume_trailers(stream).await?;
      return Ok(body);
    }

    let current_len = body.len();
    body.resize(current_len + chunk_size, 0);
    stream
      .read_exact(&mut body[current_len..])
      .await
      .map_err(error::request)?;
    async_consume_crlf(stream).await?;
  }
}

async fn async_read_crlf_line<S>(stream: &mut S) -> error::Result<Vec<u8>>
where
  S: AsyncRead + Unpin,
{
  let mut line = Vec::new();
  let mut byte = [0u8; 1];

  loop {
    let read = stream.read(&mut byte).await.map_err(error::request)?;
    if read == 0 {
      return Err(error::bad_response("Unexpected end of chunked body"));
    }

    line.push(byte[0]);
    if line.ends_with(CRLF) {
      return Ok(line);
    }
  }
}

fn parse_chunk_size(line: &[u8]) -> error::Result<usize> {
  let line = std::str::from_utf8(line).map_err(error::response)?;
  let size = line
    .trim_end_matches("\r\n")
    .split(';')
    .next()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| error::bad_response("Chunk size line is empty"))?;

  usize::from_str_radix(size, 16).map_err(|_| error::bad_response("Invalid chunk size"))
}

async fn async_consume_crlf<S>(stream: &mut S) -> error::Result<()>
where
  S: AsyncRead + Unpin,
{
  let mut suffix = [0u8; 2];
  stream
    .read_exact(&mut suffix)
    .await
    .map_err(error::request)?;
  if suffix == *CRLF {
    Ok(())
  } else {
    Err(error::bad_response("Invalid chunk terminator"))
  }
}

async fn async_consume_trailers<S>(stream: &mut S) -> error::Result<()>
where
  S: AsyncRead + Unpin,
{
  loop {
    let line = async_read_crlf_line(stream).await?;
    if line == CRLF {
      return Ok(());
    }
  }
}

// connection send
impl<'a> AsyncConnection<'a> {
  async fn async_send(&self, url: &Url) -> error::Result<Vec<u8>> {
    let addr = self.conn.addr(url)?;
    let stream = self.async_tcp_stream(&addr).await?;

    self.async_send_with_stream(url, stream).await
  }

  async fn async_send_with_stream(&self, url: &Url, stream: TcpStream) -> error::Result<Vec<u8>> {
    match url.scheme() {
      "http" => {
        let mut stream = AllowStdIo::new(stream);
        self.async_send_http(url, &mut stream).await
      }
      "https" => self.async_send_https(url, stream).await,
      _ => Err(error::url_bad_scheme(url.clone())),
    }
  }

  async fn async_send_http<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: AsyncRead + AsyncWrite + Unpin,
  {
    self.async_write_stream(stream).await?;
    self.async_read_stream(url, stream).await
  }

  async fn async_send_https(&self, url: &Url, stream: TcpStream) -> error::Result<Vec<u8>> {
    #[cfg(feature = "tls-rustls")]
    {
      return self.async_send_https_rustls(url, stream).await;
    }

    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      let mut stream = stream;
      return self.conn.block_send_https(url, &mut stream);
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

  #[cfg(feature = "tls-rustls")]
  async fn async_send_https_rustls(&self, url: &Url, stream: TcpStream) -> error::Result<Vec<u8>> {
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
        .map_err(|e| error::bad_ssl(e.to_string()))?;
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
      .map_err(|e| error::bad_ssl(e.to_string()))?;

    self.async_write_stream(&mut tls_stream).await?;
    self.async_read_stream(url, &mut tls_stream).await
  }
}

// proxy connection
impl<'a> AsyncConnection<'a> {
  async fn call_with_proxy(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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

  async fn call_with_proxy_http(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let stream = self.async_tcp_stream(&addr).await?;
    let mut stream = AllowStdIo::new(stream);
    let header = self.conn.proxy_http_header(url, proxy);

    self.async_write_request(&mut stream, &header).await?;
    self.async_read_stream(url, &mut stream).await
  }

  async fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.async_tcp_stream(&addr).await?;

    stream
      .write_all(connect_header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)?;
    read_proxy_connect_response(&mut stream)?;

    self.async_send_with_stream(url, stream).await
  }

  async fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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
    self.conn.block_send_with_stream(url, &mut stream)
  }

  async fn call_with_proxy_socks5(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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
    self.conn.block_send_with_stream(url, &mut stream)
  }
}
