use std::{io, time};
use std::sync::Arc;

use url::Url;

use crate::{Config, error};
use crate::connection::connection_reader::ConnectionReader;
use crate::request::{RawRequest, RequestBody};
use crate::types::{Proxy, RoUrl, ToUrl};

pub struct Connection<'a> {
  request: RawRequest<'a>
}

impl<'a> Connection<'a> {
  pub fn new(request: RawRequest<'a>) -> Connection<'a> {
    Self { request }
  }
}

impl<'a> Connection<'a> {
  pub fn request(&self) -> &RawRequest {
    &self.request
  }
  pub fn rourl(&self) -> &RoUrl {
    self.request.url()
  }
  pub fn url(&self) -> error::Result<Url> {
    self.request.url().to_url().map_err(error::builder)
  }
  pub fn header(&self) -> &String {
    self.request.header()
  }
  pub fn body(&self) -> &Option<RequestBody> {
    self.request.body()
  }
  pub fn proxy(&self) -> &Option<Proxy> {
    self.request.origin().proxy()
  }
  pub fn config(&self) -> &Config {
    self.request.origin().config()
  }
  pub fn count(&self) -> u32 {
    self.request.origin().count()
  }

  pub fn closed_set(&mut self, closed: bool) {
    self.request.origin_mut().closed_set(closed);
  }
}

impl<'a> Connection<'a> {
  pub fn addr(&self, url: &Url) -> error::Result<String> {
    let host = self.host(url)?;
    let port = self.port(url)?;
    Ok(format!("{}:{}", host, port))
  }

  pub fn host(&self, url: &Url) -> error::Result<String> {
    Ok(url.host_str().ok_or(error::url_bad_host(url.clone()))?.to_string())
  }

  pub fn port(&self, url: &Url) -> error::Result<u16> {
    url.port_or_known_default().ok_or(error::url_bad_host(url.clone()))
  }

  pub fn proxy_header(&self, url: &Url, proxy: &Proxy) -> error::Result<String> {
    let host = self.host(url)?;
    let port = self.port(url)?;

    //CONNECT proxy.google.com:443 HTTP/1.1
    //Host: www.google.com:443
    //Proxy-Connection: keep-alive
    let mut proxy_header = String::new();
    proxy_header.push_str(&format!("CONNECT {}:{} HTTP/1.1\r\n", host, port));
    proxy_header.push_str(&format!("Host: {}:{}\r\n", host, port));

    if let Some(username) = proxy.username() {
      let auth = if let Some(password) = proxy.password() {
        format!("{}:{}", username, password)
      } else {
        format!("{}:", username)
      };
      let auth = base64::encode(&auth);
      proxy_header.push_str(&format!("Authorization: Basic {}\r\n", auth));
    }

    proxy_header.push_str("\r\n");
    Ok(proxy_header)
  }
}

impl<'a> Connection<'a> {
  pub fn block_tcp_stream(&self, addr: &String) -> error::Result<std::net::TcpStream> {
    let config = self.config();
    let stream = std::net::TcpStream::connect(addr).map_err(error::request)?;
    stream.set_read_timeout(Some(time::Duration::from_millis(config.read_timeout()))).map_err(error::request)?;
    stream.set_write_timeout(Some(time::Duration::from_millis(config.write_timeout()))).map_err(error::request)?;
    Ok(stream)
  }

  pub fn block_write_stream<S>(&self, stream: &mut S) -> error::Result<()> where S: io::Write, {
    let header = self.header();
    let body = self.body();
    stream.write(header.as_bytes()).map_err(error::request)?;
    if let Some(body) = body {
      stream.write(body.bytes()).map_err(error::request)?;
    }
    stream.flush().map_err(error::request)?;

    Ok(())
  }

  pub fn block_read_stream<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>> where S: io::Read, {
    let mut reader = ConnectionReader::new(url, stream);
    reader.binary()
  }

  pub fn block_send(&self, url: &Url) -> error::Result<Vec<u8>> {
    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
//    self.call_tcp_stream_http(stream)
    self.block_send_with_stream(url, &mut stream)
  }

  pub fn block_send_with_stream<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    match url.scheme() {
      "http" => self.block_send_http(url, stream),
      "https" => self.block_send_https(url, stream),
      _ => return Err(error::url_bad_scheme(url.clone()))
    }
  }

  pub fn block_send_http<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    self.block_write_stream(stream)?;
    self.block_read_stream(url, stream)
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  pub fn block_send_https<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    return Err(error::no_request_features("Not have any tls features, Can't request a https url"));
  }

  #[cfg(feature = "tls-native")]
  pub fn block_send_https<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    let connector = native_tls::TlsConnector::builder().build().map_err(error::request)?;
    let mut ssl_stream;
//  if self.verify {
    ssl_stream = connector.connect(&self.host(url)?[..], stream)
      .map_err(|_| error::bad_ssl("Native tls error."))?;
//    ssl_stream = connector.connect(&self.host(url)?[..], stream).map_err(error::request)?;
//  } else {
//    ssl_stream = connector.danger_connect_without_providing_domain_for_certificate_verification_and_server_name_indication(stream).map_err(error::request)?;
//  }

    self.block_write_stream(&mut ssl_stream)?;
    self.block_read_stream(url, &mut ssl_stream)
  }

  #[cfg(feature = "tls-rustls")]
  pub fn block_send_https<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
    where
      S: io::Read + io::Write,
  {
    let mut config = rustls::ClientConfig::new();
    config
      .root_store
      .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    let rc_config = Arc::new(config);
    let host = self.host(url)?;
    let dns_name = webpki::DNSNameRef::try_from_ascii_str(&host[..]).unwrap();
    let mut client = rustls::ClientSession::new(&rc_config, dns_name);
    let mut tls = rustls::Stream::new(&mut client, stream);

    self.block_write_stream(&mut tls)?;
    self.block_read_stream(url, &mut tls)
  }
}



