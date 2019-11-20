use std::{cmp, fmt, hash};
use std::fmt::Debug;

use url::Url;

use crate::error;
use crate::error::Error;
use crate::types::{IntoPara, Para, ParaType};

#[derive(Clone, Debug)]
pub struct RoUrl {
  url: String,
  paths: Vec<String>,
  username: String,
  password: Option<String>,
  paras: Vec<Para>,
  fragment: Option<String>,
  traditional: bool,
}


pub trait ToRoUrl: Debug {
  fn to_rourl(&self) -> RoUrl;
}

pub trait ToUrl: Debug {
  // Besides parsing as a valid `Url`, the `Url` must be a valid
  // `http::Uri`, in that it makes sense to use in a network request.
  fn to_url(&self) -> error::Result<Url>;
}


impl RoUrl {
  pub fn with<S: AsRef<str>>(url: S) -> RoUrl {
    let url = url.as_ref();
    let netloc_and_para: Vec<&str> = url.split("?").collect::<Vec<&str>>();
    let url = netloc_and_para.get(0).map_or("".to_string(), |v| v.to_string());
    let mut para_string = String::new();
    let mut fragment = None;
    for (i, nap) in netloc_and_para.iter().enumerate() {
      if i == 0 { continue; }
      let para_and_fragment: Vec<&str> = nap.split("#").collect::<Vec<&str>>();
      if para_and_fragment.is_empty() {
        para_string.push_str(nap);
      } else {
        if let Some(last_para) = para_and_fragment.get(0) {
          para_string.push_str(last_para);
        }

        let fragment_string = para_and_fragment.iter()
          .enumerate()
          .filter(|(ix, _)| *ix > 0)
          .map(|(_, v)| *v)
          .collect::<Vec<&str>>()
          .join("#");
        if !fragment_string.is_empty() {
          fragment = Some(fragment_string);
        }
      }
    }
    let mut paras = (&para_string).into_paras();
    for para in &mut paras {
      *para.type_mut() = ParaType::URL;
    }
    Self {
      url,
      paths: Default::default(),
      username: Default::default(),
      password: None,
      paras,
      traditional: true,
      fragment,
    }
  }

  pub(crate) fn url_get(&self) -> &String { &self.url }
  pub(crate) fn paths_get(&self) -> &Vec<String> { &self.paths }
  pub(crate) fn username_get(&self) -> &String { &self.username }
  pub(crate) fn password_get(&self) -> &Option<String> { &self.password }
  pub(crate) fn paras_get(&self) -> &Vec<Para> { &self.paras }
  pub(crate) fn fragment_get(&self) -> &Option<String> { &self.fragment }
  pub(crate) fn traditional_get(&self) -> bool { self.traditional }

  pub(crate) fn url_mut(&mut self) -> &mut String { &mut self.url }
  pub(crate) fn paths_mut(&mut self) -> &mut Vec<String> { &mut self.paths }
  pub(crate) fn username_mut(&mut self) -> &mut String { &mut self.username }
  pub(crate) fn password_mut(&mut self) -> &mut Option<String> { &mut self.password }
  pub(crate) fn paras_mut(&mut self) -> &mut Vec<Para> { &mut self.paras }
  pub(crate) fn fragment_mut(&mut self) -> &mut Option<String> { &mut self.fragment }
  pub(crate) fn traditional_mut(&mut self) -> &mut bool { &mut self.traditional }

  pub(crate) fn url_set<S: AsRef<str>>(&mut self, url: S) -> &mut Self {
    self.url = url.as_ref().into();
    self
  }
  pub(crate) fn paths_set(&mut self, paths: Vec<String>) -> &mut Self {
    self.paths = paths;
    self
  }
  pub(crate) fn username_set<S: AsRef<str>>(&mut self, username: S) -> &mut Self {
    self.username = username.as_ref().into();
    self
  }
  pub(crate) fn password_set<S: AsRef<str>>(&mut self, password: S) -> &mut Self {
    self.password = Some(password.as_ref().into());
    self
  }
  pub(crate) fn paras_set(&mut self, paras: Vec<Para>) -> &mut Self {
    self.paras = paras;
    self
  }
  pub(crate) fn fragment_set<S: AsRef<str>>(&mut self, fragment: S) -> &mut Self {
    self.fragment = Some(fragment.as_ref().into());
    self
  }
  pub(crate) fn traditional_set(&mut self, traditional: bool) -> &mut Self {
    self.traditional = traditional;
    self
  }

  pub fn fragment<S: AsRef<str>>(&mut self, fragment: S) -> &mut Self {
    self.fragment = Some(fragment.as_ref().into());
    self
  }

  pub fn username<S: AsRef<str>>(&mut self, username: S) -> &mut Self {
    self.username = username.as_ref().into();
    self
  }

  pub fn password<S: AsRef<str>>(&mut self, password: S) -> &mut Self {
    self.password = Some(password.as_ref().into());
    self
  }

  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.paths.push(path.as_ref().into());
    self
  }

  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
    let mut paras = para.into_paras();
    for para in &mut paras {
      *para.type_mut() = ParaType::URL;
    }
    self.paras.extend(paras);
    self
  }

  pub fn paras(&mut self, paras: Vec<Para>) -> &mut Self {
    self.paras = paras;
    self
  }

  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
    self.traditional = traditional;
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
    let url_paras = url.query()
      .map(|v| v.into_paras());
    if let Some(uparas) = url_paras {
      all_paras.extend(uparas);
    }
    all_paras.extend(self.paras.clone());
    if all_paras.is_empty() {
      return None;
    }
    let para_string = all_paras.iter()
      .filter(|&p| p.is_url() || p.is_form())
      .map(|p| {
        let name = p.name();
        if self.traditional {
          return format!("{}={}", name, p.value().clone().map_or("".to_string(), |t| t));
        }
        let is_array = all_paras.iter()
          .filter(|&item| item.name() == name)
          .collect::<Vec<&Para>>()
          .len() > 1;
        let ends_with_bracket = name.ends_with("[]");
        return format!("{}{}={}", name,
                       if !ends_with_bracket && (is_array || p.array()) { "[]" } else { "" },
                       p.value().clone().map_or("".to_string(), |t| t));
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

impl ToRoUrl for &String {
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
      url.set_username(&self.username[..]).map_err(|_| error::bad_url(url.clone(), "Bad url username"))?;
    }
    if let Some(password) = &self.password {
      url.set_password(Some(&password[..])).map_err(|_| error::bad_url(url.clone(), "Bad url password"))?;
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

///// Display the serialization of this URL.
//impl fmt::Display for RoUrl {
//  #[inline]
//  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//    fmt::Display::fmt(self.to_url().expect("Can't convert RoUrl to Url"), formatter)
//  }
//}
//
///// Debug the serialization of this URL.
//impl fmt::Debug for RoUrl {
//  #[inline]
//  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//    fmt::Debug::fmt(self.to_url().expect("Can't convert RoUrl to Url"), formatter)
//  }
//}
//
//
///// URLs compare like their serialization.
//impl Eq for RoUrl {}
//
///// URLs compare like their serialization.
//impl PartialEq for RoUrl {
//  #[inline]
//  fn eq(&self, other: &Self) -> bool {
//    self.to_url().expect("Can't convert RoUrl to Url")
//      == other.to_url().expect("Can't convert RoUrl to Url")
//  }
//}
//
///// URLs compare like their serialization.
//impl Ord for RoUrl {
//  #[inline]
//  fn cmp(&self, other: &Self) -> cmp::Ordering {
//    self.to_url().expect("Can't convert RoUrl to Url")
//      .cmp(&other.to_url().expect("Can't convert RoUrl to Url"))
//  }
//}
//
///// URLs compare like their serialization.
//impl PartialOrd for RoUrl {
//  #[inline]
//  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
//    self.to_url().expect("Can't convert RoUrl to Url")
//      .partial_cmp(&other.to_url().expect("Can't convert RoUrl to Url"))
//  }
//}
//
///// URLs hash like their serialization.
//impl hash::Hash for RoUrl {
//  #[inline]
//  fn hash<H>(&self, state: &mut H)
//    where
//      H: hash::Hasher,
//  {
//    hash::Hash::hash(&self.to_url().expect("Can't convert RoUrl to Url"), state)
//  }
//}


impl<'a, IU: ToRoUrl> ToRoUrl for &'a IU {
  fn to_rourl(&self) -> RoUrl {
    (*self).to_rourl()
  }
}

impl<'a, IU: ToRoUrl> ToRoUrl for &'a mut IU {
  fn to_rourl(&self) -> RoUrl {
    (**self).to_rourl()
  }
}


//pub trait IntoUrl: Debug {
//  // Besides parsing as a valid `Url`, the `Url` must be a valid
//  // `http::Uri`, in that it makes sense to use in a network request.
//  fn into_url(&self) -> error::Result<Url>;
//}
//
//
//impl RoUrl {
//  pub fn with<S: AsRef<str>>(url: S) -> RoUrl {
//    Self {
//      url: url.as_ref().into(),
//      paths: Default::default(),
//      username: Default::default(),
//      password: None,
//      paras: vec![],
//      traditional: true,
//      fragment: None
//    }
//  }
//
//  pub fn fragment<S: AsRef<str>>(&mut self, fragment: S) -> &mut Self {
//    self.fragment = Some(fragment.as_ref().into());
//    self
//  }
//
//  pub fn username<S: AsRef<str>>(&mut self, username: S) -> &mut Self {
//    self.username = username.as_ref().into();
//    self
//  }
//
//  pub fn password<S: AsRef<str>>(&mut self, password: S) -> &mut Self {
//    self.password = Some(password.as_ref().into());
//    self
//  }
//
//  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
//    self.paths.push(path.as_ref().into());
//    self
//  }
//
//  pub fn para<P: IntoPara>(&mut self, para: P) -> &mut Self {
//    let paras = para.into_paras();
//    self.paras.extend(paras);
//    self
//  }
//
//  pub fn traditional(&mut self, traditional: bool) -> &mut Self {
//    self.traditional = traditional;
//    self
//  }
//
//  fn join_paths(&self, url: &Url) -> String {
//    let url_path = url.path();
//    let mut paths = vec![];
//    paths.push(url_path.to_string());
//    for x in &self.paths {
//      paths.push(x.clone());
//    }
//    crate::types::type_helper::safe_uri(paths.join("/"))
//  }
//
//  fn join_paras(&self, url: &Url) -> Option<String> {
//    let mut all_paras: Vec<Para> = vec![];
//    let url_paras = url.query()
//      .map(|v| v.into_paras());
//    if let Some(uparas) = url_paras {
//      all_paras.extend(uparas);
//    }
//    all_paras.extend(self.paras.clone());
//    if all_paras.is_empty() {
//      return None;
//    }
//    let para_string = all_paras.iter()
//      .filter(|&p| p.is_form())
//      .map(|p| {
//        let name = p.name();
//        if self.traditional {
//          return format!("{}={}", name, p.text().clone().map_or("".to_string(), |t| t));
//        }
//        let is_array = all_paras.iter()
//          .filter(|&item| item.name() == name)
//          .collect::<Vec<&Para>>()
//          .len() > 1;
//        let ends_with_bracket = name.ends_with("[]");
//        return format!("{}{}={}", name,
//                       if is_array && !ends_with_bracket { "[]" } else { "" },
//                       p.text().clone().map_or("".to_string(), |t| t));
//      })
//      .collect::<Vec<String>>()
//      .join("&");
//    Some(para_string)
//  }
//}
//
//impl IntoUrl for RoUrl {
//  fn into_url(&self) -> Result<Url, Error> {
//    let mut url = Url::parse(&self.url[..]).map_err(error::builder)?;
//
//    let path = &self.join_paths(&url);
//    url.set_path(&path[..]);
//
//    if !self.username.is_empty() {
//      url.set_username(&self.username[..]).map_err(|_| error::bad_url(url.clone(), "Bad url username"))?;
//    }
//    if let Some(password) = &self.password {
//      url.set_password(Some(&password[..])).map_err(|_| error::bad_url(url.clone(), "Bad url password"))?;
//    }
//    if let Some(paras) = &self.join_paras(&url) {
//      url.set_query(Some(&paras[..]));
//    }
//    if let Some(fragment) = &self.fragment {
//      url.set_fragment(Some(&fragment[..]));
//    }
//    Ok(url)
//  }
//}
//
//
//impl From<Url> for RoUrl {
//  fn from(url: Url) -> Self {
//    Self::with(url.as_str())
//  }
//}
//
///// Display the serialization of this URL.
//impl fmt::Display for RoUrl {
//  #[inline]
//  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//    fmt::Display::fmt(&self.clone().into_url().expect("Can't convert RoUrl to Url"), formatter)
//  }
//}
//
///// Debug the serialization of this URL.
//impl fmt::Debug for RoUrl {
//  #[inline]
//  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//    fmt::Debug::fmt(&self.clone().into_url().expect("Can't convert RoUrl to Url"), formatter)
//  }
//}
//
//
//
///// URLs compare like their serialization.
//impl Eq for RoUrl {}
//
///// URLs compare like their serialization.
//impl PartialEq for RoUrl {
//  #[inline]
//  fn eq(&self, other: &Self) -> bool {
//    self.clone().into_url().expect("Can't convert RoUrl to Url")
//      == other.clone().into_url().expect("Can't convert RoUrl to Url")
//  }
//}
//
///// URLs compare like their serialization.
//impl Ord for RoUrl {
//  #[inline]
//  fn cmp(&self, other: &Self) -> cmp::Ordering {
//    self.clone().into_url().expect("Can't convert RoUrl to Url")
//      .cmp(&other.clone().into_url().expect("Can't convert RoUrl to Url"))
//  }
//}
//
///// URLs compare like their serialization.
//impl PartialOrd for RoUrl {
//  #[inline]
//  fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
//    self.clone().into_url().expect("Can't convert RoUrl to Url")
//      .partial_cmp(&other.clone().into_url().expect("Can't convert RoUrl to Url"))
//  }
//}
//
///// URLs hash like their serialization.
//impl hash::Hash for RoUrl {
//  #[inline]
//  fn hash<H>(&self, state: &mut H)
//    where
//      H: hash::Hasher,
//  {
//    hash::Hash::hash(&self.clone().into_url().expect("Can't convert RoUrl to Url"), state)
//  }
//}
//
///// Return the serialization of this URL.
//impl AsRef<str> for RoUrl {
//  #[inline]
//  fn as_ref(&self) -> &str {
//    &self.url
//  }
//}
//
//
//
//
//
//
//
//
//
//
//impl IntoUrl for Url {
//  fn into_url(&self) -> Result<Url, Error> {
//    if self.has_host() {
//      Ok(self.clone())
//    } else {
////      Err(error::url_bad_scheme(self))
//      Err(error::bad_url(self.clone(), "URL scheme is not allowed"))
//    }
//  }
//}
//
//impl<'a> IntoUrl for &'a str {
//  fn into_url(&self) -> Result<Url, Error> {
//    Url::parse(self).map_err(error::builder)?.into_url()
//  }
//}
//
//impl<'a> IntoUrl for &'a String {
//  fn into_url(&self) -> Result<Url, Error> {
//    (&self[..]).into_url()
//  }
//}
//
//
//impl<'a, IU: IntoUrl> IntoUrl for &'a IU {
//  fn into_url(&self) -> Result<Url, Error> {
//    (*self).into_url()
//  }
//}
//
//impl<'a, IU: IntoUrl> IntoUrl for &'a mut IU {
//  fn into_url(&self) -> Result<Url, Error> {
//    (**self).into_url()
//  }
//}
