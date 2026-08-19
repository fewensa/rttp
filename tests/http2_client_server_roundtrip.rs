use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_client::HttpClient;
use rttp_server::server::{Http2ServerPolicy, HttpResponse, HttpServer};

#[test]
fn bounded_h2c_prior_knowledge_round_trip_reaches_the_server() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.method().to_string(),
          request.target().to_string(),
        ))
        .expect("record h2c request");
        HttpResponse::ok("workspace h2c")
      })
      .expect("serve h2c request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/workspace/h2c?matrix=true"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!(
    (
      "HTTP/2".to_string(),
      "GET".to_string(),
      "/workspace/h2c?matrix=true".to_string()
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded h2c request")
  );
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "workspace h2c",
    response.body().string().expect("h2c response body")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_prior_knowledge_round_trip_preserves_accept_charset_metadata() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let parsed = request
          .accept_charset()
          .map(|charsets| {
            charsets.map(|charsets| {
              charsets
                .charsets()
                .iter()
                .map(|range| (range.charset().to_owned(), range.quality()))
                .collect::<Vec<_>>()
            })
          })
          .map_err(|error| error.to_string());
        tx.send((
          request.version().to_string(),
          request.header("Accept-Charset").map(str::to_owned),
          parsed,
        ))
        .expect("record h2c Accept-Charset request");
        HttpResponse::ok("h2c accept-charset")
      })
      .expect("serve h2c Accept-Charset request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/workspace/h2c-accept-charset"))
    .accept_charset("utf-8")
    .expect("utf-8 should be accepted")
    .accept_charset_with_q("iso-8859-1", "0.5")
    .expect("iso-8859-1 quality should be accepted")
    .accept_charset_with_q("*", "0")
    .expect("wildcard quality should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c Accept-Charset response");

  assert_eq!(
    (
      "HTTP/2".to_string(),
      Some("utf-8, iso-8859-1;q=0.5, *;q=0".to_string()),
      Ok(Some(vec![
        ("utf-8".to_string(), 1000),
        ("iso-8859-1".to_string(), 500),
        ("*".to_string(), 0),
      ]))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded h2c Accept-Charset request")
  );
  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    "h2c accept-charset",
    response.body().string().expect("h2c response body")
  );
  handle.join().expect("h2c Accept-Charset server thread");
}

#[test]
fn h2c_prior_knowledge_rejects_malformed_accept_charset_without_losing_raw_headers() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.header("Accept-Charset").map(str::to_owned),
          request
            .accept_charset()
            .map(|_| ())
            .map_err(|error| error.to_string()),
        ))
        .expect("record malformed h2c Accept-Charset request");
        HttpResponse::ok("h2c accept-charset malformed")
      })
      .expect("serve malformed h2c Accept-Charset request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!(
      "http://{addr}/workspace/h2c-accept-charset-malformed"
    ))
    .header(("Accept-Charset", "utf-8, UTF-8"))
    .emit_http2_prior_knowledge()
    .expect("receive malformed h2c Accept-Charset response");

  let (version, raw, parsed) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("recorded malformed h2c Accept-Charset request");
  assert_eq!("HTTP/2", version);
  assert_eq!(Some("utf-8, UTF-8".to_string()), raw);
  assert!(parsed.is_err(), "malformed Accept-Charset must fail closed");
  assert_eq!("HTTP/2", response.version());
  handle
    .join()
    .expect("malformed h2c Accept-Charset server thread");
}

#[test]
fn h2c_prior_knowledge_round_trip_preserves_metadata_and_response_trailers() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        assert_eq!("HTTP/2", request.version());
        assert_eq!(Some("client-context"), request.header("x-request-context"));
        let priority = request
          .priority()
          .expect("parse request Priority")
          .expect("request Priority is present");
        assert_eq!(Some(1), priority.urgency());
        assert!(priority.incremental());
        assert_eq!(Some("token"), priority.extensions()[0].value());

        HttpResponse::ok("h2c metadata")
          .header("X-Response-Context", "server-context")
          .with_priority("u=3, i=?0, x=response")
          .expect("build response Priority")
          .with_server_timing("db;dur=53.2;desc=\"primary database\";region=us-east")
          .expect("build response Server-Timing")
          .trailer("X-Response-Trace", "trailer-context")
      })
      .expect("serve h2c metadata request");
  });

  let mut client = HttpClient::new();
  let response = client
    .get()
    .url(format!("http://{addr}/workspace/h2c-metadata"))
    .header(("X-Request-Context", "client-context"))
    .priority("u=1, i, x=token")
    .expect("configure request Priority")
    .emit_http2_prior_knowledge()
    .expect("receive h2c metadata response");

  assert_eq!("HTTP/2", response.version());
  assert_eq!(
    Some(&"server-context".to_string()),
    response.header_value("x-response-context")
  );
  assert_eq!(
    Some(&"u=3, i=?0, x=response".to_string()),
    response.header_value("priority")
  );
  assert_eq!(
    Some(&"db; dur=53.2; desc=\"primary database\"; region=us-east".to_string()),
    response.header_value("server-timing")
  );
  assert_eq!(
    Some(&"trailer-context".to_string()),
    response.trailer_value("x-response-trace")
  );
  assert_eq!(
    vec![("x-response-trace", "trailer-context")],
    response
      .trailers()
      .iter()
      .map(|trailer| (trailer.name().as_str(), trailer.value().as_str()))
      .collect::<Vec<_>>()
  );

  let priority = response
    .priority()
    .expect("parse response Priority")
    .expect("response Priority is present");
  assert_eq!(Some(3), priority.urgency());
  assert!(!priority.incremental());
  assert_eq!(Some("response"), priority.extensions()[0].value());

  let timing = response
    .server_timing()
    .expect("parse response Server-Timing")
    .expect("response Server-Timing is present");
  assert_eq!(1, timing.len());
  assert_eq!("db", timing.metrics()[0].name());
  assert_eq!(Some(53.2), timing.metrics()[0].duration());
  assert_eq!(Some("primary database"), timing.metrics()[0].description());

  assert_eq!(
    "h2c metadata",
    response.body().string().expect("h2c response body")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_upgrade_insecure_requests_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Upgrade-Insecure-Requests server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request
            .upgrade_insecure_requests()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Upgrade-Insecure-Requests");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Upgrade-Insecure-Requests request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/page"))
    .upgrade_insecure_requests()
    .expect("Upgrade-Insecure-Requests should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    ("/page".to_string(), Ok(Some("1".to_string()))),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Upgrade-Insecure-Requests")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_malformed_upgrade_insecure_requests_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Upgrade-Insecure-Requests server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Upgrade-Insecure-Requests")
            .map(str::to_string),
          request.upgrade_insecure_requests().is_err(),
        ))
        .expect("record malformed Upgrade-Insecure-Requests");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Upgrade-Insecure-Requests request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/page"))
    .header(("Upgrade-Insecure-Requests", "0"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("0".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Upgrade-Insecure-Requests")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_duplicate_upgrade_insecure_requests_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Upgrade-Insecure-Requests server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Upgrade-Insecure-Requests")
            .map(str::to_string),
          request.upgrade_insecure_requests().is_err(),
        ))
        .expect("record duplicate Upgrade-Insecure-Requests");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Upgrade-Insecure-Requests request");
  });

  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/page"),
      (":authority", &addr.to_string()),
      ("upgrade-insecure-requests", "1"),
      ("upgrade-insecure-requests", "1"),
    ],
  );

  assert_eq!(
    (Some("1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Upgrade-Insecure-Requests")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_oversized_upgrade_insecure_requests_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Upgrade-Insecure-Requests server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Upgrade-Insecure-Requests")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.upgrade_insecure_requests().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Upgrade-Insecure-Requests");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Upgrade-Insecure-Requests request");
  });

  let oversized = "1".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/page"))
    .header(("Upgrade-Insecure-Requests", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Upgrade-Insecure-Requests")
  );
  handle.join().expect("h2c server thread");
}

fn send_h2c_prior_knowledge_headers(
  addr: std::net::SocketAddr,
  fields: &[(&str, &str)],
) -> TcpStream {
  let mut stream = TcpStream::connect(addr).expect("connect raw h2c client");
  stream
    .set_read_timeout(Some(Duration::from_secs(2)))
    .expect("set raw h2c read timeout");
  stream
    .set_write_timeout(Some(Duration::from_secs(2)))
    .expect("set raw h2c write timeout");
  stream
    .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
    .expect("write HTTP/2 preface");
  write_http2_frame(&mut stream, 0x4, 0, 0, &[]);

  let mut saw_settings = false;
  let mut saw_settings_ack = false;
  while !saw_settings || !saw_settings_ack {
    let frame = read_http2_frame(&mut stream);
    if frame.0 == 0x4 && frame.1 & 0x1 == 0 {
      saw_settings = true;
    }
    if frame.0 == 0x4 && frame.1 & 0x1 == 0x1 {
      saw_settings_ack = true;
    }
  }
  write_http2_frame(&mut stream, 0x4, 0x1, 0, &[]);

  let mut block = Vec::new();
  for (name, value) in fields {
    block.push(0);
    encode_hpack_string(&mut block, name.as_bytes());
    encode_hpack_string(&mut block, value.as_bytes());
  }
  write_http2_frame(&mut stream, 0x1, 0x1 | 0x4, 1, &block);
  stream
}

fn encode_hpack_string(block: &mut Vec<u8>, value: &[u8]) {
  assert!(
    value.len() < 127,
    "raw h2c test helper only encodes short HPACK strings"
  );
  block.push(value.len() as u8);
  block.extend_from_slice(value);
}

fn write_http2_frame(
  stream: &mut impl Write,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) {
  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream
    .write_all(&header)
    .expect("write HTTP/2 frame header");
  stream
    .write_all(payload)
    .expect("write HTTP/2 frame payload");
  stream.flush().expect("flush HTTP/2 frame");
}

fn read_http2_frame(stream: &mut impl Read) -> (u8, u8, u32, Vec<u8>) {
  let mut header = [0; 9];
  stream
    .read_exact(&mut header)
    .expect("read HTTP/2 frame header");
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  let mut payload = vec![0; length];
  stream
    .read_exact(&mut payload)
    .expect("read HTTP/2 frame payload");
  let stream_id = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) & 0x7fff_ffff;
  (header[3], header[4], stream_id, payload)
}
