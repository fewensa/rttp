use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_client::{DavClass, HttpClient};
use rttp_server::server::{Http2ServerPolicy, HttpResponse, HttpScheduleTag, HttpServer, Request};

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

#[test]
fn h2c_dnt_helper_reaches_server_accessor() {
  for value in ["0", "1"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c DNT server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.target().to_string(),
            request.header("DNT").map(str::to_string),
            request
              .dnt()
              .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
              .map_err(|error| error.to_string()),
          ))
          .expect("record DNT");
          HttpResponse::ok("ok")
        })
        .expect("serve h2c DNT request");
    });

    let response = HttpClient::new()
      .get()
      .url(format!("http://{addr}/catalog"))
      .dnt(value)
      .expect("DNT should be accepted")
      .emit_http2_prior_knowledge()
      .expect("receive h2c response");

    assert_eq!("ok", response.body().string().expect("h2c response body"));
    assert_eq!(
      (
        "/catalog".to_string(),
        Some(value.to_string()),
        Ok(Some(value.to_string()))
      ),
      rx.recv_timeout(Duration::from_secs(2))
        .expect("recorded DNT")
    );
    handle.join().expect("h2c server thread");
  }
}

#[test]
fn h2c_malformed_dnt_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed DNT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("DNT").map(str::to_string),
          request.dnt().is_err(),
        ))
        .expect("record malformed DNT");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c DNT request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/catalog"))
    .header(("DNT", "?1"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("?1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed DNT")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_duplicate_dnt_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate DNT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("DNT").map(str::to_string),
          request.dnt().is_err(),
        ))
        .expect("record duplicate DNT");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c DNT request");
  });

  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/catalog"),
      (":authority", &addr.to_string()),
      ("dnt", "1"),
      ("dnt", "0"),
    ],
  );

  assert_eq!(
    (Some("1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate DNT")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_oversized_dnt_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized DNT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request.header("DNT").map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.dnt().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized DNT");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c DNT request");
  });

  let oversized = "1".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/catalog"))
    .header(("DNT", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized DNT")
  );
  handle.join().expect("h2c server thread");
}

const WEBDAV_LOCK_TOKEN: &str = "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>";
const WEBDAV_LOCK_TOKEN_MATERIAL: &str = "550e8400-e29b-41d4-a716-446655440000";
const WEBDAV_IF: &str = "(<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>)";
const WEBDAV_DESTINATION: &str = "https://dav.example.test/archive/source.txt";
const WEBDAV_SCHEDULE_TAG: &str = "\"sched-17\"";
const WEBDAV_DAV: &str = "1, 2, extended-mkcol, <https://dav.example.test/ns>";

type ObservedH2cWebDavField = (String, Option<String>, bool);

#[derive(Debug, PartialEq)]
struct ObservedH2cWebDavMetadata {
  version: String,
  depth: Result<Option<String>, String>,
  raw_depth: Option<String>,
  destination: Result<Option<String>, String>,
  raw_destination: Option<String>,
  overwrite: Result<Option<String>, String>,
  raw_overwrite: Option<String>,
  timeout: Result<Option<String>, String>,
  raw_timeout: Option<String>,
  lock_token: Result<Option<String>, String>,
  raw_lock_token: Option<String>,
  if_header: Result<Option<String>, String>,
  raw_if: Option<String>,
  if_schedule_tag_match: Result<Option<String>, String>,
  raw_if_schedule_tag_match: Option<String>,
  request_debug: String,
  lock_token_debug: String,
  if_debug: String,
}

fn observe_h2c_webdav_metadata(request: &Request) -> ObservedH2cWebDavMetadata {
  let lock_token = request.lock_token();
  let if_header = request.if_header();
  ObservedH2cWebDavMetadata {
    version: request.version().to_string(),
    depth: request
      .depth()
      .map(|depth| depth.map(|depth| depth.header_value().to_string()))
      .map_err(|error| error.to_string()),
    raw_depth: request.header("Depth").map(str::to_string),
    destination: request
      .destination()
      .map(|destination| destination.map(|destination| destination.header_value()))
      .map_err(|error| error.to_string()),
    raw_destination: request.header("Destination").map(str::to_string),
    overwrite: request
      .overwrite()
      .map(|overwrite| overwrite.map(|overwrite| overwrite.header_value().to_string()))
      .map_err(|error| error.to_string()),
    raw_overwrite: request.header("Overwrite").map(str::to_string),
    timeout: request
      .timeout()
      .map(|timeout| timeout.map(|timeout| timeout.header_value()))
      .map_err(|error| error.to_string()),
    raw_timeout: request.header("Timeout").map(str::to_string),
    lock_token: lock_token
      .as_ref()
      .map(|token| token.as_ref().map(|token| token.header_value()))
      .map_err(|error| error.to_string()),
    raw_lock_token: request.header("Lock-Token").map(str::to_string),
    if_header: if_header
      .as_ref()
      .map(|value| value.as_ref().map(|value| value.header_value()))
      .map_err(|error| error.to_string()),
    raw_if: request.header("If").map(str::to_string),
    if_schedule_tag_match: request
      .if_schedule_tag_match()
      .map(|tag| tag.map(|tag| tag.header_value()))
      .map_err(|error| error.to_string()),
    raw_if_schedule_tag_match: request.header("If-Schedule-Tag-Match").map(str::to_string),
    request_debug: format!("{request:?}"),
    lock_token_debug: match &lock_token {
      Ok(Some(token)) => format!("{token:?}"),
      other => format!("{other:?}"),
    },
    if_debug: match &if_header {
      Ok(Some(value)) => format!("{value:?}"),
      other => format!("{other:?}"),
    },
  }
}

fn h2c_webdav_field_parse_failed(request: &Request, name: &str) -> bool {
  match name {
    "Depth" | "depth" => request.depth().is_err(),
    "Destination" | "destination" => request.destination().is_err(),
    "Overwrite" | "overwrite" => request.overwrite().is_err(),
    "Timeout" | "timeout" => request.timeout().is_err(),
    "Lock-Token" | "lock-token" => request.lock_token().is_err(),
    "If" | "if" => request.if_header().is_err(),
    "If-Schedule-Tag-Match" | "if-schedule-tag-match" => request.if_schedule_tag_match().is_err(),
    other => panic!("unexpected WebDAV request field {other}"),
  }
}

fn spawn_h2c_webdav_observer() -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedH2cWebDavMetadata>,
  thread::JoinHandle<()>,
) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c WebDAV server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c WebDAV server addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(observe_h2c_webdav_metadata(&request))
          .expect("record h2c WebDAV metadata");
        HttpResponse::new(207, "Multi-Status")
          .with_dav(WEBDAV_DAV)
          .expect("DAV should be accepted")
          .with_schedule_tag(
            HttpScheduleTag::parse(WEBDAV_SCHEDULE_TAG).expect("Schedule-Tag should parse"),
          )
          .with_lock_token(WEBDAV_LOCK_TOKEN)
          .expect("response Lock-Token should be accepted")
      })
      .expect("serve h2c WebDAV request");
  });
  (addr, rx, handle)
}

fn spawn_h2c_webdav_field_observer(
  field: &'static str,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedH2cWebDavField>,
  thread::JoinHandle<()>,
) {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c WebDAV field server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c WebDAV field server addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.version().to_string(),
          request.header(field).map(str::to_string),
          h2c_webdav_field_parse_failed(&request, field),
        ))
        .expect("record h2c WebDAV field");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c WebDAV field request");
  });
  (addr, rx, handle)
}

#[test]
fn h2c_prior_knowledge_round_trip_preserves_webdav_metadata_matrix() {
  let (addr, rx, handle) = spawn_h2c_webdav_observer();

  let response = HttpClient::new()
    .put()
    .url(format!("http://{addr}/documents/source.txt"))
    .depth("INFINITY")
    .expect("Depth should be accepted")
    .destination(WEBDAV_DESTINATION)
    .expect("Destination should be accepted")
    .overwrite("F")
    .expect("Overwrite should be accepted")
    .timeout("Second-60, Infinite")
    .expect("Timeout should be accepted")
    .lock_token(WEBDAV_LOCK_TOKEN)
    .expect("Lock-Token should be accepted")
    .if_header(WEBDAV_IF)
    .expect("If should be accepted")
    .if_schedule_tag_match(WEBDAV_SCHEDULE_TAG)
    .expect("If-Schedule-Tag-Match should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c WebDAV response");

  let observed = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("recorded h2c WebDAV metadata");
  assert_eq!("HTTP/2", observed.version);
  assert_eq!(Ok(Some("infinity".to_string())), observed.depth);
  assert_eq!(Some("infinity".to_string()), observed.raw_depth);
  assert_eq!(
    Ok(Some(WEBDAV_DESTINATION.to_string())),
    observed.destination
  );
  assert_eq!(
    Some(WEBDAV_DESTINATION.to_string()),
    observed.raw_destination
  );
  assert_eq!(Ok(Some("F".to_string())), observed.overwrite);
  assert_eq!(Some("F".to_string()), observed.raw_overwrite);
  assert_eq!(
    Ok(Some("second-60, infinite".to_string())),
    observed.timeout
  );
  assert_eq!(
    Some("second-60, infinite".to_string()),
    observed.raw_timeout
  );
  assert_eq!(Ok(Some(WEBDAV_LOCK_TOKEN.to_string())), observed.lock_token);
  assert_eq!(Some(WEBDAV_LOCK_TOKEN.to_string()), observed.raw_lock_token);
  assert_eq!(Ok(Some(WEBDAV_IF.to_string())), observed.if_header);
  assert_eq!(Some(WEBDAV_IF.to_string()), observed.raw_if);
  assert_eq!(
    Ok(Some(WEBDAV_SCHEDULE_TAG.to_string())),
    observed.if_schedule_tag_match
  );
  assert_eq!(
    Some(WEBDAV_SCHEDULE_TAG.to_string()),
    observed.raw_if_schedule_tag_match
  );
  assert!(observed.request_debug.contains("[REDACTED]"));
  assert!(!observed.request_debug.contains(WEBDAV_LOCK_TOKEN_MATERIAL));
  assert!(!observed
    .lock_token_debug
    .contains(WEBDAV_LOCK_TOKEN_MATERIAL));
  assert!(!observed.if_debug.contains(WEBDAV_LOCK_TOKEN_MATERIAL));

  assert_eq!("HTTP/2", response.version());
  assert_eq!(207, response.code());
  let dav = response
    .dav()
    .expect("DAV should parse")
    .expect("DAV should be present");
  assert_eq!(
    &[
      DavClass::One,
      DavClass::Two,
      DavClass::ExtensionToken("extended-mkcol".to_string()),
      DavClass::CodedUrl("https://dav.example.test/ns".to_string()),
    ],
    dav.classes()
  );
  let schedule_tag = response
    .schedule_tag()
    .expect("Schedule-Tag should parse")
    .expect("Schedule-Tag should be present");
  assert_eq!(WEBDAV_SCHEDULE_TAG, schedule_tag.header_value());
  let lock_token = response
    .lock_token()
    .expect("response Lock-Token should parse")
    .expect("response Lock-Token should be present");
  assert_eq!(WEBDAV_LOCK_TOKEN, lock_token.as_str());
  assert!(!format!("{lock_token:?}").contains(WEBDAV_LOCK_TOKEN_MATERIAL));
  handle.join().expect("h2c WebDAV server thread");
}

#[test]
fn h2c_prior_knowledge_rejects_malformed_webdav_metadata_without_losing_raw_headers() {
  for (name, value) in [
    ("Depth", "2"),
    ("Destination", "/relative"),
    ("Overwrite", "true"),
    ("Timeout", "Second-"),
    ("Lock-Token", "<relative>"),
    ("If", "(junk)"),
    ("If-Schedule-Tag-Match", "*"),
  ] {
    let (addr, rx, handle) = spawn_h2c_webdav_field_observer(name);
    let response = HttpClient::new()
      .put()
      .url(format!("http://{addr}/workspace/h2c-webdav-malformed"))
      .header((name, value))
      .emit_http2_prior_knowledge()
      .expect("receive malformed h2c WebDAV response");
    let (version, raw, failed) = rx
      .recv_timeout(Duration::from_secs(2))
      .unwrap_or_else(|_| panic!("recorded malformed h2c {name}"));
    assert_eq!("HTTP/2", version);
    assert_eq!(
      Some(value.to_string()),
      raw,
      "raw {name} must remain visible"
    );
    assert!(failed, "malformed {name} must fail closed");
    assert_eq!("HTTP/2", response.version());
    handle
      .join()
      .unwrap_or_else(|_| panic!("malformed h2c {name} server thread"));
  }
}

#[test]
fn h2c_prior_knowledge_rejects_duplicate_webdav_metadata_without_losing_raw_headers() {
  for (name, first, second) in [
    ("depth", "0", "1"),
    (
      "destination",
      "https://dav.example.test/one",
      "https://dav.example.test/two",
    ),
    ("overwrite", "T", "F"),
    ("timeout", "Second-60", "second-60"),
    (
      "lock-token",
      WEBDAV_LOCK_TOKEN,
      "<http://example.test/locks/2>",
    ),
    ("if", "(<a:b>)", "(<b:c>)"),
    ("if-schedule-tag-match", "\"sched-16\"", WEBDAV_SCHEDULE_TAG),
  ] {
    let (addr, rx, handle) = spawn_h2c_webdav_field_observer(name);
    let authority = addr.to_string();
    let _stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "COPY"),
        (":scheme", "http"),
        (":path", "/workspace/h2c-webdav-duplicate"),
        (":authority", &authority),
        (name, first),
        (name, second),
      ],
    );
    let (version, raw, failed) = rx
      .recv_timeout(Duration::from_secs(2))
      .unwrap_or_else(|_| panic!("recorded duplicate h2c {name}"));
    assert_eq!("HTTP/2", version);
    assert_eq!(
      Some(first.to_string()),
      raw,
      "raw {name} must remain visible"
    );
    assert!(failed, "duplicate {name} must fail closed");
    handle
      .join()
      .unwrap_or_else(|_| panic!("duplicate h2c {name} server thread"));
  }
}

#[test]
fn h2c_prior_knowledge_rejects_oversized_webdav_metadata_without_losing_raw_headers() {
  for name in [
    "Depth",
    "Destination",
    "Overwrite",
    "Timeout",
    "Lock-Token",
    "If",
    "If-Schedule-Tag-Match",
  ] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind oversized h2c WebDAV server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)))
      .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
    let addr = server
      .local_addr()
      .expect("oversized h2c WebDAV server addr");
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let raw = request.header(name).map(str::to_string);
          tx.send((
            request.version().to_string(),
            raw.as_ref().map(String::len),
            h2c_webdav_field_parse_failed(&request, name),
            raw.is_some(),
          ))
          .expect("record oversized h2c WebDAV field");
          HttpResponse::ok("ok")
        })
        .expect("serve oversized h2c WebDAV request");
    });

    let oversized = "x".repeat(64 * 1024 + 1);
    let response = HttpClient::new()
      .put()
      .url(format!("http://{addr}/workspace/h2c-webdav-bounds"))
      .header((name, oversized.as_str()))
      .emit_http2_prior_knowledge()
      .expect("receive oversized h2c WebDAV response");

    assert_eq!(
      ("HTTP/2".to_string(), Some(64 * 1024 + 1), true, true),
      rx.recv_timeout(Duration::from_secs(2))
        .unwrap_or_else(|_| panic!("recorded oversized h2c {name}"))
    );
    assert_eq!("ok", response.body().string().expect("h2c response body"));
    handle
      .join()
      .unwrap_or_else(|_| panic!("oversized h2c {name} server thread"));
  }
}

#[test]
fn h2c_prior_knowledge_rejects_malformed_webdav_response_metadata_without_losing_raw_headers() {
  for (name, value) in [
    ("DAV", "1, 1"),
    ("Schedule-Tag", "*"),
    ("Lock-Token", "<relative>"),
  ] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind malformed h2c WebDAV response server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server
      .local_addr()
      .expect("malformed h2c WebDAV response server addr");
    let handle = thread::spawn(move || {
      server
        .accept_one(|_| HttpResponse::ok("ok").header(name, value))
        .expect("serve malformed h2c WebDAV response");
    });

    let response = HttpClient::new()
      .get()
      .url(format!("http://{addr}/workspace/h2c-webdav-response"))
      .emit_http2_prior_knowledge()
      .expect("malformed h2c WebDAV response should remain parseable");
    assert_eq!(Some(&value.to_string()), response.header_value(name));
    let failed = match name {
      "DAV" => response.dav().is_err(),
      "Schedule-Tag" => response.schedule_tag().is_err(),
      "Lock-Token" => response.lock_token().is_err(),
      other => panic!("unexpected WebDAV response field {other}"),
    };
    assert!(failed, "malformed h2c {name} must fail closed");
    handle
      .join()
      .unwrap_or_else(|_| panic!("malformed h2c {name} response thread"));
  }
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
