use std::io::Write;

use socks::{Socks4Stream, Socks5Stream};
use url::Url;

use crate::connection::connection::{
  Connection, ExpectContinueResult, HandoffConnection, HandoffKind, StreamingRequestBody,
};
use crate::connection::connection_reader::ResponseParts;
use crate::error;
use crate::request::RawRequest;
use crate::response::Response;
use crate::types::{Proxy, ProxyType};

pub struct BlockConnection<'a> {
  conn: Connection<'a>,
}

impl<'a> BlockConnection<'a> {
  pub fn new(request: RawRequest<'a>) -> Self {
    Self {
      conn: Connection::new(request),
    }
  }

  pub fn call(mut self) -> error::Result<Response> {
    loop {
      let url = self.conn.url().map_err(error::builder)?;
      let proxy = self.conn.proxy().clone();
      let parts = if let Some(proxy) = proxy.as_ref() {
        self.call_with_proxy(&url, proxy)?
      } else {
        self.conn.block_send_parts(&url)?
      };

      let response =
        Response::with_trailers(self.conn.rourl().clone(), parts.binary, parts.trailers)?;
      let config = self.conn.config().clone();

      if let Some(location) = response.location() {
        let req_url = url.as_str();
        if req_url == location {
          return Err(error::loop_detected(url));
        }
        if !config.auto_redirect() {
          self.conn.closed_set(true);
          return Ok(response);
        }
        let count = self.conn.count();
        if count > config.max_redirect() {
          return Err(error::too_many_redirects(url));
        }

        let redirect_url = self.conn.resolve_redirect_url(&url, location)?;
        if url == redirect_url.url {
          return Err(error::loop_detected(url));
        }
        let strip_sensitive_headers = !self.conn.is_same_origin_url(&url, &redirect_url.url);
        self.conn.request_mut().redirect_status_set(response.code());
        self.conn.request_mut().redirect_url_set(
          redirect_url.url.to_string(),
          strip_sensitive_headers,
          Some(&redirect_url.request_target),
        )?;
        self.conn.request_mut().origin_mut().count_set(count + 1);
        continue;
      }

      self.conn.closed_set(true);
      return Ok(response);
    }
  }

  pub fn call_streaming_body(mut self, body: StreamingRequestBody<'_>) -> error::Result<Response> {
    if self.conn.proxy().is_some() {
      return Err(error::builder_with_message(
        "streaming request bodies do not support proxies",
      ));
    }

    let url = self.conn.url().map_err(error::builder)?;
    let parts = self.conn.block_send_streaming_parts(&url, body)?;
    let response =
      Response::with_trailers(self.conn.rourl().clone(), parts.binary, parts.trailers)?;
    self.conn.closed_set(true);
    Ok(response)
  }

  pub fn call_connect_handoff(mut self) -> error::Result<HandoffConnection> {
    if self.conn.proxy().is_some() {
      return Err(error::builder_with_message(
        "CONNECT socket handoff does not support proxies",
      ));
    }
    let url = self.conn.url().map_err(error::builder)?;
    let handoff = self.conn.block_send_handoff(&url, HandoffKind::Connect)?;
    self.conn.closed_set(true);
    Ok(handoff)
  }

  pub fn call_upgrade_handoff(mut self) -> error::Result<HandoffConnection> {
    if self.conn.proxy().is_some() {
      return Err(error::builder_with_message(
        "Upgrade socket handoff does not support proxies",
      ));
    }
    let url = self.conn.url().map_err(error::builder)?;
    let handoff = self.conn.block_send_handoff(&url, HandoffKind::Upgrade)?;
    self.conn.closed_set(true);
    Ok(handoff)
  }
}

// proxy connection
impl<'a> BlockConnection<'a> {
  fn call_with_proxy(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    match proxy.type_() {
      ProxyType::HTTP => {
        if url.scheme() == "http" {
          self.call_with_proxy_http(url, proxy)
        } else {
          self.call_with_proxy_https(url, proxy)
        }
      }
      ProxyType::HTTPS => self.call_with_proxy_https(url, proxy),
      ProxyType::SOCKS4 => self.call_with_proxy_socks4(url, proxy),
      ProxyType::SOCKS5 => self.call_with_proxy_socks5(url, proxy),
    }
  }

  fn call_with_proxy_http(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.conn.block_tcp_stream(&addr)?;
    let header = self.conn.proxy_http_header(url, proxy);

    match self
      .conn
      .block_send_expect_continue_parts_with_header(&mut stream, &header)?
    {
      ExpectContinueResult::NotUsed => {
        stream
          .write_all(header.as_bytes())
          .map_err(error::request)?;
        if let Some(body) = self.conn.body() {
          stream.write_all(body.bytes()).map_err(error::request)?;
        }
        stream.flush().map_err(error::request)?;
      }
      ExpectContinueResult::BodySent => {}
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }

    self.conn.block_read_stream_parts(url, &mut stream)
  }

  fn call_with_proxy_https(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    //CONNECT proxy.google.com:443 HTTP/1.1
    //Host: www.google.com:443
    //Proxy-Connection: keep-alive
    let connect_header = self.conn.proxy_header(url, proxy)?;

    let addr = format!("{}:{}", proxy.host(), proxy.port());
    let mut stream = self.conn.block_tcp_stream(&addr)?;

    stream
      .write_all(connect_header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)?;
    crate::connection::connection::read_proxy_connect_response(&mut stream)?;

    self.conn.block_send_with_stream_parts(url, &mut stream)
  }

  fn call_with_proxy_socks4(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    // Keep the `socks` crate for SOCKS handshakes: it owns the proxy connection setup and
    // returns a stream that already satisfies the shared `Read + Write` send path.
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.conn.addr(url)?;
    let user = if let Some(u) = proxy.username() {
      u.to_string()
    } else {
      "".to_string()
    };
    let mut stream = Socks4Stream::connect(&addr_proxy[..], &addr_target[..], &user[..])
      .map_err(error::request)?;
    self.conn.block_send_with_stream_parts(url, &mut stream)
  }

  fn call_with_proxy_socks5(&self, url: &Url, proxy: &Proxy) -> error::Result<ResponseParts> {
    // Reimplementing SOCKS on top of socket2 would duplicate protocol logic without changing
    // how the rest of the client reads and writes the established stream.
    let addr_proxy = format!("{}:{}", proxy.host(), proxy.port());
    let addr_target = self.conn.addr(url)?;
    let mut stream = if let Some(u) = proxy.username() {
      if let Some(p) = proxy.password() {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], &p[..])
      } else {
        Socks5Stream::connect_with_password(&addr_proxy[..], &addr_target[..], &u[..], "")
      }
    } else {
      Socks5Stream::connect(&addr_proxy[..], &addr_target[..])
    }
    .map_err(error::request)?;
    self.conn.block_send_with_stream_parts(url, &mut stream)
  }
}
