use std::sync::Arc;

use async_std::prelude::*;
use socks::{Socks4Stream, Socks5Stream};
use url::Url;

use crate::connection::connection::Connection;
use crate::connection::connection_reader::ConnectionReader;
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType, ToUrl};
use crate::connection::async_std_io_block::AsyncToBlockStream;

pub struct AsyncConnection<'a> {
  conn: Connection<'a>
}

impl<'a> AsyncConnection<'a> {
  pub fn new(request: RawRequest<'a>) -> AsyncConnection<'a> {
    Self { conn: Connection::new(request) }
  }

  pub async fn async_call(mut self) -> error::Result<Response> {
    let url = self.conn.url().map_err(error::builder)?;
    let proxy = self.conn.proxy();
    let binary = if let Some(proxy) = proxy {
      self.call_with_proxy(&url, proxy).await?
    } else {
      self.async_send(&url).await?
    };
//    let binary = self.async_send(&url).await?;

    let response = Response::new(self.conn.rourl().clone(), binary)?;
    self.conn.closed_set(true);
    Ok(response)
  }
}

impl<'a> AsyncConnection<'a> {
  async fn async_tcp_stream(&self, addr: &String) -> error::Result<async_std::net::TcpStream> {
//    let async_stream = self.async_tcp_stream(addr)?;
//    Ok(async_std::net::TcpStream::from(async_stream))

    let stream = async_std::net::TcpStream::connect(addr).await.map_err(error::request)?;
    // todo: async_std tcp stream set timeout?
    Ok(stream)
  }

  async fn async_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
    where
      S: async_std::io::Write + std::marker::Unpin,
  {
    let header = self.conn.header();
    let body = self.conn.body();

    stream.write(header.as_bytes()).await.map_err(error::request)?;
    if let Some(body) = body {
      stream.write(body.bytes()).await.map_err(error::request)?;
    }
    stream.flush().await.map_err(error::request)?;

    Ok(())
  }

  async fn async_read_stream<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: async_std::io::Read + std::marker::Unpin,
  {
//    let mut reader = ConnectionReader::new(url, stream);
//    reader.binary()

    let mut buffer = vec![0u8; 1024];
    let _ = stream.read(&mut buffer).await.map_err(error::request)?;
    Ok(buffer)
  }
}

// connection send
impl<'a> AsyncConnection<'a> {
  async fn async_send(&self, url: &Url) -> error::Result<Vec<u8>> {
    let addr = self.conn.addr(url)?;
    let mut stream = self.async_tcp_stream(&addr).await?;

    self.async_send_with_stream(url, stream).await
  }

  async fn async_send_with_stream(&self, url: &Url, mut stream: async_std::net::TcpStream)
                                  -> error::Result<Vec<u8>> {
    match url.scheme() {
      "http" => self.async_send_http(url, stream).await,
      "https" => self.async_send_https(url, stream).await,
      _ => return Err(error::url_bad_scheme(url.clone()))
    }
  }

  async fn async_send_http(&self, url: &Url, mut stream: async_std::net::TcpStream)
                           -> error::Result<Vec<u8>> {
    self.async_write_stream(&mut stream).await?;
    self.async_read_stream(url, &mut stream).await
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  async fn async_send_https(&self, url: &Url, mut stream: async_std::net::TcpStream)
                            -> error::Result<Vec<u8>> {
    return Err(error::no_request_features("Not have any tls features, Can't request a https url"));
  }

  #[cfg(feature = "tls-native")]
  async fn async_send_https(&self, url: &Url, mut stream: async_std::net::TcpStream) -> error::Result<Vec<u8>> {
    let mut stream = AsyncToBlockStream::new(stream);
    let connector = native_tls::TlsConnector::builder().build().map_err(error::request)?;
    let mut ssl_stream;
//  if self.verify {
    ssl_stream = connector.connect(&self.conn.host(url)?[..], stream).map_err(error::request)?;
//  } else {
//    ssl_stream = connector.danger_connect_without_providing_domain_for_certificate_verification_and_server_name_indication(stream).map_err(error::request)?;
//  }


    // fixme: block to async
//    self.async_write_stream(&mut ssl_stream).await?;
//    self.async_read_stream(url, &mut ssl_stream).await
    self.conn.block_write_stream(&mut ssl_stream)?;
    self.conn.block_read_stream(url, &mut ssl_stream)
  }

  #[cfg(feature = "tls-rustls")]
  async fn async_send_https(&self, url: &Url, mut stream: async_std::net::TcpStream)
                            -> error::Result<Vec<u8>> {
    let mut stream = AsyncToBlockStream::new(stream);
    let mut config = rustls::ClientConfig::new();
    config
      .root_store
      .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    let rc_config = Arc::new(config);
    let host = self.host(url)?;
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(&host[..]).unwrap();
    let mut client = rustls::ClientSession::new(&rc_config, dns_name);
    let mut tls = rustls::Stream::new(&mut client, &mut stream);

    // fixme: block to async
//    self.async_write_stream(&mut tls).await?;
//    self.async_read_stream(url, &mut tls).await
    self.conn.block_write_stream(&mut tls)?;
    self.conn.block_read_stream(url, &mut tls)
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

//  fn call_with_proxy_http(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
//    let header = self.request.header();
//    let body = self.request.body();
//
//    let addr = format!("{}:{}", proxy.host(), proxy.port());
//    let mut stream = self.tcp_stream(&addr)?;
//    self.call_tcp_stream_http(stream)
//  }

  async fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.async_tcp_stream(&addr).await?;

    stream.write(connect_header.as_bytes()).await.map_err(error::request)?;
    stream.flush().await.map_err(error::request)?;

    //HTTP/1.1 200 Connection Established
    let mut res = [0u8; 1024];
    stream.read(&mut res).await.map_err(error::request)?;

    let res_s = match String::from_utf8(res.to_vec()) {
      Ok(r) => r,
      Err(_) => return Err(error::bad_proxy("parse proxy server response error."))
    };
    if !res_s.to_ascii_lowercase().contains("connection established") {
      return Err(error::bad_proxy("Proxy server response error."));
    }

    self.async_send_with_stream(url, stream).await
  }

  async fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.conn.addr(url)?;
    let user = if let Some(u) = proxy.username() { u.to_string() } else { "".to_string() };
    let mut stream = Socks4Stream::connect(&addr_proxy[..], &addr_target[..], &user[..])
      .map_err(error::request)?;
    // fixme: block to async
//    let mut stream = BlockToAsyncStream::new(&mut stream);
//    self.async_send_with_stream(url, &mut stream).await
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
    }.map_err(error::request)?;
    // fixme: block to async
//    let mut stream = BlockToAsyncStream::new(&mut stream);
//    self.async_send_with_stream(url, &mut stream).await
    self.conn.block_send_with_stream(url, &mut stream)
  }
}

