use std::net::TcpStream;

use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, AllowStdIo};
use std::io::{Read, Write};
use socks::{Socks4Stream, Socks5Stream};
use url::Url;

use crate::connection::connection::Connection;
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType, ToUrl};

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
  async fn async_tcp_stream(&self, addr: &String) -> error::Result<TcpStream> {
    self.conn.block_tcp_stream(addr)
  }

  async fn async_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: AsyncWrite + Unpin,
  {
    let header = self.conn.header();
    let body = self.conn.body();

    stream.write_all(header.as_bytes()).await.map_err(error::request)?;
    if let Some(body) = body {
      stream.write_all(body.bytes()).await.map_err(error::request)?;
    }
    stream.flush().await.map_err(error::request)?;

    Ok(())
  }

  async fn async_read_stream<S>(&self, _url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: AsyncRead + Unpin,
  {
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await.map_err(error::request)?;
    Ok(buffer)
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

  async fn async_send_https(&self, url: &Url, mut stream: TcpStream) -> error::Result<Vec<u8>> {
    #[cfg(any(feature = "tls-native", feature = "tls-rustls"))]
    {
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

    stream.write_all(connect_header.as_bytes()).map_err(error::request)?;
    stream.flush().map_err(error::request)?;

    // HTTP/1.1 200 Connection Established
    let mut res = vec![0u8; 1024];
    let bytes = stream.read(&mut res).map_err(error::request)?;

    let res_s = match String::from_utf8(res[..bytes].to_vec()) {
      Ok(r) => r,
      Err(_) => return Err(error::bad_proxy("parse proxy server response error.")),
    };
    if !res_s.to_ascii_lowercase().contains("connection established") {
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
