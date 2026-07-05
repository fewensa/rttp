use std::{io, net::ToSocketAddrs, time};

use socket2::{Domain, Protocol, Socket, Type};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use url::Url;

use crate::connection::connection_reader::{ConnectionReader, ResponseParts};
use crate::request::{RawRequest, RequestBody};
use crate::types::{Proxy, RoUrl, ToUrl};
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
    let mut proxy_header = format!(
      "{} {} HTTP/1.1\r\n{}",
      self.request.origin().method().to_uppercase(),
      absolute_url(url),
      rest
    );
    append_proxy_authorization_header(&mut proxy_header, proxy);
    proxy_header
  }

  pub fn redirect_url(&self, url: &Url, location: &str) -> error::Result<String> {
    url
      .join(location)
      .map(|redirect| redirect.to_string())
      .map_err(|_| error::bad_url(url.clone(), "Bad redirect location"))
  }

  pub fn expect_no_response_body(&self) -> bool {
    self.request.origin().method().eq_ignore_ascii_case("head")
  }
}

fn absolute_url(url: &Url) -> String {
  let mut absolute = url.clone();
  absolute.set_fragment(None);
  absolute.to_string()
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
  let mut byte = [0u8; 1];

  loop {
    let read = reader.read(&mut byte).map_err(error::request)?;
    if read == 0 {
      if header.is_empty() {
        return Err(error::bad_proxy("Proxy server response error."));
      }
      return Err(error::bad_proxy("Incomplete proxy response headers"));
    }

    header.push(byte[0]);
    if header.ends_with(b"\r\n\r\n") {
      return parse_proxy_connect_response(&header);
    }
  }
}

pub(crate) fn connect_tcp_stream<A>(addr: A, config: &Config) -> error::Result<std::net::TcpStream>
where
  A: ToSocketAddrs,
{
  let timeout_read = tcp_timeout_duration("read", config.read_timeout())?;
  let timeout_write = tcp_timeout_duration("write", config.write_timeout())?;
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

    if let Err(err) = socket.connect(&addr.into()) {
      last_err = Some(err);
      continue;
    }

    return Ok(std::net::TcpStream::from(socket));
  }

  Err(error::request(
    last_err.unwrap_or_else(|| io::Error::other("failed to connect")),
  ))
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

  pub(crate) fn block_read_stream_parts<S>(
    &self,
    url: &Url,
    stream: &mut S,
  ) -> error::Result<ResponseParts>
  where
    S: io::Read,
  {
    let mut reader = ConnectionReader::new(url, stream, self.expect_no_response_body());
    reader.response_parts()
  }

  pub(crate) fn block_send_parts(&self, url: &Url) -> error::Result<ResponseParts> {
    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
    self.block_send_with_stream_parts(url, &mut stream)
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
    self.block_write_stream(stream)?;
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
      return self.block_send_https_rustls_parts(url, stream);
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
      .map_err(|_| error::bad_ssl("Native tls handshake error"))?;

    self.block_write_stream(&mut ssl_stream)?;
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
        .map_err(|e| error::bad_ssl(e.to_string()))?;
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
    let client =
      ClientConnection::new(rc_config, server_name).map_err(|e| error::bad_ssl(e.to_string()))?;
    let mut tls = StreamOwned::new(client, stream);

    self.block_write_stream(&mut tls)?;
    self.block_read_stream_parts(url, &mut tls)
  }
}

#[cfg(test)]
mod tests {
  use std::io::{self, Cursor, Read, Write};
  use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
  use std::thread;

  use crate::request::RequestBody;
  use crate::types::Proxy;
  use crate::Config;

  use super::{
    connect_tcp_stream, parse_proxy_connect_response, proxy_authorization_value,
    read_proxy_connect_response, write_http_request,
  };

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
