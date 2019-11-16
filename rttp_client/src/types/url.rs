use url::Url;

use crate::error;
use crate::error::Error;
use crate::types::{Para, IntoPara};

#[derive(Clone, Debug)]
pub struct RoUrl {
  //  url: Url
  url: String,
  paths: Vec<String>,
  username: String,
  password: Option<String>,
  paras: Vec<Para>,
}


pub trait IntoUrl {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn into_url(self) -> error::Result<Url>;
}


impl RoUrl {
  pub fn with<S: AsRef<str>>(url: S) -> RoUrl {
    Self {
      url: url.as_ref().into(),
      paths: Default::default(),
      username: Default::default(),
      password: None,
      paras: vec![]
    }
  }

  pub fn username<S: AsRef<str>>(mut self, username: S) -> Self {
    self.username = username.as_ref().into();
    self
  }

  pub fn password<S: AsRef<str>>(mut self, password: S) -> Self {
    self.password = Some(password.as_ref().into());
    self
  }

  pub fn path<S: AsRef<str>>(mut self, path: S) -> Self {
    self.paths.push(path.as_ref().into());
    self
  }

  pub fn para<P: IntoPara>(mut self, para: P) -> Self {
    let paras = para.into_para();
    self.paras.extend(paras);
    self
  }

  fn join_paths(&self, url: &Url) -> String {
    let url_path = url.path();
    let mut paths = vec![];
    paths.push(url_path.to_string());
    for x in &self.paths {
      paths.push(x.clone());
    }
    crate::types::type_helper::safe_uri(paths.join("/"))
  }

  fn join_paras(&self, url: &Url) -> Option<String> {
    let mut all_paras: Vec<Para> = vec![];
    let url_paras = url.query()
      .map(|v| v.into_para());
    if let Some(uparas) = url_paras {
      all_paras.extend(uparas);
    }
    all_paras.extend(self.paras.clone());
    if all_paras.is_empty() {
      return None;
    }
    let para_string = all_paras.iter()
      .map(|p| format!("{}={}", p.name(), p.value()))
      .collect::<Vec<String>>()
      .join("&");
    Some(para_string)
  }
}

impl IntoUrl for RoUrl {
  fn into_url(self) -> Result<Url, Error> {
    let mut url = Url::parse(&self.url[..]).map_err(error::builder)?;

    let path = &self.join_paths(&url);
    url.set_path(&path[..]);

    if !self.username.is_empty() {
      url.set_username(&self.username[..]).map_err(|_| error::bad_url(url.clone(), "Bad url username"))?;
    }
    if let Some(password) = &self.password {
      url.set_password(Some(&password[..])).map_err(|_| error::bad_url(url.clone(), "Bad url password"))?;
    }
    if let Some(paras) = &self.join_paras(&url) {
      url.set_query(Some(&paras[..]));
    }
    Ok(url)
  }
}


impl IntoUrl for Url {
  fn into_url(self) -> Result<Url, Error> {
    if self.has_host() {
      Ok(self)
    } else {
//      Err(error::url_bad_scheme(self))
      Err(error::bad_url(self, "URL scheme is not allowed"))
    }
  }
}

impl<'a> IntoUrl for &'a str {
  fn into_url(self) -> Result<Url, Error> {
    unimplemented!()
  }
}

impl<'a> IntoUrl for &'a String {
  fn into_url(self) -> Result<Url, Error> {
    (&**self).into_url()
  }
}
