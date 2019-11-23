use std::{io, time};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

#[cfg(feature = "tls-native")]
use native_tls::TlsConnector;
use socks::{Socks4Stream, Socks5Stream};
use url::Url;

use crate::{error, HttpClient};
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType, ToUrl};
#[cfg(feature = "tls-rustls")]
use rustls::{Session, TLSError};

pub struct Connection {
  request: RawRequest
}

impl Connection {
  pub fn new(request: RawRequest) -> Self {
    Self { request }
  }

  pub fn call(&self) -> error::Result<Response> {
    let url = self.request.url().to_url().map_err(error::builder)?;

    let header = self.request.header();
    let body = self.request.body();
//    println!("{}", header);
//    if let Some(b) = body {
//      println!("{}", b.string()?);
//    }

    let proxy = self.request.origin().proxy();

    let binary = if let Some(proxy) = proxy {
      self.call_with_proxy(&url, proxy)?
    } else {
      self.send(&url)?
    };

    let config = self.request.origin().config();
    let response = Response::new(self.request.url().clone(), binary)?;

    if let Some(location) = response.location() {
      let req_url = url.as_str();
      if req_url == location {
        return Err(error::loop_detected(url));
      }
      if !config.auto_redirect() {
        return Ok(response);
      }
      let count = self.request.origin().count();
      if count > config.max_redirect() {
        return Err(error::too_many_redirects(url));
      }

      return HttpClient::with_request(self.request.origin().clone())
        .url(location)
        .count(count + 1)
        .emit();
    }

    Ok(response)
  }

  fn addr(&self, url: &Url) -> error::Result<String> {
    let host = self.host(url)?;
    let port = self.port(url)?;
    Ok(format!("{}:{}", host, port))
  }

  fn host(&self, url: &Url) -> error::Result<String> {
    Ok(url.host_str().ok_or(error::url_bad_host(url.clone()))?.to_string())
  }

  fn port(&self, url: &Url) -> error::Result<u16> {
    url.port_or_known_default().ok_or(error::url_bad_host(url.clone()))
  }

  fn tcp_stream(&self, addr: &String) -> error::Result<TcpStream> {
    let config = self.request.origin().config();
    let stream = TcpStream::connect(addr).map_err(error::request)?;
    stream.set_read_timeout(Some(time::Duration::from_millis(config.read_timeout()))).map_err(error::request)?;
    stream.set_write_timeout(Some(time::Duration::from_millis(config.write_timeout()))).map_err(error::request)?;
    Ok(stream)
  }
}


impl Connection {
  fn send(&self, url: &Url) -> error::Result<Vec<u8>> {
    let header = self.request.header();
    let body = self.request.body();

    let addr = self.addr(url)?;
    let mut stream = self.tcp_stream(&addr)?;
//    self.call_tcp_stream_http(stream)
    self.send_with_stream(url, stream)
  }

  fn send_with_stream<S>(&self, url: &Url, mut stream: S) -> error::Result<Vec<u8>>
    where
      S: 'static + io::Read + io::Write + std::marker::Sync + std::marker::Send + std::fmt::Debug,
  {
    match url.scheme() {
      "http" => self.send_http(stream),
      "https" => self.send_https(url, stream),
      _ => return Err(error::url_bad_scheme(url.clone()))
    }
  }

  fn send_http<S>(&self, mut stream: S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    let header = self.request.header();
    let body = self.request.body();
    stream.write(header.as_bytes()).map_err(error::request)?;
    if let Some(body) = body {
      stream.write(body.bytes()).map_err(error::request)?;
    }
    stream.flush().map_err(error::request)?;
    let mut binary: Vec<u8> = Vec::new();
    stream.read_to_end(&mut binary).map_err(error::request)?;
    Ok(binary)
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  fn send_https<S>(&self, url: &Url, mut stream: S) -> error::Result<Vec<u8>>
    where
      S: 'static + io::Read + io::Write + std::marker::Sync + std::marker::Send + std::fmt::Debug,
  {
    return Err(error::no_request_features("Not have any tls features, Can't request a https url"));
  }

  #[cfg(feature = "tls-native")]
  fn send_https<S>(&self, url: &Url, mut stream: S) -> error::Result<Vec<u8>>
    where
      S: 'static + io::Read + io::Write + std::marker::Sync + std::marker::Send + std::fmt::Debug,
  {
    let header = self.request.header();
    let body = self.request.body();

    let connector = TlsConnector::builder().build().map_err(error::request)?;
    let mut ssl_stream;
//  if self.verify {
    ssl_stream = connector.connect(&self.host(url)?[..], stream).map_err(error::request)?;
//  } else {
//    ssl_stream = connector.danger_connect_without_providing_domain_for_certificate_verification_and_server_name_indication(stream).map_err(error::request)?;
//  }

    ssl_stream.write(header.as_bytes()).map_err(error::request)?;
    if let Some(body) = body {
      ssl_stream.write(body.bytes()).map_err(error::request)?;
    }
    ssl_stream.flush().map_err(error::request)?;

    let mut binary: Vec<u8> = Vec::new();
    ssl_stream.read_to_end(&mut binary).map_err(error::request)?;
    Ok(binary)
  }

  #[cfg(feature = "tls-rustls")]
  fn send_https<S>(&self, url: &Url, mut stream: S) -> error::Result<Vec<u8>>
    where
      S: 'static + io::Read + io::Write + std::marker::Sync + std::marker::Send + std::fmt::Debug,
  {
    let mut config = rustls::ClientConfig::new();
    config
      .root_store
      .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    let rc_config = Arc::new(config);
    let host = self.host(url)?;
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(&host[..]).unwrap();
    let mut client = rustls::ClientSession::new(&rc_config, dns_name);
    let mut tls = rustls::Stream::new(&mut client, &mut stream);

    let header = self.request.header();
    let body = self.request.body();

    tls.write(header.as_bytes()).map_err(error::request)?;
    if let Some(body) = body {
      tls.write(body.bytes()).map_err(error::request)?;
    }
    tls.flush().map_err(error::request)?;

    let mut binary: Vec<u8> = Vec::new();
    tls.read_to_end(&mut binary).map_err(error::request)?;
    Ok(binary)
  }
}

// proxy connection
impl Connection {
  fn call_with_proxy(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    match proxy.type_() {
      ProxyType::HTTP => self.call_with_proxy_https(url, proxy),
      ProxyType::HTTPS => self.call_with_proxy_https(url, proxy),
      ProxyType::SOCKS4 => self.call_with_proxy_socks4(url, proxy),
      ProxyType::SOCKS5 => self.call_with_proxy_socks5(url, proxy),
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

  fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let header = self.request.header();
    let body = self.request.body();

    let host = self.host(url)?;
    let port = self.port(url)?;

    //CONNECT proxy.google.com:443 HTTP/1.1
    //Host: www.google.com:443
    //Proxy-Connection: keep-alive
    let mut connect_header = String::new();
    connect_header.push_str(&format!("CONNECT {}:{} HTTP/1.1\r\n", host, port));
    connect_header.push_str(&format!("Host: {}:{}\r\n", host, port));

    if let Some(username) = proxy.username() {
      let auth = if let Some(password) = proxy.password() {
        format!("{}:{}", username, password)
      } else {
        format!("{}:", username)
      };
      let auth = base64::encode(&auth);
      connect_header.push_str(&format!("Authorization: Basic {}\r\n", auth));
    }

    connect_header.push_str("\r\n");

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.tcp_stream(&addr)?;

    stream.write(connect_header.as_bytes()).map_err(error::request)?;
    stream.flush().map_err(error::request)?;

    //HTTP/1.1 200 Connection Established
    let mut res = [0u8; 1024];
    stream.read(&mut res).map_err(error::request)?;

    let res_s = match String::from_utf8(res.to_vec()) {
      Ok(r) => r,
      Err(_) => return Err(error::bad_proxy("parse proxy server response error."))
    };
    if !res_s.to_ascii_lowercase().contains("connection established") {
      return Err(error::bad_proxy("Proxy server response error."));
    }

    self.send_with_stream(url, stream)
  }

  fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.addr(url)?;
    let user = if let Some(u) = proxy.username() { u.to_string() } else { "".to_string() };
    let mut stream = Socks4Stream::connect(&addr_proxy[..], &addr_target[..], &user[..])
      .map_err(error::request)?;
    self.send_with_stream(url, stream)
  }

  fn call_with_proxy_socks5(&self, url: &Url, proxy: &Proxy) -> error::Result<Vec<u8>> {
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.addr(url)?;
    let mut stream = if let Some(u) = proxy.username() {
      if let Some(p) = proxy.password() {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], &p[..])
      } else {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], "")
      }
    } else {
      Socks5Stream::connect(&addr_proxy[..], &addr_target[..])
    }.map_err(error::request)?;
    self.send_with_stream(url, stream)
  }
}


