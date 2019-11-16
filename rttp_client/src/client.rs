use crate::error;
use crate::types::{IntoUrl, RoUrl, Para, IntoPara};

pub struct HttpClient {
  url: error::Result<RoUrl>,
  method: String,
  paths: Vec<String>,
  paras: Vec<Para>,
  traditional: bool,
}

impl Default for HttpClient {
  fn default() -> Self {
    HttpClient {
      url: Err(error::none_url()),
      method: "GET".to_string(),
      paths: vec![],
      paras: vec![],
      traditional: true
    }
  }
}

impl HttpClient {
  pub fn method<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.method = method.as_ref().to_owned();
    self
  }

  pub fn url<U: IntoUrl>(&mut self, url: U) -> &mut Self {
    self.url = url.into_url().map(|u| RoUrl::from(u));
    self
  }

  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.traditional = traditional;
    self
  }

  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.paths.push(path.as_ref().into());
    self
  }

  pub fn encode(&mut self) -> &mut Self {
    self
  }

  pub fn proxy(&mut self) -> &mut Self {
    self
  }

  pub fn auth(&mut self) -> &mut Self {
    self
  }



  pub fn header(&mut self) -> &mut Self {
    self
  }

  pub fn cookie(&mut self) -> &mut Self {
    self
  }

  pub fn content_type(&mut self) -> &mut Self {
    self
  }

  pub fn para<P: IntoPara>(&mut self, para: P) -> &Self {
    let paras = para.into_para();
    self.paras.extend(paras);
    self
  }

  pub fn raw(&mut self) -> &mut Self {
    self
  }

  pub fn binary(&mut self) -> &mut Self {
    self
  }

  pub fn emit(&self) {
    println!("{} {:?}", self.method, self.url)
  }

  pub fn enqueue(&self) {}
}
