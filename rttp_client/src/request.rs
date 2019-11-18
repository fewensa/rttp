use crate::error;
use crate::types::{Header, Para, RoUrl};

#[derive(Clone, Debug)]
pub struct Request {
  url: Option<RoUrl>,
  method: String,
  paths: Vec<String>,
  paras: Vec<Para>,
  headers: Vec<Header>,
  traditional: bool,
  encode: bool,
  raw: Option<String>,
}

impl Request {
  pub fn new() -> Self {
    Self {
      url: None,
      method: "GET".to_string(),
      paths: vec![],
      paras: vec![],
      headers: vec![],
      traditional: true,
      encode: true,
      raw: None,
    }
  }

  pub fn url(&self) -> &Option<RoUrl> { &self.url }
  pub fn method(&self) -> &String { &self.method }
  pub fn paths(&self) -> &Vec<String> { &self.paths }
  pub fn paras(&self) -> &Vec<Para> { &self.paras }
  pub fn headers(&self) -> &Vec<Header> { &self.headers }
  pub fn traditional(&self) -> bool { self.traditional }
  pub fn encode(&self) -> bool { self.encode }
  pub fn raw(&self) -> &Option<String> { &self.raw }


  pub(crate) fn url_mut(&mut self) -> &mut Option<RoUrl> { &mut self.url }
  pub(crate) fn method_mut(&mut self) -> &mut String { &mut self.method }
  pub(crate) fn paths_mut(&mut self) -> &mut Vec<String> { &mut self.paths }
  pub(crate) fn paras_mut(&mut self) -> &mut Vec<Para> { &mut self.paras }
  pub(crate) fn headers_mut(&mut self) -> &mut Vec<Header> { &mut self.headers }
  pub(crate) fn traditional_mut(&mut self) -> &mut bool { &mut self.traditional }
  pub(crate) fn encode_mut(&mut self) -> &mut bool { &mut self.encode }
  pub(crate) fn raw_mut(&mut self) -> &mut Option<String> { &mut self.raw }
}

pub struct RequestBody {
  binary: Vec<u8>
}

impl RequestBody {

  pub fn with_vec(vec: Vec<u8>) -> Self {
    Self { binary: vec }
  }

  pub fn with_text<S: AsRef<str>>(text: S) -> Self {
    Self::with_slice(text.as_ref().to_owned().as_bytes())
  }

  pub fn with_slice(slice: &[u8]) -> Self {
    Self::with_vec(slice.to_vec())
  }

  pub fn bytes(&self) -> &[u8] {
    self.binary.as_slice()
  }

  pub fn string(&self) -> error::Result<String> {
    String::from_utf8(self.binary.clone()).map_err(error::request)
  }

  pub fn len(&self) -> usize {
    self.binary.len()
  }
}
