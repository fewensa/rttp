use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::response::Response;
use rttp_client::{Config, Http2StreamingResponse, HttpClient};
use rttp_server::server::{HttpResponse, HttpServer};

#[derive(Clone, Copy)]
enum Transport {
  PriorKnowledge,
  Upgrade,
  #[cfg(feature = "async")]
  PriorKnowledgeAsync,
  #[cfg(feature = "async")]
  UpgradeAsync,
}

impl Transport {
  fn name(self) -> &'static str {
    match self {
      Self::PriorKnowledge => "prior-knowledge",
      Self::Upgrade => "upgrade",
      #[cfg(feature = "async")]
      Self::PriorKnowledgeAsync => "prior-knowledge-async",
      #[cfg(feature = "async")]
      Self::UpgradeAsync => "upgrade-async",
    }
  }

  fn emit(self, client: &mut HttpClient) -> Result<Response, rttp_client::error::Error> {
    match self {
      Self::PriorKnowledge => client.emit_http2_prior_knowledge(),
      Self::Upgrade => client.emit_http2_upgrade(),
      #[cfg(feature = "async")]
      Self::PriorKnowledgeAsync => block_on(client.rasync_http2_prior_knowledge()),
      #[cfg(feature = "async")]
      Self::UpgradeAsync => block_on(client.rasync_http2_upgrade()),
    }
  }

  fn emit_streaming(
    self,
    client: &mut HttpClient,
  ) -> Result<Http2StreamingResponse, rttp_client::error::Error> {
    match self {
      Self::PriorKnowledge => client.emit_http2_prior_knowledge_streaming(),
      Self::Upgrade => client.emit_http2_upgrade_streaming(),
      #[cfg(feature = "async")]
      Self::PriorKnowledgeAsync => block_on(client.rasync_http2_prior_knowledge_streaming()),
      #[cfg(feature = "async")]
      Self::UpgradeAsync => block_on(client.rasync_http2_upgrade_streaming()),
    }
  }
}

fn transports() -> Vec<Transport> {
  vec![
    Transport::PriorKnowledge,
    Transport::Upgrade,
    #[cfg(feature = "async")]
    Transport::PriorKnowledgeAsync,
    #[cfg(feature = "async")]
    Transport::UpgradeAsync,
  ]
}

struct Fixture {
  name: &'static str,
  encoding: &'static str,
  wire_body: Vec<u8>,
  decoded_body: Vec<u8>,
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write gzip fixture");
  encoder.finish().expect("finish gzip fixture")
}

fn zlib(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write zlib fixture");
  encoder.finish().expect("finish zlib fixture")
}

fn raw_deflate(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(bytes).expect("write raw deflate fixture");
  encoder.finish().expect("finish raw deflate fixture")
}

fn fixtures() -> Vec<Fixture> {
  let body = b"bounded HTTP/2 streaming decoding".to_vec();
  let first = gzip(b"concatenated ");
  let second = gzip(b"members");
  let gzip_body = [first, second].concat();
  let zlib_body = zlib(&body);
  let raw_body = raw_deflate(&body);
  vec![
    Fixture {
      name: "concatenated-gzip",
      encoding: "gzip",
      wire_body: gzip_body,
      decoded_body: b"concatenated members".to_vec(),
    },
    Fixture {
      name: "zlib-deflate",
      encoding: "deflate",
      wire_body: zlib_body.clone(),
      decoded_body: body.clone(),
    },
    Fixture {
      name: "raw-deflate",
      encoding: "deflate",
      wire_body: raw_body,
      decoded_body: body.clone(),
    },
    Fixture {
      name: "gzip-then-deflate",
      encoding: "gzip, deflate",
      wire_body: zlib(&gzip(&body)),
      decoded_body: body.clone(),
    },
    Fixture {
      name: "deflate-then-gzip",
      encoding: "deflate, gzip",
      wire_body: gzip(&zlib(&body)),
      decoded_body: body,
    },
  ]
}

fn unsupported_fixture() -> Fixture {
  Fixture {
    name: "unsupported-stack",
    encoding: "gzip, br",
    wire_body: gzip(b"bounded HTTP/2 streaming decoding"),
    decoded_body: Vec::new(),
  }
}

fn serve_response(
  encoding: &str,
  body: Vec<u8>,
  trailer: bool,
) -> (std::net::SocketAddr, Receiver<()>, thread::JoinHandle<()>) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c streaming server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c streaming server address");
  let (done_tx, done_rx) = mpsc::channel();
  let encoding = encoding.to_owned();
  let handle = thread::spawn(move || {
    let mut response = HttpResponse::ok(body).header("Content-Encoding", encoding);
    if trailer {
      response = response.trailer("x-streaming-trailer", "after-eof");
    }
    server
      .accept_one(|_| response)
      .expect("serve h2c streaming response");
    done_tx.send(()).expect("signal h2c response completion");
  });
  (addr, done_rx, handle)
}

fn serve_plain_response(
  body: Vec<u8>,
) -> (std::net::SocketAddr, Receiver<()>, thread::JoinHandle<()>) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c plain streaming server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server
    .local_addr()
    .expect("h2c plain streaming server address");
  let (done_tx, done_rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|_| HttpResponse::ok(body))
      .expect("serve h2c plain streaming response");
    done_tx
      .send(())
      .expect("signal h2c plain response completion");
  });
  (addr, done_rx, handle)
}

fn serve_early_drop_response() -> (
  std::net::SocketAddr,
  Receiver<io::Result<()>>,
  thread::JoinHandle<()>,
) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c early-drop server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c early-drop server address");
  let (done_tx, done_rx) = mpsc::channel();
  let mut body = Vec::with_capacity(256 * 1024);
  let mut state = 0x1234_5678u32;
  for _ in 0..256 * 1024 {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    body.push(state as u8);
  }
  let body = gzip(&body);
  let handle = thread::spawn(move || {
    let result = server.accept_one(|_| HttpResponse::ok(body).header("Content-Encoding", "gzip"));
    done_tx
      .send(result)
      .expect("signal h2c early-drop completion");
  });
  (addr, done_rx, handle)
}

fn wait_for_server(done: Receiver<()>, handle: thread::JoinHandle<()>, context: &str) {
  done
    .recv_timeout(Duration::from_secs(2))
    .unwrap_or_else(|error| panic!("{context} server did not finish: {error}"));
  handle.join().expect("h2c streaming server thread");
}

fn client_for(limit: Option<usize>) -> HttpClient {
  let mut client = HttpClient::new();
  if let Some(limit) = limit {
    client.config(
      Config::builder()
        .max_buffered_response_body_bytes(limit)
        .build(),
    );
  }
  client
}

fn assert_decoded_response(response: &Response, fixture: &Fixture, transport: Transport) {
  assert!(
    response
      .headers()
      .iter()
      .all(|header| !header.name().eq_ignore_ascii_case("Content-Encoding")),
    "{}/{} stale Content-Encoding header",
    transport.name(),
    fixture.name
  );
  assert!(
    response
      .headers()
      .iter()
      .all(|header| !header.name().eq_ignore_ascii_case("Content-Length")),
    "{}/{} stale Content-Length header",
    transport.name(),
    fixture.name
  );
  assert_eq!(
    fixture.decoded_body,
    response.body().binary(),
    "{}/{} materialized body",
    transport.name(),
    fixture.name
  );
  assert!(
    response.binary().ends_with(&fixture.wire_body),
    "{}/{} response wire capture lost encoded body",
    transport.name(),
    fixture.name
  );
}

fn read_streaming_body(response: &mut Http2StreamingResponse) -> io::Result<Vec<u8>> {
  let mut body = Vec::new();
  let mut buffer = [0u8; 3];
  loop {
    let read = response.read(&mut buffer)?;
    if read == 0 {
      return Ok(body);
    }
    body.extend_from_slice(&buffer[..read]);
  }
}

fn assert_streaming_decoded_response(
  response: &mut Http2StreamingResponse,
  fixture: &Fixture,
  transport: Transport,
) {
  assert!(
    response
      .headers()
      .iter()
      .all(|header| !header.name().eq_ignore_ascii_case("Content-Encoding")),
    "{}/{} stale Content-Encoding header",
    transport.name(),
    fixture.name
  );
  assert!(
    response
      .headers()
      .iter()
      .all(|header| !header.name().eq_ignore_ascii_case("Content-Length")),
    "{}/{} stale Content-Length header",
    transport.name(),
    fixture.name
  );
  assert_eq!(
    fixture.decoded_body,
    read_streaming_body(response).unwrap_or_else(|error| {
      panic!(
        "{}/{} streaming body read: {error}",
        transport.name(),
        fixture.name
      )
    }),
    "{}/{} streaming body",
    transport.name(),
    fixture.name
  );
  let mut eof = [0u8; 1];
  assert_eq!(
    0,
    response.read(&mut eof).expect("streaming response EOF"),
    "{}/{} streaming EOF",
    transport.name(),
    fixture.name
  );
}

#[test]
fn http2_streaming_decoding_transport_matrix() {
  for fixture in fixtures() {
    for transport in transports() {
      let (addr, done, handle) =
        serve_response(&fixture.encoding, fixture.wire_body.clone(), false);
      let mut client = client_for(None);
      client.get().url(format!("http://{addr}/{}", fixture.name));
      let response = transport.emit(&mut client).unwrap_or_else(|error| {
        panic!(
          "{}/{} should succeed: {error}",
          transport.name(),
          fixture.name
        )
      });
      assert_decoded_response(&response, &fixture, transport);
      wait_for_server(done, handle, fixture.name);
    }
  }
}

#[test]
fn http2_live_streaming_decoding_transport_matrix() {
  for fixture in fixtures() {
    for transport in transports() {
      let (addr, done, handle) =
        serve_response(&fixture.encoding, fixture.wire_body.clone(), false);
      let mut client = client_for(None);
      client.get().url(format!("http://{addr}/{}", fixture.name));
      let mut response = transport
        .emit_streaming(&mut client)
        .unwrap_or_else(|error| {
          panic!(
            "{}/{} streaming response should succeed: {error}",
            transport.name(),
            fixture.name
          )
        });
      assert_streaming_decoded_response(&mut response, &fixture, transport);
      wait_for_server(done, handle, fixture.name);
    }
  }
}

#[test]
fn http2_streaming_decoding_unsupported_stack_preserves_wire() {
  let fixture = unsupported_fixture();
  for transport in transports() {
    let (addr, done, handle) = serve_response(&fixture.encoding, fixture.wire_body.clone(), false);
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/unsupported"));
    let response = transport.emit(&mut client).unwrap_or_else(|error| {
      panic!(
        "{} should preserve unsupported coding: {error}",
        transport.name()
      )
    });
    assert_eq!(fixture.wire_body, response.body().binary());
    assert_eq!(
      Some(fixture.encoding),
      response
        .header_value("Content-Encoding")
        .map(String::as_str)
    );
    assert_eq!(fixture.wire_body, response.body().binary());
    wait_for_server(done, handle, "unsupported coding");
  }
}

#[test]
fn http2_live_streaming_decoding_unsupported_stack_preserves_wire() {
  let fixture = unsupported_fixture();
  for transport in transports() {
    let (addr, done, handle) = serve_response(&fixture.encoding, fixture.wire_body.clone(), false);
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/unsupported"));
    let mut response = transport
      .emit_streaming(&mut client)
      .unwrap_or_else(|error| {
        panic!(
          "{} should preserve unsupported coding while streaming: {error}",
          transport.name()
        )
      });
    assert_eq!(
      Some(fixture.encoding),
      response
        .headers()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("Content-Encoding"))
        .map(|header| header.value().as_str())
    );
    assert_eq!(
      fixture.wire_body,
      read_streaming_body(&mut response).expect("unsupported streaming body")
    );
    let mut eof = [0u8; 1];
    assert_eq!(
      0,
      response.read(&mut eof).expect("unsupported streaming EOF")
    );
    wait_for_server(done, handle, "unsupported streaming coding");
  }
}

#[test]
fn http2_streaming_plain_response_preserves_final_data_across_reads() {
  let body = (0..64).map(|byte| byte as u8).collect::<Vec<_>>();
  for transport in transports() {
    let (addr, done, handle) = serve_plain_response(body.clone());
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/plain-streaming"));
    let mut response = transport
      .emit_streaming(&mut client)
      .unwrap_or_else(|error| panic!("{} plain response: {error}", transport.name()));
    let mut received = Vec::new();
    let mut buffer = [0u8; 3];
    loop {
      let read = response
        .read(&mut buffer)
        .unwrap_or_else(|error| panic!("{} body read: {error}", transport.name()));
      if read == 0 {
        break;
      }
      received.extend_from_slice(&buffer[..read]);
    }
    assert_eq!(body, received, "{} body", transport.name());
    assert_eq!(0, response.read(&mut buffer).expect("plain response EOF"));
    wait_for_server(done, handle, "plain response");
  }
}

fn assert_buffered_decode_failure(transport: Transport, body: Vec<u8>, encoding: &str, name: &str) {
  let (addr, done, handle) = serve_response(encoding, body, false);
  let mut client = client_for(None);
  client.get().url(format!("http://{addr}/{name}"));
  let error = match transport.emit(&mut client) {
    Ok(_) => panic!("malformed streaming response must fail atomically"),
    Err(error) => error,
  };
  assert!(
    error
      .to_string()
      .starts_with("error decoding response body"),
    "{}/{} unexpected decode error: {error}",
    transport.name(),
    name
  );
  wait_for_server(done, handle, name);
}

fn assert_streaming_decode_failure(
  transport: Transport,
  body: Vec<u8>,
  encoding: &str,
  name: &str,
) {
  let (addr, done, handle) = serve_response(encoding, body, false);
  let mut client = client_for(None);
  client.get().url(format!("http://{addr}/{name}"));
  let mut response = transport
    .emit_streaming(&mut client)
    .unwrap_or_else(|error| panic!("{}/{} response setup: {error}", transport.name(), name));
  let error = read_streaming_body(&mut response)
    .expect_err("malformed streaming response must fail while reading");
  assert_eq!(io::ErrorKind::InvalidData, error.kind());
  assert!(
    error
      .to_string()
      .starts_with("error decoding response body"),
    "{}/{} unexpected streaming decode error: {error}",
    transport.name(),
    name
  );
  wait_for_server(done, handle, name);
}

#[test]
fn http2_streaming_decoding_malformed_streams_are_atomic() {
  for transport in transports() {
    assert_buffered_decode_failure(transport, b"not-gzip".to_vec(), "gzip", "malformed-gzip");
    let mut truncated = zlib(b"truncated");
    truncated.pop();
    assert_buffered_decode_failure(transport, truncated, "deflate", "truncated-deflate");
  }
}

#[test]
fn http2_live_streaming_decoding_malformed_streams_fail_during_read() {
  for transport in transports() {
    assert_streaming_decode_failure(transport, b"not-gzip".to_vec(), "gzip", "malformed-gzip");
    let mut truncated = zlib(b"truncated");
    truncated.pop();
    assert_streaming_decode_failure(transport, truncated, "deflate", "truncated-deflate");
  }
}

#[test]
fn http2_streaming_decoding_enforces_exact_and_over_limits() {
  let exact = vec![b'x'; 64];
  for transport in transports() {
    let (addr, done, handle) = serve_response("gzip", gzip(&exact), false);
    let mut client = client_for(Some(exact.len()));
    client.get().url(format!("http://{addr}/exact"));
    let response = transport
      .emit(&mut client)
      .expect("exact limit should succeed");
    assert_eq!(exact, response.body().binary());
    wait_for_server(done, handle, "exact limit");

    let (addr, done, handle) = serve_response("gzip", gzip(&[b'x'; 65]), false);
    let mut client = client_for(Some(exact.len()));
    client.get().url(format!("http://{addr}/over"));
    let error = match transport.emit(&mut client) {
      Ok(_) => panic!("over limit should fail before exposing a response"),
      Err(error) => error,
    };
    assert!(
      error.is_body_too_large(),
      "{} unexpected limit error: {error}",
      transport.name()
    );
    assert_eq!(Some(exact.len()), error.body_limit());
    wait_for_server(done, handle, "over limit");
  }
}

#[test]
fn http2_live_streaming_decoding_enforces_exact_and_over_limits() {
  let exact = vec![b'x'; 64];
  for transport in transports() {
    let (addr, done, handle) = serve_response("gzip", gzip(&exact), false);
    let mut client = client_for(Some(exact.len()));
    client.get().url(format!("http://{addr}/exact"));
    let mut response = transport
      .emit_streaming(&mut client)
      .expect("exact streaming limit should set up");
    assert_eq!(
      exact,
      read_streaming_body(&mut response).expect("exact limit body")
    );
    wait_for_server(done, handle, "exact streaming limit");

    let (addr, done, handle) = serve_response("gzip", gzip(&[b'x'; 65]), false);
    let mut client = client_for(Some(exact.len()));
    client.get().url(format!("http://{addr}/over"));
    let mut response = transport
      .emit_streaming(&mut client)
      .expect("over streaming limit should set up");
    let error = read_streaming_body(&mut response)
      .expect_err("over streaming limit should fail while reading");
    assert!(
      error.to_string().contains("exceeded 64 bytes"),
      "{} unexpected streaming limit error: {error}",
      transport.name()
    );
    wait_for_server(done, handle, "over streaming limit");
  }
}

#[test]
fn http2_buffered_decoding_exposes_trailers() {
  for transport in transports() {
    let body = gzip(b"trailer after completion");
    let (addr, done, handle) = serve_response("gzip", body, true);
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/trailers"));
    let response = transport.emit(&mut client).expect("trailer response");
    assert_eq!(
      Some(&"after-eof".to_string()),
      response
        .trailers()
        .iter()
        .find(|header| { header.name().eq_ignore_ascii_case("x-streaming-trailer") })
        .map(|header| header.value())
    );
    wait_for_server(done, handle, "trailers");
  }
}

#[test]
fn http2_live_streaming_decoding_exposes_trailers_after_eof() {
  for transport in transports() {
    let body = gzip(b"trailer after completion");
    let (addr, done, handle) = serve_response("gzip", body, true);
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/trailers"));
    let mut response = transport
      .emit_streaming(&mut client)
      .expect("streaming trailer response");
    assert_eq!(
      b"trailer after completion".to_vec(),
      read_streaming_body(&mut response).expect("streaming trailer body")
    );
    assert_eq!(
      Some(&"after-eof".to_string()),
      response
        .trailers()
        .iter()
        .find(|header| { header.name().eq_ignore_ascii_case("x-streaming-trailer") })
        .map(|header| header.value())
    );
    wait_for_server(done, handle, "streaming trailers");
  }
}

#[test]
fn http2_streaming_decoding_early_drop_finishes_server() {
  for transport in transports() {
    let (addr, done, handle) = serve_early_drop_response();
    let mut client = client_for(None);
    client.get().url(format!("http://{addr}/early-drop"));
    let mut response = transport
      .emit_streaming(&mut client)
      .unwrap_or_else(|error| panic!("{} early-drop response: {error}", transport.name()));
    let mut prefix = [0u8; 3];
    assert!(response.read(&mut prefix).expect("read early-drop prefix") > 0);
    drop(response);
    let result = done
      .recv_timeout(Duration::from_secs(2))
      .expect("early-drop server did not terminate");
    assert!(
      result.is_ok()
        || result.as_ref().err().is_some_and(|error| {
          matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
              | io::ErrorKind::ConnectionReset
              | io::ErrorKind::UnexpectedEof
          )
        }),
      "{} server cleanup failed: {result:?}",
      transport.name()
    );
    handle.join().expect("h2c early-drop server thread");
  }
}
