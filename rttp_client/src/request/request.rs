use std::fmt;

use crate::error;
use crate::types::{FormData, Header, Para, RoUrl, ToRoUrl};


#[derive(Clone, Debug)]
pub struct Request {
  url: Option<RoUrl>,
  method: String,
  paths: Vec<String>,
  paras: Vec<Para>,
  formdatas: Vec<FormData>,
  headers: Vec<Header>,
  traditional: bool,
  encode: bool,
  raw: Option<String>,
  binary: Vec<u8>,
}

impl Request {
  pub fn new() -> Self {
    Self {
      url: None,
      method: "GET".to_string(),
      paths: vec![],
      paras: vec![],
      formdatas: vec![],
      headers: vec![],
      traditional: true,
      encode: true,
      raw: None,
      binary: vec![],
    }
  }

  pub fn url(&self) -> &Option<RoUrl> { &self.url }
  pub fn method(&self) -> &String { &self.method }
  pub fn paths(&self) -> &Vec<String> { &self.paths }
  pub fn paras(&self) -> &Vec<Para> { &self.paras }
  pub fn formdatas(&self) -> &Vec<FormData> { &self.formdatas }
  pub fn headers(&self) -> &Vec<Header> { &self.headers }
  pub fn traditional(&self) -> bool { self.traditional }
  pub fn encode(&self) -> bool { self.encode }
  pub fn raw(&self) -> &Option<String> { &self.raw }
  pub fn binary(&self) -> &Vec<u8> { &self.binary }

  pub(crate) fn url_mut(&mut self) -> &mut Option<RoUrl> { &mut self.url }
  pub(crate) fn method_mut(&mut self) -> &mut String { &mut self.method }
  pub(crate) fn paths_mut(&mut self) -> &mut Vec<String> { &mut self.paths }
  pub(crate) fn paras_mut(&mut self) -> &mut Vec<Para> { &mut self.paras }
  pub(crate) fn formdatas_mut(&mut self) -> &mut Vec<FormData> { &mut self.formdatas }
  pub(crate) fn headers_mut(&mut self) -> &mut Vec<Header> { &mut self.headers }
  pub(crate) fn traditional_mut(&mut self) -> &mut bool { &mut self.traditional }
  pub(crate) fn encode_mut(&mut self) -> &mut bool { &mut self.encode }
  pub(crate) fn raw_mut(&mut self) -> &mut Option<String> { &mut self.raw }
  pub(crate) fn binary_mut(&mut self) -> &mut Vec<u8> { &mut self.binary }


  pub(crate) fn url_set<S: AsRef<RoUrl>>(&mut self, rourl: S) -> &mut Self {
    self.url = Some(rourl.as_ref().to_rourl());
    self
  }
  pub(crate) fn method_set<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.method = method.as_ref().into();
    self
  }
  pub(crate) fn paths_set(&mut self, paths: Vec<String>) -> &mut Self {
    self.paths = paths;
    self
  }
  pub(crate) fn paras_set(&mut self, paras: Vec<Para>) -> &mut Self {
    self.paras = paras;
    self
  }
  pub(crate) fn formdatas_set(&mut self, formdatas: Vec<FormData>) -> &mut Self {
    self.formdatas = formdatas;
    self
  }
  pub(crate) fn headers_set(&mut self, headers: Vec<Header>) -> &mut Self {
    self.headers = headers;
    self
  }
  pub(crate) fn traditional_set(&mut self, traditional: bool) -> &mut Self {
    self.traditional = traditional;
    self
  }
  pub(crate) fn encode_set(&mut self, encode: bool) -> &mut Self {
    self.encode = encode;
    self
  }
  pub(crate) fn raw_set<S: AsRef<str>>(&mut self, raw: S) -> &mut Self {
    self.raw = Some(raw.as_ref().into());
    self
  }
  pub(crate) fn binary_set(&mut self, binary: Vec<u8>) -> &mut Self {
    self.binary = binary;
    self
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<String> {
    self.headers.iter()
      .find(|h| h.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|h| h.value().clone())
  }
}



#[derive(Clone)]
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

impl fmt::Display for RequestBody {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let text = self.string().unwrap_or_default();
    fmt::Display::fmt(&text, formatter)
  }
}

impl fmt::Debug for RequestBody {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let text = self.string().unwrap_or_default();
    fmt::Debug::fmt(&text, formatter)
  }
}

