use url::Url;

use crate::error;
use crate::request::builder::common::{RawBuilder, DISPOSITION_END};
use crate::request::RequestBody;
use crate::types::{Header, RoUrl, ToUrl};

impl<'a> RawBuilder<'a> {
  pub fn build_header(
    &mut self,
    rourl: &RoUrl,
    body: &Option<RequestBody>,
  ) -> error::Result<String> {
    let url = rourl.to_url()?;

    self.auto_add_host(&url)?;
    self.auto_add_connection()?;
    self.auto_add_ua()?;
    self.auto_add_accept()?;
    self.auto_add_content_type()?;
    self.auto_add_content_length(body)?;

    let mut builder = String::new();

    // let is_http = url.scheme() == "http";
    let request_url = self.request_url(&url, false);
    builder.push_str(&format!(
      "{} {} HTTP/1.1{}",
      self.request.method().to_uppercase(),
      request_url,
      DISPOSITION_END
    ));

    for header in self.request.headers() {
      let name = header.name();
      let value = header.value().replace(DISPOSITION_END, "");

      builder.push_str(&format!("{}: {}{}", name, value, DISPOSITION_END));
    }

    builder.push_str(DISPOSITION_END);
    Ok(builder)
  }
}

impl<'a> RawBuilder<'a> {
  fn request_url(&self, url: &Url, full: bool) -> String {
    if full {
      return url.as_str().to_owned();
    }

    let mut result = format!("{}", url.path());
    if let Some(query) = url.query() {
      result.push_str(&format!("?{}", query));
    }
    if let Some(fragment) = url.fragment() {
      result.push_str(&format!("#{}", fragment));
    }
    result
  }

  fn found_header(&mut self, name: impl AsRef<str>) -> bool {
    self
      .request
      .headers()
      .iter()
      .find(|item| item.name().eq_ignore_ascii_case(name.as_ref()))
      .is_some()
  }
}

impl<'a> RawBuilder<'a> {
  fn auto_add_host(&mut self, url: &Url) -> error::Result<()> {
    if self.found_header("host") {
      return Ok(());
    }
    let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
    let header = match url.port() {
      Some(port) => Header::new("Host", format!("{}:{}", host, port)),
      None => Header::new("Host", format!("{}", host)),
    };

    self.request.headers_mut().push(header);
    Ok(())
  }

  fn auto_add_connection(&mut self) -> error::Result<()> {
    if self.found_header("connection") {
      return Ok(());
    }
    self
      .request
      .headers_mut()
      .push(Header::new("Connection", "Close"));
    Ok(())
  }

  fn auto_add_ua(&mut self) -> error::Result<()> {
    if self.found_header("user-agent") {
      return Ok(());
    }
    let ua = format!("Mozilla/5.0 rttp/{}", env!("CARGO_PKG_VERSION"));
    self
      .request
      .headers_mut()
      .push(Header::new("User-Agent", ua));
    Ok(())
  }

  fn auto_add_accept(&mut self) -> error::Result<()> {
    if self.found_header("accept") {
      return Ok(());
    }
    self
      .request
      .headers_mut()
      .push(Header::new("Accept", "*/*"));
    Ok(())
  }

  fn auto_add_content_type(&mut self) -> error::Result<()> {
    // not form-data request
    if self.request.formdatas().is_empty() && !self.found_header("content-type") {
      let header = match &self.content_type {
        Some(ct) => Header::new("Content-Type", ct.to_string()),
        None => Header::new(
          "Content-Type",
          mime::APPLICATION_WWW_FORM_URLENCODED.to_string(),
        ),
      };

      self.request.headers_mut().push(header);
      return Ok(());
    }

    // if it's form data request, replace header use generate header
    let mut headers = self.request.headers().clone();
    let origin = headers
      .iter()
      .find(|item| item.name().eq_ignore_ascii_case("content-type"))
      .cloned();

    headers.retain(|item| !item.name().eq_ignore_ascii_case("content-type"));

    let header = match &self.content_type {
      Some(ct) => Header::new("Content-Type", ct.to_string()),
      None => origin
        .unwrap_or_else(|| Header::new("Content-Type", mime::APPLICATION_OCTET_STREAM.to_string())),
    };
    headers.push(header);

    self.request.headers_set(headers);
    Ok(())
  }

  fn auto_add_content_length(&mut self, body: &Option<RequestBody>) -> error::Result<()> {
    if self.found_header("content-length") {
      return Ok(());
    }
    let len = if let Some(body) = body { body.len() } else { 0 };
    if len < 1 {
      return Ok(());
    }

    let mut headers = self.request.headers().clone();
    headers.retain(|item| !item.name().eq_ignore_ascii_case("content-length"));
    headers.push(Header::new("Content-Length", len.to_string()));
    self.request.headers_set(headers);
    Ok(())
  }
}
