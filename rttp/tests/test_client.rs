use rttp::Http;

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn test_client_http() {
  let response = Http::client()
    .url("http://httpbin.org/get")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
#[cfg(any(feature = "all", feature = "client", feature = "client_tls_native", feature = "client_tls_rustls"))]
fn test_client_https() {
  let response = Http::client()
    .url("https://bing.com")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn test_client_async_http() {
  async_std::task::block_on(async {
    let response = Http::client()
      .post()
      .url("http://httpbin.org/post")
      .form(("debug", "true", "name=Form&file=@cargo#../Cargo.toml"))
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("httpbin.org", response.host());
    println!("{}", response);
  });
}

#[test]
#[cfg(any(feature = "all", feature = "client", feature = "client_tls_native", feature = "client_tls_rustls"))]
fn test_client_async_https() {
  async_std::task::block_on(async {
    let response = Http::client()
      .post()
      .url("https://httpbin.org/get")
      .rasync()
      .await;
    assert!(response.is_ok());
    let response = response.unwrap();
    assert_eq!("httpbin.org", response.host());
    println!("{}", response);
  });
}

