use std::{io, net::TcpStream, net::ToSocketAddrs, time};

use socket2::{Domain, Protocol, Socket, Type};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use url::Url;

use crate::connection::connection_reader::{
  is_skippable_informational_status, parse_informational_response, read_response_head,
  read_response_header, read_response_parts_after_header,
  read_response_parts_after_header_with_informational_and_limit, response_status_code,
  ConnectionReader, ResponseParts, MAX_RESPONSE_HEAD_BYTES,
};
use crate::request::{RawRequest, RequestBody};
use crate::response::{InformationalResponse, Response};
use crate::types::{Header, Proxy, RoUrl, ToUrl};
use crate::{error, Config};

#[cfg(feature = "tls-rustls")]
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
#[cfg(feature = "tls-rustls")]
use rustls::client::WebPkiServerVerifier;
#[cfg(feature = "tls-rustls")]
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
#[cfg(feature = "tls-rustls")]
use rustls::{
  CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore,
  SignatureScheme, StreamOwned,
};

#[cfg(feature = "tls-rustls")]
#[derive(Debug)]
pub(crate) struct NoCertificateVerification;

#[cfg(feature = "tls-rustls")]
impl ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    _end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    Ok(ServerCertVerified::assertion())
  }

  fn verify_tls12_signature(
    &self,
    _message: &[u8],
    _cert: &CertificateDer<'_>,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    Ok(HandshakeSignatureValid::assertion())
  }

  fn verify_tls13_signature(
    &self,
    _message: &[u8],
    _cert: &CertificateDer<'_>,
    _dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    Ok(HandshakeSignatureValid::assertion())
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    vec![
      SignatureScheme::RSA_PKCS1_SHA1,
      SignatureScheme::RSA_PKCS1_SHA256,
      SignatureScheme::RSA_PKCS1_SHA384,
      SignatureScheme::RSA_PKCS1_SHA512,
      SignatureScheme::ECDSA_NISTP256_SHA256,
      SignatureScheme::ECDSA_NISTP384_SHA384,
      SignatureScheme::ECDSA_NISTP521_SHA512,
      SignatureScheme::RSA_PSS_SHA256,
      SignatureScheme::RSA_PSS_SHA384,
      SignatureScheme::RSA_PSS_SHA512,
      SignatureScheme::ED25519,
    ]
  }
}

#[cfg(feature = "tls-rustls")]
#[derive(Debug)]
pub(crate) struct NoHostnameVerification {
  verifier: Arc<WebPkiServerVerifier>,
}

#[cfg(feature = "tls-rustls")]
impl NoHostnameVerification {
  pub(crate) fn new(verifier: Arc<WebPkiServerVerifier>) -> Self {
    Self { verifier }
  }
}

#[cfg(feature = "tls-rustls")]
impl ServerCertVerifier for NoHostnameVerification {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    server_name: &ServerName<'_>,
    ocsp_response: &[u8],
    now: UnixTime,
  ) -> Result<ServerCertVerified, rustls::Error> {
    match self.verifier.verify_server_cert(
      end_entity,
      intermediates,
      server_name,
      ocsp_response,
      now,
    ) {
      Err(rustls::Error::InvalidCertificate(CertificateError::NotValidForName)) => {
        Ok(ServerCertVerified::assertion())
      }
      result => result,
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.verifier.verify_tls12_signature(message, cert, dss)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, rustls::Error> {
    self.verifier.verify_tls13_signature(message, cert, dss)
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self.verifier.supported_verify_schemes()
  }
}

pub struct Connection<'a> {
  request: RawRequest<'a>,
}

#[derive(Debug)]
pub struct HandoffConnection {
  response: Response,
  stream: TcpStream,
}

impl HandoffConnection {
  pub(crate) fn new(response: Response, stream: TcpStream) -> Self {
    Self { response, stream }
  }

  pub fn response(&self) -> &Response {
    &self.response
  }

  pub fn stream(&self) -> &TcpStream {
    &self.stream
  }

  pub fn stream_mut(&mut self) -> &mut TcpStream {
    &mut self.stream
  }

  pub fn into_parts(self) -> (Response, TcpStream) {
    (self.response, self.stream)
  }
}

pub(crate) struct RedirectUrl {
  pub(crate) url: Url,
  pub(crate) request_target: String,
}

impl<'a> Connection<'a> {
  pub fn new(request: RawRequest<'a>) -> Connection<'a> {
    Self { request }
  }
}

#[allow(dead_code)]
impl<'a> Connection<'a> {
  pub fn request(&self) -> &RawRequest<'_> {
    &self.request
  }
  pub fn request_mut(&mut self) -> &mut RawRequest<'a> {
    &mut self.request
  }
  pub fn rourl(&self) -> &RoUrl {
    self.request.url()
  }
  pub fn url(&self) -> error::Result<Url> {
    self.request.url().to_url().map_err(error::builder)
  }
  pub fn header(&self) -> &String {
    self.request.header()
  }
  pub fn content_type(&self) -> Option<String> {
    self.request.content_type()
  }
  pub fn body(&self) -> &Option<RequestBody> {
    self.request.body()
  }
  pub fn proxy(&self) -> &Option<Proxy> {
    self.request.origin().proxy()
  }
  pub fn config(&self) -> &Config {
    self.request.origin().config()
  }
  pub fn count(&self) -> u32 {
    self.request.origin().count()
  }

  pub fn closed_set(&mut self, closed: bool) {
    self.request.origin_mut().closed_set(closed);
  }
}

impl<'a> Connection<'a> {
  pub fn addr(&self, url: &Url) -> error::Result<String> {
    let host = self.host(url)?;
    let port = self.port(url)?;
    Ok(format!("{}:{}", host, port))
  }

  pub fn host(&self, url: &Url) -> error::Result<String> {
    Ok(
      url
        .host_str()
        .ok_or(error::url_bad_host(url.clone()))?
        .to_string(),
    )
  }

  pub fn port(&self, url: &Url) -> error::Result<u16> {
    url
      .port_or_known_default()
      .ok_or(error::url_bad_host(url.clone()))
  }

  pub fn proxy_header(&self, url: &Url, proxy: &Proxy) -> error::Result<String> {
    let host = self.host(url)?;
    let port = self.port(url)?;

    //CONNECT proxy.google.com:443 HTTP/1.1
    //Host: www.google.com:443
    //Proxy-Connection: keep-alive
    let mut proxy_header = String::new();
    proxy_header.push_str(&format!("CONNECT {}:{} HTTP/1.1\r\n", host, port));
    proxy_header.push_str(&format!("Host: {}:{}\r\n", host, port));
    append_proxy_authorization_header(&mut proxy_header, proxy);

    proxy_header.push_str("\r\n");
    Ok(proxy_header)
  }

  pub fn proxy_http_header(&self, url: &Url, proxy: &Proxy) -> String {
    let header = self.header();
    let (_, rest) = header.split_once("\r\n").unwrap_or(("", ""));
    let request_target = self
      .request
      .request_target()
      .map(|target| format!("{}{}", &url[..url::Position::BeforePath], target))
      .unwrap_or_else(|| absolute_url(url));
    let mut proxy_header = format!(
      "{} {} HTTP/1.1\r\n{}",
      self.request.origin().method().to_uppercase(),
      request_target,
      rest
    );
    append_proxy_authorization_header(&mut proxy_header, proxy);
    proxy_header
  }

  pub fn resolve_redirect_url(&self, url: &Url, location: &str) -> error::Result<RedirectUrl> {
    let mut redirect = url
      .join(location)
      .map_err(|_| error::bad_url(url.clone(), "Bad redirect location"))?;
    strip_userinfo_for_cross_origin_redirect(url, &mut redirect);
    let (path, query) = raw_redirect_path_and_query(url, self.request.request_target(), location)
      .unwrap_or_else(|| {
        (
          redirect.path().to_string(),
          redirect.query().map(str::to_string),
        )
      });
    redirect.set_path(&path);
    redirect.set_query(query.as_deref());

    let mut request_target = path;
    if let Some(query) = query {
      request_target.push('?');
      request_target.push_str(&query);
    }

    Ok(RedirectUrl {
      url: redirect,
      request_target,
    })
  }

  pub fn is_same_origin_url(&self, url: &Url, redirect: &Url) -> bool {
    is_same_origin(url, redirect)
  }

  pub fn expect_no_response_body(&self) -> bool {
    self.request.origin().method().eq_ignore_ascii_case("head")
      || self
        .request
        .origin()
        .method()
        .eq_ignore_ascii_case("connect")
  }
}

fn absolute_url(url: &Url) -> String {
  let mut absolute = url.clone();
  absolute.set_fragment(None);
  absolute.to_string()
}

fn raw_redirect_path_and_query(
  base: &Url,
  base_request_target: Option<&str>,
  location: &str,
) -> Option<(String, Option<String>)> {
  let (base_path, base_query) = base_request_target
    .and_then(raw_request_target_path_and_query)
    .unwrap_or_else(|| (base.path(), base.query()));
  let location = location.trim();
  let location = location
    .split_once('#')
    .map_or(location, |(before, _)| before);
  if location.is_empty() {
    return Some((base_path.to_string(), base_query.map(str::to_string)));
  }
  if let Some(rest) = location.strip_prefix("//") {
    return Some(raw_path_and_query_after_authority(rest));
  }
  if let Some(rest) = strip_absolute_url_scheme(location) {
    if let Some(rest) = rest.strip_prefix("//") {
      return Some(raw_path_and_query_after_authority(rest));
    }
    return None;
  }
  if let Some(query) = location.strip_prefix('?') {
    return Some((base_path.to_string(), Some(query.to_string())));
  }

  let (path, query) = split_path_and_query(location);
  if path.starts_with('/') {
    return Some((remove_literal_dot_segments(path), query.map(str::to_string)));
  }

  let mut merged = base_path_directory(base_path);
  merged.push_str(path);
  Some((
    remove_literal_dot_segments(&merged),
    query.map(str::to_string),
  ))
}

fn raw_request_target_path_and_query(target: &str) -> Option<(&str, Option<&str>)> {
  if target.starts_with('/') {
    return Some(split_path_and_query(target));
  }
  None
}

fn strip_absolute_url_scheme(location: &str) -> Option<&str> {
  let scheme_end = location.find(':')?;
  let scheme = &location[..scheme_end];
  if scheme.is_empty() || !scheme.as_bytes()[0].is_ascii_alphabetic() {
    return None;
  }
  if !scheme
    .bytes()
    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
  {
    return None;
  }
  Some(&location[scheme_end + 1..])
}

fn raw_path_and_query_after_authority(rest: &str) -> (String, Option<String>) {
  let path_start = rest.find(['/', '?', '#']).unwrap_or(rest.len());
  let path_and_query = &rest[path_start..];
  if path_and_query.is_empty() {
    return ("/".to_string(), None);
  }
  if let Some(query) = path_and_query.strip_prefix('?') {
    return ("/".to_string(), Some(query.to_string()));
  }
  let (path, query) = split_path_and_query(path_and_query);
  (remove_literal_dot_segments(path), query.map(str::to_string))
}

fn split_path_and_query(value: &str) -> (&str, Option<&str>) {
  value
    .split_once('?')
    .map_or((value, None), |(path, query)| (path, Some(query)))
}

fn base_path_directory(path: &str) -> String {
  let directory_end = path.rfind('/').map_or(0, |index| index + 1);
  path[..directory_end].to_string()
}

fn remove_literal_dot_segments(path: &str) -> String {
  let raw_segments: Vec<&str> = path.split('/').collect();
  let mut segments: Vec<&str> = Vec::new();
  for (index, segment) in raw_segments.iter().enumerate() {
    let is_last = index + 1 == raw_segments.len();
    match *segment {
      "." => {
        if is_last {
          segments.push("");
        }
      }
      ".." => {
        if segments.last().is_some_and(|last| !last.is_empty()) {
          segments.pop();
        }
        if is_last {
          segments.push("");
        }
      }
      _ => segments.push(segment),
    }
  }
  let normalized = segments.join("/");
  if normalized.is_empty() {
    "/".to_string()
  } else {
    normalized
  }
}

fn is_same_origin(left: &Url, right: &Url) -> bool {
  left.scheme() == right.scheme()
    && left.host_str() == right.host_str()
    && left.port_or_known_default() == right.port_or_known_default()
}

fn strip_userinfo_for_cross_origin_redirect(base: &Url, redirect: &mut Url) {
  if is_same_origin(base, redirect) {
    return;
  }

  let _ = redirect.set_username("");
  let _ = redirect.set_password(None);
}

fn proxy_authorization_value(proxy: &Proxy) -> Option<String> {
  proxy.username().as_ref().map(|username| {
    let auth = if let Some(password) = proxy.password() {
      format!("{}:{}", username, password)
    } else {
      format!("{}:", username)
    };
    STANDARD.encode(auth.as_bytes())
  })
}

fn append_proxy_authorization_header(header: &mut String, proxy: &Proxy) {
  if let Some(auth) = proxy_authorization_value(proxy) {
    header.push_str(&format!("Proxy-Authorization: Basic {}\r\n", auth));
  }
}

fn write_http_request<W>(
  stream: &mut W,
  header: &str,
  body: Option<&RequestBody>,
) -> error::Result<()>
where
  W: io::Write,
{
  stream
    .write_all(header.as_bytes())
    .map_err(error::request)?;
  if let Some(body) = body {
    stream.write_all(body.bytes()).map_err(error::request)?;
  }
  stream.flush().map_err(error::request)?;
  Ok(())
}

pub(crate) fn request_expects_continue(header: &str, body: Option<&RequestBody>) -> bool {
  let _ = (header, body);
  false
}

fn response_header_has_upgrade(header: &[u8]) -> error::Result<bool> {
  let header = String::from_utf8(header.to_vec()).map_err(error::response)?;
  let mut has_upgrade_header = false;
  let mut connection_has_upgrade = false;

  for line in header.lines().skip(1) {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };
    if name.eq_ignore_ascii_case("Upgrade") && !value.trim().is_empty() {
      has_upgrade_header = true;
    }
    if name.eq_ignore_ascii_case("Connection")
      && value
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
    {
      connection_has_upgrade = true;
    }
  }

  Ok(has_upgrade_header && connection_has_upgrade)
}

pub(crate) enum ExpectContinueResult {
  NotUsed,
  BodySent(Vec<InformationalResponse>),
  Final(ResponseParts),
}

pub(crate) fn prepend_informational_responses(
  mut parts: ResponseParts,
  mut informational_responses: Vec<InformationalResponse>,
) -> ResponseParts {
  if informational_responses.is_empty() {
    return parts;
  }
  informational_responses.extend(parts.informational_responses);
  parts.informational_responses = informational_responses;
  parts
}

pub(crate) enum HandoffKind {
  Connect,
  Upgrade,
}

pub(crate) enum StreamingRequestBody<'a> {
  Fixed {
    reader: &'a mut dyn io::Read,
    content_length: u64,
  },
  Chunked {
    reader: &'a mut dyn io::Read,
    trailers: &'a [Header],
  },
}

pub(crate) fn parse_proxy_connect_response(header: &[u8]) -> error::Result<()> {
  let header = String::from_utf8(header.to_vec())
    .map_err(|_| error::bad_proxy("parse proxy server response error."))?;
  let status_line = header
    .lines()
    .next()
    .ok_or_else(|| error::bad_proxy("Proxy server response error."))?;
  let status_code = status_line
    .split_whitespace()
    .nth(1)
    .ok_or_else(|| error::bad_proxy("Proxy server response error."))?
    .parse::<u16>()
    .map_err(|_| error::bad_proxy("parse proxy server response error."))?;

  if status_code == 200 {
    Ok(())
  } else {
    Err(error::bad_proxy(format!(
      "Proxy server response error: {}",
      status_line
    )))
  }
}

pub(crate) fn read_proxy_connect_response<R>(reader: &mut R) -> error::Result<()>
where
  R: io::Read,
{
  let mut header = Vec::new();
  let mut informational_responses = 0;
  let mut byte = [0u8; 1];

  loop {
    if header.len() == MAX_RESPONSE_HEAD_BYTES {
      return Err(error::bad_proxy("Proxy response head is too large"));
    }
    let read = reader.read(&mut byte).map_err(error::request)?;
    if read == 0 {
      if header.is_empty() {
        return Err(error::bad_proxy("Proxy server response error."));
      }
      return Err(error::bad_proxy("Incomplete proxy response headers"));
    }

    header.push(byte[0]);
    if header.ends_with(b"\r\n\r\n") {
      let status_code = proxy_connect_response_status_code(&header)?;
      if is_skippable_informational_status(status_code) {
        if informational_responses == MAX_PROXY_CONNECT_INFORMATIONAL_RESPONSES {
          return Err(error::bad_proxy("Too many informational proxy responses"));
        }
        informational_responses += 1;
        header.clear();
        continue;
      }
      return parse_proxy_connect_response(&header);
    }
  }
}

pub(crate) const MAX_PROXY_CONNECT_INFORMATIONAL_RESPONSES: usize = 16;

fn proxy_connect_response_status_code(header: &[u8]) -> error::Result<u16> {
  let header = String::from_utf8(header.to_vec())
    .map_err(|_| error::bad_proxy("parse proxy server response error."))?;
  header
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(1))
    .ok_or_else(|| error::bad_proxy("Proxy server response error."))?
    .parse::<u16>()
    .map_err(|_| error::bad_proxy("parse proxy server response error."))
}

pub(crate) fn connect_tcp_stream<A>(addr: A, config: &Config) -> error::Result<std::net::TcpStream>
where
  A: ToSocketAddrs,
{
  let timeout_read = tcp_timeout_duration("read", config.read_timeout())?;
  let timeout_write = tcp_timeout_duration("write", config.write_timeout())?;
  connect_tcp_stream_with_io_timeouts(addr, config, timeout_read, timeout_write)
}

pub(crate) fn connect_tcp_stream_with_io_timeouts<A>(
  addr: A,
  config: &Config,
  timeout_read: time::Duration,
  timeout_write: time::Duration,
) -> error::Result<std::net::TcpStream>
where
  A: ToSocketAddrs,
{
  let timeout_connect = tcp_connect_timeout_duration(config.connect_timeout())?;
  let mut last_err = None;

  let addrs = addr.to_socket_addrs().map_err(error::request)?;
  for addr in addrs {
    let domain = Domain::for_address(addr);
    let socket = match Socket::new(domain, Type::STREAM, Some(Protocol::TCP)) {
      Ok(socket) => socket,
      Err(err) => {
        last_err = Some(err);
        continue;
      }
    };

    if let Err(err) = socket.set_read_timeout(Some(timeout_read)) {
      last_err = Some(err);
      continue;
    }
    if let Err(err) = socket.set_write_timeout(Some(timeout_write)) {
      last_err = Some(err);
      continue;
    }

    if let Err(err) = socket.connect_timeout(&addr.into(), timeout_connect) {
      last_err = Some(err);
      continue;
    }

    return Ok(std::net::TcpStream::from(socket));
  }

  Err(error::connect(
    last_err.unwrap_or_else(|| io::Error::other("failed to connect")),
  ))
}

fn tcp_connect_timeout_duration(millis: u64) -> error::Result<time::Duration> {
  if millis == 0 {
    return Err(error::request(io::Error::new(
      io::ErrorKind::InvalidInput,
      "connect timeout must be greater than 0",
    )));
  }
  tcp_timeout_duration("connect", millis)
}

fn tcp_timeout_duration(name: &str, millis: u64) -> error::Result<time::Duration> {
  if millis > i64::MAX as u64 {
    return Err(error::request(io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("{} timeout is too large", name),
    )));
  }
  Ok(time::Duration::from_millis(millis))
}

impl<'a> Connection<'a> {
  pub fn block_tcp_stream(&self, addr: &String) -> error::Result<std::net::TcpStream> {
    connect_tcp_stream(addr, self.config())
  }

  pub fn block_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: io::Write,
  {
    write_http_request(stream, self.header(), self.body().as_ref())
  }

  pub(crate) fn block_write_request_header_with<S>(
    &self,
    stream: &mut S,
    header: &str,
  ) -> error::Result<()>
  where
    S: io::Write,
  {
    stream
      .write_all(header.as_bytes())
      .map_err(error::request)?;
    stream.flush().map_err(error::request)
  }

  fn block_write_request_body<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: io::Write,
  {
    if let Some(body) = self.body() {
      stream.write_all(body.bytes()).map_err(error::request)?;
    }
    stream.flush().map_err(error::request)
  }

  fn block_write_streaming_request<S>(
    &self,
    stream: &mut S,
    mut body: StreamingRequestBody<'_>,
  ) -> error::Result<()>
  where
    S: io::Write,
  {
    stream
      .write_all(self.header().as_bytes())
      .map_err(error::request)?;
    match &mut body {
      StreamingRequestBody::Fixed {
        reader,
        content_length,
      } => write_fixed_streaming_body(stream, *reader, *content_length)?,
      StreamingRequestBody::Chunked { reader, trailers } => {
        write_chunked_streaming_body(stream, *reader, trailers)?
      }
    }
    stream.flush().map_err(error::request)
  }

  pub(crate) fn block_read_stream_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read,
  {
    let mut reader = ConnectionReader::new_with_limit(
      url,
      stream,
      self.expect_no_response_body(),
      self.config().max_buffered_response_body_bytes(),
    );
    reader.response_parts()
  }

  pub(crate) fn block_send_expect_continue_parts<S>(
    &self,
    stream: &mut S,
  ) -> error::Result<ExpectContinueResult>
  where
    S: io::Read + io::Write,
  {
    self.block_send_expect_continue_parts_with_header(stream, self.header())
  }

  pub(crate) fn block_send_expect_continue_parts_with_header<S>(
    &self,
    stream: &mut S,
    header: &str,
  ) -> error::Result<ExpectContinueResult>
  where
    S: io::Read + io::Write,
  {
    if !request_expects_continue(header, self.body().as_ref()) {
      return Ok(ExpectContinueResult::NotUsed);
    }

    self.block_write_request_header_with(stream, header)?;
    let mut informational_responses = Vec::new();
    loop {
      let header = read_response_header(stream)?;
      let status_code = response_status_code(&header)?;
      if status_code == 100 {
        informational_responses.push(parse_informational_response(&header)?);
        self.block_write_request_body(stream)?;
        return Ok(ExpectContinueResult::BodySent(informational_responses));
      }
      if is_skippable_informational_status(status_code) {
        informational_responses.push(parse_informational_response(&header)?);
        continue;
      }
      return read_response_parts_after_header_with_informational_and_limit(
        stream,
        self.expect_no_response_body(),
        header,
        informational_responses,
        self.config().max_buffered_response_body_bytes(),
      )
      .map(ExpectContinueResult::Final);
    }
  }

  pub(crate) fn block_send_parts(&self, url: &Url) -> error::Result<ResponseParts> {
    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
    self.block_send_with_stream_parts(url, &mut stream)
  }

  pub(crate) fn block_send_handoff(
    &self,
    url: &Url,
    kind: HandoffKind,
  ) -> error::Result<HandoffConnection> {
    if url.scheme() != "http" {
      return Err(error::builder_with_message(
        "socket handoff only supports plain http URLs",
      ));
    }

    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
    self.block_write_stream(&mut stream)?;

    let header = read_response_head(&mut stream)?;
    let status_code = response_status_code(&header)?;
    match kind {
      HandoffKind::Connect if (200..300).contains(&status_code) => {
        let response = Response::with_trailers(self.rourl().clone(), header, Vec::new())?;
        Ok(HandoffConnection::new(response, stream))
      }
      HandoffKind::Upgrade if status_code == 101 && response_header_has_upgrade(&header)? => {
        let response = Response::with_trailers(self.rourl().clone(), header, Vec::new())?;
        Ok(HandoffConnection::new(response, stream))
      }
      HandoffKind::Connect => {
        let _ = read_response_parts_after_header(&mut stream, false, header)?;
        Err(error::bad_response(format!(
          "CONNECT failed with HTTP status {}",
          status_code
        )))
      }
      HandoffKind::Upgrade => {
        let _ = read_response_parts_after_header(&mut stream, false, header)?;
        Err(error::bad_response(format!(
          "Upgrade failed with HTTP status {}",
          status_code
        )))
      }
    }
  }

  pub(crate) fn block_send_streaming_parts(
    &self,
    url: &Url,
    body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts> {
    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
    self.block_send_streaming_with_stream_parts(url, &mut stream, body)
  }

  fn block_send_streaming_with_stream_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
    body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    match url.scheme() {
      "http" => {
        self.block_write_streaming_request(stream, body)?;
        self.block_read_stream_parts(url, stream)
      }
      "https" => self.block_send_https_streaming_parts(url, stream, body),
      _ => Err(error::url_bad_scheme(url.clone())),
    }
  }

  pub(crate) fn block_send_with_stream_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    match url.scheme() {
      "http" => self.block_send_http_parts(url, stream),
      "https" => self.block_send_https_parts(url, stream),
      _ => Err(error::url_bad_scheme(url.clone())),
    }
  }

  pub(crate) fn block_send_http_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    match self.block_send_expect_continue_parts(stream)? {
      ExpectContinueResult::NotUsed => self.block_write_stream(stream)?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .block_read_stream_parts(url, stream)
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.block_read_stream_parts(url, stream)
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  pub(crate) fn block_send_https_parts<S>(
    &self,
    _url: &Url,
    _stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    Err(error::no_request_features(
      "Not have any tls features, Can't request a https url",
    ))
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  fn block_send_https_streaming_parts<S>(
    &self,
    _url: &Url,
    _stream: &mut S,
    _body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    Err(error::no_request_features(
      "Not have any tls features, Can't request a https url",
    ))
  }

  #[cfg(any(feature = "tls-native", feature = "tls-rustls"))]
  pub(crate) fn block_send_https_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    #[cfg(all(feature = "tls-native", feature = "tls-rustls"))]
    {
      self.block_send_https_rustls_parts(url, stream)
    }
    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      return self.block_send_https_native_parts(url, stream);
    }
    #[cfg(all(feature = "tls-rustls", not(feature = "tls-native")))]
    {
      return self.block_send_https_rustls_parts(url, stream);
    }
  }

  #[cfg(any(feature = "tls-native", feature = "tls-rustls"))]
  fn block_send_https_streaming_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
    body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    #[cfg(all(feature = "tls-native", feature = "tls-rustls"))]
    {
      self.block_send_https_rustls_streaming_parts(url, stream, body)
    }
    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      return self.block_send_https_native_streaming_parts(url, stream, body);
    }
    #[cfg(all(feature = "tls-rustls", not(feature = "tls-native")))]
    {
      return self.block_send_https_rustls_streaming_parts(url, stream, body);
    }
  }

  #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
  fn block_send_https_native_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    let config = self.config();
    let connector = native_tls::TlsConnector::builder()
      .danger_accept_invalid_certs(!config.verify_ssl_cert())
      .danger_accept_invalid_hostnames(!config.verify_ssl_hostname())
      .build()
      .map_err(error::request)?;
    let mut ssl_stream = connector
      .connect(&self.host(url)?[..], stream)
      .map_err(native_tls_handshake_error)?;

    match self.block_send_expect_continue_parts(&mut ssl_stream)? {
      ExpectContinueResult::NotUsed => self.block_write_stream(&mut ssl_stream)?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .block_read_stream_parts(url, &mut ssl_stream)
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.block_read_stream_parts(url, &mut ssl_stream)
  }

  #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
  fn block_send_https_native_streaming_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
    body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    let config = self.config();
    let connector = native_tls::TlsConnector::builder()
      .danger_accept_invalid_certs(!config.verify_ssl_cert())
      .danger_accept_invalid_hostnames(!config.verify_ssl_hostname())
      .build()
      .map_err(error::request)?;
    let mut ssl_stream = connector
      .connect(&self.host(url)?[..], stream)
      .map_err(native_tls_handshake_error)?;

    self.block_write_streaming_request(&mut ssl_stream, body)?;
    self.block_read_stream_parts(url, &mut ssl_stream)
  }

  #[cfg(feature = "tls-rustls")]
  fn block_send_https_rustls_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    let config = self.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if !config.verify_ssl_cert() {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
    } else if !config.verify_ssl_hostname() {
      let verifier = WebPkiServerVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(error::bad_ssl)?;
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoHostnameVerification::new(verifier)))
        .with_no_client_auth()
    } else {
      builder
        .with_root_certificates(root_store)
        .with_no_client_auth()
    };
    let rc_config = Arc::new(rustls_config);
    let host = self.host(url)?;
    let server_name = match host.parse::<std::net::IpAddr>() {
      Ok(ip) => ServerName::IpAddress(ip.into()),
      Err(_) => ServerName::try_from(host.as_str())
        .map_err(|_| error::bad_ssl(format!("Invalid server name: {}", host)))?
        .to_owned(),
    };
    let client = ClientConnection::new(rc_config, server_name).map_err(error::bad_ssl)?;
    let mut tls = StreamOwned::new(client, stream);

    match self.block_send_expect_continue_parts(&mut tls)? {
      ExpectContinueResult::NotUsed => self.block_write_stream(&mut tls)?,
      ExpectContinueResult::BodySent(informational_responses) => {
        return self
          .block_read_stream_parts(url, &mut tls)
          .map(|parts| prepend_informational_responses(parts, informational_responses));
      }
      ExpectContinueResult::Final(parts) => return Ok(parts),
    }
    self.block_read_stream_parts(url, &mut tls)
  }

  #[cfg(feature = "tls-rustls")]
  fn block_send_https_rustls_streaming_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
    body: StreamingRequestBody<'_>,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read + io::Write,
  {
    let config = self.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if !config.verify_ssl_cert() {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
    } else if !config.verify_ssl_hostname() {
      let verifier = WebPkiServerVerifier::builder(Arc::new(root_store))
        .build()
        .map_err(error::bad_ssl)?;
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoHostnameVerification::new(verifier)))
        .with_no_client_auth()
    } else {
      builder
        .with_root_certificates(root_store)
        .with_no_client_auth()
    };
    let rc_config = Arc::new(rustls_config);
    let host = self.host(url)?;
    let server_name = match host.parse::<std::net::IpAddr>() {
      Ok(ip) => ServerName::IpAddress(ip.into()),
      Err(_) => ServerName::try_from(host.as_str())
        .map_err(|_| error::bad_ssl(format!("Invalid server name: {}", host)))?
        .to_owned(),
    };
    let client = ClientConnection::new(rc_config, server_name).map_err(error::bad_ssl)?;
    let mut tls = StreamOwned::new(client, stream);

    self.block_write_streaming_request(&mut tls, body)?;
    self.block_read_stream_parts(url, &mut tls)
  }
}

#[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
fn native_tls_handshake_error<S>(error: native_tls::HandshakeError<S>) -> error::Error {
  match error {
    native_tls::HandshakeError::Failure(error) => error::bad_ssl(error),
    native_tls::HandshakeError::WouldBlock(_) => error::bad_ssl(io::Error::new(
      io::ErrorKind::WouldBlock,
      "native TLS handshake would block",
    )),
  }
}

fn write_fixed_streaming_body<W>(
  writer: &mut W,
  reader: &mut dyn io::Read,
  content_length: u64,
) -> error::Result<()>
where
  W: io::Write,
{
  let mut remaining = content_length;
  let mut buffer = [0u8; 8 * 1024];
  while remaining > 0 {
    let limit = buffer.len().min(remaining as usize);
    let read = reader.read(&mut buffer[..limit]).map_err(error::request)?;
    if read == 0 {
      return Err(error::request(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "streaming request body ended before Content-Length",
      )));
    }
    writer.write_all(&buffer[..read]).map_err(error::request)?;
    remaining -= read as u64;
  }
  Ok(())
}

fn write_chunked_streaming_body<W>(
  writer: &mut W,
  reader: &mut dyn io::Read,
  trailers: &[Header],
) -> error::Result<()>
where
  W: io::Write,
{
  let mut buffer = [0u8; 8 * 1024];
  loop {
    let read = reader.read(&mut buffer).map_err(error::request)?;
    if read == 0 {
      write_chunked_trailers(writer, trailers)?;
      return Ok(());
    }
    write!(writer, "{:x}\r\n", read).map_err(error::request)?;
    writer.write_all(&buffer[..read]).map_err(error::request)?;
    writer.write_all(b"\r\n").map_err(error::request)?;
  }
}

fn write_chunked_trailers<W>(writer: &mut W, trailers: &[Header]) -> error::Result<()>
where
  W: io::Write,
{
  writer.write_all(b"0\r\n").map_err(error::request)?;
  for trailer in trailers {
    write!(writer, "{}: {}\r\n", trailer.name(), trailer.value()).map_err(error::request)?;
  }
  writer.write_all(b"\r\n").map_err(error::request)
}

#[cfg(test)]
mod tests {
  use std::io::{self, Cursor, Read, Write};
  use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
  use std::thread;
  use std::time::{Duration, Instant};

  use crate::request::RequestBody;
  use crate::types::Proxy;
  use crate::Config;
  use socket2::{Domain, Protocol, Socket, Type};
  use url::Url;

  use super::{
    connect_tcp_stream, parse_proxy_connect_response, proxy_authorization_value,
    read_proxy_connect_response, strip_userinfo_for_cross_origin_redirect, write_http_request,
    MAX_PROXY_CONNECT_INFORMATIONAL_RESPONSES,
  };
  use crate::connection::connection_reader::MAX_RESPONSE_HEAD_BYTES;

  struct PartialWriter {
    max_chunk: usize,
    written: Vec<u8>,
  }

  impl PartialWriter {
    fn new(max_chunk: usize) -> Self {
      Self {
        max_chunk,
        written: Vec::new(),
      }
    }
  }

  impl Write for PartialWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      let take = buf.len().min(self.max_chunk);
      self.written.extend_from_slice(&buf[..take]);
      Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn test_write_http_request_retries_until_full_payload_is_written() {
    let header = "POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n";
    let body = RequestBody::with_text("hello");
    let mut writer = PartialWriter::new(3);

    write_http_request(&mut writer, header, Some(&body)).unwrap();

    assert_eq!(
      format!("{}hello", header).as_bytes(),
      writer.written.as_slice()
    );
  }

  #[test]
  fn test_proxy_authorization_value_encodes_credentials() {
    let proxy = Proxy::http_with_authorization("127.0.0.1", 8080, "user", "secret");

    assert_eq!(
      Some("dXNlcjpzZWNyZXQ=".to_string()),
      proxy_authorization_value(&proxy)
    );
  }

  #[test]
  fn test_parse_proxy_connect_response_requires_200_status() {
    let header = b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n";
    let err = parse_proxy_connect_response(header).unwrap_err();

    assert!(err
      .to_string()
      .contains("407 Proxy Authentication Required"));
  }

  #[test]
  fn test_read_proxy_connect_response_waits_for_complete_headers() {
    let header = b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: test\r\n\r\n";
    let mut reader = Cursor::new(header);

    read_proxy_connect_response(&mut reader).unwrap();
  }

  #[test]
  fn test_read_proxy_connect_response_skips_interim_headers() {
    let raw = concat!(
      "HTTP/1.1 103 Early Hints\r\n",
      "Link: </proxy.css>; rel=preload\r\n",
      "\r\n",
      "HTTP/1.1 200 Connection Established\r\n",
      "Proxy-Agent: test\r\n",
      "\r\n"
    );
    let mut reader = Cursor::new(raw.as_bytes());

    read_proxy_connect_response(&mut reader).unwrap();
    assert_eq!(raw.len() as u64, reader.position());
  }

  #[test]
  fn test_read_proxy_connect_response_rejects_oversized_head() {
    let raw = format!(
      "HTTP/1.1 200 Connection Established\r\nX-Fill: {}",
      "a".repeat(MAX_RESPONSE_HEAD_BYTES)
    );
    let mut reader = Cursor::new(raw.as_bytes());

    let error = read_proxy_connect_response(&mut reader)
      .expect_err("oversized proxy response head should be rejected");

    assert!(error
      .to_string()
      .contains("Proxy response head is too large"));
    assert_eq!(MAX_RESPONSE_HEAD_BYTES as u64, reader.position());
  }

  #[test]
  fn test_read_proxy_connect_response_rejects_excessive_informational_sequence() {
    let raw =
      "HTTP/1.1 103 Early Hints\r\n\r\n".repeat(MAX_PROXY_CONNECT_INFORMATIONAL_RESPONSES + 1);
    let mut reader = Cursor::new(raw.as_bytes());

    let error = read_proxy_connect_response(&mut reader)
      .expect_err("excessive informational proxy responses should be rejected");

    assert!(error
      .to_string()
      .contains("Too many informational proxy responses"));
  }

  #[test]
  fn test_strip_userinfo_for_cross_origin_redirect_removes_redirect_credentials() {
    let base = Url::parse("http://user:secret@example.test/start").unwrap();
    let mut cross_origin = Url::parse("http://next:secret@other.test/final").unwrap();

    strip_userinfo_for_cross_origin_redirect(&base, &mut cross_origin);

    assert_eq!("", cross_origin.username());
    assert_eq!(None, cross_origin.password());
  }

  #[test]
  fn test_strip_userinfo_for_cross_origin_redirect_preserves_same_origin_credentials() {
    let base = Url::parse("http://user:secret@example.test/start").unwrap();
    let mut same_origin = Url::parse("http://next:secret@example.test/final").unwrap();

    strip_userinfo_for_cross_origin_redirect(&base, &mut same_origin);

    assert_eq!("next", same_origin.username());
    assert_eq!(Some("secret"), same_origin.password());
  }

  #[test]
  fn test_connect_tcp_stream_iterates_ipv4_and_ipv6_addresses_until_connects() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let addrs = [
      SocketAddr::new(Ipv6Addr::LOCALHOST.into(), port),
      SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
    ];
    let server = thread::spawn(move || {
      let (mut stream, _) = listener.accept().unwrap();
      let mut byte = [0];
      stream.read_exact(&mut byte).unwrap();
      byte[0]
    });

    let mut stream = connect_tcp_stream(&addrs[..], &Config::default()).unwrap();
    stream.write_all(&[42]).unwrap();

    assert_eq!(42, server.join().unwrap());
  }

  #[test]
  fn test_connect_tcp_stream_reports_timeout_configuration_errors() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let config = Config::builder()
      .read_timeout(u64::MAX)
      .write_timeout(u64::MAX)
      .build();

    let err = connect_tcp_stream(&[addr][..], &config).unwrap_err();

    assert!(err.to_string().contains("too large"));
  }

  #[test]
  fn test_connect_tcp_stream_rejects_zero_connect_timeout() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let config = Config::builder().connect_timeout(0).build();

    let err = connect_tcp_stream(&[addr][..], &config).unwrap_err();

    assert!(err
      .to_string()
      .contains("connect timeout must be greater than 0"));
  }

  #[test]
  fn test_connect_tcp_stream_rejects_too_large_connect_timeout() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let config = Config::builder().connect_timeout(u64::MAX).build();

    let err = connect_tcp_stream(&[addr][..], &config).unwrap_err();

    assert!(err.to_string().contains("connect timeout is too large"));
  }

  #[test]
  fn test_connect_tcp_stream_times_out() {
    let listener = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
    listener
      .bind(&SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0).into())
      .unwrap();
    listener.listen(1).unwrap();
    let addr = listener.local_addr().unwrap().as_socket().unwrap();
    let mut queued_connections = Vec::new();
    loop {
      let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
      match socket.connect_timeout(&addr.into(), Duration::from_millis(50)) {
        Ok(()) => queued_connections.push(socket),
        Err(err)
          if matches!(
            err.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
          ) =>
        {
          break;
        }
        Err(err) => panic!("failed to saturate local listen queue: {err}"),
      }
    }

    let config = Config::builder().connect_timeout(25).build();
    let started = Instant::now();
    let err = connect_tcp_stream(&[addr][..], &config).unwrap_err();

    assert!(err.is_timeout());
    assert!(started.elapsed() < Duration::from_secs(1));
  }

  #[test]
  fn test_connect_tcp_stream_reports_write_timeout_configuration_errors() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    let config = Config::builder()
      .read_timeout(1000)
      .write_timeout(u64::MAX)
      .build();

    let err = connect_tcp_stream(&[addr][..], &config).unwrap_err();

    assert!(err.to_string().contains("write timeout is too large"));
  }
}
