use url::Url;

use crate::error;
use crate::request::Request;
use crate::types::ToUrl;

#[derive(Clone, Debug)]
pub struct Connection<'a> {
  request: &'a Request
}

impl<'a> Connection<'a> {
  pub fn new(request: &'a Request) -> Self {
    Self { request }
  }

  pub fn call(self) -> error::Result<()> {
    let url = self.request.url()
      .clone()
      .ok_or(error::none_url())?
      .to_url()?;
    match url.scheme() {
      "http" => self.call_http(&url),
      "https" => self.call_https(&url),
      _ => Err(error::url_bad_scheme(url))
    }
  }
}

impl<'a> Connection<'a> {
  fn call_http(&self, url: &Url) -> error::Result<()> {
//    println!("http {}", url);
    let header = self.build_header(url)?;
    println!("{}", header);
    Ok(())
  }
}

impl<'a> Connection<'a> {
  fn call_https(&self, url: &Url) -> error::Result<()> {
    println!("https");
    Ok(())
  }
}


impl<'a> Connection<'a> {
  fn request_url(&self, url: &Url, full: bool) -> String {
    if full {
      url.as_str().to_owned()
    } else {
      let mut result = format!("{}", url.path());
      if let Some(query) = url.query() {
        result.push_str(&format!("?{}", query));
      }
      if let Some(fragment) = url.fragment() {
        result.push_str(&format!("#{}", fragment));
      }
      result
    }
  }

  fn build_header(&self, url: &Url) -> error::Result<String> {
    let mut builder = String::new();

    let request_url = self.request_url(url, true);
    let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
    let port: u16 = url.port().map_or_else(|| {
      match url.scheme() {
        "https" => 443,
        _ => 80
      }
    }, |v| v);

    builder.push_str(&format!("{} {} HTTP/1.1\r\n", self.request.method().to_uppercase(), request_url));

    let mut found_host = false;
    let mut found_connection = false;
    for header in self.request.headers() {
      let name = header.name();
      let value = header.value().replace("\r\n", "");
      if name.eq_ignore_ascii_case("host") {
        found_host = true;
      }
      if name.eq_ignore_ascii_case("connection") {
        found_connection = true;
      }
      builder.push_str(&format!("{}: {}\r\n", name, value))
    }
    if !found_host {
      builder.push_str(&format!("Host: {}:{}\r\n", host, port));
    }
    if !found_connection {
      builder.push_str(&format!("Connection: Close\r\n"));
    }

    builder.push_str("\r\n");
    Ok(builder)
  }
}

