use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread::{self, JoinHandle};

fn read_http_request<R: Read>(stream: &mut R) -> Vec<u8> {
  let mut request = Vec::new();
  let mut buf = [0u8; 1024];
  let mut content_length = None;

  loop {
    let Ok(read) = stream.read(&mut buf) else {
      break;
    };
    if read == 0 {
      break;
    }

    request.extend_from_slice(&buf[..read]);

    let header_end = request.windows(4).position(|window| window == b"\r\n\r\n");
    if content_length.is_none() {
      if let Some(header_end) = header_end {
        let headers = String::from_utf8_lossy(&request[..header_end + 4]);
        content_length = headers
          .lines()
          .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
              value.trim().parse::<usize>().ok()
            } else {
              None
            }
          })
          .or(Some(0));
      }
    }

    if let (Some(header_end), Some(content_length)) = (header_end, content_length) {
      let expected_len = header_end + 4 + content_length;
      if request.len() >= expected_len {
        break;
      }
    }
  }

  request
}

pub fn spawn_http_server() -> (SocketAddr, JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind http server");
  let addr = listener.local_addr().expect("local addr");
  let handle = thread::spawn(move || {
    if let Ok((mut stream, _)) = listener.accept() {
      let _ = read_http_request(&mut stream);
      let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
      let _ = stream.write_all(response);
    }
  });
  (addr, handle)
}

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
      let _ = read_http_request(&mut tls);
      let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
      let _ = tls.write_all(response);
      let _ = tls.flush();
      tls.conn.send_close_notify();
      let _ = tls.flush();
    }
  });
  (addr, handle)
}
