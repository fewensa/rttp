#[cfg(feature = "tls-rustls")]
mod support;

#[cfg(feature = "tls-rustls")]
use std::io::{Read, Write};
#[cfg(feature = "tls-rustls")]
use std::net::TcpStream;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

#[cfg(feature = "tls-rustls")]
use rustls;
#[cfg(feature = "tls-rustls")]
use rustls::Session;
#[cfg(feature = "tls-rustls")]
use webpki;

#[test]
#[cfg(feature = "tls-rustls")]
fn test_rustls() {
  let (addr, _handle) = support::spawn_tls_server();
  let mut config = rustls::ClientConfig::new();
  config
    .dangerous()
    .set_certificate_verifier(Arc::new(NoCertificateVerification));

  let dns_name = webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();
  let mut sess = rustls::ClientSession::new(&Arc::new(config), dns_name);
  let mut sock = TcpStream::connect(addr).unwrap();
  let mut tls = rustls::Stream::new(&mut sess, &mut sock);
  tls.write_all(
    concat!(
      "GET / HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Connection: close\r\n",
      "Accept-Encoding: identity\r\n",
      "\r\n"
    )
    .as_bytes(),
  )
  .unwrap();
  let _ciphersuite = tls.sess.get_negotiated_ciphersuite().unwrap();
  let mut plaintext = Vec::new();
  tls.read_to_end(&mut plaintext).unwrap();
  let text = String::from_utf8(plaintext).unwrap();
  assert!(text.contains("200 OK"));
}

#[cfg(feature = "tls-rustls")]
struct NoCertificateVerification;

#[cfg(feature = "tls-rustls")]
impl rustls::ServerCertVerifier for NoCertificateVerification {
  fn verify_server_cert(
    &self,
    _roots: &rustls::RootCertStore,
    _presented_certs: &[rustls::Certificate],
    _dns_name: webpki::DNSNameRef,
    _ocsp_response: &[u8],
  ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
    Ok(rustls::ServerCertVerified::assertion())
  }
}
