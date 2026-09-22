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
fn compatibility_facade_reexports_core_response_metadata() {
  let _: rttp::Connection = rttp::Connection::parse("keep-alive").expect("Connection should parse");
  let _: rttp::ConnectionParseError =
    rttp::Connection::parse("").expect_err("empty Connection should fail");

  let _: rttp::TransferEncoding =
    rttp::TransferEncoding::parse("chunked").expect("Transfer-Encoding should parse");
  let _: rttp::TransferEncodingParseError =
    rttp::TransferEncoding::parse("").expect_err("empty Transfer-Encoding should fail");

  let keep_alive: rttp::KeepAlive =
    rttp::KeepAlive::parse("timeout=5, max=100, custom=token").expect("Keep-Alive should parse");
  let _: &[rttp::KeepAliveExtension] = keep_alive.extensions();
  let _: rttp::KeepAliveParseError =
    rttp::KeepAlive::parse("").expect_err("empty Keep-Alive should fail");

  let _: rttp::Vary = rttp::Vary::parse("Accept-Encoding").expect("Vary should parse");
  let _: rttp::VaryParseError = rttp::Vary::parse("").expect_err("empty Vary should fail");

  let _: rttp::Trailer = rttp::Trailer::parse("X-Checksum").expect("Trailer should parse");
  let _: rttp::TrailerParseError = rttp::Trailer::parse("").expect_err("empty Trailer should fail");
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
  let _: rttp::AcceptPostParseError = rttp_client::response::AcceptPost::parse("application/json,")
    .expect_err("malformed Accept-Post should fail");
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
fn compatibility_facade_roundtrips_accept_post_metadata_over_http11() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind Accept-Post server");
  let addr = server.local_addr().expect("Accept-Post server address");
  let handle = std::thread::spawn(move || {
    server
      .accept_one(|_| {
        rttp::server::HttpResponse::ok("OK")
          .with_accept_post([
            r#"Text/Plain; title="a,b\"c""#,
            "application/json; profile=summary",
          ])
          .expect("Accept-Post declaration should parse")
      })
      .expect("serve Accept-Post response");
  });

  let response = rttp::Http::client()
    .get()
    .url(format!("http://{addr}/accept-post"))
    .emit()
    .expect("Accept-Post response should parse");
  let metadata = response
    .accept_post()
    .expect("Accept-Post metadata should parse")
    .expect("Accept-Post metadata should be present");
  assert_eq!(2, metadata.len());
  assert_eq!("Text", metadata.media_types()[0].type_());
  assert_eq!("Plain", metadata.media_types()[0].subtype());
  assert_eq!("a,b\"c", metadata.media_types()[0].parameters()[0].value());
  assert_eq!("summary", metadata.media_types()[1].parameters()[0].value());
  handle.join().expect("Accept-Post server thread");
}

#[test]
#[cfg(any(feature = "all", feature = "client"))]
fn compatibility_facade_reexports_client_hints_response_metadata() {
  let dpr: rttp::Dpr = rttp::Dpr::parse("1.5").expect("DPR should parse");
  assert_eq!(1.5, dpr.ratio());
  assert_eq!("1.5", dpr.header_value());
  let _: rttp::DprParseError = rttp::Dpr::parse("0").expect_err("zero DPR should fail");

  let sec_ch_dpr: rttp::SecChDpr = rttp::SecChDpr::parse("1.5").expect("Sec-CH-DPR should parse");
  assert_eq!(1.5, sec_ch_dpr.ratio());
  assert_eq!("1.5", sec_ch_dpr.header_value());
  let _: rttp::SecChDprParseError =
    rttp::SecChDpr::parse("0").expect_err("zero Sec-CH-DPR should fail");

  let downlink: rttp::Downlink = rttp::Downlink::parse("10.25").expect("Downlink should parse");
  assert_eq!(10.25, downlink.mbps());
  assert_eq!("10.25", downlink.header_value());
  let _: rttp::DownlinkParseError =
    rttp::Downlink::parse("-1").expect_err("negative Downlink should fail");

  let device_memory: rttp::DeviceMemory =
    rttp::DeviceMemory::parse("8").expect("Device-Memory should parse");
  assert_eq!(8.0, device_memory.gib());
  assert_eq!("8", device_memory.header_value());
  let _: rttp::DeviceMemoryParseError =
    rttp::DeviceMemory::parse("-1").expect_err("negative Device-Memory should fail");

  let prefers_color_scheme: rttp::PrefersColorScheme =
    rttp::PrefersColorScheme::parse("DaRk").expect("Prefers-Color-Scheme should parse");
  assert_eq!("dark", prefers_color_scheme.header_value());
  let _: rttp::PrefersColorSchemeParseError = rttp::PrefersColorScheme::parse("system")
    .expect_err("unknown Prefers-Color-Scheme should fail");

  let sec_ch_ua_mobile: rttp::SecChUaMobile =
    rttp::SecChUaMobile::parse("\t?1\t").expect("Sec-CH-UA-Mobile should parse");
  assert_eq!("?1", sec_ch_ua_mobile.header_value());
  assert!(sec_ch_ua_mobile.is_mobile());
  let _: rttp::SecChUaMobileParseError =
    rttp::SecChUaMobile::parse("true").expect_err("unknown Sec-CH-UA-Mobile should fail");

  let prefers_contrast: rttp::PrefersContrast =
    rttp::PrefersContrast::parse("CuStOm").expect("Prefers-Contrast should parse");
  assert_eq!("custom", prefers_contrast.header_value());
  let _: rttp::PrefersContrastParseError =
    rttp::PrefersContrast::parse("auto").expect_err("unknown Prefers-Contrast should fail");

  let prefers_reduced_motion: rttp::PrefersReducedMotion =
    rttp::PrefersReducedMotion::parse("ReDuCe").expect("Prefers-Reduced-Motion should parse");
  assert_eq!("reduce", prefers_reduced_motion.header_value());
  let _: rttp::PrefersReducedMotionParseError = rttp::PrefersReducedMotion::parse("auto")
    .expect_err("unknown Prefers-Reduced-Motion should fail");

  let prefers_reduced_transparency: rttp::PrefersReducedTransparency =
    rttp::PrefersReducedTransparency::parse("ReDuCe")
      .expect("Prefers-Reduced-Transparency should parse");
  assert_eq!("reduce", prefers_reduced_transparency.header_value());
  let _: rttp::PrefersReducedTransparencyParseError =
    rttp::PrefersReducedTransparency::parse("auto")
      .expect_err("unknown Prefers-Reduced-Transparency should fail");

  let width: rttp::Width = rttp::Width::parse("1440").expect("Width should parse");
  assert_eq!(1440, width.value());
  let _: rttp::WidthParseError =
    rttp::Width::parse("1.0").expect_err("malformed Width should fail");

  let viewport_width: rttp::ViewportWidth =
    rttp::ViewportWidth::parse("1440").expect("Viewport-Width should parse");
  assert_eq!(1440, viewport_width.value());
  let _: rttp::ViewportWidthParseError =
    rttp::ViewportWidth::parse("1.0").expect_err("malformed Viewport-Width should fail");

  let viewport_height: rttp::SecChViewportHeight =
    rttp::SecChViewportHeight::parse("900").expect("Sec-CH-Viewport-Height should parse");
  assert_eq!(900, viewport_height.value());
  let _: rttp::SecChViewportHeightParseError = rttp::SecChViewportHeight::parse("1.0")
    .expect_err("malformed Sec-CH-Viewport-Height should fail");

  let viewport_width: rttp::SecChViewportWidth =
    rttp::SecChViewportWidth::parse("1440").expect("Sec-CH-Viewport-Width should parse");
  assert_eq!(1440, viewport_width.value());
  let _: rttp::SecChViewportWidthParseError = rttp::SecChViewportWidth::parse("1.0")
    .expect_err("malformed Sec-CH-Viewport-Width should fail");

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
#[cfg(any(feature = "all", feature = "tls-native", feature = "tls-rustls"))]
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
  all(feature = "async", feature = "tls-native"),
  all(feature = "async", feature = "tls-rustls")
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
