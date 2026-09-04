#[cfg(feature = "async")]
use futures::executor::block_on;
use std::thread;
use std::time::Duration;

use rttp_client::response::Response;
use rttp_client::HttpClient;
use rttp_server::server::HttpResponse;
use rttp_test_support as fixtures;

const BODY: &str = "cache-metadata-http11";
const TIMEOUT: Duration = Duration::from_secs(2);
const CDN_CACHE_CONTROL_VALUES: &[&str] = &[
  "max-age=0, stale-while-revalidate=30, cdn-example=\"a, b\"",
  "immutable",
];

fn client() -> HttpClient {
  rttp::Http::client()
}

fn bind_facade_server() -> rttp_server::server::HttpServer {
  rttp::Http::server("127.0.0.1:0")
    .expect("bind cache metadata facade server")
    .with_read_timeout(Some(TIMEOUT))
    .with_write_timeout(Some(TIMEOUT))
}

fn cache_status_case() -> &'static fixtures::cache_status::ResponseCase {
  &fixtures::cache_status::response_cases()[0]
}

fn warning_case() -> &'static fixtures::warning::ResponseCase {
  &fixtures::warning::response_cases()[0]
}

fn cache_metadata_response(cache_control_values: &[&str]) -> HttpResponse {
  let mut response = HttpResponse::ok(BODY);
  for value in cache_control_values {
    response = response.header("Cache-Control", *value);
  }
  for value in CDN_CACHE_CONTROL_VALUES {
    response = response.header("CDN-Cache-Control", *value);
  }
  for value in cache_status_case().values {
    response = response.header("Cache-Status", *value);
  }
  for value in warning_case().values {
    response = response.header("Warning", *value);
  }
  response.with_age(0)
}

fn emit_sync(response: HttpResponse) -> Response {
  let server = bind_facade_server();
  let addr = server.local_addr().expect("cache metadata server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| response)
      .expect("serve sync cache metadata response");
  });

  let client_response = client()
    .get()
    .url(format!("http://{addr}/asset"))
    .emit()
    .expect("sync cache metadata response should parse");
  handle.join().expect("sync cache metadata server thread");
  client_response
}

#[cfg(feature = "async")]
fn emit_async(response: HttpResponse) -> Response {
  let server = bind_facade_server();
  let addr = server
    .local_addr()
    .expect("async cache metadata server addr");
  let handle = thread::spawn(move || {
    server
      .accept_one(move |_| response)
      .expect("serve async cache metadata response");
  });

  let client_response = block_on(async {
    client()
      .get()
      .url(format!("http://{addr}/asset"))
      .rasync()
      .await
      .expect("async cache metadata response should parse")
  });
  handle.join().expect("async cache metadata server thread");
  client_response
}

fn raw_values<'a>(response: &'a Response, name: &str) -> Vec<&'a str> {
  response
    .header_values(name)
    .into_iter()
    .map(String::as_str)
    .collect()
}

fn assert_server_cache_control(
  response: &HttpResponse,
  expected: &fixtures::cache_control::ResponseCase,
) {
  let cache_control = response
    .cache_control()
    .expect("server Cache-Control should parse")
    .expect("server Cache-Control should be present");
  assert_eq!(expected.no_cache, cache_control.no_cache());
  assert_eq!(expected.no_cache_fields, cache_control.no_cache_fields());
  assert_eq!(expected.no_store, cache_control.no_store());
  assert_eq!(expected.max_age, cache_control.max_age());
  assert_eq!(expected.s_maxage, cache_control.s_maxage());
  assert_eq!(expected.private, cache_control.private());
  assert_eq!(expected.private_fields, cache_control.private_fields());
  assert_eq!(expected.public, cache_control.public());
  assert_eq!(expected.must_revalidate, cache_control.must_revalidate());
  assert_eq!(expected.proxy_revalidate, cache_control.proxy_revalidate());
  assert_eq!(expected.immutable, cache_control.immutable());
  assert_eq!(
    expected.stale_while_revalidate,
    cache_control.stale_while_revalidate()
  );
  assert_eq!(expected.stale_if_error, cache_control.stale_if_error());
  assert_eq!(expected.extensions.len(), cache_control.extensions().len());
  for ((name, value), extension) in expected.extensions.iter().zip(cache_control.extensions()) {
    assert_eq!(*name, extension.name());
    assert_eq!(*value, extension.value());
  }
}

fn assert_server_response_metadata(
  response: &HttpResponse,
  expected: &fixtures::cache_control::ResponseCase,
) {
  assert_server_cache_control(response, expected);

  let cdn_cache_control = response
    .cdn_cache_control()
    .expect("server CDN-Cache-Control should parse")
    .expect("server CDN-Cache-Control should be present");
  assert_eq!(4, cdn_cache_control.len());
  assert_eq!("max-age", cdn_cache_control.directives()[0].name());
  assert_eq!(Some("0"), cdn_cache_control.directives()[0].value());
  assert_eq!(
    "stale-while-revalidate",
    cdn_cache_control.directives()[1].name()
  );
  assert_eq!(Some("30"), cdn_cache_control.directives()[1].value());
  assert_eq!("cdn-example", cdn_cache_control.directives()[2].name());
  assert_eq!(Some("a, b"), cdn_cache_control.directives()[2].value());
  assert_eq!("immutable", cdn_cache_control.directives()[3].name());
  assert_eq!(None, cdn_cache_control.directives()[3].value());

  let cache_status = response
    .cache_status()
    .expect("server Cache-Status should parse")
    .expect("server Cache-Status should be present");
  assert_eq!(2, cache_status.len());
  assert_eq!(
    "OriginCache",
    cache_status.members()[0].identifier().as_str()
  );
  assert_eq!(Some(0), cache_status.members()[0].ttl());
  assert_eq!(
    "CDN Company Here",
    cache_status.members()[1].identifier().as_str()
  );
  assert_eq!(Some(0), cache_status.members()[1].fwd_status());
  assert_eq!(Some(false), cache_status.members()[1].stored());
  assert_eq!(Some(0), response.age().expect("server Age should parse"));

  let wire = String::from_utf8(response.to_bytes()).expect("server response should be UTF-8");
  assert_eq!(
    expected.values.len(),
    wire.matches("\r\nCache-Control: ").count()
  );
  assert_eq!(2, wire.matches("\r\nCDN-Cache-Control: ").count());
  assert_eq!(2, wire.matches("\r\nCache-Status: ").count());
  assert_eq!(2, wire.matches("\r\nWarning: ").count());
  assert!(wire.contains("\r\nAge: 0\r\n"));
}

fn assert_client_cache_control(
  response: &Response,
  expected: &fixtures::cache_control::ResponseCase,
) {
  assert_eq!(
    expected.values,
    raw_values(response, "Cache-Control").as_slice()
  );
  let cache_control = response
    .cache_control()
    .expect("client Cache-Control should parse")
    .expect("client Cache-Control should be present");
  assert_eq!(expected.no_cache, cache_control.no_cache());
  assert_eq!(expected.no_cache_fields, cache_control.no_cache_fields());
  assert_eq!(expected.no_store, cache_control.no_store());
  assert_eq!(expected.max_age, cache_control.max_age());
  assert_eq!(expected.s_maxage, cache_control.s_maxage());
  assert_eq!(expected.private, cache_control.private());
  assert_eq!(expected.private_fields, cache_control.private_fields());
  assert_eq!(expected.public, cache_control.public());
  assert_eq!(expected.must_revalidate, cache_control.must_revalidate());
  assert_eq!(expected.proxy_revalidate, cache_control.proxy_revalidate());
  assert_eq!(expected.immutable, cache_control.immutable());
  assert_eq!(
    expected.stale_while_revalidate,
    cache_control.stale_while_revalidate()
  );
  assert_eq!(expected.stale_if_error, cache_control.stale_if_error());
  assert_eq!(expected.extensions.len(), cache_control.extensions().len());
  for ((name, value), extension) in expected.extensions.iter().zip(cache_control.extensions()) {
    assert_eq!(*name, extension.name());
    assert_eq!(*value, extension.value());
  }
}

fn assert_client_response_metadata(
  response: &Response,
  expected: &fixtures::cache_control::ResponseCase,
) {
  assert_eq!(200, response.code());
  assert_eq!(
    BODY,
    response
      .body()
      .string()
      .expect("response body should parse")
  );
  assert_client_cache_control(response, expected);

  assert_eq!(
    CDN_CACHE_CONTROL_VALUES,
    raw_values(response, "CDN-Cache-Control").as_slice()
  );
  let cdn_cache_control = response
    .cdn_cache_control()
    .expect("client CDN-Cache-Control should parse")
    .expect("client CDN-Cache-Control should be present");
  assert_eq!(4, cdn_cache_control.len());
  assert_eq!(
    "max-age=0, stale-while-revalidate=30, cdn-example=\"a, b\", immutable",
    cdn_cache_control.header_value()
  );
  assert_eq!("cdn-example", cdn_cache_control.directives()[2].name());
  assert_eq!(Some("a, b"), cdn_cache_control.directives()[2].value());

  assert_eq!(
    cache_status_case().values,
    raw_values(response, "Cache-Status").as_slice()
  );
  let cache_status = response
    .cache_status()
    .expect("client Cache-Status should parse")
    .expect("client Cache-Status should be present");
  assert_eq!(2, cache_status.len());
  let first = &cache_status.members()[0];
  assert_eq!("OriginCache", first.identifier().as_str());
  assert!(first.identifier().is_token());
  assert_eq!(Some(true), first.hit());
  assert_eq!(Some(0), first.ttl());
  assert_eq!(
    Some("origin-miss"),
    first.detail().map(|detail| detail.as_str())
  );
  assert!(first.detail().expect("Cache-Status detail").is_string());
  assert_eq!("trace", first.extensions()[0].name());
  assert_eq!(Some("\"edge, warm\""), first.extensions()[0].value());

  let second = &cache_status.members()[1];
  assert_eq!("CDN Company Here", second.identifier().as_str());
  assert!(second.identifier().is_string());
  assert_eq!(Some("stale"), second.fwd());
  assert_eq!(Some(0), second.fwd_status());
  assert_eq!(Some(false), second.stored());
  assert_eq!(Some("/asset"), second.key());
  assert_eq!("ext-token", second.extensions()[0].name());
  assert_eq!(None, second.extensions()[0].value());

  assert_eq!(
    warning_case().values,
    raw_values(response, "Warning").as_slice()
  );
  let warning = response
    .warning()
    .expect("client Warning should parse")
    .expect("client Warning should be present");
  assert_eq!(
    warning_case().codes,
    warning
      .items()
      .iter()
      .map(|item| item.code())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    warning_case().agents,
    warning
      .items()
      .iter()
      .map(|item| item.agent())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    warning_case().texts,
    warning
      .items()
      .iter()
      .map(|item| item.text())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    warning_case().dated,
    warning
      .items()
      .iter()
      .map(|item| item.date().is_some())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(warning_case().values.join(", "), warning.header_value());
  assert_eq!(Some("0"), response.header_value("Age").map(String::as_str));
  assert_eq!(Some(0), response.age().expect("client Age should parse"));
}

#[test]
fn cache_metadata_facades_are_reachable_through_public_paths() {
  let client_age: rttp_client::response::Age = rttp_client::response::Age::new(0);
  let _: rttp_client::response::AgeParseError =
    rttp_client::response::Age::parse("").expect_err("invalid client Age");
  assert_eq!("0", client_age.header_value());

  let client_cache_control: rttp_client::response::CacheControl =
    rttp_client::response::CacheControl::parse("extension=\"quoted value\"")
      .expect("client Cache-Control");
  let _: &rttp_client::response::CacheControlExtension = &client_cache_control.extensions()[0];
  let client_cache_status: rttp_client::response::CacheStatus =
    rttp_client::response::CacheStatus::parse("OriginCache").expect("client Cache-Status");
  let _: &rttp_client::response::CacheStatusMember = &client_cache_status.members()[0];
  let _: rttp_client::response::CdnCacheControl =
    rttp_client::response::CdnCacheControl::parse("max-age=0").expect("client CDN metadata");
  let client_warning: rttp_client::response::Warning =
    rttp_client::response::Warning::parse(r#"000 - "zero""#).expect("client Warning");
  let _: &rttp_client::response::WarningValue = &client_warning.items()[0];

  let facade_age: rttp::Age = rttp::Age::new(0);
  let _: rttp::AgeParseError = rttp::Age::parse("").expect_err("invalid facade Age");
  let facade_cache_control: rttp::CacheControl =
    rttp::CacheControl::parse("extension=\"quoted value\"").expect("facade Cache-Control");
  let _: &rttp::CacheControlExtension = &facade_cache_control.extensions()[0];
  let facade_warning: rttp::Warning =
    rttp::Warning::parse(r#"000 - "zero""#).expect("facade Warning");
  let _: &rttp::WarningValue = &facade_warning.items()[0];
  assert_eq!(facade_age, rttp::Age::new(0));

  let server_response = rttp::server::HttpResponse::ok("")
    .header("Cache-Control", "max-age=0")
    .header("CDN-Cache-Control", "max-age=0")
    .header("Cache-Status", "OriginCache")
    .with_age(0);
  let _: Result<
    Option<rttp::server::HttpResponseCacheControl>,
    rttp::server::HttpCacheControlParseError,
  > = server_response.cache_control();
  let _: Result<
    Option<rttp::server::HttpCdnCacheControl>,
    rttp::server::HttpCdnCacheControlParseError,
  > = server_response.cdn_cache_control();
  let _: Result<Option<rttp::server::HttpCacheStatus>, rttp::server::HttpCacheStatusParseError> =
    server_response.cache_status();
  let _: Result<Option<u64>, rttp::server::HttpAgeParseError> = server_response.age();
}

#[test]
fn sync_http11_cache_metadata_response_matrix() {
  for case in fixtures::cache_control::response_cases() {
    let server_response = cache_metadata_response(case.values);
    assert_server_response_metadata(&server_response, case);
    let client_response = emit_sync(server_response);
    assert_client_response_metadata(&client_response, case);
  }
}

#[cfg(feature = "async")]
#[test]
fn async_http11_cache_metadata_response_matrix() {
  for case in fixtures::cache_control::response_cases() {
    let server_response = cache_metadata_response(case.values);
    assert_server_response_metadata(&server_response, case);
    let client_response = emit_async(server_response);
    assert_client_response_metadata(&client_response, case);
  }
}

#[test]
fn sync_http11_cache_metadata_absence_returns_none() {
  let server_response = HttpResponse::ok("absent");
  assert!(server_response
    .cache_control()
    .expect("absent server Cache-Control")
    .is_none());
  assert!(server_response
    .cdn_cache_control()
    .expect("absent server CDN-Cache-Control")
    .is_none());
  assert!(server_response
    .cache_status()
    .expect("absent server Cache-Status")
    .is_none());
  assert!(server_response.age().expect("absent server Age").is_none());

  let response = emit_sync(server_response);
  assert!(response
    .cache_control()
    .expect("absent client Cache-Control")
    .is_none());
  assert!(response
    .cdn_cache_control()
    .expect("absent client CDN-Cache-Control")
    .is_none());
  assert!(response
    .cache_status()
    .expect("absent client Cache-Status")
    .is_none());
  assert!(response.warning().expect("absent client Warning").is_none());
  assert!(response.age().expect("absent client Age").is_none());
  assert_eq!(None, response.header_value("Cache-Control"));
  assert_eq!(None, response.header_value("CDN-Cache-Control"));
  assert_eq!(None, response.header_value("Cache-Status"));
  assert_eq!(None, response.header_value("Warning"));
  assert_eq!("absent", response.body().string().expect("absence body"));
}

fn raw_http11_response(headers: &[(&str, &str)], body: &str) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for (name, value) in headers {
    response.push_str(name);
    response.push_str(": ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str(&format!("Content-Length: {}\r\n\r\n{}", body.len(), body));
  response.into_bytes()
}

fn assert_sync_rejected(
  label: &str,
  headers: &[(&str, &str)],
  header_name: &str,
  expected_values: &[&str],
  rejects: impl Fn(&Response) -> bool,
) {
  let raw_response = raw_http11_response(headers, "malformed");
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);
  let response = client()
    .get()
    .url(format!("http://{addr}/matrix/{label}"))
    .emit()
    .unwrap_or_else(|error| panic!("{label} response should remain parseable: {error}"));

  assert!(rejects(&response), "{label} typed accessor should reject");
  assert_eq!(
    expected_values,
    raw_values(&response, header_name).as_slice()
  );
  assert_eq!(
    "malformed",
    response.body().string().expect("malformed body")
  );
  handle.join().expect("malformed raw response server thread");
}

#[test]
fn sync_http11_cache_metadata_malformed_peers_preserve_raw_fields_and_body() {
  for case in fixtures::cache_control::invalid_response_cases() {
    assert_sync_rejected(
      case.name,
      &[("Cache-Control", case.value)],
      "Cache-Control",
      &[case.value],
      |response| response.cache_control().is_err(),
    );
  }
  for value in ["max-age=", r#"cdn-example="unterminated"#] {
    assert_sync_rejected(
      "CDN-Cache-Control malformed",
      &[("CDN-Cache-Control", value)],
      "CDN-Cache-Control",
      &[value],
      |response| response.cdn_cache_control().is_err(),
    );
  }
  for case in fixtures::cache_status::invalid_cases() {
    assert_sync_rejected(
      case.name,
      &[("Cache-Status", case.value)],
      "Cache-Status",
      &[case.value],
      |response| response.cache_status().is_err(),
    );
  }
  for case in fixtures::warning::invalid_cases() {
    assert_sync_rejected(
      case.name,
      &[("Warning", case.value)],
      "Warning",
      &[case.value],
      |response| response.warning().is_err(),
    );
  }
  for case in fixtures::age_expires::invalid_age_cases() {
    assert_sync_rejected(
      case.name,
      &[("Age", case.value)],
      "Age",
      &[case.value],
      |response| response.age().is_err(),
    );
  }

  assert_sync_rejected(
    "duplicate Age fields",
    &[("Age", "0"), ("Age", "60")],
    "Age",
    &["0", "60"],
    |response| response.age().is_err(),
  );
}

#[test]
fn sync_http11_cache_metadata_bounds_preserve_raw_fields_and_body() {
  let too_many_cache_control = fixtures::cache_control::too_many_directives_value();
  assert_sync_rejected(
    "Cache-Control over-count",
    &[("Cache-Control", too_many_cache_control.as_str())],
    "Cache-Control",
    &[too_many_cache_control.as_str()],
    |response| response.cache_control().is_err(),
  );
  let oversized_cache_control = fixtures::cache_control::oversized_value();
  assert_sync_rejected(
    "Cache-Control oversized",
    &[("Cache-Control", oversized_cache_control.as_str())],
    "Cache-Control",
    &[oversized_cache_control.as_str()],
    |response| response.cache_control().is_err(),
  );

  let too_many_cdn = fixtures::cache_control::too_many_directives_value();
  assert_sync_rejected(
    "CDN-Cache-Control over-count",
    &[("CDN-Cache-Control", too_many_cdn.as_str())],
    "CDN-Cache-Control",
    &[too_many_cdn.as_str()],
    |response| response.cdn_cache_control().is_err(),
  );
  let oversized_cdn = fixtures::cache_control::oversized_value();
  assert_sync_rejected(
    "CDN-Cache-Control oversized",
    &[("CDN-Cache-Control", oversized_cdn.as_str())],
    "CDN-Cache-Control",
    &[oversized_cdn.as_str()],
    |response| response.cdn_cache_control().is_err(),
  );

  let too_many_members = fixtures::cache_status::too_many_members_value();
  assert_sync_rejected(
    "Cache-Status member over-count",
    &[("Cache-Status", too_many_members.as_str())],
    "Cache-Status",
    &[too_many_members.as_str()],
    |response| response.cache_status().is_err(),
  );
  let too_many_parameters = fixtures::cache_status::too_many_parameters_value();
  assert_sync_rejected(
    "Cache-Status parameter over-count",
    &[("Cache-Status", too_many_parameters.as_str())],
    "Cache-Status",
    &[too_many_parameters.as_str()],
    |response| response.cache_status().is_err(),
  );
  let oversized_status = fixtures::cache_status::oversized_value();
  assert_sync_rejected(
    "Cache-Status oversized",
    &[("Cache-Status", oversized_status.as_str())],
    "Cache-Status",
    &[oversized_status.as_str()],
    |response| response.cache_status().is_err(),
  );

  let too_many_warnings = fixtures::warning::too_many_items_value();
  assert_sync_rejected(
    "Warning item over-count",
    &[("Warning", too_many_warnings.as_str())],
    "Warning",
    &[too_many_warnings.as_str()],
    |response| response.warning().is_err(),
  );
  let oversized_warning = fixtures::warning::oversized_value();
  assert_sync_rejected(
    "Warning oversized",
    &[("Warning", oversized_warning.as_str())],
    "Warning",
    &[oversized_warning.as_str()],
    |response| response.warning().is_err(),
  );

  let oversized_age = format!("1{}", "0".repeat(64 * 1024));
  assert_sync_rejected(
    "Age oversized",
    &[("Age", oversized_age.as_str())],
    "Age",
    &[oversized_age.as_str()],
    |response| response.age().is_err(),
  );

  let server_over_count = HttpResponse::ok(BODY)
    .header("Cache-Control", &too_many_cache_control)
    .header("CDN-Cache-Control", &too_many_cdn)
    .header("Cache-Status", &too_many_members)
    .header("Age", &oversized_age);
  assert!(server_over_count.cache_control().is_err());
  assert!(server_over_count.cdn_cache_control().is_err());
  assert!(server_over_count.cache_status().is_err());
  assert!(server_over_count.age().is_err());
  let wire = String::from_utf8(server_over_count.to_bytes()).expect("bounded server response");
  assert!(wire.contains(&too_many_cache_control));
  assert!(wire.contains(&too_many_cdn));
  assert!(wire.contains(&too_many_members));
  assert!(wire.contains(&oversized_age));
}
