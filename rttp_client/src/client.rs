use crate::connection::Connection;
use crate::error;
use crate::request::Request;
use crate::types::{Header, IntoHeader, IntoPara, Para, RoUrl, ToFormData, ToRoUrl, ToUrl};

#[derive(Debug)]
pub struct HttpClient {
  request: Request,
}

impl Default for HttpClient {
  fn default() -> Self {
    Self {
      request: Request::new()
    }
  }
}

impl HttpClient {

  pub fn get(&mut self) -> &mut Self {
    self.method("GET")
  }

  pub fn post(&mut self) -> &mut Self {
    self.method("POST")
  }

  pub fn put(&mut self) -> &mut Self {
    self.method("PUT")
  }

  pub fn delete(&mut self) -> &mut Self {
    self.method("DELETE")
  }

  pub fn options(&mut self) -> &mut Self {
    self.method("OPTIONS")
  }

  pub fn head(&mut self) -> &mut Self {
    self.method("HEAD")
  }

  pub fn method<S: AsRef<str>>(&mut self, method: S) -> &mut Self {
    self.request.method_set(method);
    self
  }

  pub fn url<U: ToRoUrl>(&mut self, url: U) -> &mut Self {
    self.request.url_set(url.to_rourl());
    self
  }

  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.request.traditional_set(traditional);
    self
  }

  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    let mut paths = self.request.paths_mut();
    paths.push(path.as_ref().into());
    self
  }

  pub fn encode(&mut self, encode: bool) -> &mut Self {
    *self.request.encode_mut() = encode;
    self
  }

  pub fn proxy(&mut self) -> &mut Self {
    self
  }

  pub fn auth(&mut self) -> &mut Self {
    self
  }


  pub fn header<P: IntoHeader>(&mut self, header: P) -> &mut Self {
    let mut headers = self.request.headers_mut();
    for h in header.into_headers() {
      let mut exi = headers.iter_mut()
        .find(|d| d.name().eq_ignore_ascii_case(h.name()));

      if let Some(eh) = exi {
        if h.name().eq_ignore_ascii_case("cookie") {
          let new_cookie_value = format!("{};{}", eh.value(), h.value());
          eh.replace(Header::new("Cookie", new_cookie_value));
          continue;
        }

        eh.replace(h);
        continue;
      }
      headers.push(h);
    }
    self
  }

  pub fn cookie<S: AsRef<str>>(&mut self, cookie: S) -> &mut Self {
    self.header(("Cookie", cookie.as_ref()))
  }

  pub fn content_type<S: AsRef<str>>(&mut self, content_type: S) -> &mut Self {
    self.header(("Content-Type", content_type.as_ref()))
  }

  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
    let paras = para.into_paras();
    let mut req_paras = self.request.paras_mut();
    req_paras.extend(paras);
    self
  }

  pub fn form<S: ToFormData>(&mut self, formdata: S) -> &mut Self {
    let formdatas = formdata.to_formdatas();
    let mut req_formdatas = self.request.formdatas_mut();
    req_formdatas.extend(formdatas);
    self
  }

  pub fn raw<S: AsRef<str>>(&mut self, raw: S) -> &mut Self {
    self.request.raw_set(raw);
    self
  }

  pub fn binary(&mut self, binary: Vec<u8>) -> &mut Self {
    self.request.binary_set(binary);
    self
  }

  pub fn emit(&self) -> error::Result<()> {
    Connection::new(self.request.clone()).call()
  }

  pub fn enqueue(&self) {}
}
