use mime::Mime;

use crate::error;
use crate::request::{RawRequest, Request};

pub static HYPHENS: &str = "---------------------------";
pub static DISPOSITION_PREFIX: &str = "--";
pub static DISPOSITION_END: &str = "\r\n";

#[derive(Debug)]
pub struct RawBuilder<'a> {
  pub content_type: Option<Mime>,
  pub request: &'a mut Request,
}

impl<'a> RawBuilder<'a> {
  pub fn new(request: &'a mut Request) -> Self {
    Self {
      content_type: None,
      request,
    }
  }
}

// impl<'a> RawBuilder<'a> {
//   pub fn request(&mut self) -> &mut Request {
//     self.request
//   }
//
//   pub fn content_type(&self) -> &Option<Mime> {
//     &self.content_type
//   }
//
//   pub fn content_type_set(&mut self, content_type: Mime) -> &mut Self {
//     self.content_type = Some(content_type);
//     self
//   }
// }

impl<'a> RawBuilder<'a> {
  pub fn raw_request_block(mut self) -> error::Result<RawRequest<'a>> {
    let mut rourl = self.request.url().clone().ok_or(error::none_url())?;
    if rourl.traditional_get().is_none() {
      rourl.traditional(self.request.traditional());
    }

    self.rebuild_paras(&mut rourl);
    self.rebuild_url(&mut rourl);
    let body = self.build_body_block(&mut rourl)?;
    let header = self.build_header(&rourl, &body)?;
    Ok(RawRequest {
      origin: self.request,
      url: rourl,
      header,
      body,
    })
  }

  #[cfg(feature = "async")]
  pub async fn raw_request_async_std(mut self) -> error::Result<RawRequest<'a>> {
    let mut rourl = self.request.url().clone().ok_or(error::none_url())?;
    if rourl.traditional_get().is_none() {
      rourl.traditional(self.request.traditional());
    }

    self.rebuild_paras(&mut rourl);
    self.rebuild_url(&mut rourl);
    let body = self.build_body_async(&mut rourl).await?;
    let header = self.build_header(&rourl, &body)?;
    Ok(RawRequest {
      origin: self.request,
      url: rourl,
      header,
      body,
    })
  }
}
