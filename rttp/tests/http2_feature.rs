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
        tx.send(request.version().to_string())
          .expect("send request version");
        HttpResponse::ok("wrapper h2")
      })
      .expect("serve h2 request");
  });

  let response = rttp::Http::client()
    .url(format!("http://{}/wrapper", addr))
    .emit_http2_prior_knowledge()
    .expect("wrapper h2 response");

  assert_eq!("HTTP/2", rx.recv().expect("receive request version"));
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
