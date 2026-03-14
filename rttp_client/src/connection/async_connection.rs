use std::net::{TcpStream, ToSocketAddrs};

use futures::io::{AllowStdIo, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use socks::{Socks4Stream, Socks5Stream};
use socket2::{Domain, Protocol, Socket, Type};
use std::io::{self, Read, Write};
use std::time;
use url::Url;

#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use crate::connection::connection::Connection;
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType};

const HEADER_END: &[u8] = b"\r\n\r\n";
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
    let url = self.conn.url().map_err(error::builder)?;
    let proxy = self.conn.proxy();
    let binary = if let Some(proxy) = proxy {
      self.call_with_proxy(&url, proxy).await?
    } else {
      self.async_send(&url).await?
    };

    let response = Response::new(self.conn.rourl().clone(), binary)?;
    self.conn.closed_set(true);
    Ok(response)
  }
}

impl<'a> AsyncConnection<'a> {
  async fn async_tcp_stream(&self, addr: &str) -> error::Result<TcpStream> {
    let config = self.conn.config();
    let timeout_read = time::Duration::from_millis(config.read_timeout());
    let timeout_write = time::Duration::from_millis(config.write_timeout());
    let mut last_err = None;

    let addrs = addr.to_socket_addrs().map_err(error::request)?;
    for addr in addrs {
      let domain = Domain::for_address(addr);
      let socket = match Socket::new(domain, Type::STREAM, Some(Protocol::TCP)) {
        Ok(s) => s,
        Err(e) => {
          last_err = Some(e);
          continue;
        }
      };

      if let Err(e) = socket.set_read_timeout(Some(timeout_read)) {
        last_err = Some(e);
        continue;
      }
      if let Err(e) = socket.set_write_timeout(Some(timeout_write)) {
        last_err = Some(e);
        continue;
      }

      if let Err(e) = socket.connect(&addr.into()) {
        last_err = Some(e);
        continue;
      }

      return Ok(TcpStream::from(socket));
    }

    Err(error::request(
      last_err.unwrap_or_else(|| io::Error::other("failed to connect")),
    ))
  }

  async fn async_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    let header = self.conn.header();
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
    if is_chunked_encoded(&binary) {
      binary.extend_from_slice(&async_read_chunked_body(stream).await?);
    } else {
      stream
        .read_to_end(&mut binary)
        .await
        .map_err(error::request)?;
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
    if header.ends_with(HEADER_END) {
      return Ok(header);
    }
  }
}

fn is_chunked_encoded(header: &[u8]) -> bool {
  String::from_utf8_lossy(header).lines().any(|line| {
    let Some((name, value)) = line.split_once(':') else {
      return false;
    };

    name.eq_ignore_ascii_case("Transfer-Encoding")
      && value
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
  })
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
  async fn async_send_https_rustls(
    &self,
    url: &Url,
    stream: TcpStream,
  ) -> error::Result<Vec<u8>> {
    use futures_rustls::TlsConnector;
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, RootCertStore};

    use crate::connection::connection::NoCertificateVerification;

    let config = self.conn.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if config.verify_ssl_cert() {
      builder
        .with_root_certificates(root_store)
        .with_no_client_auth()
    } else {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
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
      ProxyType::HTTP => self.call_with_proxy_https(url, proxy).await,
      ProxyType::HTTPS => self.call_with_proxy_https(url, proxy).await,
      ProxyType::SOCKS4 => self.call_with_proxy_socks4(url, proxy).await,
      ProxyType::SOCKS5 => self.call_with_proxy_socks5(url, proxy).await,
    }
  }

  async fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.async_tcp_stream(&addr).await?;

    stream
      .write_all(connect_header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)?;

    // HTTP/1.1 200 Connection Established
    let mut res = vec![0u8; 1024];
    let bytes = stream.read(&mut res).map_err(error::request)?;

    let res_s = match String::from_utf8(res[..bytes].to_vec()) {
      Ok(r) => r,
      Err(_) => return Err(error::bad_proxy("parse proxy server response error.")),
    };
    if !res_s
      .to_ascii_lowercase()
      .contains("connection established")
    {
      return Err(error::bad_proxy("Proxy server response error."));
    }

    self.async_send_with_stream(url, stream).await
  }

  async fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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
