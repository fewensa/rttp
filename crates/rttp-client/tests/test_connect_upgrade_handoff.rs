use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[cfg(feature = "async")]
use std::net::Shutdown;
#[cfg(feature = "async")]
use std::time::Duration;

#[cfg(feature = "async")]
use async_io::Timer;
#[cfg(feature = "async")]
use futures::executor::block_on;
#[cfg(feature = "async")]
use futures::future::{select, Either};
#[cfg(feature = "async")]
use futures::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(feature = "async")]
use futures::pin_mut;
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
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: \tkeep-alive, \tUpGrAdE\t\r\nUpgrade: \tWebSocket \t\r\n\r\nserver-bytes",
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
    Some(&"WebSocket".to_string()),
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
fn upgrade_rejects_non_ows_connection_upgrade_padding() {
  for padding in ["\u{00a0}", "\u{2003}"] {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
    let addr = listener.local_addr().expect("upgrade server addr");
    let response = format!(
      "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade{padding}\r\nUpgrade: websocket\r\n\r\n"
    );

    let handle = thread::spawn(move || {
      let (mut stream, _) = listener.accept().expect("accept upgrade");
      let _request = read_request_head(&mut stream);
      stream
        .write_all(response.as_bytes())
        .expect("write invalid upgrade response");
    });

    let err = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .upgrade()
      .expect_err("non-OWS Connection: Upgrade padding must fail");

    assert!(
      err
        .to_string()
        .contains("Upgrade failed with HTTP status 101"),
      "unexpected error for padding {padding:?}: {err}"
    );
    handle.join().expect("server thread");
  }
}

#[test]
fn upgrade_rejects_non_ows_upgrade_padding() {
  for upgrade in [
    "\u{00a0}websocket",
    "websocket\u{00a0}",
    "\u{2003}websocket",
    "websocket\u{2003}",
  ] {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
    let addr = listener.local_addr().expect("upgrade server addr");
    let response = format!(
      "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: {upgrade}\r\n\r\n"
    );

    let handle = thread::spawn(move || {
      let (mut stream, _) = listener.accept().expect("accept upgrade");
      let _request = read_request_head(&mut stream);
      stream
        .write_all(response.as_bytes())
        .expect("write invalid upgrade response");
    });

    let err = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .upgrade()
      .expect_err("non-OWS Upgrade padding must fail");

    assert!(
      err
        .to_string()
        .contains("Upgrade failed with HTTP status 101"),
      "unexpected error for Upgrade value {upgrade:?}: {err}"
    );
    handle.join().expect("upgrade server thread");
  }
}

#[test]
fn upgrade_skips_interim_responses_before_101() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let request = String::from_utf8(read_request_head(&mut stream)).expect("request utf8");
    assert!(request.starts_with("GET /chat HTTP/1.1\r\n"));
    stream
      .write_all(
        concat!(
          "HTTP/1.1 103 Early Hints\r\n",
          "Link: </style.css>; rel=preload\r\n",
          "\r\n",
          "HTTP/1.1 101 Switching Protocols\r\n",
          "Connection: Upgrade\r\n",
          "Upgrade: websocket\r\n",
          "\r\n",
          "server-bytes"
        )
        .as_bytes(),
      )
      .expect("write interim and final upgrade responses");
  });

  let mut upgraded = HttpClient::new()
    .url(format!("http://{}/chat", addr))
    .header(("Connection", "Upgrade"))
    .header(("Upgrade", "websocket"))
    .upgrade()
    .expect("upgrade connection");

  assert_eq!(101, upgraded.response().code());
  let mut server_bytes = [0u8; 12];
  upgraded
    .stream_mut()
    .read_exact(&mut server_bytes)
    .expect("read upgraded server bytes");
  assert_eq!(b"server-bytes", &server_bytes);

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

#[test]
#[cfg(feature = "async")]
fn async_connect_returns_socket_after_successful_tunnel_response() {
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

  block_on(async {
    let mut tunnel = HttpClient::new()
      .url(format!("http://{}", addr))
      .rasync_connect()
      .await
      .expect("establish tunnel");

    assert_eq!(200, tunnel.response().code());
    tunnel
      .stream_mut()
      .write_all(b"ping")
      .await
      .expect("write ping");
    let mut pong = [0u8; 4];
    tunnel
      .stream_mut()
      .read_exact(&mut pong)
      .await
      .expect("read pong");
    assert_eq!(b"pong", &pong);
  });

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_upgrade_returns_socket_after_101_and_does_not_parse_upgraded_bytes() {
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
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: \tkeep-alive, \tUpGrAdE\t\r\nUpgrade: \tWebSocket \t\r\n\r\nserver-bytes",
      )
      .expect("write upgrade response and bytes");
    let mut client_bytes = [0u8; 12];
    stream
      .read_exact(&mut client_bytes)
      .expect("read upgraded client bytes");
    assert_eq!(b"client-bytes", &client_bytes);
  });

  block_on(async {
    let mut upgraded = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect("upgrade connection");

    assert_eq!(101, upgraded.response().code());
    assert_eq!(
      Some(&"WebSocket".to_string()),
      upgraded.response().header_value("Upgrade")
    );
    let mut server_bytes = [0u8; 12];
    upgraded
      .stream_mut()
      .read_exact(&mut server_bytes)
      .await
      .expect("read upgraded server bytes");
    assert_eq!(b"server-bytes", &server_bytes);
    upgraded
      .stream_mut()
      .write_all(b"client-bytes")
      .await
      .expect("write upgraded client bytes");
  });

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_upgrade_skips_interim_responses_before_101() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let request = String::from_utf8(read_request_head(&mut stream)).expect("request utf8");
    assert!(request.starts_with("GET /chat HTTP/1.1\r\n"));
    stream
      .write_all(
        concat!(
          "HTTP/1.1 103 Early Hints\r\n",
          "Link: </style.css>; rel=preload\r\n",
          "\r\n",
          "HTTP/1.1 101 Switching Protocols\r\n",
          "Connection: Upgrade\r\n",
          "Upgrade: websocket\r\n",
          "\r\n",
          "server-bytes"
        )
        .as_bytes(),
      )
      .expect("write interim and final upgrade responses");
  });

  block_on(async {
    let mut upgraded = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect("upgrade connection");

    assert_eq!(101, upgraded.response().code());
    let mut server_bytes = [0u8; 12];
    upgraded
      .stream_mut()
      .read_exact(&mut server_bytes)
      .await
      .expect("read upgraded server bytes");
    assert_eq!(b"server-bytes", &server_bytes);
  });

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_failed_connect_reads_http_response_and_closes_socket() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind failed connect server");
  let addr = listener.local_addr().expect("failed connect server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept failed connect");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(
        b"HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\nConnection: close\r\n\r\nforbidden",
      )
      .expect("write failed connect response");
    let mut extra = [0u8; 1];
    assert_eq!(0, stream.read(&mut extra).expect("client should close"));
  });

  let err = block_on(async {
    HttpClient::new()
      .url(format!("http://{}", addr))
      .rasync_connect()
      .await
      .expect_err("non-2xx connect must fail")
  });

  assert!(err
    .to_string()
    .contains("CONNECT failed with HTTP status 403"));

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_failed_upgrade_reads_http_response_and_closes_socket() {
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

  let err = block_on(async {
    HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect_err("non-101 upgrade must fail")
  });

  assert!(err
    .to_string()
    .contains("Upgrade failed with HTTP status 426"));

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_connect_rejects_malformed_response_head() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind malformed connect server");
  let addr = listener
    .local_addr()
    .expect("malformed connect server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept malformed connect");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 xyz Not a status\r\n\r\n")
      .expect("write malformed connect response");
  });

  let err = block_on(async {
    HttpClient::new()
      .url(format!("http://{}", addr))
      .rasync_connect()
      .await
      .expect_err("malformed connect head must fail")
  });

  assert!(
    err.to_string().contains("Response status not have code"),
    "unexpected error: {err}"
  );

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_upgrade_rejects_truncated_response_head() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind truncated upgrade server");
  let addr = listener
    .local_addr()
    .expect("truncated upgrade server addr");

  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept truncated upgrade");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\n")
      .expect("write truncated upgrade response");
    stream
      .shutdown(std::net::Shutdown::Write)
      .expect("close truncated upgrade response");
  });

  let err = block_on(async {
    HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect_err("truncated upgrade head must fail")
  });

  assert!(
    err.to_string().contains("Incomplete http response headers"),
    "unexpected error: {err}"
  );

  handle.join().expect("server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_connect_delivers_peer_bytes_once_before_eof() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind connect server");
  let addr = listener.local_addr().expect("connect server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept connect");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nconnect-bytes")
      .expect("write connect response and bytes");
    stream
      .shutdown(Shutdown::Write)
      .expect("half-close connect peer");
  });

  block_on(async {
    let mut tunnel = HttpClient::new()
      .url(format!("http://{}", addr))
      .rasync_connect()
      .await
      .expect("establish connect handoff");
    let mut bytes = Vec::new();
    tunnel
      .stream_mut()
      .read_to_end(&mut bytes)
      .await
      .expect("read connect bytes");
    assert_eq!(b"connect-bytes", bytes.as_slice());
    let mut again = [0u8; 1];
    assert_eq!(
      0,
      tunnel
        .stream_mut()
        .read(&mut again)
        .await
        .expect("read EOF")
    );
  });
  handle.join().expect("connect server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_upgrade_delivers_peer_bytes_once_before_eof() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\nupgrade-bytes")
      .expect("write upgrade response and bytes");
    stream
      .shutdown(Shutdown::Write)
      .expect("half-close upgrade peer");
  });

  block_on(async {
    let mut upgraded = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect("establish upgrade handoff");
    let mut bytes = Vec::new();
    upgraded
      .stream_mut()
      .read_to_end(&mut bytes)
      .await
      .expect("read upgrade bytes");
    assert_eq!(b"upgrade-bytes", bytes.as_slice());
    let mut again = [0u8; 1];
    assert_eq!(
      0,
      upgraded
        .stream_mut()
        .read(&mut again)
        .await
        .expect("read EOF")
    );
  });
  handle.join().expect("upgrade server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_connect_caller_close_is_peer_eof() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind connect server");
  let addr = listener.local_addr().expect("connect server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept connect");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
      .expect("write connect response");
    stream
      .set_read_timeout(Some(Duration::from_secs(1)))
      .expect("set peer read timeout");
    let mut byte = [0u8; 1];
    assert_eq!(0, stream.read(&mut byte).expect("observe connect EOF"));
    stream
      .write_all(b"still-open")
      .expect("write after connect write-half close");
  });

  block_on(async {
    let mut tunnel = HttpClient::new()
      .url(format!("http://{}", addr))
      .rasync_connect()
      .await
      .expect("establish connect handoff");
    tunnel
      .stream_mut()
      .close()
      .await
      .expect("close connect write half");
    // Join before drop so peer EOF cannot come from closing the socket.
    handle.join().expect("connect server thread");
    let mut bytes = Vec::new();
    tunnel
      .stream_mut()
      .read_to_end(&mut bytes)
      .await
      .expect("read after connect write-half close");
    assert_eq!(b"still-open", bytes.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn async_upgrade_caller_close_is_peer_eof() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let _request = read_request_head(&mut stream);
    stream
      .write_all(
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
      )
      .expect("write upgrade response");
    stream
      .set_read_timeout(Some(Duration::from_secs(1)))
      .expect("set peer read timeout");
    let mut byte = [0u8; 1];
    assert_eq!(0, stream.read(&mut byte).expect("observe upgrade EOF"));
    stream
      .write_all(b"still-open")
      .expect("write after upgrade write-half close");
  });

  block_on(async {
    let mut upgraded = HttpClient::new()
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"))
      .rasync_upgrade()
      .await
      .expect("establish upgrade handoff");
    upgraded
      .stream_mut()
      .close()
      .await
      .expect("close upgrade write half");
    // Join before drop so peer EOF cannot come from closing the socket.
    handle.join().expect("upgrade server thread");
    let mut bytes = Vec::new();
    upgraded
      .stream_mut()
      .read_to_end(&mut bytes)
      .await
      .expect("read after upgrade write-half close");
    assert_eq!(b"still-open", bytes.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn async_dropped_connect_handoff_closes_pending_socket() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind connect server");
  let addr = listener.local_addr().expect("connect server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept connect");
    let _request = read_request_head(&mut stream);
    stream
      .set_read_timeout(Some(Duration::from_secs(1)))
      .expect("set peer read timeout");
    let mut byte = [0u8; 1];
    assert_eq!(
      0,
      stream.read(&mut byte).expect("observe dropped connect EOF")
    );
  });

  block_on(async {
    let mut client = HttpClient::new();
    client.url(format!("http://{}", addr));
    let handoff = client.rasync_connect();
    pin_mut!(handoff);
    let timer = Timer::after(Duration::from_millis(100));
    pin_mut!(timer);
    assert!(matches!(select(handoff, timer).await, Either::Right(_)));
  });
  handle.join().expect("connect server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_dropped_upgrade_handoff_closes_pending_socket() {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade server");
  let addr = listener.local_addr().expect("upgrade server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept upgrade");
    let _request = read_request_head(&mut stream);
    stream
      .set_read_timeout(Some(Duration::from_secs(1)))
      .expect("set peer read timeout");
    let mut byte = [0u8; 1];
    assert_eq!(
      0,
      stream.read(&mut byte).expect("observe dropped upgrade EOF")
    );
  });

  block_on(async {
    let mut client = HttpClient::new();
    client
      .url(format!("http://{}/chat", addr))
      .header(("Connection", "Upgrade"))
      .header(("Upgrade", "websocket"));
    let handoff = client.rasync_upgrade();
    pin_mut!(handoff);
    let timer = Timer::after(Duration::from_millis(100));
    pin_mut!(timer);
    assert!(matches!(select(handoff, timer).await, Either::Right(_)));
  });
  handle.join().expect("upgrade server thread");
}
