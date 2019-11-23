use std::fmt;

use crate::{error, Config};
use crate::types::{FormData, Header, Para, Proxy, RoUrl, ToRoUrl};

#[derive(Clone, Debug)]
pub struct Request {
  closed: bool,
  count: u32,
  config: Config,
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
  proxy: Option<Proxy>,
}

impl Request {
  pub fn new() -> Self {
    Self {
      closed: false,
      count: 1,
      config: Default::default(),
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
      proxy: None,
    }
  }

  pub fn closed(&self) -> bool { self.closed }
  pub fn config(&self) -> &Config { &self.config }
  pub fn count(&self) -> u32 { self.count }
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
  pub fn proxy(&self) -> &Option<Proxy> { &self.proxy }

  pub(crate) fn closed_mut(&mut self) -> &mut bool { &mut self.closed }
  pub(crate) fn config_mut(&mut self) -> &mut Config { &mut self.config }
  pub(crate) fn count_mut(&mut self) -> &mut u32 { &mut self.count }
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
  pub(crate) fn proxy_mut(&mut self) -> &mut Option<Proxy> { &mut self.proxy }


  pub(crate) fn closed_set(&mut self, closed: bool) -> &mut Self {
    self.closed = closed;
    self
  }
  pub(crate) fn config_set<C: AsRef<Config>>(&mut self, config: C) -> &mut Self {
    self.config = config.as_ref().clone();
    self
  }
  pub(crate) fn count_set(&mut self, count: u32) -> &mut Self {
    self.count = count;
    self
  }
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
  pub(crate) fn proxy_set(&mut self, proxy: Proxy) -> &mut Self {
    self.proxy = Some(proxy);
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

