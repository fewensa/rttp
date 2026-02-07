use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};

pub fn spawn_http_server() -> (SocketAddr, JoinHandle<()>) {
  spawn_http_server_count(1)
}

pub fn spawn_http_server_count(count: usize) -> (SocketAddr, JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind http server");
  let addr = listener.local_addr().expect("local addr");
  let handle = thread::spawn(move || {
    for _ in 0..count {
      if let Ok((mut stream, _)) = listener.accept() {
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
        let _ = stream.write_all(response);
      }
    }
  });
  (addr, handle)
}

pub fn spawn_redirect_server() -> (SocketAddr, JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect server");
  let addr = listener.local_addr().expect("redirect addr");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let mut buf = [0u8; 1024];
      let _ = stream.read(&mut buf);
      let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        addr
      );
      let _ = stream.write_all(response.as_bytes());
    }

    if let Ok((mut stream, _)) = listener.accept() {
      let mut buf = [0u8; 1024];
      let _ = stream.read(&mut buf);
      let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
      let _ = stream.write_all(response);
    }
  });
  (addr, handle)
}

#[cfg(feature = "tls-rustls")]
pub fn spawn_tls_server() -> (SocketAddr, JoinHandle<()>) {
  use rcgen::generate_simple_self_signed;
  use rustls::{NoClientAuth, ServerConfig, ServerSession, Stream};
  use std::sync::Arc;

  let cert = generate_simple_self_signed(vec!["localhost".to_string()]).expect("generate cert");
  let cert_der = cert.serialize_der().expect("cert der");
  let key_der = cert.serialize_private_key_der();

  let mut config = ServerConfig::new(NoClientAuth::new());
  config
    .set_single_cert(vec![rustls::Certificate(cert_der)], rustls::PrivateKey(key_der))
    .expect("set cert");
  let config = Arc::new(config);

  let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls server");
  let addr = listener.local_addr().expect("tls addr");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let mut session = ServerSession::new(&config);
      let mut tls = Stream::new(&mut session, &mut stream);
      let mut buf = [0u8; 1024];
      let _ = tls.read(&mut buf);
      let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
      let _ = tls.write_all(response);
      let _ = tls.flush();
    }
  });
  (addr, handle)
}
