use std::{io, net::ToSocketAddrs, time};

use socket2::{Domain, Protocol, Socket, Type};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

use url::Url;

use crate::connection::connection_reader::ConnectionReader;
use crate::request::{RawRequest, RequestBody};
use crate::types::{Proxy, RoUrl, ToUrl};
use crate::{error, Config};

#[cfg(feature = "tls-rustls")]
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
#[cfg(feature = "tls-rustls")]
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
#[cfg(feature = "tls-rustls")]
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, SignatureScheme, StreamOwned};

#[cfg(feature = "tls-rustls")]
#[derive(Debug)]
struct NoCertificateVerification;

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
  pub fn request(&self) -> &RawRequest {
    &self.request
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

    if let Some(username) = proxy.username() {
      let auth = if let Some(password) = proxy.password() {
        format!("{}:{}", username, password)
      } else {
        format!("{}:", username)
      };
      let auth = STANDARD.encode(auth.as_bytes());
      proxy_header.push_str(&format!("Authorization: Basic {}\r\n", auth));
    }

    proxy_header.push_str("\r\n");
    Ok(proxy_header)
  }
}

impl<'a> Connection<'a> {
  pub fn block_tcp_stream(&self, addr: &String) -> error::Result<std::net::TcpStream> {
    let config = self.config();
    let timeout_read = time::Duration::from_millis(config.read_timeout());
    let timeout_write = time::Duration::from_millis(config.write_timeout());
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

      let stream = std::net::TcpStream::from(socket);
      return Ok(stream);
    }

    Err(error::request(last_err.unwrap_or_else(|| {
      io::Error::new(io::ErrorKind::Other, "failed to connect")
    })))
  }

  pub fn block_write_stream<S>(&self, stream: &mut S) -> error::Result<()>
  where
    S: io::Write,
  {
    let header = self.header();
    let body = self.body();

    // println!("{}", header);
    // if let Some(body) = body {
    //   println!("\n\n");
    //   let content_type = self
    //     .content_type()
    //     .map(|v| v.to_lowercase())
    //     .unwrap_or("".to_string());
    //   let mut raw_types = vec![
    //     "application/x-www-form-urlencoded",
    //     "application/json",
    //     "text/plain",
    //   ];
    //   raw_types.retain(|item| content_type.contains(item));
    //   if raw_types.is_empty() {
    //   } else {
    //     let body_text = String::from_utf8(body.bytes().to_vec()).map_err(error::request)?;
    //     println!("{}", body_text);
    //   }
    // }

    stream.write(header.as_bytes()).map_err(error::request)?;
    if let Some(body) = body {
      stream.write(body.bytes()).map_err(error::request)?;
    }
    stream.flush().map_err(error::request)?;

    Ok(())
  }

  pub fn block_read_stream<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read,
  {
    let mut reader = ConnectionReader::new(url, stream);
    reader.binary()
  }

  pub fn block_send(&self, url: &Url) -> error::Result<Vec<u8>> {
    let addr = self.addr(url)?;
    let mut stream = self.block_tcp_stream(&addr)?;
    self.block_send_with_stream(url, &mut stream)
  }

  pub fn block_send_with_stream<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read + io::Write,
  {
    match url.scheme() {
      "http" => self.block_send_http(url, stream),
      "https" => self.block_send_https(url, stream),
      _ => return Err(error::url_bad_scheme(url.clone())),
    }
  }

  pub fn block_send_http<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read + io::Write,
  {
    self.block_write_stream(stream)?;
    self.block_read_stream(url, stream)
  }

  #[cfg(not(any(feature = "tls-native", feature = "tls-rustls")))]
  pub fn block_send_https<S>(&self, _url: &Url, _stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read + io::Write,
  {
    return Err(error::no_request_features(
      "Not have any tls features, Can't request a https url",
    ));
  }

  #[cfg(any(feature = "tls-native", feature = "tls-rustls"))]
  pub fn block_send_https<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read + io::Write,
  {
    #[cfg(all(feature = "tls-native", feature = "tls-rustls"))]
    {
      return self.block_send_https_rustls(url, stream);
    }
    #[cfg(all(feature = "tls-native", not(feature = "tls-rustls")))]
    {
      return self.block_send_https_native(url, stream);
    }
    #[cfg(all(feature = "tls-rustls", not(feature = "tls-native")))]
    {
      return self.block_send_https_rustls(url, stream);
    }
  }

  #[cfg(feature = "tls-native")]
  fn block_send_https_native<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
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
    self.block_read_stream(url, &mut ssl_stream)
  }

  #[cfg(feature = "tls-rustls")]
  fn block_send_https_rustls<S>(&self, url: &Url, stream: &mut S) -> error::Result<Vec<u8>>
  where
    S: io::Read + io::Write,
  {
    let config = self.config();
    let mut root_store = RootCertStore::empty();
    if config.verify_ssl_cert() {
      root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let builder = ClientConfig::builder();
    let rustls_config = if config.verify_ssl_cert() {
      builder.with_root_certificates(root_store).with_no_client_auth()
    } else {
      builder
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
    };
    let rc_config = Arc::new(rustls_config);
    let host = self.host(url)?;
    let server_name = ServerName::try_from(host.as_str())
      .map_err(|_| error::bad_ssl(format!("Invalid server name: {}", host)))?
      .to_owned();
    let client =
      ClientConnection::new(rc_config, server_name).map_err(|e| error::bad_ssl(e.to_string()))?;
    let mut tls = StreamOwned::new(client, stream);

    self.block_write_stream(&mut tls)?;
    self.block_read_stream(url, &mut tls)
  }
}
