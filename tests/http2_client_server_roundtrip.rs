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
fn h2c_rtt_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c RTT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request.header("RTT").map(str::to_string),
          request
            .rtt()
            .map(|metadata| metadata.map(|metadata| metadata.header_value()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record RTT");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c RTT request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .rtt("\t150\t")
    .expect("RTT should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("150".to_string()),
      Ok(Some("150".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded RTT")
  );
  handle.join().expect("h2c server thread");
}

#[test]
fn h2c_downlink_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Downlink server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request.header("Downlink").map(str::to_string),
          request
            .downlink()
            .map(|metadata| metadata.map(|metadata| metadata.header_value()))
            .map_err(|error| error.to_string()),
          request
            .downlink()
            .ok()
            .flatten()
            .map(|metadata| metadata.mbps()),
        ))
        .expect("record Downlink");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Downlink request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .downlink("\t10.25\t")
    .expect("Downlink should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("10.25".to_string()),
      Ok(Some("10.25".to_string())),
      Some(10.25)
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Downlink")
  );
  handle.join().expect("h2c Downlink server thread");
}

#[test]
fn h2c_malformed_downlink_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Downlink server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("Downlink").map(str::to_string),
          request.downlink().is_err(),
        ))
        .expect("record malformed Downlink");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Downlink request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Downlink", "1e1"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("1e1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Downlink")
  );
  handle.join().expect("malformed h2c Downlink server thread");
}

#[test]
fn h2c_duplicate_downlink_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Downlink server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("Downlink").map(str::to_string),
          request.downlink().is_err(),
        ))
        .expect("record duplicate Downlink");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Downlink request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Downlink", "1"),
      ("downlink", "2"),
    ],
  );

  assert_eq!(
    (Some("1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Downlink")
  );
  handle.join().expect("duplicate h2c Downlink server thread");
}

#[test]
fn h2c_oversized_downlink_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Downlink server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request.header("Downlink").map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.downlink().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Downlink");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Downlink request");
  });

  let oversized = "1".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Downlink", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Downlink")
  );
  handle.join().expect("oversized h2c Downlink server thread");
}

#[test]
fn h2c_device_memory_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Device-Memory server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request.header("Device-Memory").map(str::to_string),
          request
            .device_memory()
            .map(|metadata| metadata.map(|metadata| metadata.header_value()))
            .map_err(|error| error.to_string()),
          request
            .device_memory()
            .ok()
            .flatten()
            .map(|metadata| metadata.gib()),
        ))
        .expect("record Device-Memory");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Device-Memory request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .device_memory("\t8\t")
    .expect("Device-Memory should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("8".to_string()),
      Ok(Some("8".to_string())),
      Some(8.0)
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Device-Memory")
  );
  handle.join().expect("h2c Device-Memory server thread");
}

#[test]
fn h2c_malformed_device_memory_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Device-Memory server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("Device-Memory").map(str::to_string),
          request.device_memory().is_err(),
        ))
        .expect("record malformed Device-Memory");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Device-Memory request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Device-Memory", "1e1"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("1e1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Device-Memory")
  );
  handle
    .join()
    .expect("malformed h2c Device-Memory server thread");
}

#[test]
fn h2c_duplicate_device_memory_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Device-Memory server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("Device-Memory").map(str::to_string),
          request.device_memory().is_err(),
        ))
        .expect("record duplicate Device-Memory");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Device-Memory request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Device-Memory", "1"),
      ("device-memory", "2"),
    ],
  );

  assert_eq!(
    (Some("1".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Device-Memory")
  );
  handle
    .join()
    .expect("duplicate h2c Device-Memory server thread");
}

#[test]
fn h2c_oversized_device_memory_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Device-Memory server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request.header("Device-Memory").map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.device_memory().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Device-Memory");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Device-Memory request");
  });

  let oversized = "1".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Device-Memory", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Device-Memory")
  );
  handle
    .join()
    .expect("oversized h2c Device-Memory server thread");
}

#[test]
fn h2c_prefers_color_scheme_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Prefers-Color-Scheme server")
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
            .header("Sec-CH-Prefers-Color-Scheme")
            .map(str::to_string),
          request
            .prefers_color_scheme()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Prefers-Color-Scheme");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Prefers-Color-Scheme request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .prefers_color_scheme("\tDaRk\t")
    .expect("Prefers-Color-Scheme should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("dark".to_string()),
      Ok(Some("dark".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Prefers-Color-Scheme")
  );
  handle
    .join()
    .expect("h2c Prefers-Color-Scheme server thread");
}

#[test]
fn h2c_malformed_prefers_color_scheme_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Prefers-Color-Scheme server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Color-Scheme")
            .map(str::to_string),
          request.prefers_color_scheme().is_err(),
        ))
        .expect("record malformed Prefers-Color-Scheme");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Prefers-Color-Scheme request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Color-Scheme", "system"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("system".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Prefers-Color-Scheme")
  );
  handle
    .join()
    .expect("malformed h2c Prefers-Color-Scheme server thread");
}

#[test]
fn h2c_duplicate_prefers_color_scheme_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Prefers-Color-Scheme server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Color-Scheme")
            .map(str::to_string),
          request.prefers_color_scheme().is_err(),
        ))
        .expect("record duplicate Prefers-Color-Scheme");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Prefers-Color-Scheme request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Sec-CH-Prefers-Color-Scheme", "light"),
      ("sec-ch-prefers-color-scheme", "dark"),
    ],
  );

  assert_eq!(
    (Some("light".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Prefers-Color-Scheme")
  );
  handle
    .join()
    .expect("duplicate h2c Prefers-Color-Scheme server thread");
}

#[test]
fn h2c_oversized_prefers_color_scheme_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Prefers-Color-Scheme server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Sec-CH-Prefers-Color-Scheme")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.prefers_color_scheme().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Prefers-Color-Scheme");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Prefers-Color-Scheme request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Color-Scheme", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Prefers-Color-Scheme")
  );
  handle
    .join()
    .expect("oversized h2c Prefers-Color-Scheme server thread");
}

#[test]
fn h2c_non_ascii_prefers_color_scheme_reaches_server_accessor_with_raw_header() {
  for value in ["dark🍎", "dark\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c non-ASCII Prefers-Color-Scheme server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request
              .header("Sec-CH-Prefers-Color-Scheme")
              .map(str::to_string),
            request
              .prefers_color_scheme()
              .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
              .map_err(|error| error.to_string()),
          ))
          .expect("record non-ASCII Prefers-Color-Scheme");
          HttpResponse::ok("ok")
        })
        .expect("serve non-ASCII h2c Prefers-Color-Scheme request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("sec-ch-prefers-color-scheme", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded non-ASCII Prefers-Color-Scheme");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed
      .as_ref()
      .expect_err("non-ASCII Prefers-Color-Scheme must fail closed");
    assert!(
      error.contains("Sec-CH-Prefers-Color-Scheme"),
      "non-ASCII Prefers-Color-Scheme error should identify the field: {error}"
    );
    handle
      .join()
      .expect("non-ASCII h2c Prefers-Color-Scheme server thread");
  }
}

#[test]
fn h2c_prefers_reduced_motion_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Prefers-Reduced-Motion server")
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
            .header("Sec-CH-Prefers-Reduced-Motion")
            .map(str::to_string),
          request
            .prefers_reduced_motion()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Prefers-Reduced-Motion");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Prefers-Reduced-Motion request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .prefers_reduced_motion("\tReDuCe\t")
    .expect("Prefers-Reduced-Motion should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("reduce".to_string()),
      Ok(Some("reduce".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Prefers-Reduced-Motion")
  );
  handle
    .join()
    .expect("h2c Prefers-Reduced-Motion server thread");
}

#[test]
fn h2c_malformed_prefers_reduced_motion_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Prefers-Reduced-Motion server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Motion")
            .map(str::to_string),
          request.prefers_reduced_motion().is_err(),
        ))
        .expect("record malformed Prefers-Reduced-Motion");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Prefers-Reduced-Motion request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Motion", "auto"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("auto".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Prefers-Reduced-Motion")
  );
  handle
    .join()
    .expect("malformed h2c Prefers-Reduced-Motion server thread");
}

#[test]
fn h2c_duplicate_prefers_reduced_motion_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Prefers-Reduced-Motion server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Motion")
            .map(str::to_string),
          request.prefers_reduced_motion().is_err(),
        ))
        .expect("record duplicate Prefers-Reduced-Motion");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Prefers-Reduced-Motion request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Sec-CH-Prefers-Reduced-Motion", "no-preference"),
      ("sec-ch-prefers-reduced-motion", "reduce"),
    ],
  );

  assert_eq!(
    (Some("no-preference".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Prefers-Reduced-Motion")
  );
  handle
    .join()
    .expect("duplicate h2c Prefers-Reduced-Motion server thread");
}

#[test]
fn h2c_oversized_prefers_reduced_motion_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Prefers-Reduced-Motion server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Sec-CH-Prefers-Reduced-Motion")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.prefers_reduced_motion().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Prefers-Reduced-Motion");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Prefers-Reduced-Motion request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Motion", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Prefers-Reduced-Motion")
  );
  handle
    .join()
    .expect("oversized h2c Prefers-Reduced-Motion server thread");
}

#[test]
fn h2c_control_and_non_ascii_prefers_reduced_motion() {
  for value in ["reduce\u{0001}", "reduce\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c control/non-ASCII Prefers-Reduced-Motion server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let raw = request
            .header("Sec-CH-Prefers-Reduced-Motion")
            .map(str::to_string);
          let parsed = request
            .prefers_reduced_motion()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string());
          tx.send((raw, parsed))
            .expect("record control/non-ASCII Prefers-Reduced-Motion");
          HttpResponse::ok("ok")
        })
        .expect("serve control/non-ASCII h2c Prefers-Reduced-Motion request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("sec-ch-prefers-reduced-motion", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded control/non-ASCII Prefers-Reduced-Motion");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed
      .as_ref()
      .expect_err("control/non-ASCII Prefers-Reduced-Motion must fail closed");
    assert!(
      error.contains("Sec-CH-Prefers-Reduced-Motion"),
      "control/non-ASCII Prefers-Reduced-Motion error should identify the field: {error}"
    );
    handle
      .join()
      .expect("control/non-ASCII h2c Prefers-Reduced-Motion server thread");
  }
}

#[test]
fn h2c_prefers_reduced_transparency_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Prefers-Reduced-Transparency server")
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
            .header("Sec-CH-Prefers-Reduced-Transparency")
            .map(str::to_string),
          request
            .prefers_reduced_transparency()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Prefers-Reduced-Transparency");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Prefers-Reduced-Transparency request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .prefers_reduced_transparency("\tReDuCe\t")
    .expect("Prefers-Reduced-Transparency should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("reduce".to_string()),
      Ok(Some("reduce".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Prefers-Reduced-Transparency")
  );
  handle
    .join()
    .expect("h2c Prefers-Reduced-Transparency server thread");
}

#[test]
fn h2c_malformed_prefers_reduced_transparency_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Prefers-Reduced-Transparency server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Transparency")
            .map(str::to_string),
          request.prefers_reduced_transparency().is_err(),
        ))
        .expect("record malformed Prefers-Reduced-Transparency");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Prefers-Reduced-Transparency request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Transparency", "auto"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("auto".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Prefers-Reduced-Transparency")
  );
  handle
    .join()
    .expect("malformed h2c Prefers-Reduced-Transparency server thread");
}

#[test]
fn h2c_control_and_non_ascii_prefers_reduced_transparency_reaches_server_accessor_with_raw_header()
{
  for value in ["reduce\u{0001}", "reduce\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c control/non-ASCII Prefers-Reduced-Transparency server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request
              .header("Sec-CH-Prefers-Reduced-Transparency")
              .map(str::to_string),
            request.prefers_reduced_transparency().is_err(),
          ))
          .expect("record control/non-ASCII Prefers-Reduced-Transparency");
          HttpResponse::ok("ok")
        })
        .expect("serve control/non-ASCII h2c Prefers-Reduced-Transparency request");
    });

    let authority = addr.to_string();
    let _stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("Sec-CH-Prefers-Reduced-Transparency", value),
      ],
    );

    assert_eq!(
      (Some(value.to_string()), true),
      rx.recv_timeout(Duration::from_secs(2))
        .expect("recorded control/non-ASCII Prefers-Reduced-Transparency")
    );
    handle
      .join()
      .expect("control/non-ASCII h2c Prefers-Reduced-Transparency server thread");
  }
}

#[test]
fn h2c_duplicate_prefers_reduced_transparency_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Prefers-Reduced-Transparency server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Transparency")
            .map(str::to_string),
          request.prefers_reduced_transparency().is_err(),
        ))
        .expect("record duplicate Prefers-Reduced-Transparency");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Prefers-Reduced-Transparency request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Sec-CH-Prefers-Reduced-Transparency", "no-preference"),
      ("sec-ch-prefers-reduced-transparency", "reduce"),
    ],
  );

  assert_eq!(
    (Some("no-preference".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Prefers-Reduced-Transparency")
  );
  handle
    .join()
    .expect("duplicate h2c Prefers-Reduced-Transparency server thread");
}

#[test]
fn h2c_oversized_prefers_reduced_transparency_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Prefers-Reduced-Transparency server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Sec-CH-Prefers-Reduced-Transparency")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.prefers_reduced_transparency().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Prefers-Reduced-Transparency");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Prefers-Reduced-Transparency request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Transparency", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Prefers-Reduced-Transparency")
  );
  handle
    .join()
    .expect("oversized h2c Prefers-Reduced-Transparency server thread");
}

#[test]
fn h2c_prefers_reduced_data_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Prefers-Reduced-Data server")
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
            .header("Sec-CH-Prefers-Reduced-Data")
            .map(str::to_string),
          request
            .prefers_reduced_data()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Prefers-Reduced-Data");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Prefers-Reduced-Data request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .prefers_reduced_data("\tReDuCe\t")
    .expect("Prefers-Reduced-Data should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("reduce".to_string()),
      Ok(Some("reduce".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Prefers-Reduced-Data")
  );
  handle
    .join()
    .expect("h2c Prefers-Reduced-Data server thread");
}

#[test]
fn h2c_malformed_prefers_reduced_data_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Prefers-Reduced-Data server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Data")
            .map(str::to_string),
          request.prefers_reduced_data().is_err(),
        ))
        .expect("record malformed Prefers-Reduced-Data");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Prefers-Reduced-Data request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Data", "auto"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("auto".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Prefers-Reduced-Data")
  );
  handle
    .join()
    .expect("malformed h2c Prefers-Reduced-Data server thread");
}

#[test]
fn h2c_control_and_non_ascii_prefers_reduced_data_reaches_server_accessor_with_raw_header() {
  for value in ["reduce\u{0001}", "reduce\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c control/non-ASCII Prefers-Reduced-Data server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request
              .header("Sec-CH-Prefers-Reduced-Data")
              .map(str::to_string),
            request.prefers_reduced_data().is_err(),
          ))
          .expect("record control/non-ASCII Prefers-Reduced-Data");
          HttpResponse::ok("ok")
        })
        .expect("serve control/non-ASCII h2c Prefers-Reduced-Data request");
    });

    let authority = addr.to_string();
    let _stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("Sec-CH-Prefers-Reduced-Data", value),
      ],
    );

    assert_eq!(
      (Some(value.to_string()), true),
      rx.recv_timeout(Duration::from_secs(2))
        .expect("recorded control/non-ASCII Prefers-Reduced-Data")
    );
    handle
      .join()
      .expect("control/non-ASCII h2c Prefers-Reduced-Data server thread");
  }
}

#[test]
fn h2c_duplicate_prefers_reduced_data_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Prefers-Reduced-Data server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Reduced-Data")
            .map(str::to_string),
          request.prefers_reduced_data().is_err(),
        ))
        .expect("record duplicate Prefers-Reduced-Data");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Prefers-Reduced-Data request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Sec-CH-Prefers-Reduced-Data", "no-preference"),
      ("sec-ch-prefers-reduced-data", "reduce"),
    ],
  );

  assert_eq!(
    (Some("no-preference".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Prefers-Reduced-Data")
  );
  handle
    .join()
    .expect("duplicate h2c Prefers-Reduced-Data server thread");
}

#[test]
fn h2c_oversized_prefers_reduced_data_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Prefers-Reduced-Data server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Sec-CH-Prefers-Reduced-Data")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.prefers_reduced_data().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Prefers-Reduced-Data");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Prefers-Reduced-Data request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Reduced-Data", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Prefers-Reduced-Data")
  );
  handle
    .join()
    .expect("oversized h2c Prefers-Reduced-Data server thread");
}

#[test]
fn h2c_prefers_contrast_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c Prefers-Contrast server")
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
            .header("Sec-CH-Prefers-Contrast")
            .map(str::to_string),
          request
            .prefers_contrast()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record Prefers-Contrast");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c Prefers-Contrast request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .prefers_contrast("\tCuStOm\t")
    .expect("Prefers-Contrast should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("custom".to_string()),
      Ok(Some("custom".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded Prefers-Contrast")
  );
  handle.join().expect("h2c Prefers-Contrast server thread");
}

#[test]
fn h2c_malformed_prefers_contrast_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed Prefers-Contrast server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Contrast")
            .map(str::to_string),
          request.prefers_contrast().is_err(),
        ))
        .expect("record malformed Prefers-Contrast");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c Prefers-Contrast request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Contrast", "auto"))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some("auto".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded malformed Prefers-Contrast")
  );
  handle
    .join()
    .expect("malformed h2c Prefers-Contrast server thread");
}

#[test]
fn h2c_control_and_non_ascii_prefers_contrast() {
  for value in ["custom\u{0001}", "custom\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c control/non-ASCII Prefers-Contrast server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let raw = request
            .header("Sec-CH-Prefers-Contrast")
            .map(str::to_string);
          let parsed = request
            .prefers_contrast()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string());
          tx.send((raw, parsed))
            .expect("record control/non-ASCII Prefers-Contrast");
          HttpResponse::ok("ok")
        })
        .expect("serve control/non-ASCII h2c Prefers-Contrast request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("sec-ch-prefers-contrast", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded control/non-ASCII Prefers-Contrast");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed
      .as_ref()
      .expect_err("control/non-ASCII Prefers-Contrast must fail closed");
    assert!(
      error.contains("Sec-CH-Prefers-Contrast"),
      "control/non-ASCII Prefers-Contrast error should identify the field: {error}"
    );
    handle
      .join()
      .expect("control/non-ASCII h2c Prefers-Contrast server thread");
  }
}

#[test]
fn h2c_non_ascii_prefers_contrast_reaches_server_accessor_with_raw_header() {
  for value in ["custom🍎", "custom\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c non-ASCII Prefers-Contrast server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request
              .header("Sec-CH-Prefers-Contrast")
              .map(str::to_string),
            request
              .prefers_contrast()
              .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
              .map_err(|error| error.to_string()),
          ))
          .expect("record non-ASCII Prefers-Contrast");
          HttpResponse::ok("ok")
        })
        .expect("serve non-ASCII h2c Prefers-Contrast request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("sec-ch-prefers-contrast", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded non-ASCII Prefers-Contrast");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed
      .as_ref()
      .expect_err("non-ASCII Prefers-Contrast must fail closed");
    assert!(
      error.contains("Sec-CH-Prefers-Contrast"),
      "non-ASCII Prefers-Contrast error should identify the field: {error}"
    );
    handle
      .join()
      .expect("non-ASCII h2c Prefers-Contrast server thread");
  }
}

#[test]
fn h2c_duplicate_prefers_contrast_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate Prefers-Contrast server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request
            .header("Sec-CH-Prefers-Contrast")
            .map(str::to_string),
          request.prefers_contrast().is_err(),
        ))
        .expect("record duplicate Prefers-Contrast");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c Prefers-Contrast request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("Sec-CH-Prefers-Contrast", "more"),
      ("sec-ch-prefers-contrast", "less"),
    ],
  );

  assert_eq!(
    (Some("more".to_string()), true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded duplicate Prefers-Contrast")
  );
  handle
    .join()
    .expect("duplicate h2c Prefers-Contrast server thread");
}

#[test]
fn h2c_oversized_prefers_contrast_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized Prefers-Contrast server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request
          .header("Sec-CH-Prefers-Contrast")
          .map(str::to_string);
        tx.send((
          raw.as_ref().map(String::len),
          request.prefers_contrast().is_err(),
          raw.is_some(),
        ))
        .expect("record oversized Prefers-Contrast");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c Prefers-Contrast request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header(("Sec-CH-Prefers-Contrast", oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded oversized Prefers-Contrast")
  );
  handle
    .join()
    .expect("oversized h2c Prefers-Contrast server thread");
}

#[test]
fn h2c_ect_helper_reaches_server_accessor() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c ECT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.target().to_string(),
          request.header("ECT").map(str::to_string),
          request
            .ect()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record ECT");
        HttpResponse::ok("ok")
      })
      .expect("serve h2c ECT request");
  });

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .ect("\tSLoW-2G\t")
    .expect("ECT should be accepted")
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  assert_eq!(
    (
      "/asset".to_string(),
      Some("slow-2g".to_string()),
      Ok(Some("slow-2g".to_string()))
    ),
    rx.recv_timeout(Duration::from_secs(2))
      .expect("recorded ECT")
  );
  handle.join().expect("h2c ECT server thread");
}

#[test]
fn h2c_malformed_ect_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c malformed ECT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("ECT").map(str::to_string),
          request
            .ect()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record malformed ECT");
        HttpResponse::ok("ok")
      })
      .expect("serve malformed h2c ECT request");
  });

  let authority = addr.to_string();
  let mut stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("ect", "5g"),
    ],
  );

  let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
  assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
  assert_eq!(
    0x4, flags,
    "h2c response headers should end the header block"
  );
  assert_eq!(1, stream_id);
  assert_eq!(
    Some(&0x88),
    payload.first(),
    "h2c response should be 200 OK"
  );

  let (raw, parsed) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("recorded malformed ECT");
  assert_eq!(Some("5g".to_string()), raw);
  let error = parsed.as_ref().expect_err("malformed ECT must fail closed");
  assert!(
    error.contains("ECT"),
    "malformed ECT error should identify the field: {error}"
  );
  handle.join().expect("malformed h2c ECT server thread");
}

#[test]
fn h2c_control_and_non_ascii_ect() {
  for value in ["4g\u{0001}", "4g\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c control/non-ASCII ECT server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let raw = request.header("ECT").map(str::to_string);
          let parsed = request
            .ect()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string());
          tx.send((raw, parsed))
            .expect("record control/non-ASCII ECT");
          HttpResponse::ok("ok")
        })
        .expect("serve control/non-ASCII h2c ECT request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("ect", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded control/non-ASCII ECT");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed
      .as_ref()
      .expect_err("control/non-ASCII ECT must fail closed");
    assert!(
      error.contains("ECT"),
      "control/non-ASCII ECT error should identify the field: {error}"
    );
    handle
      .join()
      .expect("control/non-ASCII h2c ECT server thread");
  }
}

#[test]
fn h2c_non_ascii_ect_reaches_server_accessor_with_raw_header() {
  for value in ["4g🍎", "4g\u{0080}"] {
    let server = HttpServer::bind("127.0.0.1:0")
      .expect("bind h2c non-ASCII ECT server")
      .with_read_timeout(Some(Duration::from_secs(2)))
      .with_write_timeout(Some(Duration::from_secs(2)));
    let addr = server.local_addr().expect("h2c server address");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((
            request.header("ECT").map(str::to_string),
            request
              .ect()
              .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
              .map_err(|error| error.to_string()),
          ))
          .expect("record non-ASCII ECT");
          HttpResponse::ok("ok")
        })
        .expect("serve non-ASCII h2c ECT request");
    });

    let authority = addr.to_string();
    let mut stream = send_h2c_prior_knowledge_headers(
      addr,
      &[
        (":method", "GET"),
        (":scheme", "http"),
        (":path", "/asset"),
        (":authority", authority.as_str()),
        ("ect", value),
      ],
    );

    let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
    assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
    assert_eq!(
      0x4, flags,
      "h2c response headers should end the header block"
    );
    assert_eq!(1, stream_id);
    assert_eq!(
      Some(&0x88),
      payload.first(),
      "h2c response should be 200 OK"
    );

    let (raw, parsed) = rx
      .recv_timeout(Duration::from_secs(2))
      .expect("recorded non-ASCII ECT");
    assert_eq!(Some(value.to_string()), raw);
    let error = parsed.as_ref().expect_err("non-ASCII ECT must fail closed");
    assert!(
      error.contains("ECT"),
      "non-ASCII ECT error should identify the field: {error}"
    );
    handle.join().expect("non-ASCII h2c ECT server thread");
  }
}

#[test]
fn h2c_duplicate_ect_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c duplicate ECT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.header("ECT").map(str::to_string),
          request
            .ect()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record duplicate ECT");
        HttpResponse::ok("ok")
      })
      .expect("serve duplicate h2c ECT request");
  });

  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("ECT", "4g"),
      ("ect", "3g"),
    ],
  );

  let (raw, parsed) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("recorded duplicate ECT");
  assert_eq!(Some("4g".to_string()), raw);
  let error = parsed.as_ref().expect_err("duplicate ECT must fail closed");
  assert!(
    error.contains("ECT"),
    "duplicate ECT error should identify the field: {error}"
  );
  handle.join().expect("duplicate h2c ECT server thread");
}

#[test]
fn h2c_oversized_ect_reaches_server_accessor_with_raw_header() {
  let server = HttpServer::bind("127.0.0.1:0")
    .expect("bind h2c oversized ECT server")
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)))
    .with_http2_policy(
      Http2ServerPolicy::new()
        .with_max_frame_size(256 * 1024)
        .with_max_header_list_size(256 * 1024),
    );
  let addr = server.local_addr().expect("h2c server address");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request.header("ECT").map(str::to_string);
        tx.send((
          raw.clone(),
          request
            .ect()
            .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
            .map_err(|error| error.to_string()),
        ))
        .expect("record oversized ECT");
        HttpResponse::ok("ok")
      })
      .expect("serve oversized h2c ECT request");
  });

  let oversized = "a".repeat(64 * 1024 + 1);
  let authority = addr.to_string();
  let mut stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      ("ect", oversized.as_str()),
    ],
  );

  let (frame_type, flags, stream_id, payload) = read_http2_frame(&mut stream);
  assert_eq!(0x1, frame_type, "h2c response should start with HEADERS");
  assert_eq!(
    0x4, flags,
    "h2c response headers should end the header block"
  );
  assert_eq!(1, stream_id);
  assert_eq!(
    Some(&0x88),
    payload.first(),
    "h2c response should be 200 OK"
  );

  let (raw, parsed) = rx
    .recv_timeout(Duration::from_secs(2))
    .expect("recorded oversized ECT");
  assert_eq!(Some(oversized.clone()), raw);
  let error = parsed.as_ref().expect_err("oversized ECT must fail closed");
  assert!(
    error.contains("ECT"),
    "oversized ECT error should identify the field: {error}"
  );
  handle.join().expect("oversized h2c ECT server thread");
}

#[derive(Clone, Copy)]
struct SecChUaFieldSpec {
  name: &'static str,
  lowercase_name: &'static str,
  valid_input: &'static str,
  valid_value: &'static str,
  malformed_value: &'static str,
  structured_malformed_values: &'static [&'static str],
  duplicate_first: &'static str,
  duplicate_second: &'static str,
  client_helper: fn(&mut HttpClient, &str) -> Result<(), String>,
  accessor: fn(&Request) -> Result<Option<String>, String>,
  valid_version: Option<&'static str>,
}

const SEC_CH_UA_ARCH: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Arch",
  lowercase_name: "sec-ch-ua-arch",
  valid_input: "\t\"x86\" \t",
  valid_value: "\"x86\"",
  malformed_value: "x86",
  structured_malformed_values: &[r#""bad\escape""#, r#""x86";foo=bar"#, r#""x86", "arm64""#],
  duplicate_first: r#""x86""#,
  duplicate_second: r#""arm64""#,
  client_helper: set_sec_ch_ua_arch,
  accessor: observe_sec_ch_ua_arch,
  valid_version: None,
};

const SEC_CH_UA_BITNESS: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Bitness",
  lowercase_name: "sec-ch-ua-bitness",
  valid_input: "\t\"64\" \t",
  valid_value: "\"64\"",
  malformed_value: "64",
  structured_malformed_values: &[r#""bad\escape""#, r#""64";foo=bar"#, r#""64", "32""#],
  duplicate_first: r#""64""#,
  duplicate_second: r#""32""#,
  client_helper: set_sec_ch_ua_bitness,
  accessor: observe_sec_ch_ua_bitness,
  valid_version: None,
};

const SEC_CH_UA_PLATFORM_VERSION: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Platform-Version",
  lowercase_name: "sec-ch-ua-platform-version",
  valid_input: "\t\"14.0.0\" \t",
  valid_value: "\"14.0.0\"",
  malformed_value: "14.0.0",
  structured_malformed_values: &[
    r#""bad\escape""#,
    r#""14.0.0";foo=bar"#,
    r#""14.0.0", "15.0.0""#,
  ],
  duplicate_first: r#""14.0.0""#,
  duplicate_second: r#""15.0.0""#,
  client_helper: set_sec_ch_ua_platform_version,
  accessor: observe_sec_ch_ua_platform_version,
  valid_version: None,
};

const SEC_CH_UA: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA",
  lowercase_name: "sec-ch-ua",
  valid_input: "\t\"Chromium\";v=\"120\", \"Not(A:Brand\";v=\"99.0\" \t",
  valid_value: "\"Chromium\";v=\"120\", \"Not(A:Brand\";v=\"99.0\"",
  malformed_value: "Chromium;v=\"120\"",
  structured_malformed_values: &[
    r#""Chromium""#,
    r#""Chromium";foo="bar""#,
    r#""Chromium";v="120";v="121""#,
  ],
  duplicate_first: r#""Chromium";v="120""#,
  duplicate_second: r#""Firefox";v="121""#,
  client_helper: set_sec_ch_ua,
  accessor: observe_sec_ch_ua,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_FULL_VERSION_LIST: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Full-Version-List",
  lowercase_name: "sec-ch-ua-full-version-list",
  valid_input: "\t\"Chromium\";v=\"120.0\", \"Not(A:Brand\";v=\"99.0\" \t",
  valid_value: "\"Chromium\";v=\"120.0\", \"Not(A:Brand\";v=\"99.0\"",
  malformed_value: "Chromium;v=\"120.0\"",
  structured_malformed_values: &[
    r#""Chromium""#,
    r#""Chromium";foo="bar""#,
    r#""Chromium";v="120.0";v="121.0""#,
  ],
  duplicate_first: r#""Chromium";v="120.0""#,
  duplicate_second: r#""Firefox";v="121.0""#,
  client_helper: set_sec_ch_ua_full_version_list,
  accessor: observe_sec_ch_ua_full_version_list,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_FORM_FACTORS: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Form-Factors",
  lowercase_name: "sec-ch-ua-form-factors",
  valid_input: "\t\"Desktop\", \"Tablet\" \t",
  valid_value: "\"Desktop\", \"Tablet\"",
  malformed_value: "Desktop",
  structured_malformed_values: &[
    r#""Desktop";foo=bar"#,
    r#""Desktop", ("Tablet")"#,
    r#""Desktop", 123"#,
  ],
  duplicate_first: r#""Desktop""#,
  duplicate_second: r#""Tablet""#,
  client_helper: set_sec_ch_ua_form_factors,
  accessor: observe_sec_ch_ua_form_factors,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_PLATFORM: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Platform",
  lowercase_name: "sec-ch-ua-platform",
  valid_input: "\t\"Windows\" \t",
  valid_value: "\"Windows\"",
  malformed_value: "Windows",
  structured_malformed_values: &[
    r#""bad\escape""#,
    r#""Windows";foo=bar"#,
    r#""Windows", "Linux""#,
  ],
  duplicate_first: r#""Windows""#,
  duplicate_second: r#""Linux""#,
  client_helper: set_sec_ch_ua_platform,
  accessor: observe_sec_ch_ua_platform,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_MODEL: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Model",
  lowercase_name: "sec-ch-ua-model",
  valid_input: "\t\"Pixel 8\" \t",
  valid_value: "\"Pixel 8\"",
  malformed_value: "Pixel 8",
  structured_malformed_values: &[
    r#""bad\escape""#,
    r#""Pixel 8";foo=bar"#,
    r#""Pixel 8", "Galaxy S24""#,
  ],
  duplicate_first: r#""Pixel 8""#,
  duplicate_second: r#""Galaxy S24""#,
  client_helper: set_sec_ch_ua_model,
  accessor: observe_sec_ch_ua_model,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_MOBILE: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-Mobile",
  lowercase_name: "sec-ch-ua-mobile",
  valid_input: "\t?1 \t",
  valid_value: "?1",
  malformed_value: "true",
  structured_malformed_values: &[r#"?1;foo=bar"#, "?1, ?1", r#""?1""#],
  duplicate_first: "?0",
  duplicate_second: "?1",
  client_helper: set_sec_ch_ua_mobile,
  accessor: observe_sec_ch_ua_mobile,
  valid_version: Some("HTTP/2"),
};

const SEC_CH_UA_WOW64: SecChUaFieldSpec = SecChUaFieldSpec {
  name: "Sec-CH-UA-WoW64",
  lowercase_name: "sec-ch-ua-wow64",
  valid_input: "\t?1 \t",
  valid_value: "?1",
  malformed_value: "true",
  structured_malformed_values: &[r#"?1;foo=bar"#, "?1, ?1", r#""?1""#],
  duplicate_first: "?0",
  duplicate_second: "?1",
  client_helper: set_sec_ch_ua_wow64,
  accessor: observe_sec_ch_ua_wow64,
  valid_version: Some("HTTP/2"),
};

struct ObservedH2cSecChUa {
  version: String,
  target: String,
  raw: Option<String>,
  parsed: Result<Option<String>, String>,
}

fn set_sec_ch_ua_arch(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_arch(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_bitness(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_bitness(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_platform_version(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_platform_version(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_full_version_list(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_full_version_list(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_form_factors(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_form_factors(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_platform(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_platform(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_mobile(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_mobile(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_model(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_model(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_arch(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_arch()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_bitness(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_bitness()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_platform_version(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_platform_version()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua()
    .map(|metadata| metadata.map(|metadata| metadata.header_value()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_full_version_list(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_full_version_list()
    .map(|metadata| metadata.map(|metadata| metadata.header_value()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_form_factors(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_form_factors()
    .map(|metadata| metadata.map(|metadata| metadata.header_value()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_platform(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_platform()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_mobile(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_mobile()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_model(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_model()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn set_sec_ch_ua_wow64(client: &mut HttpClient, value: &str) -> Result<(), String> {
  client
    .sec_ch_ua_wow64(value)
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn observe_sec_ch_ua_wow64(request: &Request) -> Result<Option<String>, String> {
  request
    .sec_ch_ua_wow64()
    .map(|metadata| metadata.map(|metadata| metadata.header_value().to_string()))
    .map_err(|error| error.to_string())
}

fn spawn_h2c_sec_ch_ua_observer(
  field: SecChUaFieldSpec,
  case: &'static str,
  allow_oversized: bool,
) -> (
  std::net::SocketAddr,
  mpsc::Receiver<ObservedH2cSecChUa>,
  thread::JoinHandle<()>,
) {
  let server = HttpServer::bind("127.0.0.1:0")
    .unwrap_or_else(|error| panic!("bind h2c {case} {} server: {error:?}", field.name))
    .with_read_timeout(Some(Duration::from_secs(2)))
    .with_write_timeout(Some(Duration::from_secs(2)));
  let server = if allow_oversized {
    server.with_http2_policy(Http2ServerPolicy::new().with_max_header_list_size(256 * 1024))
  } else {
    server
  };
  let addr = server
    .local_addr()
    .unwrap_or_else(|error| panic!("h2c {case} {} server address: {error:?}", field.name));
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let raw = request.header(field.name).map(str::to_string);
        tx.send(ObservedH2cSecChUa {
          version: request.version().to_string(),
          target: request.target().to_string(),
          parsed: (field.accessor)(&request),
          raw,
        })
        .unwrap_or_else(|error| panic!("record {case} {}: {error:?}", field.name));
        HttpResponse::ok("ok")
      })
      .unwrap_or_else(|error| panic!("serve h2c {case} {} request: {error:?}", field.name));
  });

  (addr, rx, handle)
}

fn receive_h2c_sec_ch_ua(
  rx: &mpsc::Receiver<ObservedH2cSecChUa>,
  field: SecChUaFieldSpec,
  case: &'static str,
) -> ObservedH2cSecChUa {
  rx.recv_timeout(Duration::from_secs(2))
    .unwrap_or_else(|error| panic!("recorded {case} {}: {error:?}", field.name))
}

fn join_h2c_sec_ch_ua(handle: thread::JoinHandle<()>, field: SecChUaFieldSpec, case: &'static str) {
  handle
    .join()
    .unwrap_or_else(|_| panic!("{case} h2c {} server thread", field.name));
}

fn run_h2c_sec_ch_ua_helper(field: SecChUaFieldSpec) {
  let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "valid", false);
  let mut client = HttpClient::new();
  client.get().url(format!("http://{addr}/asset"));
  (field.client_helper)(&mut client, field.valid_input)
    .unwrap_or_else(|error| panic!("{} should be accepted: {error}", field.name));
  let response = client
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  let observed = receive_h2c_sec_ch_ua(&rx, field, "valid");
  if let Some(version) = field.valid_version {
    assert_eq!(version, observed.version);
  }
  assert_eq!("/asset", observed.target);
  assert_eq!(Some(field.valid_value.to_string()), observed.raw);
  assert_eq!(Ok(Some(field.valid_value.to_string())), observed.parsed);
  join_h2c_sec_ch_ua(handle, field, "valid");
}

fn assert_h2c_sec_ch_ua_error(
  field: SecChUaFieldSpec,
  case: &'static str,
  parsed: &Result<Option<String>, String>,
) {
  let error = parsed
    .as_ref()
    .expect_err("malformed Sec-CH-UA metadata must fail closed");
  assert!(
    error.contains(field.name),
    "{case} {} error should identify the field: {error}",
    field.name
  );
}

fn run_h2c_sec_ch_ua_malformed(field: SecChUaFieldSpec) {
  let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "malformed", false);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header((field.name, field.malformed_value))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  let observed = receive_h2c_sec_ch_ua(&rx, field, "malformed");
  assert_eq!(Some(field.malformed_value.to_string()), observed.raw);
  assert_h2c_sec_ch_ua_error(field, "malformed", &observed.parsed);
  join_h2c_sec_ch_ua(handle, field, "malformed");
}

fn run_h2c_sec_ch_ua_non_ascii(field: SecChUaFieldSpec) {
  for value in [r#""🍎""#, "\"\u{80}\""] {
    let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "non-ASCII", false);
    let response = HttpClient::new()
      .get()
      .url(format!("http://{addr}/asset"))
      .header((field.name, value))
      .emit_http2_prior_knowledge()
      .expect("receive h2c response");

    assert_eq!("ok", response.body().string().expect("h2c response body"));
    let observed = receive_h2c_sec_ch_ua(&rx, field, "non-ASCII");
    assert_eq!(Some(value.to_string()), observed.raw);
    assert_h2c_sec_ch_ua_error(field, "non-ASCII", &observed.parsed);
    join_h2c_sec_ch_ua(handle, field, "non-ASCII");
  }
}

fn run_h2c_sec_ch_ua_structured_malformed(field: SecChUaFieldSpec) {
  for &value in field.structured_malformed_values {
    let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "structured malformed", false);
    let response = HttpClient::new()
      .get()
      .url(format!("http://{addr}/asset"))
      .header((field.name, value))
      .emit_http2_prior_knowledge()
      .expect("receive h2c response");

    assert_eq!("ok", response.body().string().expect("h2c response body"));
    let observed = receive_h2c_sec_ch_ua(&rx, field, "structured malformed");
    assert_eq!(Some(value.to_string()), observed.raw);
    assert_h2c_sec_ch_ua_error(field, "structured malformed", &observed.parsed);
    join_h2c_sec_ch_ua(handle, field, "structured malformed");
  }
}

fn run_h2c_sec_ch_ua_duplicate(field: SecChUaFieldSpec) {
  run_h2c_sec_ch_ua_repeated_fields(field, None);
}

fn run_h2c_sec_ch_ua_repeated_fields(field: SecChUaFieldSpec, combined: Option<&str>) {
  let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "duplicate", false);
  let authority = addr.to_string();
  let _stream = send_h2c_prior_knowledge_headers(
    addr,
    &[
      (":method", "GET"),
      (":scheme", "http"),
      (":path", "/asset"),
      (":authority", authority.as_str()),
      (field.name, field.duplicate_first),
      (field.lowercase_name, field.duplicate_second),
    ],
  );

  let observed = receive_h2c_sec_ch_ua(&rx, field, "duplicate");
  assert_eq!(Some(field.duplicate_first.to_string()), observed.raw);
  match combined {
    Some(combined) => assert_eq!(Ok(Some(combined.to_owned())), observed.parsed),
    None => assert_h2c_sec_ch_ua_error(field, "duplicate", &observed.parsed),
  }
  join_h2c_sec_ch_ua(handle, field, "duplicate");
}

fn run_h2c_sec_ch_ua_oversized(field: SecChUaFieldSpec) {
  let (addr, rx, handle) = spawn_h2c_sec_ch_ua_observer(field, "oversized", true);
  let oversized = "a".repeat(64 * 1024 + 1);
  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/asset"))
    .header((field.name, oversized.as_str()))
    .emit_http2_prior_knowledge()
    .expect("receive h2c response");

  assert_eq!("ok", response.body().string().expect("h2c response body"));
  let observed = receive_h2c_sec_ch_ua(&rx, field, "oversized");
  assert_eq!(
    (Some(64 * 1024 + 1), true, true),
    (
      observed.raw.as_ref().map(String::len),
      observed.parsed.is_err(),
      observed.raw.is_some(),
    )
  );
  assert_h2c_sec_ch_ua_error(field, "oversized", &observed.parsed);
  join_h2c_sec_ch_ua(handle, field, "oversized");
}

#[test]
fn h2c_sec_ch_ua_arch_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_arch_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_arch_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_arch_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_arch_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_arch_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_ARCH);
}

#[test]
fn h2c_sec_ch_ua_bitness_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_bitness_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_bitness_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_bitness_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_bitness_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_bitness_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_BITNESS);
}

#[test]
fn h2c_sec_ch_ua_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_full_version_list_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_FULL_VERSION_LIST);
}

#[test]
fn h2c_sec_ch_ua_form_factors_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_FORM_FACTORS);
}

#[test]
fn h2c_sec_ch_ua_form_factors_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_FORM_FACTORS);
}

#[test]
fn h2c_sec_ch_ua_form_factors_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_FORM_FACTORS);
}

#[test]
fn h2c_sec_ch_ua_form_factors_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_FORM_FACTORS);
}

#[test]
fn h2c_sec_ch_ua_form_factors_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_repeated_fields(SEC_CH_UA_FORM_FACTORS, Some(r#""Desktop", "Tablet""#));
}

#[test]
fn h2c_sec_ch_ua_form_factors_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_FORM_FACTORS);
}

#[test]
fn h2c_sec_ch_ua_platform_version_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_version_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_version_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_version_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_version_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_version_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_PLATFORM_VERSION);
}

#[test]
fn h2c_sec_ch_ua_platform_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_platform_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_platform_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_platform_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_platform_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_platform_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_PLATFORM);
}

#[test]
fn h2c_sec_ch_ua_model_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_model_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_model_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_model_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_model_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_model_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_MODEL);
}

#[test]
fn h2c_sec_ch_ua_mobile_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_mobile_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_mobile_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_mobile_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_mobile_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_mobile_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_MOBILE);
}

#[test]
fn h2c_sec_ch_ua_wow64_helper_reaches_server_accessor() {
  run_h2c_sec_ch_ua_helper(SEC_CH_UA_WOW64);
}

#[test]
fn h2c_sec_ch_ua_wow64_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_malformed(SEC_CH_UA_WOW64);
}

#[test]
fn h2c_sec_ch_ua_wow64_non_ascii_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_non_ascii(SEC_CH_UA_WOW64);
}

#[test]
fn h2c_sec_ch_ua_wow64_structured_malformed_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_structured_malformed(SEC_CH_UA_WOW64);
}

#[test]
fn h2c_sec_ch_ua_wow64_duplicate_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_duplicate(SEC_CH_UA_WOW64);
}

#[test]
fn h2c_sec_ch_ua_wow64_oversized_reaches_server_accessor_with_raw_header() {
  run_h2c_sec_ch_ua_oversized(SEC_CH_UA_WOW64);
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
  encode_hpack_integer(block, value.len(), 7);
  block.extend_from_slice(value);
}

fn encode_hpack_integer(block: &mut Vec<u8>, mut value: usize, prefix_bits: u8) {
  let max = (1usize << prefix_bits) - 1;
  if value < max {
    block.push(value as u8);
    return;
  }
  block.push(max as u8);
  value -= max;
  while value >= 128 {
    block.push(((value % 128) as u8) | 0x80);
    value /= 128;
  }
  block.push(value as u8);
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
