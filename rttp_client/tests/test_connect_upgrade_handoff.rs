use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use rttp_client::HttpClient;

fn read_request_head(stream: &mut impl Read) -> Vec<u8> {
  let mut request = Vec::new();
  let mut byte = [0u8; 1];
  while !request.ends_with(b"\r\n\r\n") {
    stream.read_exact(&mut byte).expect("read request byte");
    request.push(byte[0]);
  }
  request
}

#[test]
fn connect_returns_socket_after_successful_tunnel_response() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind tunnel server");
  let addr = listener.local_addr().expect("tunnel server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept tunnel");
    let request = read_request_head(&mut stream);
    let request = String::from_utf8(request).expect("request utf8");
    assert!(request.starts_with(&format!("CONNECT {} HTTP/1.1\r\n", addr)));
    stream
      .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
      .expect("write connect response");
    let mut ping = [0u8; 4];
    stream.read_exact(&mut ping).expect("read tunnel payload");
    assert_eq!(b"ping", &ping);
    stream.write_all(b"pong").expect("write tunnel payload");
  });

  let mut tunnel = HttpClient::new()
    .url(format!("http://{}", addr))
    .connect()
    .expect("establish tunnel");

  assert_eq!(200, tunnel.response().code());
  tunnel.stream_mut().write_all(b"ping").expect("write ping");
  let mut pong = [0u8; 4];
  tunnel
    .stream_mut()
    .read_exact(&mut pong)
    .expect("read pong");
  assert_eq!(b"pong", &pong);

  handle.join().expect("server thread");
}

#[test]
fn upgrade_returns_socket_after_101_and_does_not_parse_upgraded_bytes() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let request = String::from_utf8(read_request_head(&mut stream)).expect("request utf8");
    assert!(request.starts_with("GET /chat HTTP/1.1\r\n"));
    assert!(request.contains("\r\nConnection: Upgrade\r\n"));
    assert!(request.contains("\r\nUpgrade: websocket\r\n"));
    stream
      .write_all(
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\nserver-bytes",
      )
      .expect("write upgrade response and bytes");
    let mut client_bytes = [0u8; 12];
    stream
      .read_exact(&mut client_bytes)
      .expect("read upgraded client bytes");
    assert_eq!(b"client-bytes", &client_bytes);
  });

  let mut upgraded = HttpClient::new()
    .url(format!("http://{}/chat", addr))
    .header(("Connection", "Upgrade"))
    .header(("Upgrade", "websocket"))
    .upgrade()
    .expect("upgrade connection");

  assert_eq!(101, upgraded.response().code());
  assert_eq!(
    Some(&"websocket".to_string()),
    upgraded.response().header_value("Upgrade")
  );
  let mut server_bytes = [0u8; 12];
  upgraded
    .stream_mut()
    .read_exact(&mut server_bytes)
    .expect("read upgraded server bytes");
  assert_eq!(b"server-bytes", &server_bytes);
  upgraded
    .stream_mut()
    .write_all(b"client-bytes")
    .expect("write upgraded client bytes");

  handle.join().expect("server thread");
}

#[test]
fn failed_upgrade_reads_http_response_and_closes_socket() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed upgrade server");
  let addr = listener.local_addr().expect("failed upgrade server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept failed upgrade");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(
        b"HTTP/1.1 426 Upgrade Required\r\nContent-Length: 16\r\nConnection: close\r\n\r\nupgrade required",
      )
      .expect("write failed upgrade response");
    let mut extra = [0u8; 1];
    assert_eq!(0, stream.read(&mut extra).expect("client should close"));
  });

  let err = HttpClient::new()
    .url(format!("http://{}/chat", addr))
    .header(("Connection", "Upgrade"))
    .header(("Upgrade", "websocket"))
    .upgrade()
    .expect_err("non-101 upgrade must fail");

  assert!(err
    .to_string()
    .contains("Upgrade failed with HTTP status 426"));

  handle.join().expect("server thread");
}
