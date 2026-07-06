use crate::error;
use crate::request::builder::RawBuilder;
use crate::request::{Request, RequestBody};
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
  pub(crate) fn redirect_url_set<S: ToRoUrl>(&mut self, rourl: S) -> error::Result<()> {
    let rourl = rourl.to_rourl();
    let url = rourl.to_url()?;
    let mut request_target = url.path().to_string();
    if let Some(query) = url.query() {
      request_target.push_str(&format!("?{}", query));
    }

    self.origin.url_set(&rourl);
    self.url = rourl;

    if let Some((_, rest)) = self.header.split_once("\r\n") {
      self.header = format!(
        "{} {} HTTP/1.1\r\n{}",
        self.origin.method().to_uppercase(),
        request_target,
        rest
      );
    }

    Ok(())
  }
}
