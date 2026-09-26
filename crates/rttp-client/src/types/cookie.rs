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
    let mut text = format!(
      "{}={}",
      serialize_legacy_cookie_field(&self.name),
      serialize_legacy_cookie_field(&self.value),
    );
    if let Some(path) = &self.path {
      text.push_str(&format!("; path={}", serialize_legacy_cookie_field(path)));
    }
    if let Some(domain) = &self.domain {
      text.push_str(&format!(
        "; domain={}",
        serialize_legacy_cookie_field(domain)
      ));
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
      text.push_str(&format!(
        "; SameSite={}",
        serialize_legacy_cookie_field(same_site)
      ));
    }
    text
  }
}

fn serialize_legacy_cookie_field(value: &str) -> String {
  let quoted = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');
  let payload = if quoted {
    &value[1..value.len() - 1]
  } else {
    value
  };
  let sanitized: String = payload
    .bytes()
    .filter(|&byte| is_generated_quoted_cookie_value_byte(byte))
    .map(char::from)
    .collect();
  if quoted || !sanitized.bytes().all(is_cookie_octet) {
    format!("\"{}\"", sanitized)
  } else {
    sanitized
  }
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
    for (index, item) in text.as_ref().split(';').enumerate() {
      let (name, value) = item
        .split_once('=')
        .map_or((item, ""), |(name, value)| (name, value));
      let name = name.trim_matches([' ', '\t']);
      let value = value.trim_matches([' ', '\t']);
      if index == 0 {
        builder.name(name);
        builder.value(value);
        continue;
      }
      match name.to_ascii_lowercase().as_str() {
        "expires" => {
          let value = value.replace("-", " ");
          if let Ok(v) = httpdate::parse_http_date(&value[..]) {
            builder.expires(v);
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
        _ => {}
      }
    }
    Ok(builder.build())
  }
}

#[cfg(test)]
mod tests {
  use super::{Cookie, ToCookie};

  fn assert_no_forbidden_wire_bytes(serialized: &str) {
    assert!(
      !serialized.bytes().any(|byte| {
        byte == b'\r'
          || byte == b'\n'
          || byte == b'\0'
          || byte == 0x7f
          || (byte < 0x20 && byte != b'\t')
          || byte > 0x7e
      }),
      "serialized cookie must not emit forbidden control or non-ASCII bytes: {serialized:?}"
    );
  }

  #[test]
  fn string_preserves_valid_cookie_octets() {
    let cookie = Cookie::builder()
      .name("token")
      .value("a!#$%&'()*+-./:<=>?@[]^_`{|}~")
      .build();

    assert_eq!(cookie.string(), "token=a!#$%&'()*+-./:<=>?@[]^_`{|}~");
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_quotes_safe_printable_values() {
    let cookie = Cookie::builder()
      .name("token")
      .value("abc def")
      .path("/with space")
      .build();

    assert_eq!(cookie.string(), "token=\"abc def\"; path=\"/with space\"");
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_strips_cr_lf_nul_controls_and_invalid_bytes() {
    let cookie = Cookie::builder()
      .name("tok\ren")
      .value("a\nb\0c\"d\\e\u{7f}f\u{00a0}g")
      .path("/p\r\nath")
      .domain("ex\0ample.com")
      .same_site("La\nx")
      .build();

    let serialized = cookie.string();
    assert_eq!(
      serialized,
      "token=abcdefg; path=/path; domain=example.com; SameSite=Lax"
    );
    assert_no_forbidden_wire_bytes(&serialized);
    assert!(!serialized.contains('"'));
    assert!(!serialized.contains('\\'));
  }

  #[test]
  fn string_preserves_parsed_quoted_token_safe_value() {
    let cookie = Cookie::parse(r#"session="abc123""#).unwrap();

    assert_eq!(cookie.value(), r#""abc123""#);
    assert_eq!(cookie.string(), r#"session="abc123""#);
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_preserves_parsed_quoted_value_with_space() {
    let cookie = Cookie::parse(r#"session="abc def""#).unwrap();

    assert_eq!(cookie.value(), r#""abc def""#);
    assert_eq!(cookie.string(), r#"session="abc def""#);
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_preserves_quoted_value_from_str_to_cookie() {
    let cookie = r#"session="abc def""#.to_cookie().unwrap();

    assert_eq!(cookie.string(), r#"session="abc def""#);
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_quotes_remaining_safe_printables_after_stripping_forbidden_bytes() {
    let cookie = Cookie::builder().name("token").value("ab\nc def").build();

    assert_eq!(cookie.string(), r#"token="abc def""#);
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn string_round_trips_parsed_valid_cookie_wire_form() {
    let cookie = Cookie::parse(
      "session=abc123; Path=/app; Domain=example.com; Secure; HttpOnly; SameSite=Lax",
    )
    .unwrap();

    assert_eq!(
      cookie.string(),
      "session=abc123; path=/app; domain=example.com; secure; httpOnly; SameSite=Lax"
    );
    assert_eq!(cookie.name(), "session");
    assert_eq!(cookie.value(), "abc123");
    assert_eq!(cookie.path().as_deref(), Some("/app"));
    assert_eq!(cookie.domain().as_deref(), Some("example.com"));
    assert!(cookie.secure());
    assert!(cookie.http_only());
    assert_eq!(cookie.same_site().as_deref(), Some("Lax"));
    assert_no_forbidden_wire_bytes(&cookie.string());
  }

  #[test]
  fn parse_preserves_embedded_equals_in_cookie_value() {
    let cookie = Cookie::parse("token=a=b=c").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "a=b=c");
  }

  #[test]
  fn parse_keeps_valueless_cookie_attributes_as_flags() {
    let cookie = Cookie::parse("token=value; Secure; HttpOnly; HostOnly").unwrap();

    assert!(cookie.secure());
    assert!(cookie.http_only());
    assert!(cookie.host_only());
  }

  #[test]
  fn parse_ignores_unknown_attribute_before_recognized_attributes() {
    let cookie = Cookie::parse("token=value; Unknown=discard; Path=/; Secure").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
    assert_eq!(cookie.path().as_deref(), Some("/"));
    assert!(cookie.secure());
  }

  #[test]
  fn parse_ignores_unknown_attribute_between_recognized_attributes() {
    let cookie = Cookie::parse("token=value; Path=/; Unknown=discard; HttpOnly").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
    assert_eq!(cookie.path().as_deref(), Some("/"));
    assert!(cookie.http_only());
  }

  #[test]
  fn parse_ignores_unknown_attribute_after_recognized_attributes() {
    let cookie = Cookie::parse("token=value; Secure; Unknown").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
    assert!(cookie.secure());
  }

  #[test]
  fn parse_malformed_expires_keeps_cookie_without_persistence() {
    let cookie = Cookie::parse("token=value; Expires=not-a-date; Path=/; Secure").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
    assert!(cookie.expires().is_none());
    assert!(!cookie.persistent());
    assert_eq!(cookie.path().as_deref(), Some("/"));
    assert!(cookie.secure());
  }

  #[test]
  fn parse_valid_imf_fixdate_expires_sets_persistence() {
    let cookie =
      Cookie::parse("token=value; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Path=/").unwrap();

    assert!(cookie.expires().is_some());
    assert!(cookie.persistent());
    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
  }

  #[test]
  fn parse_only_trims_http_whitespace() {
    let cookie =
      Cookie::parse("token=\u{00a0}value\u{00a0};\u{000b}Path=/;\u{000c}Secure").unwrap();

    assert_eq!(cookie.value(), "\u{00a0}value\u{00a0}");
    assert!(cookie.path().is_none());
    assert!(!cookie.secure());
  }

  #[test]
  fn parse_trims_space_and_horizontal_tab() {
    let cookie = Cookie::parse("\t token \t=\tvalue\t;\t Path\t=\t/\t;\t Secure\t").unwrap();

    assert_eq!(cookie.name(), "token");
    assert_eq!(cookie.value(), "value");
    assert_eq!(cookie.path().as_deref(), Some("/"));
    assert!(cookie.secure());
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
    fmt::Debug::fmt(self, formatter)
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
