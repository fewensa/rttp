use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp_client::HttpClient;
use rttp_server::server::{HttpResponse, HttpServer};

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
