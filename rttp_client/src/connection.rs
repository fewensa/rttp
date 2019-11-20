use crate::error;
use crate::request::RawRequest;

pub struct Connection {
  request: RawRequest
}

impl Connection {
  pub fn new(request: RawRequest) -> Self {
    Self { request }
  }

  pub fn call(&self) -> error::Result<()> {
    Ok(())
  }
}
