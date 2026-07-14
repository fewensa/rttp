#[cfg(any(
  feature = "all",
  feature = "client",
  feature = "async",
  feature = "tls-native",
  feature = "tls-rustls"
))]
use rttp_test_support as support;

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
#[cfg(any(feature = "all", feature = "client"))]
fn compatibility_facade_reexports_accept_response_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Patch: application/json\r\n",
    "Accept-Post: text/plain\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("https://example.test"),
    raw.as_bytes().to_vec(),
  )
  .expect("response should parse");

  let _: rttp::AcceptPatch = response
    .accept_patch()
    .expect("Accept-Patch should parse")
    .expect("Accept-Patch should be present");
  let _: rttp::AcceptPost = response
    .accept_post()
    .expect("Accept-Post should parse")
    .expect("Accept-Post should be present");
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
      .form(("debug", "true", "name=Form&file=@cargo#../../Cargo.toml"))
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
