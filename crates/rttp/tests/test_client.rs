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

  let accept_patch: rttp::AcceptPatch = response
    .accept_patch()
    .expect("Accept-Patch should parse")
    .expect("Accept-Patch should be present");
  let _: &[rttp::MediaType] = accept_patch.media_types();
  assert_eq!("application", accept_patch.media_types()[0].type_());
  assert_eq!("json", accept_patch.media_types()[0].subtype());
  let _: rttp::AcceptPatchParseError =
    rttp_client::response::AcceptPatch::parse("application/json,")
      .expect_err("malformed Accept-Patch should fail");
  let _: rttp::AcceptPost = response
    .accept_post()
    .expect("Accept-Post should parse")
    .expect("Accept-Post should be present");
}

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn compatibility_facade_roundtrips_accept_patch_metadata_over_http11() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind Accept-Patch server");
  let addr = server.local_addr().expect("Accept-Patch server address");
  let handle = std::thread::spawn(move || {
    server
      .accept_one(|_| {
        rttp::server::HttpResponse::ok("OK")
          .with_accept_patch([
            "application/merge-patch+json; charset=utf-8",
            "application/json",
          ])
          .expect("Accept-Patch declaration should parse")
      })
      .expect("serve Accept-Patch response");
  });

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/accept-patch"))
    .emit()
    .expect("Accept-Patch response should parse");
  let metadata = response
    .accept_patch()
    .expect("Accept-Patch metadata should parse")
    .expect("Accept-Patch metadata should be present");
  assert_eq!(2, metadata.len());
  assert_eq!("application", metadata.media_types()[0].type_());
  assert_eq!("merge-patch+json", metadata.media_types()[0].subtype());
  assert_eq!("utf-8", metadata.media_types()[0].parameters()[0].value());
  handle.join().expect("Accept-Patch server thread");
}

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn compatibility_facade_reexports_client_hints_response_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-CH: Sec-CH-UA, DPR\r\n",
    "Critical-CH: Sec-CH-UA\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("https://example.test"),
    raw.as_bytes().to_vec(),
  )
  .expect("response should parse");

  let _: rttp::AcceptCh = response
    .accept_ch()
    .expect("Accept-CH should parse")
    .expect("Accept-CH should be present");
  let _: rttp::CriticalCh = response
    .critical_ch()
    .expect("Critical-CH should parse")
    .expect("Critical-CH should be present");
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
