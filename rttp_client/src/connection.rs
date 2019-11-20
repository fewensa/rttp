use std::io::{Read, Write};
use std::net::TcpStream;
use std::time;

use native_tls::TlsConnector;

use crate::error;
use crate::request::RawRequest;
use crate::types::ToUrl;
use url::Url;

pub struct Connection {
  request: RawRequest
}

impl Connection {
  pub fn new(request: RawRequest) -> Self {
    Self { request }
  }

  pub fn call(&self) -> error::Result<()> {
    let url = self.request.url().to_url().map_err(error::builder)?;

    let header = self.request.header();
    let body = self.request.body();
    println!("{}", header);
    if let Some(b) = body {
      println!("{}", b.string()?);
    }

    let binary = match url.scheme() {
      "http" => self.call_http(&url)?,
      "https" => self.call_https(&url)?,
      _ => return Err(error::url_bad_scheme(url.clone()))
    };

    let st = String::from_utf8_lossy(binary.as_slice());
    println!("{}", st);

    Ok(())
  }

  fn addr(&self, url: &Url) -> error::Result<String> {
    let host = self.host(url)?;
    let port = url.port_or_known_default().ok_or(error::url_bad_host(url.clone()))?;
    Ok(format!("{}:{}", host, port))
  }

  fn host(&self, url: &Url) -> error::Result<String> {
    Ok(url.host_str().ok_or(error::url_bad_host(url.clone()))?.to_string())
  }
}


impl Connection {
  fn call_http(&self, url: &Url) -> error::Result<Vec<u8>> {
    let header = self.request.header();
    let body = self.request.body();

    let addr= self.addr(url)?;

    let mut stream = TcpStream::connect(addr).map_err(error::request)?;
    stream.set_read_timeout(Some(time::Duration::from_secs(5000))).map_err(error::request)?;
    stream.set_write_timeout(Some(time::Duration::from_secs(5000))).map_err(error::request)?;

    stream.write(header.as_bytes()).map_err(error::request)?;

    if let Some(body) = body {
      stream.write(body.bytes()).map_err(error::request)?;
    }
    stream.flush().map_err(error::request)?;

    let mut binary :Vec<u8>= Vec::new();
    stream.read_to_end(&mut binary).map_err(error::request)?;
    Ok(binary)
  }
}

impl Connection {
  fn call_https(&self, url: &Url) -> error::Result<Vec<u8>> {
    let header = self.request.header();
    let body = self.request.body();

    let addr= self.addr(url)?;

    let stream = TcpStream::connect(addr).map_err(error::request)?;
    stream.set_read_timeout(Some(time::Duration::from_secs(5000))).map_err(error::request)?;
    stream.set_write_timeout(Some(time::Duration::from_secs(5000))).map_err(error::request)?;

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

    let mut binary :Vec<u8>= Vec::new();
    ssl_stream.read_to_end(&mut binary).map_err(error::request)?;
    Ok(binary)
  }
}
