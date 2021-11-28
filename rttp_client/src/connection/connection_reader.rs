use std::io;

use url::Url;

use crate::error;
use crate::response::Response;
use crate::types::RoUrl;

pub struct ConnectionReader<'a> {
  url: &'a Url,
  reader: Box<&'a mut dyn io::Read>,
}

impl<'a> ConnectionReader<'a> {
  pub fn new(url: &'a Url, reader: &'a mut dyn io::Read) -> ConnectionReader<'a> {
    Self {
      url,
      reader: Box::new(reader),
    }
  }

  pub fn binary(&mut self) -> error::Result<Vec<u8>> {
    let mut binary: Vec<u8> = Vec::new();
    let _ = self
      .reader
      .read_to_end(&mut binary)
      .map_err(error::request)?;
    Ok(binary)
  }

  pub fn response(&mut self) -> error::Result<Response> {
    Response::new(RoUrl::from(self.url.clone()), self.binary()?)
  }

  // todo Connection reader will read more type from io::Reader, like Chunk data, and Stream data.
}
