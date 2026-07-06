use crate::error;
use crate::request::builder::RawBuilder;
use crate::request::{Request, RequestBody};
#[cfg(feature = "async")]
use crate::types::Header;
use crate::types::RoUrl;
#[cfg(feature = "async")]
use crate::types::{ToRoUrl, ToUrl};

#[derive(Debug)]
pub struct RawRequest<'a> {
  pub(crate) origin: &'a mut Request,
  pub(crate) url: RoUrl,
  pub(crate) header: String,
  pub(crate) body: Option<RequestBody>,
}

impl<'a> RawRequest<'a> {
  pub fn block_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).raw_request_block()
  }

  #[cfg(feature = "async")]
  pub async fn async_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).raw_request_async().await
  }

  pub fn origin(&self) -> &Request {
    self.origin
  }

  pub fn url(&self) -> &RoUrl {
    &self.url
  }

  pub fn header(&self) -> &String {
    &self.header
  }

  pub fn body(&self) -> &Option<RequestBody> {
    &self.body
  }

  pub fn content_type(&self) -> Option<String> {
    self.origin.header("content-type")
  }

  pub(crate) fn origin_mut(&mut self) -> &mut Request {
    self.origin
  }

  #[cfg(feature = "async")]
  pub(crate) fn redirect_status_set(&mut self, status_code: u32) {
    self.origin.redirect_status_set(status_code);
    if status_code == 303 && !self.origin.method().eq_ignore_ascii_case("head") {
      self.body = None;
      self.header = Self::redirect_body_headers_remove(&self.header);
    }
  }

  #[cfg(feature = "async")]
  pub(crate) fn redirect_url_set<S: ToRoUrl>(&mut self, rourl: S) -> error::Result<()> {
    let rourl = rourl.to_rourl();
    let url = rourl.to_url()?;
    let host_header = Self::redirect_host_header(&url)?;
    let mut request_target = url.path().to_string();
    if let Some(query) = url.query() {
      request_target.push_str(&format!("?{}", query));
    }

    self.origin.url_set(&rourl);
    self.redirect_host_set(host_header.clone());
    self.url = rourl;

    if let Some((_, rest)) = self.header.split_once("\r\n") {
      let rest = Self::redirect_header_host_set(rest, &host_header);
      self.header = format!(
        "{} {} HTTP/1.1\r\n{}",
        self.origin.method().to_uppercase(),
        request_target,
        rest
      );
    }

    Ok(())
  }

  #[cfg(feature = "async")]
  fn redirect_host_header(url: &url::Url) -> error::Result<Header> {
    let host = url.host_str().ok_or(error::url_bad_host(url.clone()))?;
    Ok(match url.port() {
      Some(port) => Header::new("Host", format!("{}:{}", host, port)),
      None => Header::new("Host", host),
    })
  }

  #[cfg(feature = "async")]
  fn redirect_host_set(&mut self, header: Header) {
    if let Some(origin_header) = self
      .origin
      .headers_mut()
      .iter_mut()
      .find(|item| item.name().eq_ignore_ascii_case("host"))
    {
      origin_header.replace(header);
    } else {
      self.origin.headers_mut().push(header);
    }
  }

  #[cfg(feature = "async")]
  fn redirect_header_host_set(rest: &str, header: &Header) -> String {
    let mut rewritten = String::new();
    let mut replaced = false;

    for line in rest.split_inclusive("\r\n") {
      let header_name = line
        .trim_end_matches("\r\n")
        .split_once(':')
        .map(|(name, _)| name);

      if header_name.is_some_and(|name| name.eq_ignore_ascii_case("host")) {
        rewritten.push_str(&format!("{}: {}\r\n", header.name(), header.value()));
        replaced = true;
      } else {
        rewritten.push_str(line);
      }
    }

    if replaced {
      rewritten
    } else {
      format!("{}: {}\r\n{}", header.name(), header.value(), rewritten)
    }
  }

  #[cfg(feature = "async")]
  fn redirect_body_headers_remove(header: &str) -> String {
    let Some((request_line, rest)) = header.split_once("\r\n") else {
      return header.to_string();
    };
    let mut rewritten = format!("{}\r\n", request_line);

    for line in rest.split_inclusive("\r\n") {
      let header_name = line
        .trim_end_matches("\r\n")
        .split_once(':')
        .map(|(name, _)| name);

      if header_name.is_some_and(|name| {
        name.eq_ignore_ascii_case("content-length")
          || name.eq_ignore_ascii_case("content-type")
          || name.eq_ignore_ascii_case("transfer-encoding")
      }) {
        continue;
      }

      rewritten.push_str(line);
    }

    rewritten
  }
}
