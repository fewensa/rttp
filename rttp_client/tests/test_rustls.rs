use std::io::{Read, stdout, Write};
use std::net::ToSocketAddrs;
use std::sync::Arc;

#[cfg(feature = "tls-rustls")]
use rustls;
#[cfg(feature = "tls-rustls")]
use rustls::Session;
#[cfg(feature = "tls-rustls")]
use socket2::{Domain, Protocol, Socket, Type};
#[cfg(feature = "tls-rustls")]
use webpki;
#[cfg(feature = "tls-rustls")]
use webpki_roots;

#[test]
#[cfg(feature = "tls-rustls")]
fn test_rustls() {
  let mut config = rustls::ClientConfig::new();
  config.root_store.add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

  let dns_name = webpki::DNSNameRef::try_from_ascii_str("bing.com").unwrap();
  let mut sess = rustls::ClientSession::new(&Arc::new(config), dns_name);
  let socket_addr = "bing.com:443"
    .to_socket_addrs()
    .unwrap()
    .next()
    .expect("resolve address");
  let domain = Domain::for_address(socket_addr);
  let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP)).unwrap();
  socket.connect(&socket_addr.into()).unwrap();
  let mut sock = socket.into_tcp_stream();
  let mut tls = rustls::Stream::new(&mut sess, &mut sock);
  tls.write(concat!("GET / HTTP/1.1\r\n",
  "Host: bing.com\r\n",
  "Connection: close\r\n",
  "Accept-Encoding: identity\r\n",
  "\r\n")
    .as_bytes())
    .unwrap();
  let ciphersuite = tls.sess.get_negotiated_ciphersuite().unwrap();
//  writeln!(&mut std::io::stderr(), "Current ciphersuite: {:?}", ciphersuite.suite).unwrap();
  let mut plaintext = Vec::new();
  tls.read_to_end(&mut plaintext).unwrap();
//  stdout().write_all(&plaintext).unwrap();
  let text = String::from_utf8(plaintext).unwrap();
  println!("{}", text);
}
