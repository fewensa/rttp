use std::collections::HashMap;
use std::fmt;

use crate::error;

#[derive(Clone, Eq, PartialEq)]
pub struct Header {
  name: String,
  value: String,
}

#[allow(clippy::wrong_self_convention)]
pub trait IntoHeader {
  fn into_headers(&self) -> Vec<Header>;
}

impl Header {
  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().into(),
      value: value.as_ref().into(),
    }
  }

  pub(crate) fn validate_outbound(&self) -> error::Result<()> {
    if !is_http_token(&self.name) {
      return Err(error::builder_with_message(
        "Invalid outbound HTTP header name",
      ));
    }
    if !self.value.bytes().all(is_header_value_byte) {
      return Err(error::builder_with_message(
        "Invalid outbound HTTP header value",
      ));
    }
    Ok(())
  }

  pub(crate) fn from_http1<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().trim_matches([' ', '\t']).into(),
      value: value.as_ref().trim_matches([' ', '\t']).into(),
    }
  }

  pub(crate) fn replace(&mut self, header: Header) -> &mut Self {
    self.name = header.name().clone();
    self.value = header.value().clone();
    self
  }

  pub fn name(&self) -> &String {
    &self.name
  }

  pub fn value(&self) -> &String {
    &self.value
  }

  pub fn value_as_isize(&self) -> Result<isize, std::num::ParseIntError> {
    self.value.parse()
  }

  pub fn value_as_usize(&self) -> Result<usize, std::num::ParseIntError> {
    self.value.parse()
  }
}

impl fmt::Debug for Header {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Header")
      .field("name", &self.name)
      .field("value", &debug_header_value(&self.name, &self.value))
      .finish()
  }
}

fn debug_header_value<'a>(name: &str, value: &'a str) -> DebugHeaderValue<'a> {
  if is_sensitive_debug_header(name) {
    DebugHeaderValue::Redacted
  } else {
    DebugHeaderValue::Visible(value)
  }
}

enum DebugHeaderValue<'a> {
  Redacted,
  Visible(&'a str),
}

impl fmt::Debug for DebugHeaderValue<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Redacted => formatter.write_str("\"[REDACTED]\""),
      Self::Visible(value) => fmt::Debug::fmt(value, formatter),
    }
  }
}

fn is_sensitive_debug_header(name: &str) -> bool {
  name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("cookie")
    || name.eq_ignore_ascii_case("idempotency-key")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("set-cookie")
}

impl IntoHeader for &str {
  fn into_headers(&self) -> Vec<Header> {
    let header = self.split_once(':').map_or_else(
      || Header::new(*self, ""),
      |(name, value)| Header::new(name, value.trim_matches([' ', '\t'])),
    );
    vec![header]
  }
}

impl IntoHeader for String {
  fn into_headers(&self) -> Vec<Header> {
    (&self[..]).into_headers()
  }
}

impl IntoHeader for Header {
  fn into_headers(&self) -> Vec<Header> {
    vec![self.clone()]
  }
}

impl<K: AsRef<str> + Eq + std::hash::Hash, V: AsRef<str>> IntoHeader for HashMap<K, V> {
  fn into_headers(&self) -> Vec<Header> {
    let mut rets = Vec::with_capacity(self.len());
    for key in self.keys() {
      if let Some(value) = self.get(key) {
        rets.push(Header::new(key, value))
      }
    }
    rets
  }
}

impl<IU: IntoHeader> IntoHeader for &IU {
  fn into_headers(&self) -> Vec<Header> {
    (*self).into_headers()
  }
}

impl<IU: IntoHeader> IntoHeader for &mut IU {
  fn into_headers(&self) -> Vec<Header> {
    (**self).into_headers()
  }
}

macro_rules! replace_expr {
  ($_t:tt $sub:ty) => {
    $sub
  };
}

macro_rules! tuple_to_header {
  ( $( $item:ident )+ ) => {
    impl<T: IntoHeader> IntoHeader for (
      $(replace_expr!(
        ($item)
        T
      ),)+
    )
    {
      fn into_headers(&self) -> Vec<Header> {
        let mut rets = vec![];
        let ($($item,)+) = self;
        let mut _name = "".to_string();
        let mut _position = 0;
        $(
          let headers = $item.into_headers();
          if !headers.is_empty() {

            if headers.len() > 1 ||
              headers.get(0).filter(|&v| !v.value().is_empty()).is_some()
            {
              rets.extend(headers);
              _position = 0;
            } else {
              if let Some(first) = headers.get(0) {
                if _position == 0 {
                  _name = first.name().clone();
                  _position = 1;
                } else {
                  rets.push(Header::new(&_name, first.name()));
                  _position = 0;
                }
              }
            }
          }
        )+
        rets
      }
    }
  };
}

tuple_to_header! { a }
tuple_to_header! { a b }
tuple_to_header! { a b c }
tuple_to_header! { a b c d }
tuple_to_header! { a b c d e }
tuple_to_header! { a b c d e f }
tuple_to_header! { a b c d e f g }
tuple_to_header! { a b c d e f g h }
tuple_to_header! { a b c d e f g h i }
tuple_to_header! { a b c d e f g h i j }
tuple_to_header! { a b c d e f g h i j k }
tuple_to_header! { a b c d e f g h i j k l }
tuple_to_header! { a b c d e f g h i j k l m }
tuple_to_header! { a b c d e f g h i j k l m n }
tuple_to_header! { a b c d e f g h i j k l m n o }
tuple_to_header! { a b c d e f g h i j k l m n o p }
tuple_to_header! { a b c d e f g h i j k l m n o p q }
tuple_to_header! { a b c d e f g h i j k l m n o p q r }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x y }
tuple_to_header! { a b c d e f g h i j k l m n o p q r s t u v w x y z }

fn is_http_token(value: &str) -> bool {
  !value.is_empty()
    && value.bytes().all(|byte| {
      byte.is_ascii_alphanumeric()
        || matches!(
          byte,
          b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
        )
    })
}

fn is_header_value_byte(byte: u8) -> bool {
  byte == b'\t' || byte == b' ' || (0x21..=0x7e).contains(&byte) || byte >= 0x80
}

#[cfg(test)]
mod tests {
  use super::Header;

  #[test]
  fn debug_redacts_sensitive_header_values() {
    for (name, secret) in [
      ("Authorization", "Bearer origin-token"),
      ("Proxy-Authorization", "Basic cHJveHk6c2VjcmV0"),
      ("Cookie", "session=private"),
      ("Set-Cookie", "session=private"),
      ("Idempotency-Key", "charge-2026-08-19-9f3c"),
    ] {
      let debug = format!("{:?}", Header::new(name, secret));
      assert!(debug.contains(name));
      assert!(debug.contains("[REDACTED]"));
      assert!(!debug.contains(secret));
    }
  }

  #[test]
  fn debug_preserves_non_sensitive_header_values() {
    let debug = format!("{:?}", Header::new("Accept", "application/json"));
    assert!(debug.contains("Accept"));
    assert!(debug.contains("application/json"));
  }
}
