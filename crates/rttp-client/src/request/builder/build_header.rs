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
    self.auto_add_content_type(body)?;
    self.auto_add_content_length(body)?;

    let mut builder = String::new();

    // let is_http = url.scheme() == "http";
    let request_url = self.request_url(&url, false)?;
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
  fn request_url(&self, url: &Url, full: bool) -> error::Result<String> {
    if full {
      return Ok(url.as_str().to_owned());
    }

    if self.request.method().eq_ignore_ascii_case("connect") {
      return connect_authority(url);
    }

    let mut result = url.path().to_string();
    if let Some(query) = url.query() {
      result.push_str(&format!("?{}", query));
    }
    Ok(result)
  }

  fn found_header(&mut self, name: impl AsRef<str>) -> bool {
    self
      .request
      .headers()
      .iter()
      .any(|item| item.name().eq_ignore_ascii_case(name.as_ref()))
  }
}

fn connect_authority(url: &Url) -> error::Result<String> {
  let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
  let port = url
    .port_or_known_default()
    .ok_or(error::url_bad_host(url.clone()))?;

  Ok(format!("{}:{}", format_host_for_authority(host), port))
}

fn format_host_for_authority(host: &str) -> String {
  if host.contains(':') && !host.starts_with('[') {
    format!("[{}]", host)
  } else {
    host.to_string()
  }
}

impl<'a> RawBuilder<'a> {
  fn auto_add_host(&mut self, url: &Url) -> error::Result<()> {
    let host = expected_host_header(url, self.request.method())?;
    if let Some(header) = self
      .request
      .headers()
      .iter()
      .find(|item| item.name().eq_ignore_ascii_case("host"))
    {
      if header.value().eq_ignore_ascii_case(host.value()) {
        return Ok(());
      }
      return Err(error::bad_url(
        url.clone(),
        format!(
          "Host header '{}' conflicts with URL authority '{}'",
          header.value(),
          host.value()
        ),
      ));
    }

    self.request.headers_mut().push(host);
    Ok(())
  }

  fn auto_add_connection(&mut self) -> error::Result<()> {
    let declares_te = self.found_header("te");
    if let Some(header) = self
      .request
      .headers_mut()
      .iter_mut()
      .find(|header| header.name().eq_ignore_ascii_case("connection"))
    {
      if declares_te
        && !header
          .value()
          .split(',')
          .any(|token| token.trim().eq_ignore_ascii_case("te"))
      {
        header.replace(Header::new("Connection", format!("{}, TE", header.value())));
      }
      return Ok(());
    }
    self.request.headers_mut().push(Header::new(
      "Connection",
      if declares_te { "Close, TE" } else { "Close" },
    ));
    Ok(())
  }
}

fn expected_host_header(url: &Url, method: &str) -> error::Result<Header> {
  let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
  let host = format_host_for_authority(host);
  Ok(match (method.eq_ignore_ascii_case("connect"), url.port()) {
    (true, Some(port)) => Header::new("Host", format!("{}:{}", host, port)),
    (true, None) => Header::new(
      "Host",
      format!(
        "{}:{}",
        host,
        url
          .port_or_known_default()
          .ok_or(error::url_bad_host(url.clone()))?
      ),
    ),
    (false, Some(port)) => Header::new("Host", format!("{}:{}", host, port)),
    (false, None) => Header::new("Host", host),
  })
}

impl<'a> RawBuilder<'a> {
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

  fn auto_add_content_type(&mut self, body: &Option<RequestBody>) -> error::Result<()> {
    let is_form_data = !self.request.formdatas().is_empty();
    let has_content_type = self.found_header("content-type");

    if !is_form_data {
      if has_content_type || body.is_none() {
        return Ok(());
      }

      let content_type = self
        .content_type
        .clone()
        .unwrap_or(mime::APPLICATION_WWW_FORM_URLENCODED);
      self
        .request
        .headers_mut()
        .push(Header::new("Content-Type", content_type));
      return Ok(());
    }

    // if it's form data request, replace header with generated multipart header
    let mut headers = self.request.headers().clone();
    let origin = headers
      .iter()
      .find(|item| item.name().eq_ignore_ascii_case("content-type"))
      .cloned();

    headers.retain(|item| !item.name().eq_ignore_ascii_case("content-type"));

    let header = match &self.content_type {
      Some(ct) => Header::new("Content-Type", ct),
      None => {
        origin.unwrap_or_else(|| Header::new("Content-Type", &mime::APPLICATION_OCTET_STREAM))
      }
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
