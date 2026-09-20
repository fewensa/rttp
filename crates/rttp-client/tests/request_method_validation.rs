use rttp_test_support as support;

#[cfg(feature = "async")]
use futures::executor::block_on;
use rttp_client::types::Proxy;
use rttp_client::HttpClient;
use std::time::Duration;

fn capture_optional_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
  request(format!("http://{}", addr));
  handle.join().expect("optional raw request capture server")
}

fn capture_request(request: impl FnOnce(String)) -> Vec<u8> {
  let (addr, handle) = support::capture_raw_http_request();
  request(format!("http://{}", addr));
  handle.join().expect("raw request capture server")
}

fn capture_optional_proxy_request(request: impl FnOnce(Proxy)) -> Vec<u8> {
  let (addr, handle) = support::capture_optional_raw_http_request(Duration::from_millis(250));
  request(Proxy::http("127.0.0.1", u32::from(addr.port())));
  handle
    .join()
    .expect("optional proxy request capture server")
}

#[test]
fn invalid_methods_are_rejected_before_connecting() {
  let invalid_methods = [
    "",
    "GET POST",
    " GET",
    "GET ",
    "GET\tPOST",
    "GET,POST",
    "GET/POST",
    "GET:POST",
    "GET\r\nX-Injected: value",
    "GET\n",
    "GET\0",
    "GET\u{1f}",
    "GET\u{7f}",
    "GETé",
  ];

  for method in invalid_methods {
    let request = capture_optional_request(|base_url| {
      let error = HttpClient::new()
        .method(method)
        .url(format!("{base_url}/invalid-method"))
        .emit()
        .expect_err("invalid outbound method must be rejected");
      assert!(
        error.is_builder(),
        "unexpected error for {method:?}: {error}"
      );
    });
    assert!(
      request.is_empty(),
      "invalid method {method:?} must not open a socket"
    );
  }
}

#[test]
fn valid_extension_methods_are_preserved() {
  let request = capture_request(|base_url| {
    HttpClient::new()
      .method("PROPFIND")
      .url(format!("{base_url}/collection"))
      .emit()
      .expect("valid extension method should be sent");
  });
  let request = String::from_utf8(request).expect("captured request should be utf-8");

  assert!(request.starts_with("PROPFIND /collection HTTP/1.1\r\n"));
}

#[test]
fn invalid_methods_are_rejected_before_proxy_connect() {
  let request = capture_optional_proxy_request(|proxy| {
    let error = HttpClient::new()
      .method("BAD METHOD")
      .url("https://method-validation.invalid/resource")
      .proxy(proxy)
      .emit()
      .expect_err("invalid outbound method must be rejected");
    assert!(error.is_builder());
  });

  assert!(
    request.is_empty(),
    "invalid method must not connect to a proxy"
  );
}

#[cfg(feature = "async")]
#[test]
fn async_invalid_methods_are_rejected_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let error = block_on(
      HttpClient::new()
        .method("BAD\r\nMETHOD")
        .url(format!("{base_url}/invalid-async-method"))
        .rasync(),
    )
    .expect_err("invalid async outbound method must be rejected");
    assert!(error.is_builder());
  });

  assert!(request.is_empty());
}

#[cfg(feature = "http2")]
#[test]
fn http2_invalid_methods_are_rejected_before_connecting() {
  let request = capture_optional_request(|base_url| {
    let error = HttpClient::new()
      .method("BAD METHOD")
      .url(format!("{base_url}/invalid-http2-method"))
      .emit_http2_prior_knowledge()
      .expect_err("invalid HTTP/2 outbound method must be rejected");
    assert!(error.is_builder());
  });

  assert!(request.is_empty());
}
