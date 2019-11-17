use crate::request::Request;
use crate::error;

#[derive(Clone, Debug)]
pub struct Connection<'a> {
  request: &'a Request
}

impl<'a> Connection<'a> {
  pub fn new(request: &'a Request) -> Self {
    Self { request }
  }

  pub fn call(&self) -> error::Result<()> {
    println!("{:#?}", self.request);
    Ok(())
  }
}

impl<'a> Connection<'a> {

}
