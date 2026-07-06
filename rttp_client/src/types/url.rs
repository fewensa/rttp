//use std::{cmp, fmt, hash};
use std::fmt::Debug;
use std::str::FromStr;

use url::Url;

use crate::error;
use crate::error::Error;
use crate::types::{IntoPara, Para, ParaType};

/// Url builder
///
/// # Examples
///
/// ```rust
/// # use rttp_client::types::{RoUrl, Para};
/// let rourl = RoUrl::with("http://httpbin.org")
///   .path("get")
///   .para("name=value")
///   .para("name=value&name=value")
///   .para(("name", "value", "name=value&name=value"))
///   .para(Para::with_form("name", "value"));
/// ```
#[derive(Clone, Debug)]
pub struct RoUrl {
  url: String,
  paths: Vec<String>,
  username: String,
  password: Option<String>,
  paras: Vec<Para>,
  fragment: Option<String>,
  traditional: Option<bool>,
}

pub trait ToRoUrl: Debug {
  fn to_rourl(&self) -> RoUrl;
}

pub trait ToUrl: Debug {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn to_url(&self) -> error::Result<Url>;
}

#[allow(dead_code)]
impl RoUrl {
  /// Create a rourl
  /// # Examples
  /// ```rust
  /// use rttp_client::types::RoUrl;
  /// RoUrl::with("http://httpbin.org/get");
  /// ```
  pub fn with(url: impl AsRef<str>) -> RoUrl {
    let url = url.as_ref();
    let (url_without_fragment, fragment) =
      url.split_once("#").map_or((url, None), |(url, fragment)| {
        (url, Some(fragment.to_string()))
      });
    let (url, para_string) = url_without_fragment
      .split_once("?")
      .map_or((url_without_fragment, ""), |(url, para)| (url, para));
    let mut paras = para_string.into_paras();
    for para in &mut paras {
      *para.type_mut() = ParaType::URL;
    }
    Self {
      url: url.to_string(),
      paths: Default::default(),
      username: Default::default(),
      password: None,
      paras,
      traditional: None,
      fragment,
    }
  }

  pub(crate) fn url_get(&self) -> &String {
    &self.url
  }
  pub(crate) fn paths_get(&self) -> &Vec<String> {
    &self.paths
  }
  pub(crate) fn username_get(&self) -> &String {
    &self.username
  }
  pub(crate) fn password_get(&self) -> &Option<String> {
    &self.password
  }
  pub(crate) fn paras_get(&self) -> &Vec<Para> {
    &self.paras
  }
  pub(crate) fn fragment_get(&self) -> &Option<String> {
    &self.fragment
  }
  pub(crate) fn traditional_get(&self) -> Option<bool> {
    self.traditional
  }

  /// Set fragment to url
  pub fn fragment<S: AsRef<str>>(&mut self, fragment: S) -> &mut Self {
    self.fragment = Some(fragment.as_ref().into());
    self
  }

  /// Set username
  pub fn username<S: AsRef<str>>(&mut self, username: S) -> &mut Self {
    self.username = username.as_ref().into();
    self
  }

  /// Set password
  pub fn password<S: AsRef<str>>(&mut self, password: S) -> &mut Self {
    self.password = Some(password.as_ref().into());
    self
  }

  /// Add path to url
  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.paths.push(path.as_ref().into());
    self
  }

  /// Add para to url
  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
    let mut paras = para.into_paras();
    for para in &mut paras {
      *para.type_mut() = ParaType::URL;
    }
    self.paras.extend(paras);
    self
  }

  /// Set paras fo rurl
  pub fn paras(&mut self, paras: Vec<Para>) -> &mut Self {
    self.paras = paras;
    self
  }

  /// Set is traditional
  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.traditional = Some(traditional);
    self
  }
}

impl RoUrl {
  fn join_paths(&self, url: &Url) -> String {
    let url_path = url.path();
    let mut paths = vec![];
    paths.push(url_path.to_string());
    for x in &self.paths {
      paths.push(x.clone());
    }
    crate::types::type_helper::stand_uri(paths.join("/"))
  }

  fn join_paras(&self, url: &Url) -> Option<String> {
    let mut all_paras: Vec<Para> = vec![];
    let url_paras = url.query().map(|v| v.into_paras());
    if let Some(uparas) = url_paras {
      all_paras.extend(uparas);
    }
    all_paras.extend(self.paras.clone());
    if all_paras.is_empty() {
      return None;
    }
    let para_string = all_paras
      .iter()
      .filter(|&p| p.is_url() || p.is_form())
      .map(|p| {
        let name = p.name();
        let traditional = self.traditional.unwrap_or(true);
        if traditional {
          return format!(
            "{}={}",
            name,
            p.value().clone().map_or("".to_string(), |t| t)
          );
        }
        let is_array = all_paras
          .iter()
          .filter(|&item| item.name() == name)
          .collect::<Vec<&Para>>()
          .len()
          > 1;
        let ends_with_bracket = name.ends_with("[]");
        format!(
          "{}{}={}",
          name,
          if !ends_with_bracket && (is_array || p.array()) {
            "[]"
          } else {
            ""
          },
          p.value().clone().map_or("".to_string(), |t| t)
        )
      })
      .collect::<Vec<String>>()
      .join("&");
    Some(para_string)
  }
}

impl ToRoUrl for RoUrl {
  fn to_rourl(&self) -> RoUrl {
    Self {
      url: self.url.clone(),
      paths: self.paths.clone(),
      username: self.username.clone(),
      password: self.password.clone(),
      paras: self.paras.clone(),
      fragment: self.fragment.clone(),
      traditional: self.traditional,
    }
  }
}

impl ToRoUrl for &str {
  fn to_rourl(&self) -> RoUrl {
    RoUrl::with(self)
  }
}

impl ToRoUrl for String {
  fn to_rourl(&self) -> RoUrl {
    (&self[..]).to_rourl()
  }
}

impl ToUrl for RoUrl {
  fn to_url(&self) -> Result<Url, Error> {
    let mut url = Url::parse(&self.url[..]).map_err(error::builder)?;

    let path = &self.join_paths(&url);
    url.set_path(&path[..]);

    if !self.username.is_empty() {
      url
        .set_username(&self.username[..])
        .map_err(|_| error::bad_url(url.clone(), "Bad url username"))?;
    }
    if let Some(password) = &self.password {
      url
        .set_password(Some(&password[..]))
        .map_err(|_| error::bad_url(url.clone(), "Bad url password"))?;
    }
    if let Some(paras) = &self.join_paras(&url) {
      url.set_query(Some(&paras[..]));
    }
    if let Some(fragment) = &self.fragment {
      url.set_fragment(Some(&fragment[..]));
    }
    Ok(url)
  }
}

impl From<Url> for RoUrl {
  fn from(url: Url) -> Self {
    Self::with(url.as_str())
  }
}

impl AsRef<RoUrl> for RoUrl {
  fn as_ref(&self) -> &RoUrl {
    self
  }
}

impl<IU: ToRoUrl> ToRoUrl for &IU {
  fn to_rourl(&self) -> RoUrl {
    (*self).to_rourl()
  }
}

impl<IU: ToRoUrl> ToRoUrl for &mut IU {
  fn to_rourl(&self) -> RoUrl {
    (**self).to_rourl()
  }
}

impl FromStr for RoUrl {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(RoUrl::with(s))
  }
}
