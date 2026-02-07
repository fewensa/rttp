use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

#[cfg(feature = "tls-rustls")]
use rcgen;
#[cfg(feature = "tls-rustls")]
use rustls;
#[cfg(feature = "tls-rustls")]
use webpki;

// Requires the `tls-rustls` feature. Example:
// cargo test -p rttp_client --features tls-rustls
#[test]
#[cfg(feature = "tls-rustls")]
fn test_rustls() {
  let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
  let cert_der = cert.serialize_der().unwrap();
  let key_der = cert.serialize_private_key_der();

  let mut server_config = rustls::ServerConfig::new(rustls::NoClientAuth::new());
  server_config
    .set_single_cert(
      vec![rustls::Certificate(cert_der.clone())],
      rustls::PrivateKey(key_der),
    )
    .unwrap();

  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let addr = listener.local_addr().unwrap();
  let server_config = Arc::new(server_config);
  let server_handle = std::thread::spawn(move || {
    if let Ok((mut tcp, _)) = listener.accept() {
      let mut sess = rustls::ServerSession::new(&server_config);
      let mut tls = rustls::Stream::new(&mut sess, &mut tcp);
      let mut buf = [0u8; 1024];
      let _ = tls.read(&mut buf);
      tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
        .unwrap();
      tls.flush().unwrap();
    }
  });

  let mut client_config = rustls::ClientConfig::new();
  client_config
    .root_store
    .add(&rustls::Certificate(cert_der))
    .unwrap();
  let dns_name = webpki::DNSNameRef::try_from_ascii_str("localhost").unwrap();
  let mut sess = rustls::ClientSession::new(&Arc::new(client_config), dns_name);
  let mut sock = TcpStream::connect(addr).unwrap();
  let mut tls = rustls::Stream::new(&mut sess, &mut sock);
  tls.write_all(
    concat!(
      "GET / HTTP/1.1\r\n",
      "Host: localhost\r\n",
      "Connection: close\r\n",
      "\r\n"
    )
    .as_bytes(),
  )
  .unwrap();
  tls.flush().unwrap();
  let mut plaintext = Vec::new();
  tls.read_to_end(&mut plaintext).unwrap();
  server_handle.join().unwrap();
  let text = String::from_utf8(plaintext).unwrap();
  assert!(text.contains("200 OK"));
}
