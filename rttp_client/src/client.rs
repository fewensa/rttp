use crate::types::IntoUrl;

pub struct HttpClient {
  method: String
}

impl Default for HttpClient {
  fn default() -> Self {
    HttpClient {
      method: "GET".to_string()
    }
  }
}

impl HttpClient {
  pub fn method<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.method = method.as_ref().to_owned();
    self
  }

  pub fn url<U: IntoUrl>(&mut self, url: U) -> &mut Self {
    self
  }

  pub fn charset(&mut self) -> &mut Self {
    self
  }

  pub fn traditional(&mut self) -> &mut Self {
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

  pub fn para(&mut self) -> &mut Self {
    self
   }

  pub fn raw(&mut self) -> &mut Self {
    self
  }

  pub fn binary(&mut self) -> &mut Self {
    self
  }

  pub fn emit(&self) {}

  pub fn enqueue(&self) {}
}
