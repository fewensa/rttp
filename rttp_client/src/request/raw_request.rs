use crate::error;
use crate::request::{Request, RequestBody};
use crate::request::raw_builder::RawBuilder;
use crate::types::RoUrl;

#[derive(Debug)]
pub struct RawRequest<'a> {
  pub(crate) origin: &'a mut Request,
  pub(crate) url: RoUrl,
  pub(crate) header: String,
  pub(crate) body: Option<RequestBody>,
}

impl<'a> RawRequest<'a> {
  pub fn block_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).block_raw_request()
  }

  #[cfg(feature = "async")]
  pub async fn async_new(request: &'a mut Request) -> error::Result<RawRequest<'a>> {
    RawBuilder::new(request).async_raw_request().await
  }

  pub fn origin(&self) -> &Request { &self.origin }
  pub fn url(&self) -> &RoUrl { &self.url }
  pub fn header(&self) -> &String { &self.header }
  pub fn body(&self) -> &Option<RequestBody> { &self.body }

  pub(crate) fn origin_mut(&mut self) -> &mut Request { &mut self.origin }
}


