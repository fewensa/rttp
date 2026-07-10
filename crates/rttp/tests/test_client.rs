#[cfg(any(feature = "all", feature = "client"))]
mod support;

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn test_client_http() {
  let (addr, _handle) = support::spawn_http_server();
  let response = rttp::Http::client()
    .url(format!("http://{}/get", addr))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("127.0.0.1", response.host());
  println!("{}", response);
}

#[test]
#[cfg(any(
  feature = "all",
  feature = "client",
  feature = "tls-native",
  feature = "tls-rustls"
))]
fn test_client_https() {
  let (addr, _handle) = support::spawn_tls_server();
  let response = rttp::Http::client()
    .url(format!("https://localhost:{}/get", addr.port()))
    .config(
      rttp_client::Config::builder()
        .verify_ssl_cert(false)
        .verify_ssl_hostname(false),
    )
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("localhost", response.host());
  println!("{}", response);
}

#[test]
#[cfg(any(feature = "all", feature = "async"))]
fn test_client_async_http() {
  async_std::task::block_on(async {
    let (addr, _handle) = support::spawn_http_server();
    let response = rttp::Http::client()
      .post()
      .url(format!("http://{}/post", addr))
      .form(("debug", "true", "name=Form&file=@cargo#../Cargo.toml"))
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("127.0.0.1", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(any(
  feature = "all",
  feature = "async",
  feature = "tls-native",
  feature = "tls-rustls"
))]
fn test_client_async_https() {
  async_std::task::block_on(async {
    let (addr, _handle) = support::spawn_tls_server();
    let response = rttp::Http::client()
      .post()
      .url(format!("https://localhost:{}/get", addr.port()))
      .config(
        rttp_client::Config::builder()
          .verify_ssl_cert(false)
          .verify_ssl_hostname(false),
      )
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("localhost", response.host());
    println!("{}", response);
  });
}
