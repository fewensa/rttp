#![cfg(feature = "http2")]

use std::sync::mpsc;
use std::thread;

use rttp::server::HttpResponse;

#[test]
fn wrapper_http2_feature_exposes_prior_knowledge_client_path() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.version().to_string(), request.body().to_vec()))
          .expect("send request version");
        HttpResponse::ok("wrapper h2")
      })
      .expect("serve h2 request");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/wrapper", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response");

  let (request_version, request_body) = rx.recv().expect("receive request version");
  assert_eq!("HTTP/2", request_version);
  assert!(request_body.is_empty());
  assert_eq!("HTTP/2", response.version());
  assert_eq!("wrapper h2", response.body().string().unwrap());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_feature_exposes_response_trailers_from_prior_knowledge_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");

  let handle = thread::spawn(move || {
    server
      .accept_one(|_| {
        HttpResponse::ok("wrapper h2 trailers")
          .header("Trailer", "X-Trace, X-Signature")
          .trailer("X-Trace", "abc")
          .trailer("X-Signature", "signed")
      })
      .expect("serve h2 trailer response");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/trailers", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 trailer response");

  assert_eq!("HTTP/2", response.version());
  assert_eq!("wrapper h2 trailers", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
  assert_eq!(
    Some(&"signed".to_string()),
    response.trailer_value("X-SIGNATURE")
  );
  assert!(response.header_value("Trailer").is_none());

  handle.join().expect("server thread");
}

#[test]
fn wrapper_http2_prior_knowledge_post_body_round_trips_between_client_and_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();
  let request_body = b"body over h2 from rttp_client".to_vec();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.version().to_string(),
          request.header("content-type").map(str::to_string),
          request.body().to_vec(),
        ))
        .expect("send parsed h2 request");
        HttpResponse::new(201, "Created")
          .header("Trailer", "X-Trace, X-Upload-Status")
          .body("stored over h2")
          .trailer("X-Trace", "post-body-parity")
          .trailer("X-Upload-Status", "stored")
      })
      .expect("serve h2 POST request");
  });

  let response = rttp::Http::client()
    .post()
    .url(format!("http://{}/upload", addr))
    .content_type("application/octet-stream")
    .binary(request_body.clone())
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 POST response");

  let (method, target, version, content_type, observed_body) =
    rx.recv().expect("receive parsed h2 request");
  assert_eq!("POST", method);
  assert_eq!("/upload", target);
  assert_eq!("HTTP/2", version);
  assert_eq!(Some("application/octet-stream".to_string()), content_type);
  assert_eq!(request_body, observed_body);

  assert_eq!("HTTP/2", response.version());
  assert_eq!(201, response.code());
  assert_eq!("stored over h2", response.body().string().unwrap());
  assert_eq!(2, response.trailers().len());
  assert_eq!(
    Some(&"post-body-parity".to_string()),
    response.trailer_value("x-trace")
  );
  assert_eq!(
    Some(&"stored".to_string()),
    response.trailer_value("X-UPLOAD-STATUS")
  );
  assert!(response.header_value("Trailer").is_none());

  handle.join().expect("server thread");
}
