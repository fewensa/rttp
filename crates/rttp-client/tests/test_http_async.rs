#[cfg(feature = "async")]
use rttp_test_support as support;

#[cfg(feature = "async")]
use std::collections::HashMap;

#[cfg(feature = "async")]
use flate2::write::{GzEncoder, ZlibEncoder};
#[cfg(feature = "async")]
use flate2::Compression;
#[cfg(feature = "async")]
use futures::channel::oneshot;
#[cfg(feature = "async")]
use futures::executor::block_on;
#[cfg(feature = "async")]
use futures::io::{AllowStdIo, AsyncRead, AsyncReadExt, Cursor as AsyncCursor};
#[cfg(feature = "async")]
use rttp_client::types::{Proxy, StatusCode};
#[cfg(feature = "async")]
use rttp_client::{
  async_streaming_response_after_header, Config, HttpClient,
  DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
};
#[cfg(feature = "async")]
use std::io::{Cursor, Read, Write};
#[cfg(feature = "async")]
use std::net::TcpListener;
#[cfg(feature = "async")]
use std::sync::mpsc;
#[cfg(feature = "async")]
use std::thread;
#[cfg(feature = "async")]
use std::time::{Duration, Instant};

#[cfg(feature = "async")]
fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
  encoder.write_all(bytes).expect("write gzip fixture");
  encoder.finish().expect("finish gzip fixture")
}

#[cfg(feature = "async")]
fn zlib_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write zlib fixture");
  encoder.finish().expect("finish zlib fixture")
}

#[cfg(feature = "async")]
fn assert_decode_error(error: rttp_client::error::Error) {
  assert!(
    error
      .to_string()
      .starts_with("error decoding response body"),
    "unexpected error: {error}"
  );
  assert!(!error.is_body_too_large());
}

#[cfg(feature = "async")]
fn client() -> HttpClient {
  HttpClient::new()
}

#[cfg(feature = "async")]
fn buffered_response_config(limit: usize) -> Config {
  Config::builder()
    .max_buffered_response_body_bytes(limit)
    .build()
}

#[cfg(feature = "async")]
fn assert_body_too_large(error: rttp_client::error::Error, limit: usize) {
  assert!(error.is_body_too_large(), "unexpected error: {error}");
  assert_eq!(Some(limit), error.body_limit());
}

#[cfg(feature = "async")]
async fn async_read_test_response_head<S: AsyncRead + Unpin>(stream: &mut S) -> Vec<u8> {
  let mut head = Vec::new();
  let mut byte = [0u8; 1];
  while !head.ends_with(b"\r\n\r\n") {
    stream
      .read_exact(&mut byte)
      .await
      .expect("read response head");
    head.push(byte[0]);
  }
  head
}

#[cfg(feature = "async")]
fn spawn_async_chunked_upload_capture_server() -> (std::net::SocketAddr, thread::JoinHandle<Vec<u8>>)
{
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind async upload capture server");
  let addr = listener.local_addr().expect("async upload capture addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept async upload");
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.ends_with(b"\r\n\r\n") {
      stream.read_exact(&mut byte).expect("read request head");
      request.push(byte[0]);
    }
    while !request.ends_with(b"\r\n0\r\n\r\n") {
      stream.read_exact(&mut byte).expect("read chunked body");
      request.push(byte[0]);
    }
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nuploaded")
      .expect("write response");
    request
  });
  (addr, handle)
}

#[cfg(feature = "async")]
fn spawn_async_head_metadata_server() -> (std::net::SocketAddr, thread::JoinHandle<Vec<u8>>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind async HEAD metadata server");
  let addr = listener
    .local_addr()
    .expect("async HEAD metadata server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener
      .accept()
      .expect("accept async HEAD metadata request");
    let request = support::read_http_request(&mut stream);
    stream
      .write_all(
        concat!(
          "HTTP/1.1 200 OK\r\n",
          "Content-Length: 7\r\n",
          "Transfer-Encoding: chunked\r\n",
          "X-Object-Size: 7\r\n",
          "Connection: close\r\n",
          "\r\n",
          "ignored"
        )
        .as_bytes(),
      )
      .expect("write async HEAD metadata response");
    request
  });
  (addr, handle)
}

#[cfg(feature = "async")]
fn spawn_delayed_socks5_http_server() -> (
  std::net::SocketAddr,
  thread::JoinHandle<bool>,
  oneshot::Receiver<()>,
  mpsc::Sender<()>,
) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed SOCKS5 server");
  let addr = listener.local_addr().expect("delayed SOCKS5 server addr");
  let (handshake_sender, handshake_receiver) = oneshot::channel();
  let (release_sender, release_receiver) = mpsc::channel();
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept SOCKS5 connection");
    let mut greeting = [0u8; 3];
    stream
      .read_exact(&mut greeting)
      .expect("read SOCKS5 greeting");
    assert_eq!([5, 1, 0], greeting);

    handshake_sender
      .send(())
      .expect("signal delayed SOCKS5 handshake");
    let progressed_before_timeout = release_receiver
      .recv_timeout(Duration::from_millis(500))
      .is_ok();
    stream
      .write_all(&[5, 0])
      .expect("write SOCKS5 greeting response");

    let mut request = [0u8; 10];
    stream
      .read_exact(&mut request)
      .expect("read SOCKS5 connect request");
    assert_eq!([5, 1, 0, 1], request[..4]);
    stream
      .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
      .expect("write SOCKS5 connect response");

    support::read_http_request(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
      .expect("write delayed SOCKS5 response");
    progressed_before_timeout
  });
  (addr, handle, handshake_receiver, release_sender)
}

#[cfg(feature = "async")]
fn spawn_socks4_http_server() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind SOCKS4 server");
  let addr = listener.local_addr().expect("SOCKS4 server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept SOCKS4 connection");
    let mut request = [0u8; 9];
    stream
      .read_exact(&mut request)
      .expect("read SOCKS4 connect request");
    assert_eq!([4, 1], request[..2]);
    assert_eq!([127, 0, 0, 1], request[4..8]);
    assert_eq!(0, request[8]);
    stream
      .write_all(&[0, 90, 0, 0, 127, 0, 0, 1])
      .expect("write SOCKS4 connect response");

    support::read_http_request(&mut stream);
    stream
      .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
      .expect("write SOCKS4 HTTP response");
  });
  (addr, handle)
}

#[cfg(feature = "async")]
fn spawn_stalled_http_server() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind stalled HTTP server");
  let addr = listener.local_addr().expect("stalled HTTP server addr");
  let handle = thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept stalled HTTP request");
    support::read_http_request(&mut stream);
    thread::sleep(Duration::from_millis(250));
  });
  (addr, handle)
}

#[test]
#[cfg(feature = "async")]
fn test_async_http() {
  let (addr, _handle) = support::spawn_http_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/post", addr))
      .form("debug=true")
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_content_length_response_enforces_exact_body_limit() {
  block_on(async {
    for (body, should_succeed) in [(b"12345".as_slice(), true), (b"123456".as_slice(), false)] {
      let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      )
      .into_bytes();
      response.extend_from_slice(body);
      let (addr, _handle) = support::spawn_chunked_response_server(response);
      let result = client()
        .url(format!("http://{addr}/fixed"))
        .config(buffered_response_config(5))
        .rasync()
        .await;

      if should_succeed {
        assert_eq!(b"12345", result.unwrap().body().binary());
      } else {
        assert_body_too_large(result.unwrap_err(), 5);
      }
    }
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_chunked_response_enforces_exact_body_limit() {
  block_on(async {
    for (chunk, should_succeed) in [("5\r\n12345\r\n", true), ("6\r\n123456\r\n", false)] {
      let response = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{chunk}0\r\n\r\n"
      );
      let (addr, _handle) = support::spawn_chunked_response_server(response);
      let result = client()
        .url(format!("http://{addr}/chunked"))
        .config(buffered_response_config(5))
        .rasync()
        .await;

      if should_succeed {
        assert_eq!(b"12345", result.unwrap().body().binary());
      } else {
        assert_body_too_large(result.unwrap_err(), 5);
      }
    }
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_eof_delimited_response_enforces_exact_body_limit() {
  block_on(async {
    for (body, should_succeed) in [("12345", true), ("123456", false)] {
      let response = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{body}");
      let (addr, _handle) = support::spawn_chunked_response_server(response);
      let result = client()
        .url(format!("http://{addr}/eof"))
        .config(buffered_response_config(5))
        .rasync()
        .await;

      if should_succeed {
        assert_eq!(b"12345", result.unwrap().body().binary());
      } else {
        assert_body_too_large(result.unwrap_err(), 5);
      }
    }
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_gzip_response_limits_decoded_body() {
  let decoded = vec![b'a'; 256];
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(&decoded).unwrap();
  let compressed = encoder.finish().unwrap();
  assert!(compressed.len() <= 64);
  let mut response = format!(
    "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    compressed.len()
  )
  .into_bytes();
  response.extend_from_slice(&compressed);
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .url(format!("http://{addr}/gzip"))
      .config(buffered_response_config(64))
      .rasync()
      .await
      .unwrap_err();

    assert_body_too_large(error, 64);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_streaming_response_can_read_body_larger_than_buffered_limit() {
  block_on(async {
    let body_len = DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES + 1;
    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {body_len}\r\n\r\n").into_bytes();
    let mut stream = AllowStdIo::new(Cursor::new(vec![b'x'; body_len]));
    let mut response = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();

    response.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(body_len, body.len());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_gzip_response_exposes_decoded_body_headers() {
  let body = gzip_bytes(b"decoded");
  let mut raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  )
  .into_bytes();
  raw.extend_from_slice(&body);
  let (addr, _handle) = support::spawn_chunked_response_server(raw);

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/gzip", addr))
      .rasync()
      .await
      .expect("async buffered gzip response");

    assert_eq!(b"decoded", response.body().binary());
    assert!(response.header("Content-Encoding").is_none());
    assert!(response.header("Content-Length").is_none());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_deflate_response_exposes_decoded_body_headers() {
  let body = zlib_bytes(b"decoded");
  let mut raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Encoding: deflate\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  )
  .into_bytes();
  raw.extend_from_slice(&body);
  let (addr, _handle) = support::spawn_chunked_response_server(raw);

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/deflate", addr))
      .rasync()
      .await
      .expect("async buffered deflate response");

    assert_eq!(b"decoded", response.body().binary());
    assert!(response.header("Content-Encoding").is_none());
    assert!(response.header("Content-Length").is_none());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_buffered_deflate_response_limits_decoded_body() {
  let decoded = vec![b'a'; 256];
  let compressed = zlib_bytes(&decoded);
  assert!(compressed.len() <= 64);
  let mut response = format!(
    "HTTP/1.1 200 OK\r\nContent-Encoding: deflate\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    compressed.len()
  )
  .into_bytes();
  response.extend_from_slice(&compressed);
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .url(format!("http://{addr}/deflate"))
      .config(buffered_response_config(64))
      .rasync()
      .await
      .unwrap_err();

    assert_body_too_large(error, 64);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_invalid_deflate_returns_typed_decode_error() {
  let body = b"not-zlib";
  let mut raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Encoding: deflate\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  )
  .into_bytes();
  raw.extend_from_slice(body);
  let (addr, _handle) = support::spawn_chunked_response_server(raw);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/deflate", addr))
      .rasync()
      .await
      .expect_err("malformed deflate response should fail");

    assert_decode_error(error);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_head_response_is_bodyless_and_preserves_headers() {
  let (addr, handle) = spawn_async_head_metadata_server();
  block_on(async {
    let response = client()
      .head()
      .url(format!("http://{}/metadata", addr))
      .rasync()
      .await
      .expect("async HEAD metadata response");

    assert_eq!("", response.body().string().unwrap());
    assert_eq!(
      Some("7"),
      response.header_value("Content-Length").map(String::as_str)
    );
    assert_eq!(
      Some("chunked"),
      response
        .header_value("Transfer-Encoding")
        .map(String::as_str)
    );
    assert_eq!(
      Some("7"),
      response.header_value("X-Object-Size").map(String::as_str)
    );
  });

  let request = handle.join().expect("async HEAD metadata server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("HEAD /metadata HTTP/1.1\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_preserves_informational_response_metadata() {
  let (addr, _handle) = support::spawn_informational_then_ok_server("103 Early Hints");
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/early-hints", addr))
      .rasync()
      .await
      .expect("async informational response");

    assert_eq!(200, response.code());
    assert_eq!("final body", response.body().string().unwrap());
    assert_eq!(
      Some("yes"),
      response.header_value("X-Final").map(String::as_str)
    );
    assert!(response.header_value("X-Interim").is_none());

    let informational = response.informational_responses();
    assert_eq!(1, informational.len());
    assert_eq!(StatusCode::EARLY_HINTS, informational[0].status());
    assert_eq!("Early Hints", informational[0].reason());
    assert_eq!(
      Some("ignored"),
      informational[0]
        .header_value("X-Interim")
        .map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked() {
  let (addr, _handle) = support::spawn_chunked_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(2, response.trailers().len());
    assert_eq!(
      Some("abc"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("signed"),
      response.trailer("x-signature").map(|h| h.value().as_str())
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_valid_extension_preserves_trailers_without_leaking_extension() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "4;foo=bar\r\nWiki\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ));

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("Wiki", response.body().string().unwrap());
    assert_eq!(2, response.trailers().len());
    assert_eq!(
      Some("abc"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("signed"),
      response.trailer_value("x-signature").map(String::as_str)
    );
    assert!(response.trailer("foo").is_none());
    assert!(response.trailer_value("foo").is_none());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_streaming_chunked_upload_writes_incremental_framing() {
  let (addr, handle) = spawn_async_chunked_upload_capture_server();
  block_on(async {
    let payload = vec![b'a'; 96 * 1024];
    let response = client()
      .post()
      .url(format!("http://{}/upload", addr))
      .rasync_streaming_chunked(AsyncCursor::new(payload))
      .await
      .expect("async stream chunked upload");

    assert_eq!("uploaded", response.body().string().unwrap());
  });

  let request = handle.join().expect("upload server thread");
  let request = String::from_utf8(request).expect("request utf8");
  assert!(request.starts_with("POST /upload HTTP/1.1\r\n"));
  assert!(request.contains("\r\nTransfer-Encoding: chunked\r\n"));
  assert!(!request.contains("\r\nContent-Length:"));
  assert!(request.ends_with("\r\n0\r\n\r\n"));
}

#[test]
#[cfg(feature = "async")]
fn test_async_streaming_response_constructor_is_exported() {
  block_on(async {
    let head = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 5\r\n", "\r\n")
      .as_bytes()
      .to_vec();
    let mut stream = AllowStdIo::new(Cursor::new(b"hello"));
    let mut response = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();

    response.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(b"hello", body.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_204_with_misleading_content_length_keeps_next_response_readable() {
  block_on(async {
    let raw = concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: no-content\r\n",
      "\r\n",
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 4\r\n",
      "Connection: close\r\n",
      "\r\n",
      "next"
    );
    let mut stream = AllowStdIo::new(Cursor::new(raw.as_bytes()));
    let head = async_read_test_response_head(&mut stream).await;

    let mut first = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();
    first.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(204, first.code().unwrap());
    assert_eq!(b"", body.as_slice());
    assert_eq!(
      Some("7"),
      first
        .headers()
        .unwrap()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("content-length"))
        .map(|header| header.value().as_str())
    );
    assert_eq!(
      Some("no-content"),
      first
        .headers()
        .unwrap()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("x-trace"))
        .map(|header| header.value().as_str())
    );

    let head = async_read_test_response_head(&mut stream).await;
    let mut second = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();
    second.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(200, second.code().unwrap());
    assert_eq!(b"next", body.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_304_with_misleading_chunked_framing_keeps_next_response_readable() {
  block_on(async {
    let raw = concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "ETag: \"abc\"\r\n",
      "\r\n",
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 4\r\n",
      "Connection: close\r\n",
      "\r\n",
      "next"
    );
    let mut stream = AllowStdIo::new(Cursor::new(raw.as_bytes()));
    let head = async_read_test_response_head(&mut stream).await;

    let mut first = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();
    first.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(304, first.code().unwrap());
    assert_eq!(b"", body.as_slice());
    assert_eq!(
      Some("chunked"),
      first
        .headers()
        .unwrap()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("transfer-encoding"))
        .map(|header| header.value().as_str())
    );
    assert_eq!(
      Some("\"abc\""),
      first
        .headers()
        .unwrap()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("etag"))
        .map(|header| header.value().as_str())
    );

    let head = async_read_test_response_head(&mut stream).await;
    let mut second = async_streaming_response_after_header(&mut stream, false, head)
      .await
      .unwrap();
    let mut body = Vec::new();
    second.body_mut().read_to_end(&mut body).await.unwrap();

    assert_eq!(200, second.code().unwrap());
    assert_eq!(b"next", body.as_slice());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_socket2_server_chunked_trailers_match_sync_accessors() {
  let (addr, _handle) = support::spawn_socket2_chunked_trailer_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("socket2 chunked body", response.body().string().unwrap());
    assert_eq!(2, response.trailers().len());
    assert_eq!(
      Some("abc"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("abc"),
      response.trailer_value("x-trace").map(String::as_str)
    );
    assert_eq!(
      Some("signed"),
      response.trailer("x-signature").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("signed"),
      response.trailer_value("X-SIGNATURE").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_duplicate_trailers_are_exposed_in_wire_order() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=bar\r\nchunked\r\n",
    "6\r\n body!\r\n",
    "0\r\n",
    "X-Trace: first\r\n",
    "x-trace: second\r\n",
    "X-Signature: signed\r\n",
    "\r\n"
  ));

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(3, response.trailers().len());
    assert_eq!("X-Trace", response.trailers()[0].name());
    assert_eq!("first", response.trailers()[0].value());
    assert_eq!("x-trace", response.trailers()[1].name());
    assert_eq!("second", response.trailers()[1].value());
    assert_eq!("X-Signature", response.trailers()[2].name());
    assert_eq!("signed", response.trailers()[2].value());
    assert_eq!(
      Some("first"),
      response.trailer("X-TRACE").map(|h| h.value().as_str())
    );
    assert_eq!(
      Some("first"),
      response.trailer_value("x-trace").map(String::as_str)
    );
    assert_eq!(
      vec!["first", "second"],
      response
        .trailers_of_name("x-trace")
        .iter()
        .map(|header| header.value().as_str())
        .collect::<Vec<_>>()
    );
    assert_eq!(
      vec!["first", "second"],
      response
        .trailer_values("X-TRACE")
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_quoted_extensions_are_accepted() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;foo=\"bar;baz\";answer=42\r\nchunked\r\n",
    "6;empty;quoted=\"\\\\\\\"\"\r\n body!\r\n",
    "0;done=\"yes\"\r\n",
    "X-Trace: abc\r\n",
    "\r\n"
  ));

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(
      Some("abc"),
      response.trailer("x-trace").map(|h| h.value().as_str())
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_quoted_extensions_accept_obs_text() {
  let mut response = b"HTTP/1.1 200 OK\r\n\
Transfer-Encoding: chunked\r\n\
Connection: close\r\n\
\r\n\
7;meta=\""
    .to_vec();
  response.push(0xff);
  response.extend_from_slice(b"\"\r\nchunked\r\n0\r\n\r\n");

  let (addr, _handle) = support::spawn_chunked_response_server(response);
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!("chunked", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_transfer_encoding_chunked_with_content_length_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 2\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("ambiguous response framing should be rejected");

    assert!(
      error
        .to_string()
        .contains("Transfer-Encoding conflicts with Content-Length"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_non_chunked_transfer_coding_before_chunked_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: gzip, chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("unsupported transfer coding should be rejected");

    assert!(
      error
        .to_string()
        .contains("Unsupported Transfer-Encoding response body"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_transfer_encoding_without_final_chunked_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: gzip\r\n",
    "Connection: close\r\n",
    "\r\n",
    "unframed"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/gzip", addr))
      .rasync()
      .await
      .expect_err("unsupported transfer coding should be rejected");

    assert!(
      error
        .to_string()
        .contains("Unsupported Transfer-Encoding response body"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_malformed_extension_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "7;bad name=value\r\nchunked\r\n",
    "0\r\n",
    "\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("malformed chunk extension should be rejected");

    assert!(
      error.to_string().contains("Invalid chunk extension"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_malformed_size_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "z\r\n",
    "OK\r\n",
    "0\r\n",
    "\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("malformed chunk size should be rejected");

    assert!(
      error.to_string().starts_with("error receive response")
        && error.to_string().contains("Invalid chunk size"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_missing_crlf_after_chunk_data_is_rejected() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK",
    "0\r\n",
    "\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("missing chunk data terminator should be rejected");

    assert!(
      error.to_string().contains("Invalid chunk terminator"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_missing_final_zero_chunk_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("missing final zero chunk should be rejected");

    assert!(
      error.to_string().starts_with("error receive response")
        && error.to_string().contains("Unexpected end of chunked body"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_truncated_trailer_block_is_rejected_as_response_error() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "X-Trace: abc"
  ));

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("truncated chunk trailer block should be rejected");

    assert!(
      error.to_string().starts_with("error receive response")
        && error.to_string().contains("Unexpected end of chunked body"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_oversized_extension_is_rejected() {
  let extension = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7;foo={extension}\r\n\
     chunked\r\n\
     0\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("oversized chunk extension should be rejected");

    assert!(
      error
        .to_string()
        .contains("chunked response line is too large"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_chunked_oversized_trailer_is_rejected() {
  let trailer = "a".repeat(16 * 1024);
  let response = format!(
    "HTTP/1.1 200 OK\r\n\
     Transfer-Encoding: chunked\r\n\
     Connection: close\r\n\
     \r\n\
     7\r\n\
     chunked\r\n\
     0\r\n\
     X-Trace: {trailer}\r\n\
     \r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("oversized chunk trailer should be rejected");

    assert!(
      error
        .to_string()
        .contains("chunked response line is too large"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_forbidden_chunked_response_trailer_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "WWW-Authenticate: unsafe\r\n",
    "\r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("forbidden chunk trailer should be rejected");

    assert!(
      error.to_string().contains("Forbidden trailer header"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_malformed_chunked_response_trailer_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n",
    "Bad Name: unsafe\r\n",
    "\r\n"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/chunked", addr))
      .rasync()
      .await
      .expect_err("malformed chunk trailer should be rejected");

    assert!(
      error.to_string().contains("Invalid trailer header"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_malformed_response_header_without_colon_is_rejected() {
  let response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "BrokenHeader\r\n",
    "Content-Length: 2\r\n",
    "Connection: close\r\n",
    "\r\n",
    "OK"
  );
  let (addr, _handle) = support::spawn_chunked_response_server(response);
  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/broken", addr))
      .rasync()
      .await
      .expect_err("malformed response header should be rejected");

    assert!(
      error.to_string().contains("Invalid response header"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_duplicate_set_cookie_headers_are_preserved() {
  let (addr, _handle) = support::spawn_duplicate_set_cookie_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/cookies", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    let duplicate_headers = response
      .headers()
      .iter()
      .filter(|header| {
        header.name().eq_ignore_ascii_case("set-cookie")
          || header.name().eq_ignore_ascii_case("cache-control")
      })
      .map(|header| (header.name().as_str(), header.value().as_str()))
      .collect::<Vec<_>>();
    assert_eq!(
      vec![
        ("Set-Cookie", "session=abc; Path=/; HttpOnly"),
        ("cache-control", "no-cache"),
        ("SET-COOKIE", "theme=dark; Path=/; SameSite=Lax"),
        ("Cache-Control", "max-age=60")
      ],
      duplicate_headers
    );
    assert_eq!(
      vec![
        &"session=abc; Path=/; HttpOnly".to_string(),
        &"theme=dark; Path=/; SameSite=Lax".to_string()
      ],
      response.header_values("set-cookie")
    );
    assert_eq!(
      vec![&"no-cache".to_string(), &"max-age=60".to_string()],
      response.header_values("CACHE-CONTROL")
    );
    assert_eq!(
      Some(&"session=abc; Path=/; HttpOnly".to_string()),
      response.header_value("set-cookie")
    );
    assert_eq!(2, response.cookies().len());
    assert!(response.cookie("session").is_some());
    assert!(response.cookie("theme").is_some());
    assert_eq!(
      Some(&"text/plain".to_string()),
      response.header_value("content-type")
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_content_length_response_does_not_wait_for_eof() {
  let (addr, _handle) = support::spawn_keep_alive_server();
  block_on(async {
    let response = client()
      .get()
      .config(Config::builder().read_timeout(100))
      .url(format!("http://{}/keep-alive", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("OK", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_read_timeout_remains_enforced() {
  let (addr, handle) = spawn_stalled_http_server();
  let start = Instant::now();
  let error = block_on(async {
    client()
      .get()
      .config(Config::builder().read_timeout(50))
      .url(format!("http://{}/timeout", addr))
      .rasync()
      .await
      .expect_err("stalled async response should time out")
  });

  assert!(start.elapsed() < Duration::from_millis(200));
  assert!(error.to_string().contains("operation timed out"));
  handle.join().expect("stalled HTTP server thread");
}

#[test]
#[cfg(feature = "async")]
fn test_async_redirect_reuses_socket_when_connection_close_is_absent() {
  let (addr, handle) = support::spawn_redirect_connection_lifecycle_server(false);
  block_on(async {
    let response = client()
      .get()
      .config(Config::builder().auto_redirect(true))
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("final", response.body().string().unwrap());
  });
  assert_eq!(vec![2], handle.join().unwrap());
}

#[test]
#[cfg(feature = "async")]
fn test_async_redirect_uses_fresh_socket_after_connection_close() {
  let (addr, handle) = support::spawn_redirect_connection_lifecycle_server(true);
  block_on(async {
    let response = client()
      .get()
      .config(Config::builder().auto_redirect(true))
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("final", response.body().string().unwrap());
  });
  assert_eq!(vec![1, 1], handle.join().unwrap());
}

#[test]
#[cfg(feature = "async")]
fn test_async_keep_alive_content_length_response_leaves_client_reusable() {
  let (addr, _handle) = support::spawn_keep_alive_server_count(2);
  block_on(async {
    let mut client = client();

    let first = client
      .get()
      .config(Config::builder().read_timeout(100))
      .url(format!("http://{}/keep-alive", addr))
      .rasync()
      .await
      .unwrap();
    assert_eq!("OK", first.body().string().unwrap());

    let second = client
      .get()
      .url(format!("http://{}/keep-alive", addr))
      .rasync()
      .await
      .unwrap();
    assert_eq!("OK", second.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_skips_100_continue_before_final_response() {
  let (addr, _handle) = support::spawn_continue_then_ok_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue", addr))
      .expect_continue()
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("OK", response.reason());
    assert_eq!(
      Some(&"text/plain".to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
    assert!(response.header_value("X-Interim").is_none());
    assert_eq!("final body", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_sends_body_before_observing_100_continue() {
  let (addr, handle) = support::spawn_expect_continue_gate_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue-gate", addr))
      .header(("Expect", "100-continue"))
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("accepted", response.body().string().unwrap());
  });

  let request = handle.join().expect("expect continue gate thread");
  assert!(!request.is_empty(), "request should be captured");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(b"request body"));
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_sends_body_when_expect_continue_gets_final_response() {
  let (addr, handle) = support::spawn_expect_continue_reject_gate_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}/continue-reject", addr))
      .header(("Expect", "100-continue"))
      .raw("request body")
      .rasync()
      .await
      .unwrap();

    assert_eq!(417, response.code());
    assert_eq!("Expectation Failed", response.body().string().unwrap());
  });

  let request = handle.join().expect("expect continue reject gate thread");
  assert!(!request.is_empty(), "request should be captured");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(b"request body"));
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_skips_103_early_hints_before_final_response() {
  let (addr, _handle) = support::spawn_informational_then_ok_server("103 Early Hints");
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/early-hints", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(200, response.code());
    assert_eq!("OK", response.reason());
    assert_eq!(
      Some(&"text/plain".to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!(Some(&"yes".to_string()), response.header_value("X-Final"));
    assert!(response.header_value("X-Interim").is_none());
    assert_eq!("final body", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_client_returns_101_switching_protocols_as_terminal_response() {
  let (addr, _handle) = support::spawn_switching_protocols_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/upgrade", addr))
      .rasync()
      .await
      .unwrap();

    assert_eq!(101, response.code());
    assert_eq!("Switching Protocols", response.reason());
    assert_eq!(
      Some(&"Upgrade".to_string()),
      response.header_value("Connection")
    );
    assert_eq!(
      Some(&"websocket".to_string()),
      response.header_value("Upgrade")
    );
    assert_eq!(
      Some(&"test-accept".to_string()),
      response.header_value("Sec-WebSocket-Accept")
    );
    assert_eq!("", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect() {
  let (addr, _handle) = support::spawn_redirect_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/", addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert!(response.ok());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_redirect_is_not_followed_by_default() {
  let (addr, _handle) = support::spawn_redirect_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/", addr))
      .rasync()
      .await
      .expect("default redirect policy should return the redirect response");

    assert_eq!(302, response.code());
    assert!(response.is_redirect());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_uses_first_raw_location_when_typed_location_is_duplicate() {
  let (addr, _handle) = support::spawn_duplicate_location_redirect_target_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect/from", addr))
      .rasync()
      .await
      .expect("auto redirect should use the first raw Location header");

    assert_eq!("/final", response.body().string().unwrap());
  });
}

#[cfg(feature = "async")]
async fn assert_async_redirect_resolves_to_target<F>(location: F, expected_target: &str)
where
  F: FnOnce(std::net::SocketAddr) -> String + Send + 'static,
{
  let (addr, _handle) = support::spawn_redirect_target_echo_server(location);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url(format!("http://{}/redirect/from?old=1", addr))
    .rasync()
    .await;

  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!(expected_target, response.body().string().unwrap());
}

#[cfg(feature = "async")]
fn assert_redirect_error_has_url_context(
  error: &rttp_client::error::Error,
  message: &str,
  expected_path: &str,
) {
  assert!(error.is_redirect());
  let error_message = error.to_string();
  assert!(error_message.contains(message));
  assert!(error_message.contains(" for url (http://"));
  assert!(error_message.contains(expected_path));
  assert!(!error_message.contains("Authorization"));
  assert!(!error_message.contains("Bearer secret"));
  assert!(!error_message.contains("Cookie"));
  assert!(!error_message.contains("session=secret"));
  assert!(!error_message.contains("Proxy-Authorization"));
  assert!(!error_message.contains("proxy-secret"));
  assert_eq!(
    expected_path,
    error.url().expect("redirect error url").path()
  );
}

#[cfg(feature = "async")]
struct CapturedRequest {
  method: String,
  target: String,
  headers: HashMap<String, String>,
  body: Vec<u8>,
}

#[cfg(feature = "async")]
fn captured_request(request: Vec<u8>) -> CapturedRequest {
  let header_end = request
    .windows(4)
    .position(|window| window == b"\r\n\r\n")
    .expect("captured request headers");
  let header = String::from_utf8_lossy(&request[..header_end]);
  let mut lines = header.lines();
  let request_line = lines.next().expect("captured request line");
  let mut request_line_parts = request_line.split_whitespace();
  let method = request_line_parts
    .next()
    .expect("captured request method")
    .to_string();
  let target = request_line_parts
    .next()
    .expect("captured request target")
    .to_string();
  let headers = lines
    .filter_map(|line| {
      let (name, value) = line.split_once(':')?;
      Some((name.to_ascii_lowercase(), value.trim().to_string()))
    })
    .collect();
  let body = request[header_end + 4..].to_vec();

  CapturedRequest {
    method,
    target,
    headers,
    body,
  }
}

#[cfg(feature = "async")]
async fn captured_async_redirected_post(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .post()
    .url(format!("http://{}/redirect", addr))
    .raw("redirect-body")
    .rasync()
    .await;

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[cfg(feature = "async")]
async fn captured_async_redirected_put(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .put()
    .url(format!("http://{}/redirect", addr))
    .raw("redirect-body")
    .rasync()
    .await;

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[cfg(feature = "async")]
async fn captured_async_redirected_head(status_code: u16, reason: &'static str) -> CapturedRequest {
  let (addr, handle) = support::spawn_status_redirect_request_capture_server(status_code, reason);
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .head()
    .url(format!("http://{}/redirect", addr))
    .rasync()
    .await;

  assert!(response.is_ok());
  captured_request(handle.join().expect("redirect capture thread"))
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_301_post_becomes_get_without_body_or_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(301, "Moved Permanently").await;

    assert_eq!("GET", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"", request.body.as_slice());
    assert!(!request.headers.contains_key("content-length"));
    assert!(!request.headers.contains_key("content-type"));
    assert!(!request.headers.contains_key("transfer-encoding"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_302_post_becomes_get_without_body_or_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(302, "Found").await;

    assert_eq!("GET", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"", request.body.as_slice());
    assert!(!request.headers.contains_key("content-length"));
    assert!(!request.headers.contains_key("content-type"));
    assert!(!request.headers.contains_key("transfer-encoding"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_301_put_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_put(301, "Moved Permanently").await;

    assert_eq!("PUT", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_302_put_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_put(302, "Found").await;

    assert_eq!("PUT", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_303_post_becomes_get_without_body_or_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(303, "See Other").await;

    assert_eq!("GET", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"", request.body.as_slice());
    assert!(!request.headers.contains_key("content-length"));
    assert!(!request.headers.contains_key("content-type"));
    assert!(!request.headers.contains_key("transfer-encoding"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_303_post_allows_same_url_after_method_changes() {
  let (addr, _handle) = support::spawn_same_url_303_redirect_method_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true).max_redirect(1))
      .post()
      .url(format!("http://{}/submit", addr))
      .raw("redirect-body")
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("GET /submit HTTP/1.1", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_303_head_preserves_method() {
  block_on(async {
    let request = captured_async_redirected_head(303, "See Other").await;

    assert_eq!("HEAD", request.method);
    assert_eq!("/final?via=redirect", request.target);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_307_post_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(307, "Temporary Redirect").await;

    assert_eq!("POST", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_308_post_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_post(308, "Permanent Redirect").await;

    assert_eq!("POST", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_307_put_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_put(307, "Temporary Redirect").await;

    assert_eq!("PUT", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_308_put_preserves_method_body_and_body_framing() {
  block_on(async {
    let request = captured_async_redirected_put(308, "Permanent Redirect").await;

    assert_eq!("PUT", request.method);
    assert_eq!("/final?via=redirect", request.target);
    assert_eq!(b"redirect-body", request.body.as_slice());
    assert_eq!(
      Some("13"),
      request.headers.get("content-length").map(String::as_str)
    );
    assert_eq!(
      Some("text/plain"),
      request.headers.get("content-type").map(String::as_str)
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_absolute_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|addr| format!("http://{}/final", addr), "/final")
      .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_absolute_path_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|_| "/absolute-path".to_string(), "/absolute-path")
      .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_relative_child_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |_| "relative-child".to_string(),
      "/redirect/relative-child",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_parent_relative_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(|_| "../sibling".to_string(), "/sibling").await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_query_only_location() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |_| "?query-only".to_string(),
      "/redirect/from?query-only",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_location_with_fragment_without_sending_fragment() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |_| "fragment-child#section".to_string(),
      "/redirect/fragment-child",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_resolves_absolute_location_with_fragment_without_sending_fragment() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |addr| format!("http://{}/absolute-fragment#section", addr),
      "/absolute-fragment",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_percent_encoded_path_and_query_octets() {
  block_on(async {
    assert_async_redirect_resolves_to_target(
      |_| "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b".to_string(),
      "/files/%2e%2e/a%2fb/c%FF?next=%2fdone%3fx%3d1%FF&space=a%20b",
    )
    .await;
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_uses_preserved_percent_encoded_path_as_relative_base() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/files/%2e%2e/a/"),
      ("/files/%2e%2e/a/", "next%2fhop?token=%2e%2e"),
    ],
    3,
  );
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "/files/%2e%2e/a/next%2fhop?token=%2e%2e",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_chain_that_finishes_within_max_redirect() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![("/start", "/hop-one"), ("/hop-one", "/final?done=1")],
    3,
  );
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("/final?done=1", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_enforces_max_redirect_bound() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/hop-one"),
      ("/hop-one", "/hop-two"),
      ("/hop-two", "/final"),
    ],
    3,
  );
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true).max_redirect(2))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .expect_err("redirect chain should exceed max_redirect");

    assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-two");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_with_zero_max_redirect_fails_before_first_hop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/start", "/final?done=1")], 1);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true).max_redirect(0))
      .get()
      .url(format!("http://{}/start", addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .rasync()
      .await
      .expect_err("max_redirect=0 should reject the first redirect");

    assert_redirect_error_has_url_context(&error, "too many redirects", "/start");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_with_one_max_redirect_fails_on_second_hop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![("/start", "/hop-one"), ("/hop-one", "/final?done=1")],
    2,
  );
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true).max_redirect(1))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .expect_err("max_redirect=1 should allow one redirect and reject the second");

    assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-one");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_uses_default_max_redirect_when_enabled() {
  let (addr, _handle) = support::spawn_redirect_chain_server(
    vec![
      ("/start", "/hop-one"),
      ("/hop-one", "/hop-two"),
      ("/hop-two", "/hop-three"),
      ("/hop-three", "/hop-four"),
      ("/hop-four", "/hop-five"),
      ("/hop-five", "/final"),
    ],
    6,
  );
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/start", addr))
      .rasync()
      .await
      .expect_err("auto_redirect default max should reject the sixth redirect");

    assert_redirect_error_has_url_context(&error, "too many redirects", "/hop-five");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_a_b_a_loop() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/a", "/b"), ("/b", "/a")], 3);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true).max_redirect(10))
      .get()
      .url(format!("http://{}/a", addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .rasync()
      .await
      .expect_err("A -> B -> A should be detected as a loop");

    assert_redirect_error_has_url_context(&error, "infinite redirect loop detected", "/b");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_self_redirect() {
  let (addr, _handle) = support::spawn_redirect_chain_server(vec![("/self", "/self")], 1);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/self", addr))
      .rasync()
      .await
      .expect_err("self redirect should be detected as a loop");

    assert_redirect_error_has_url_context(&error, "infinite redirect loop detected", "/self");
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_loop_after_relative_location_is_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/redirect/from?old=1", "?old=1")], 8);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect/from?old=1", addr))
      .rasync()
      .await
      .expect_err("redirect should resolve back to current URL");

    assert!(error.is_redirect());
    assert!(error
      .to_string()
      .contains("infinite redirect loop detected"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_detects_loop_after_dot_segments_are_normalized() {
  let (addr, _handle) =
    support::spawn_redirect_chain_server(vec![("/a/current", "../a/current")], 8);
  block_on(async {
    let error = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/a/current", addr))
      .rasync()
      .await
      .expect_err("redirect should normalize back to current URL");

    assert!(error.is_redirect());
    assert!(error
      .to_string()
      .contains("infinite redirect loop detected"));
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_rebuilds_host_for_cross_authority_location() {
  let (origin_addr, target_addr, _handle) =
    support::spawn_cross_authority_redirect_host_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", origin_addr))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(target_addr.to_string(), response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_strips_sensitive_headers_for_cross_authority_location() {
  let (origin_addr, _target_addr, _handle) =
    support::spawn_cross_authority_redirect_header_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", origin_addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .lock_token("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
      .expect("Lock-Token should be accepted")
      .header(("X-Trace", "trace-123"))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "authorization=\ncookie=\nproxy-authorization=\nlock-token=\nx-trace=trace-123",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_strips_sensitive_headers_and_userinfo_for_cross_authority_location() {
  let (origin_addr, _target_addr, _handle) =
    support::spawn_cross_authority_redirect_userinfo_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", origin_addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .lock_token("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
      .expect("Lock-Token should be accepted")
      .header(("X-Trace", "trace-123"))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "request-target=/final\nauthorization=\ncookie=\nproxy-authorization=\nlock-token=\nx-trace=trace-123",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_auto_redirect_preserves_sensitive_headers_for_same_authority_location() {
  let (addr, _handle) = support::spawn_same_authority_redirect_header_echo_server();
  block_on(async {
    let response = client()
      .config(Config::builder().auto_redirect(true))
      .get()
      .url(format!("http://{}/redirect", addr))
      .header(("Authorization", "Bearer secret"))
      .header(("Cookie", "session=secret"))
      .header(("Proxy-Authorization", "Basic proxy-secret"))
      .lock_token("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
      .expect("Lock-Token should be accepted")
      .header(("X-Trace", "trace-123"))
      .rasync()
      .await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!(
      "authorization=Bearer secret\ncookie=session=secret\nproxy-authorization=Basic proxy-secret\nlock-token=<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\nx-trace=trace-123",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(all(feature = "async", any(feature = "tls-native", feature = "tls-rustls")))]
fn test_async_https() {
  let (addr, _handle) = support::spawn_tls_server();
  block_on(async {
    let response = client()
      .post()
      .url(format!("https://{}/get", addr))
      .config(
        rttp_client::Config::builder()
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(all(feature = "async", feature = "tls-rustls"))]
fn test_async_https_to_http_redirect_is_rejected_before_target_connection() {
  let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
  let target_addr = target.local_addr().expect("redirect target address");
  let (addr, _handle) = support::spawn_tls_redirect_server(format!("http://{}/final", target_addr));
  block_on(async {
    let error = client()
      .get()
      .url(format!("https://{}/", addr))
      .config(
        Config::builder()
          .auto_redirect(true)
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await
      .expect_err("HTTPS to HTTP redirect should be rejected by default");

    assert!(error.is_redirect());
    assert!(error.to_string().contains("HTTPS to HTTP redirect"));
    assert_eq!("https", error.url().expect("redirect source URL").scheme());
  });
  target
    .set_nonblocking(true)
    .expect("set redirect target nonblocking");
  assert!(matches!(
    target.accept(),
    Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock
  ));
}

#[test]
#[cfg(all(feature = "async", feature = "tls-rustls"))]
fn test_async_https_to_http_redirect_can_be_enabled_explicitly() {
  let (target_addr, _target_handle) = support::spawn_http_server();
  let (addr, _handle) = support::spawn_tls_redirect_server(format!("http://{}/final", target_addr));
  block_on(async {
    let response = client()
      .get()
      .url(format!("https://{}/", addr))
      .config(
        Config::builder()
          .auto_redirect(true)
          .allow_https_to_http_redirects(true)
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await
      .expect("explicit HTTPS to HTTP redirect opt-in should succeed");

    assert!(response.ok());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_http_proxy_uses_absolute_form_for_http_requests() {
  let (addr, _handle) = support::spawn_http_proxy_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://example.test/proxy?q=1")
      .proxy(Proxy::http("127.0.0.1", u32::from(addr.port())))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(
      "GET http://example.test/proxy?q=1 HTTP/1.1",
      response.body().string().unwrap()
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_http_proxy_with_auth_uses_proxy_authorization_header() {
  let (addr, _handle) = support::spawn_http_proxy_auth_echo_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://example.test/proxy?q=1")
      .proxy(Proxy::http_with_authorization(
        "127.0.0.1",
        u32::from(addr.port()),
        "user",
        "secret",
      ))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!("Basic dXNlcjpzZWNyZXQ=", response.body().string().unwrap());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_eof_delimited_response_body_is_read_to_connection_close() {
  let (addr, _handle) = support::spawn_eof_delimited_response_server("async eof body");
  block_on(async {
    let response = client().url(format!("http://{}/eof", addr)).rasync().await;

    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("async eof body", response.body().string().unwrap());
    assert_eq!(
      Some(&"close".to_string()),
      response.header_value("Connection")
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_truncated_content_length_response_is_rejected() {
  let (addr, _handle) = support::spawn_truncated_content_length_server();
  block_on(async {
    let error = client()
      .url(format!("http://{}/truncated", addr))
      .rasync()
      .await
      .expect_err("truncated fixed-length body should be rejected");

    assert!(
      error.to_string().contains("failed to fill whole buffer")
        || error.to_string().contains("unexpected end of file"),
      "unexpected error: {error}"
    );
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_proxy_socks5() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) = support::spawn_socks5_proxy_server();
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/get", addr))
      .proxy(Proxy::socks5("127.0.0.1", proxy_addr.port().into()))
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_proxy_socks4() {
  let (proxy_addr, proxy_handle) = spawn_socks4_http_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://127.0.0.1:80/socks4")
      .proxy(Proxy::socks4("127.0.0.1", proxy_addr.port().into()))
      .rasync()
      .await
      .expect("async SOCKS4 response");
    assert_eq!("OK", response.body().string().unwrap());
  });
  proxy_handle.join().expect("SOCKS4 server thread");
}

#[test]
#[cfg(feature = "async")]
fn test_async_proxy_socks5_with_authentication() {
  let (addr, _handle) = support::spawn_http_server();
  let (proxy_addr, _proxy_handle) =
    support::spawn_socks5_proxy_server_with_credentials("username", "password");
  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/get", addr))
      .proxy(Proxy::socks5_with_authorization(
        "127.0.0.1",
        proxy_addr.port().into(),
        "username",
        "password",
      ))
      .rasync()
      .await
      .expect("authenticated async SOCKS5 response");
    assert_eq!("127.0.0.1", response.host());
  });
}

#[test]
#[cfg(feature = "async")]
fn test_async_socks5_handshake_does_not_block_other_tasks() {
  let (proxy_addr, proxy_handle, handshake_receiver, release_sender) =
    spawn_delayed_socks5_http_server();

  block_on(async {
    let mut client = client();
    let request = client
      .get()
      .url("http://127.0.0.1:80/progress")
      .proxy(Proxy::socks5("127.0.0.1", proxy_addr.port().into()))
      .rasync();
    let progress = async {
      handshake_receiver
        .await
        .expect("delayed SOCKS5 handshake start");
      release_sender.send(()).expect("release SOCKS5 handshake");
    };

    let (response, ()) = futures::join!(request, progress);
    assert_eq!("OK", response.unwrap().body().string().unwrap());
  });

  assert!(
    proxy_handle.join().expect("delayed SOCKS5 server thread"),
    "progress task was blocked by the SOCKS5 handshake"
  );
}

#[test]
#[cfg(all(feature = "async", any(feature = "tls-native", feature = "tls-rustls")))]
fn test_async_https_proxy_with_auth_uses_connect_tunnel() {
  let (proxy_addr, target_addr, _proxy_handle) =
    support::spawn_https_proxy_server_with_credentials("user", "secret");
  block_on(async {
    let response = client()
      .get()
      .url(format!("https://localhost:{}/", target_addr.port()))
      .proxy(Proxy::http_with_authorization(
        "127.0.0.1",
        u32::from(proxy_addr.port()),
        "user",
        "secret",
      ))
      .config(
        rttp_client::Config::builder()
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("OK", response.body().string().unwrap());
  });
}

#[test]
#[cfg(all(feature = "async", feature = "tls-rustls"))]
fn test_async_https_proxy_rejects_excessive_informational_connect_responses() {
  let response = "HTTP/1.1 103 Early Hints\r\n\r\n".repeat(17);
  let (proxy_addr, proxy_handle) = support::spawn_proxy_connect_response_server(response);

  block_on(async {
    let error = client()
      .get()
      .url("https://localhost/")
      .proxy(Proxy::http("127.0.0.1", u32::from(proxy_addr.port())))
      .rasync()
      .await
      .expect_err("excessive informational proxy responses should fail");

    assert!(error
      .to_string()
      .contains("Too many informational proxy responses"));
  });
  assert!(
    proxy_handle.join().expect("proxy response server"),
    "async client should close the rejected proxy connection"
  );
}
