mod support;

use futures::executor::block_on;
use rttp_client::types::Proxy;
use rttp_client::{Config, HttpClient};

fn client() -> HttpClient {
  HttpClient::new()
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
#[cfg(all(feature = "async", feature = "tls-rustls"))]
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

#[cfg(feature = "async")]
fn test_async_http_proxy_uses_absolute_form_for_http_requests() {
  let (addr, _handle) = support::spawn_http_proxy_server();
  block_on(async {
    let response = client()
      .get()
      .url("http://example.com/proxy?q=1")
      .proxy(Proxy::http("127.0.0.1", u32::from(addr.port())))
      .rasync()
      .await;
    assert!(response.is_ok());

    let response = response.unwrap();
    assert_eq!(
      "GET http://example.com/proxy?q=1 HTTP/1.1",
      response.body().string().unwrap()
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
