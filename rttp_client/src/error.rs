#![cfg_attr(target_arch = "wasm32", allow(unused))]
use std::error::Error as StdError;
use std::fmt;
use std::io;

use url::Url;

use crate::types::StatusCode;

/// A `Result` alias where the `Err` case is `rttp_client::Error`.
pub type Result<T> = std::result::Result<T, Error>;

/// The Errors that may occur when processing a `Request`.
pub struct Error {
  inner: Box<Inner>,
}

pub(crate) type BoxError = Box<dyn StdError + Send + Sync>;

struct Inner {
  kind: Kind,
  source: Option<BoxError>,
  url: Option<Url>,
}

impl Error {
  pub(crate) fn new<E>(kind: Kind, source: Option<E>) -> Error
    where
      E: Into<BoxError>,
  {
    Error {
      inner: Box::new(Inner {
        kind,
        source: source.map(Into::into),
        url: None,
      }),
    }
  }

  /// Returns a possible URL related to this error.
  pub fn url(&self) -> Option<&Url> {
    self.inner.url.as_ref()
  }

  /// Returns true if the error is from a type Builder.
  pub fn is_builder(&self) -> bool {
    match self.inner.kind {
      Kind::Builder => true,
      _ => false,
    }
  }

  /// Returns true if the error is from a `RedirectPolicy`.
  pub fn is_redirect(&self) -> bool {
    match self.inner.kind {
      Kind::Redirect => true,
      _ => false,
    }
  }

  /// Returns true if the error is from `Response::error_for_status`.
  pub fn is_status(&self) -> bool {
    match self.inner.kind {
      Kind::Status(_) => true,
      _ => false,
    }
  }

  /// Returns true if the error is related to a timeout.
  pub fn is_timeout(&self) -> bool {
    self.source().map(|e| e.is::<TimedOut>()).unwrap_or(false)
  }

  /// Returns the status code, if the error was generated from a response.
  pub fn status(&self) -> Option<StatusCode> {
    match self.inner.kind {
      Kind::Status(code) => Some(code),
      _ => None,
    }
  }

  // private

  pub(crate) fn with_url(mut self, url: Url) -> Error {
    self.inner.url = Some(url);
    self
  }

  #[allow(unused)]
  pub(crate) fn into_io(self) -> io::Error {
    io::Error::new(io::ErrorKind::Other, self)
  }
}

impl fmt::Debug for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut builder = f.debug_struct("rhttp_client::Error");

    builder.field("kind", &self.inner.kind);

    if let Some(ref url) = self.inner.url {
      builder.field("url", url);
    }
    if let Some(ref source) = self.inner.source {
      builder.field("source", source);
    }

    builder.finish()
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    struct ForUrl<'a>(Option<&'a Url>);

    impl fmt::Display for ForUrl<'_> {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(url) = self.0 {
          write!(f, " for url ({})", url.as_str())
        } else {
          Ok(())
        }
      }
    }

    match self.inner.kind {
      Kind::Builder => f.write_str("builder error")?,
      Kind::Request => f.write_str("error sending request")?,
      Kind::Body => f.write_str("request or response body error")?,
      Kind::Decode => f.write_str("error decoding response body")?,
      Kind::Redirect => f.write_str("error following redirect")?,
      Kind::Status(ref code) => {
        let prefix = if code.is_client_error() {
          "HTTP status client error"
        } else {
          debug_assert!(code.is_server_error());
          "HTTP status server error"
        };
        write!(f, "{} ({})", prefix, code)?;
      }
    };

    ForUrl(self.inner.url.as_ref()).fmt(f)?;

    if let Some(ref e) = self.inner.source {
      write!(f, ": {}", e)?;
    }

    Ok(())
  }
}

impl StdError for Error {
  fn source(&self) -> Option<&(dyn StdError + 'static)> {
    self.inner.source.as_ref().map(|e| &**e as _)
  }
}

#[cfg(target_arch = "wasm32")]
impl From<crate::error::Error> for wasm_bindgen::JsValue {
  fn from(err: Error) -> wasm_bindgen::JsValue {
    js_sys::Error::from(err).into()
  }
}

#[cfg(target_arch = "wasm32")]
impl From<crate::error::Error> for js_sys::Error {
  fn from(err: Error) -> js_sys::Error {
    js_sys::Error::new(&format!("{}", err))
  }
}

#[derive(Debug)]
pub(crate) enum Kind {
  Builder,
  Request,
  Redirect,
  Status(StatusCode),
  Body,
  Decode,
}

// constructors

pub(crate) fn builder<E: Into<BoxError>>(e: E) -> Error {
  Error::new(Kind::Builder, Some(e))
}

pub(crate) fn body<E: Into<BoxError>>(e: E) -> Error {
  Error::new(Kind::Body, Some(e))
}

pub(crate) fn decode<E: Into<BoxError>>(e: E) -> Error {
  Error::new(Kind::Decode, Some(e))
}

pub(crate) fn request<E: Into<BoxError>>(e: E) -> Error {
  Error::new(Kind::Request, Some(e))
}

pub(crate) fn loop_detected(url: Url) -> Error {
  Error::new(Kind::Redirect, Some("infinite redirect loop detected")).with_url(url)
}

pub(crate) fn too_many_redirects(url: Url) -> Error {
  Error::new(Kind::Redirect, Some("too many redirects")).with_url(url)
}

pub(crate) fn status_code(url: Url, status: StatusCode) -> Error {
  Error::new(Kind::Status(status), None::<Error>).with_url(url)
}

pub(crate) fn url_bad_scheme(url: Url) -> Error {
  Error::new(Kind::Builder, Some("URL scheme is not allowed")).with_url(url)
}

pub(crate) fn url_bad_host(url: Url) -> Error {
  Error::new(Kind::Builder, Some("URL host is not allowed")).with_url(url)
}

pub(crate) fn bad_url<S: AsRef<str>>(url: Url, message: S) -> Error {
  Error::new(Kind::Builder, Some(message.as_ref())).with_url(url)
}

pub(crate) fn none_url() -> Error {
  Error::new(Kind::Builder, Some("None request url"))
}

pub(crate) fn builder_with_message<S: AsRef<str>>(message: S) -> Error {
  Error::new(Kind::Builder, Some(message.as_ref()))
}

pub(crate) fn bad_proxy<S: AsRef<str>>(message: S) -> Error {
  Error::new(Kind::Request, Some(message.as_ref()))
}

//if_wasm! {
//    pub(crate) fn wasm(js_val: wasm_bindgen::JsValue) -> BoxError {
//        format!("{:?}", js_val).into()
//    }
//}

// io::Error helpers

#[allow(unused)]
pub(crate) fn into_io(e: Error) -> io::Error {
  e.into_io()
}

#[allow(unused)]
pub(crate) fn decode_io(e: io::Error) -> Error {
  if e.get_ref().map(|r| r.is::<Error>()).unwrap_or(false) {
    *e.into_inner()
      .expect("io::Error::get_ref was Some(_)")
      .downcast::<Error>()
      .expect("StdError::is() was true")
  } else {
    decode(e)
  }
}

// internal Error "sources"

#[derive(Debug)]
pub(crate) struct TimedOut;

#[derive(Debug)]
pub(crate) struct BlockingClientInAsyncContext;

impl fmt::Display for TimedOut {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str("operation timed out")
  }
}

impl StdError for TimedOut {}

impl fmt::Display for BlockingClientInAsyncContext {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str("blocking Client used inside a Future context")
  }
}

impl StdError for BlockingClientInAsyncContext {}

