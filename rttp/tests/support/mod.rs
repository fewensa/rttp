#[path = "../../../tests/support/local_http.rs"]
mod local_http;

use std::io::Write;
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};

pub use local_http::spawn_ok_http_server as spawn_http_server;

pub fn spawn_tls_server() -> (SocketAddr, JoinHandle<()>) {
  use rcgen::generate_simple_self_signed;
  use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
  use rustls::{ServerConfig, ServerConnection, StreamOwned};
  use std::sync::Arc;

  let cert = generate_simple_self_signed(vec!["localhost".to_string()]).expect("generate cert");
  let cert_der = cert.serialize_der().expect("cert der");
  let key_der = cert.serialize_private_key_der();

  let config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(
      vec![CertificateDer::from(cert_der)],
      PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_der)),
    )
    .expect("set cert");
  let config = Arc::new(config);

  let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls server");
  let addr = listener.local_addr().expect("tls addr");
  let handle = thread::spawn(move || {
    if let Ok((stream, _)) = listener.accept() {
      let session = ServerConnection::new(config.clone()).expect("server connection");
      let mut tls = StreamOwned::new(session, stream);
      let _ = local_http::read_http_request(&mut tls);
      let _ = tls.write_all(local_http::HTTP_OK_RESPONSE);
      let _ = tls.flush();
      tls.conn.send_close_notify();
      let _ = tls.flush();
    }
  });
  (addr, handle)
}
