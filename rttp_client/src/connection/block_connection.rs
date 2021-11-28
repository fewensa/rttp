use std::io::{Read, Write};

#[cfg(feature = "tls-native")]
use native_tls::TlsConnector;
#[cfg(feature = "tls-rustls")]
use rustls::{Session, TLSError};
use socks::{Socks4Stream, Socks5Stream};
use url::Url;

use crate::connection::connection::Connection;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType};
use crate::{error, HttpClient};

pub struct BlockConnection<'a> {
  conn: Connection<'a>,
}

impl<'a> BlockConnection<'a> {
  pub fn new(request: RawRequest<'a>) -> Self {
    Self {
      conn: Connection::new(request),
    }
  }

  pub fn call(mut self) -> error::Result<Response> {
    let url = self.conn.url().map_err(error::builder)?;
    let proxy = self.conn.proxy();
    let binary = if let Some(proxy) = proxy {
      self.call_with_proxy(&url, proxy)?
    } else {
      self.conn.block_send(&url)?
    };

    let config = self.conn.config();
    let response = Response::new(self.conn.rourl().clone(), binary)?;

    if let Some(location) = response.location() {
      let req_url = url.as_str();
      if req_url == location {
        return Err(error::loop_detected(url));
      }
      if !config.auto_redirect() {
        return Ok(response);
      }
      let count = self.conn.count();
      if count > config.max_redirect() {
        return Err(error::too_many_redirects(url));
      }

      return HttpClient::with_request(self.conn.request().origin().clone())
        .url(location)
        .count(count + 1)
        .emit();
    }

    self.conn.closed_set(true);
    Ok(response)
  }
}

// proxy connection
impl<'a> BlockConnection<'a> {
  fn call_with_proxy(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    match proxy.type_() {
      ProxyType::HTTP | ProxyType::HTTPS => self.call_with_proxy_https(url, proxy),
      ProxyType::SOCKS4 => self.call_with_proxy_socks4(url, proxy),
      ProxyType::SOCKS5 => self.call_with_proxy_socks5(url, proxy),
    }
  }

  fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    //CONNECT proxy.google.com:443 HTTP/1.1
    //Host: www.google.com:443
    //Proxy-Connection: keep-alive
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.conn.block_tcp_stream(&addr)?;

    stream
      .write(connect_header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)?;

    //HTTP/1.1 200 Connection Established
    let mut res = [0u8; 1024];
    stream.read(&mut res).map_err(error::request)?;

    let res_s = String::from_utf8(res.to_vec())
      .map_err(|_| error::bad_proxy("parse proxy server response error."))?;
    if !res_s
      .to_ascii_lowercase()
      .contains("connection established")
    {
      return Err(error::bad_proxy(format!(
        "Proxy server response error: {}",
        res_s
      )));
    }

    self.conn.block_send_with_stream(url, &mut stream)
  }

  fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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

  fn call_with_proxy_socks5(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
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
