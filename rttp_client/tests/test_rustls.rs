#[cfg(feature = "tls-rustls")]
mod support;

#[cfg(feature = "tls-rustls")]
use std::io::{Read, Write};
#[cfg(feature = "tls-rustls")]
use std::net::TcpStream;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

#[cfg(feature = "tls-rustls")]
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
#[cfg(feature = "tls-rustls")]
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
#[cfg(feature = "tls-rustls")]
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};

#[test]
#[cfg(feature = "tls-rustls")]
fn test_rustls() {
  let (addr, _handle) = support::spawn_tls_server();
  let config = ClientConfig::builder()
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
    .with_no_client_auth();

  let dns_name = ServerName::try_from("localhost").unwrap().to_owned();
  let sess = ClientConnection::new(Arc::new(config), dns_name).unwrap();
  let sock = TcpStream::connect(addr).unwrap();
  let mut tls = StreamOwned::new(sess, sock);
  tls
    .write_all(
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
  let _ciphersuite = tls.conn.negotiated_cipher_suite().unwrap();
  let mut plaintext = Vec::new();
  tls.read_to_end(&mut plaintext).unwrap();
  let text = String::from_utf8(plaintext).unwrap();
  assert!(text.contains("200 OK"));
}

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
