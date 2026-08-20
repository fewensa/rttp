use std::fmt;
use std::time::SystemTime;

use rttp_protocol::cookie::HttpSetCookie;

use crate::error;

#[derive(Clone)]
pub struct Cookie {
  name: String,
  value: String,
  expires: Option<SystemTime>,
  path: Option<String>,
  domain: Option<String>,
  max_age: Option<u64>,
  secure: bool,
  http_only: bool,
  persistent: bool,
  host_only: bool,
  same_site: Option<String>,
}

impl Cookie {
  pub fn name(&self) -> &String {
    &self.name
  }
  pub fn value(&self) -> &String {
    &self.value
  }
  pub fn expires(&self) -> &Option<SystemTime> {
    &self.expires
  }
  pub fn path(&self) -> &Option<String> {
    &self.path
  }
  pub fn domain(&self) -> &Option<String> {
    &self.domain
  }
  pub fn secure(&self) -> bool {
    self.secure
  }
  pub fn http_only(&self) -> bool {
    self.http_only
  }
  pub fn persistent(&self) -> bool {
    self.persistent
  }
  pub fn host_only(&self) -> bool {
    self.host_only
  }
  pub fn same_site(&self) -> &Option<String> {
    &self.same_site
  }

  pub fn string(&self) -> String {
    let mut text = format!("{}={}", self.name, serialize_cookie_value(&self.value),);
    if let Some(path) = &self.path {
      text.push_str(&format!("; path={}", path));
    }
    if let Some(domain) = &self.domain {
      text.push_str(&format!("; domain={}", domain));
    }
    if self.persistent {
      if let Some(expires) = self.expires {
        let http_date = httpdate::fmt_http_date(expires);
        text.push_str(&format!("; expires={}", http_date));
      } else if let Some(max_age) = self.max_age {
        text.push_str(&format!("; max-age={}", max_age));
      } else {
        text.push_str("; max-age=0")
      }
    }
    if self.secure {
      text.push_str("; secure")
    }
    if self.http_only {
      text.push_str("; httpOnly")
    }
    if self.host_only {
      text.push_str("; hostOnly")
    }
    if let Some(same_site) = &self.same_site {
      text.push_str(&format!("; SameSite={}", same_site));
    }
    text
  }
}

fn serialize_cookie_value(value: &str) -> String {
  if value.bytes().all(is_cookie_octet) {
    return value.to_owned();
  }
  if value.bytes().all(is_generated_quoted_cookie_value_byte) {
    return format!("\"{}\"", value);
  }
  value.to_owned()
}

fn is_cookie_octet(byte: u8) -> bool {
  matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e)
}

fn is_generated_quoted_cookie_value_byte(byte: u8) -> bool {
  matches!(byte, 0x20..=0x7e) && byte != b'"' && byte != b'\\'
}

impl Cookie {
  pub(crate) fn from_set_cookie(cookie: &HttpSetCookie) -> Self {
    let expires = cookie
      .expires()
      .and_then(|value| httpdate::parse_http_date(&value.replace('-', " ")).ok());
    let host_only = cookie.extension_attributes().any(|attribute| {
      attribute.name().eq_ignore_ascii_case("hostonly")
        || attribute.name().eq_ignore_ascii_case("host_only")
    });
    Self {
      name: cookie.name().to_owned(),
      value: cookie.value().to_owned(),
      expires,
      path: cookie.path().map(str::to_owned),
      domain: cookie.domain().map(str::to_owned),
      max_age: cookie.max_age(),
      secure: cookie.secure(),
      http_only: cookie.http_only(),
      persistent: expires.is_some() || cookie.max_age().is_some(),
      host_only,
      same_site: cookie.same_site().map(|value| value.as_str().to_owned()),
    }
  }

  pub fn builder() -> CookieBuilder {
    CookieBuilder::new()
  }

  pub fn parse<S: AsRef<str>>(text: S) -> error::Result<Self> {
    let mut builder = Cookie::builder();
    let parts: Vec<&str> = text.as_ref().split(";").collect();
    for (index, item) in parts.iter().enumerate() {
      let nvs: Vec<&str> = item.split("=").collect();
      let name = nvs
        .first()
        .ok_or(error::bad_cookie("Cookie not have name"))?
        .trim();
      let value: String = nvs
        .iter()
        .enumerate()
        .filter(|(ix, _)| *ix > 0)
        .map(|(_, v)| *v)
        .collect::<Vec<&str>>()
        .join("=");
      let value = value.trim();
      if index == 0 {
        builder.name(name);
        builder.value(value);
        continue;
      }
      match name.to_ascii_lowercase().as_str() {
        "expires" => {
          let value = value.replace("-", " ");
          match httpdate::parse_http_date(&value[..]) {
            Ok(v) => {
              builder.expires(v);
            }
            Err(e) => eprintln!("=> {:?}", e),
          }
        }
        "path" => {
          builder.path(value);
        }
        "domain" => {
          builder.domain(value);
        }
        "secure" => {
          if value.is_empty() {
            builder.secure(true);
          } else {
            builder.secure(
              value
                .parse()
                .map_err(|_| error::bad_cookie("Cookie secure can not parse to bool"))?,
            );
          }
        }
        "http_only" | "httponly" => {
          if value.is_empty() {
            builder.http_only(true);
          } else {
            builder.http_only(
              value
                .parse()
                .map_err(|_| error::bad_cookie("Cookie httpOnly can not parse to bool"))?,
            );
          }
        }
        "host_only" | "hostonly" => {
          if value.is_empty() {
            builder.host_only(true);
          } else {
            builder.host_only(
              value
                .parse()
                .map_err(|_| error::bad_cookie("Cookie hostOnly can not parse to bool"))?,
            );
          }
        }
        "same_site" | "samesite" => {
          builder.same_site(value);
        }
        _ => {
          builder.name(name);
          builder.value(value);
        }
      }
    }
    Ok(builder.build())
  }
}

impl fmt::Debug for Cookie {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter
      .debug_struct("Cookie")
      .field("name", &self.name)
      .field("value", &"[REDACTED]")
      .field("expires", &self.expires.as_ref().map(|_| "[REDACTED]"))
      .field("path", &self.path.as_ref().map(|_| "[REDACTED]"))
      .field("domain", &self.domain.as_ref().map(|_| "[REDACTED]"))
      .field("max_age", &self.max_age)
      .field("secure", &self.secure)
      .field("http_only", &self.http_only)
      .field("persistent", &self.persistent)
      .field("host_only", &self.host_only)
      .field("same_site", &self.same_site)
      .finish()
  }
}

impl fmt::Display for Cookie {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&self.string(), formatter)
  }
}

#[allow(dead_code)]
#[allow(clippy::wrong_self_convention)]
pub trait ToCookie {
  fn to_cookie(&self) -> error::Result<Cookie>;
}

impl ToCookie for Cookie {
  fn to_cookie(&self) -> error::Result<Cookie> {
    Ok(self.clone())
  }
}

impl ToCookie for String {
  fn to_cookie(&self) -> error::Result<Cookie> {
    (&self[..]).to_cookie()
  }
}

impl ToCookie for &str {
  fn to_cookie(&self) -> error::Result<Cookie> {
    Cookie::parse(self)
  }
}

#[derive(Clone)]
pub struct CookieBuilder {
  cookie: Cookie,
}

impl CookieBuilder {
  pub fn new() -> Self {
    Self {
      cookie: Cookie {
        name: "".to_string(),
        value: "".to_string(),
        expires: None,
        path: None,
        domain: None,
        max_age: None,
        secure: false,
        http_only: false,
        persistent: false,
        host_only: false,
        same_site: None,
      },
    }
  }

  pub fn build(&self) -> Cookie {
    self.cookie.clone()
  }

  pub fn same_site<S: AsRef<str>>(&mut self, same_site: S) -> &mut Self {
    self.cookie.same_site = Some(same_site.as_ref().to_owned());
    self
  }
  pub fn name<S: AsRef<str>>(&mut self, name: S) -> &mut Self {
    self.cookie.name = name.as_ref().to_owned();
    self
  }
  pub fn value<S: AsRef<str>>(&mut self, value: S) -> &mut Self {
    self.cookie.value = value.as_ref().to_owned();
    self
  }
  pub fn expires(&mut self, expires: SystemTime) -> &mut Self {
    self.cookie.expires = Some(expires);
    self.cookie.persistent = true;
    self
  }
  pub fn path<S: AsRef<str>>(&mut self, path: S) -> &mut Self {
    self.cookie.path = Some(path.as_ref().to_owned());
    self
  }
  pub fn domain<S: AsRef<str>>(&mut self, domain: S) -> &mut Self {
    self.cookie.domain = Some(domain.as_ref().to_owned());
    self
  }
  pub fn secure(&mut self, secure: bool) -> &mut Self {
    self.cookie.secure = secure;
    self
  }
  pub fn http_only(&mut self, http_only: bool) -> &mut Self {
    self.cookie.http_only = http_only;
    self
  }
  pub fn host_only(&mut self, host_only: bool) -> &mut Self {
    self.cookie.host_only = host_only;
    self
  }
}

impl AsRef<Cookie> for Cookie {
  fn as_ref(&self) -> &Cookie {
    self
  }
}

impl AsRef<Cookie> for CookieBuilder {
  fn as_ref(&self) -> &Cookie {
    &self.cookie
  }
}
