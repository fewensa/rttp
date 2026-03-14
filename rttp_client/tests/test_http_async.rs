mod support;

use futures::executor::block_on;
use rttp_client::types::Proxy;
use rttp_client::HttpClient;

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

#[test]
#[ignore]
#[cfg(feature = "async")]
fn test_async_proxy_socks5() {
  block_on(async {
    let response = client()
      .get()
      .url("http://google.com")
      .proxy(Proxy::socks5("127.0.0.1", 1080))
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("google.com", response.host());
    println!("{}", response);
  });
}
