use mime::Mime;

use crate::error;
use crate::request::{Request, RequestBody};
use crate::types::RoUrl;

pub struct AsyncRawRequest<'a> {
  origin: &'a mut Request,

  url: RoUrl,
  header: String,
  body: Option<RequestBody>,
}


impl<'a> AsyncRawRequest<'a> {
  pub fn new(request: &'a mut Request) -> error::Result<Self> {
    AsyncStandardization::new(request).standard()
  }

  pub fn origin(&self) -> &Request { &self.origin }
  pub fn url(&self) -> &RoUrl { &self.url }
  pub fn header(&self) -> &String { &self.header }
  pub fn body(&self) -> &Option<RequestBody> { &self.body }
}

pub struct AsyncStandardization<'a> {
  content_type: Option<Mime>,
  request: &'a mut Request,
}

impl<'a> AsyncStandardization<'a> {
  pub fn new(request: &'a mut Request) -> Self {
    Self {
      content_type: None,
      request,
    }
  }

  pub fn standard(mut self) -> error::Result<AsyncRawRequest<'a>> {
    let mut rourl = self.request.url()
      .clone()
      .ok_or(error::none_url())?;

    let header = "".to_string();
    let body = None;
    Ok(AsyncRawRequest {
      origin: self.request,
      url: rourl,
      header,
      body,
    })
  }
}
