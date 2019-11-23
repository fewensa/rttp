use async_std::task;

use rttp_client::{Config, HttpClient};
use rttp_client::types::Proxy;

fn client() -> HttpClient {
  HttpClient::new()
}

#[test]
fn test_async_http() {
  task::block_on(async {
    let response = client()
      .post()
      .url("http://httpbin.org/post")
      .form(("debug", "true", "name=Form&file=@cargo#../Cargo.toml"))
      .enqueue()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("httpbin.org", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(any(feature = "tls-rustls", feature = "tls-native"))]
fn test_async_https() {
  task::block_on(async {
    let response = client()
      .post()
      .url("https://httpbin.org/get")
      .enqueue()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("httpbin.org", response.host());
    println!("{}", response);
  });
}

#[test]
#[ignore]
fn test_async_proxy_socks5() {
  task::block_on(async {
    let response = client()
      .get()
      .url("http://google.com")
      .proxy(Proxy::socks5("127.0.0.1", 1080))
      .enqueue()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("google.com", response.host());
    println!("{}", response);
  });
}

