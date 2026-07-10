use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

use rttp::server::{HttpHandoff, HttpResponse};
use rttp_client::HttpClient;

#[test]
fn connect_authority_form_hands_off_socket_after_response_boundary() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("CONNECT", request.method());
        assert_eq!("example.com:443", request.target());
        HttpHandoff::connect(
          HttpResponse::new(200, "Connection Established"),
          |mut stream| {
            let mut ping = [0u8; 4];
            stream.read_exact(&mut ping)?;
            assert_eq!(b"ping", &ping);
            stream.write_all(b"pong")?;
            Ok(())
          },
        )
      })
      .expect("serve connect handoff");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\nping")
    .expect("write connect request");

  let mut response_and_tunnel = Vec::new();
  let mut buffer = [0u8; 128];
  loop {
    let read = stream.read(&mut buffer).expect("read tunnel response");
    assert_ne!(0, read);
    response_and_tunnel.extend_from_slice(&buffer[..read]);
    if response_and_tunnel.ends_with(b"pong") {
      break;
    }
  }

  assert_eq!(
    b"HTTP/1.1 200 Connection Established\r\n\r\npong",
    response_and_tunnel.as_slice()
  );

  handle.join().expect("server thread");
}

#[test]
fn upgrade_request_hands_off_socket_and_preserves_buffered_bytes() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|request| {
        assert_eq!("GET", request.method());
        assert_eq!(Some("websocket"), request.header("Upgrade"));
        HttpHandoff::upgrade(
          HttpResponse::new(101, "Switching Protocols")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket"),
          |mut stream| {
            let mut hello = [0u8; 5];
            stream.read_exact(&mut hello)?;
            assert_eq!(b"hello", &hello);
            stream.write_all(b"world")?;
            Ok(())
          },
        )
      })
      .expect("serve upgrade handoff");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      b"GET /chat HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive, Upgrade\r\nUpgrade: websocket\r\n\r\nhello",
    )
    .expect("write upgrade request");

  let mut response_and_upgraded = Vec::new();
  let mut buffer = [0u8; 128];
  loop {
    let read = stream.read(&mut buffer).expect("read upgrade response");
    assert_ne!(0, read);
    response_and_upgraded.extend_from_slice(&buffer[..read]);
    if response_and_upgraded.ends_with(b"world") {
      break;
    }
  }

  assert_eq!(
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Connection: Upgrade\r\n",
      "Upgrade: websocket\r\n",
      "\r\n",
      "world"
    )
    .as_bytes(),
    response_and_upgraded.as_slice()
  );

  handle.join().expect("server thread");
}

#[test]
fn rttp_client_upgrade_interoperates_with_socket2_handoff_matrix() {
  for (path, protocol, server_bytes, client_bytes) in [
    (
      "/chat",
      "websocket",
      b"server-websocket".as_slice(),
      b"client-websocket".as_slice(),
    ),
    (
      "/events",
      "event-stream",
      b"server-events".as_slice(),
      b"client-events".as_slice(),
    ),
  ] {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");

    let handle = thread::spawn(move || {
      server
        .accept_one_handoff(|request| {
          assert_eq!("GET", request.method());
          assert_eq!(path, request.target());
          assert_eq!(Some(protocol), request.header("Upgrade"));
          assert!(request
            .header("Connection")
            .expect("Connection header")
            .eq_ignore_ascii_case("Upgrade"));

          HttpHandoff::upgrade(
            HttpResponse::new(101, "Switching Protocols")
              .header("Connection", "Upgrade")
              .header("Upgrade", protocol),
            move |mut stream| {
              stream.write_all(server_bytes)?;

              let mut received = vec![0; client_bytes.len()];
              stream.read_exact(&mut received)?;
              assert_eq!(client_bytes, received.as_slice());

              Ok(())
            },
          )
        })
        .expect("serve upgrade handoff");
    });

    let mut upgraded = HttpClient::new()
      .url(format!("http://{}{}", addr, path))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", protocol))
      .upgrade()
      .expect("upgrade connection");

    assert_eq!(101, upgraded.response().code());
    assert_eq!(
      Some(&protocol.to_string()),
      upgraded.response().header_value("Upgrade")
    );

    let mut received = vec![0; server_bytes.len()];
    upgraded
      .stream_mut()
      .read_exact(&mut received)
      .expect("read upgraded server bytes");
    assert_eq!(server_bytes, received.as_slice());

    upgraded
      .stream_mut()
      .write_all(client_bytes)
      .expect("write upgraded client bytes");

    handle.join().expect("server thread");
  }
}

#[test]
fn invalid_connect_handoff_request_returns_bad_request_without_handoff() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one_handoff(|_request| {
        HttpHandoff::connect(
          HttpResponse::new(200, "Connection Established"),
          |_stream| panic!("invalid CONNECT target must not be handed off"),
        )
      })
      .expect("serve invalid connect");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(b"CONNECT /not-authority HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .expect("write invalid connect request");
  stream
    .shutdown(std::net::Shutdown::Write)
    .expect("shutdown client write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert_eq!(
    "HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\nConnection: close\r\n\r\nBad Request",
    response
  );

  handle.join().expect("server thread");
}
