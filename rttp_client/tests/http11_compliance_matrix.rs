#[cfg(feature = "async")]
use futures::executor::block_on;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{
  HttpByteRange, HttpByteRangeError, HttpConditionalMetadata, HttpConditionalRequestOutcome,
  HttpEntityTag, HttpIfRangeRequestOutcome, HttpResponse, Request,
};
use rttp_client::HttpClient;
use rttp_http11_test_fixtures as fixtures;

fn client() -> HttpClient {
  HttpClient::new()
}

fn cache_control_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Cache-Control: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn vary_response(values: &[&str]) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    response.push_str("Vary: ");
    response.push_str(value);
    response.push_str("\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

fn age_expires_response(age: &str, expires: &str, include_cache_metadata: bool) -> Vec<u8> {
  let mut response = String::from("HTTP/1.1 200 OK\r\n");
  response.push_str("Age: ");
  response.push_str(age);
  response.push_str("\r\nExpires: ");
  response.push_str(expires);
  response.push_str("\r\n");
  if include_cache_metadata {
    response.push_str("Cache-Control: public, max-age=60\r\n");
    response.push_str("Vary: Accept-Encoding\r\n");
  }
  response.push_str("Content-Length: 2\r\n\r\nOK");
  response.into_bytes()
}

const RANGE_BODY: &[u8] = b"0123456789abcdef";
const CONDITIONAL_LAST_MODIFIED: &str = "Sun, 06 Nov 1994 08:49:37 GMT";
const CONDITIONAL_STALE_DATE: &str = "Sun, 06 Nov 1994 08:49:36 GMT";
const CONDITIONAL_FRESH_DATE: &str = "Sun, 06 Nov 1994 08:49:38 GMT";
const CONDITIONAL_BODY: &str = "cache representation";

const NO_BODY_STATUS_WITH_FRAMING_CASES: &[(&str, &[u8], u32, &str, &str)] = &[
  (
    "204 content-length",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    204,
    "Content-Length",
    "7",
  ),
  (
    "204 chunked",
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    204,
    "Transfer-Encoding",
    "chunked",
  ),
  (
    "304 content-length",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    )
    .as_bytes(),
    304,
    "Content-Length",
    "7",
  ),
  (
    "304 chunked",
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    )
    .as_bytes(),
    304,
    "Transfer-Encoding",
    "chunked",
  ),
];

fn range_response(request: Request) -> HttpResponse {
  match request.header("range") {
    Some(range_header) => match HttpByteRange::parse(range_header, RANGE_BODY.len()) {
      Ok(range) => HttpResponse::partial_content(RANGE_BODY, range),
      Err(HttpByteRangeError::UnsatisfiedRange) => {
        HttpResponse::range_not_satisfiable(RANGE_BODY.len())
      }
      Err(error) => HttpResponse::new(400, "Bad Request").body(error.to_string()),
    },
    None => HttpResponse::ok(RANGE_BODY),
  }
}

fn if_range_response(request: Request, metadata: HttpConditionalMetadata) -> HttpResponse {
  match request.evaluate_if_range(&metadata, RANGE_BODY.len()) {
    Ok(HttpIfRangeRequestOutcome::PartialContent(range)) => {
      HttpResponse::partial_content(RANGE_BODY, range)
        .header("ETag", r#""abc""#)
        .header("Last-Modified", CONDITIONAL_LAST_MODIFIED)
    }
    Ok(HttpIfRangeRequestOutcome::RangeNotSatisfiable) => {
      HttpResponse::range_not_satisfiable(RANGE_BODY.len())
    }
    Ok(HttpIfRangeRequestOutcome::FullResponse) => HttpResponse::ok(RANGE_BODY)
      .header("ETag", r#""abc""#)
      .header("Last-Modified", CONDITIONAL_LAST_MODIFIED),
    Err(error) => HttpResponse::new(400, "Bad Request").body(error.to_string()),
  }
}

fn spawn_range_server() -> (std::net::SocketAddr, thread::JoinHandle<Option<String>>) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send observed range");
        range_response(request)
      })
      .expect("serve range request");
    rx.recv().expect("observed range")
  });

  (addr, handle)
}

fn spawn_if_range_server(
  metadata: HttpConditionalMetadata,
) -> (
  std::net::SocketAddr,
  thread::JoinHandle<(Option<String>, Option<String>)>,
) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind if-range server");
  let addr = server.local_addr().expect("if-range server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed = (
          request.header("range").map(str::to_string),
          request.header("if-range").map(str::to_string),
        );
        tx.send(observed).expect("send observed if-range headers");
        if_range_response(request, metadata.clone())
      })
      .expect("serve if-range request");
    rx.recv().expect("observed if-range headers")
  });

  (addr, handle)
}

fn conditional_metadata() -> HttpConditionalMetadata {
  HttpConditionalMetadata::new()
    .entity_tag(HttpEntityTag::strong("abc"))
    .last_modified(httpdate::parse_http_date(CONDITIONAL_LAST_MODIFIED).expect("metadata date"))
}

fn conditional_response(request: Request) -> HttpResponse {
  let metadata = conditional_metadata();
  match request.evaluate_conditional(&metadata) {
    HttpConditionalRequestOutcome::Proceed => HttpResponse::ok(CONDITIONAL_BODY)
      .header("ETag", r#""abc""#)
      .header("Last-Modified", CONDITIONAL_LAST_MODIFIED),
    HttpConditionalRequestOutcome::NotModified => HttpResponse::not_modified(&metadata),
    HttpConditionalRequestOutcome::PreconditionFailed => HttpResponse::precondition_failed(),
  }
}

fn spawn_conditional_server() -> (std::net::SocketAddr, thread::JoinHandle<Option<String>>) {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind conditional server");
  let addr = server.local_addr().expect("conditional server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let observed_validator = [
          "If-None-Match",
          "If-Match",
          "If-Modified-Since",
          "If-Unmodified-Since",
        ]
        .iter()
        .find_map(|name| request.header(name).map(|value| format!("{name}: {value}")));
        tx.send(observed_validator)
          .expect("send observed validator");
        conditional_response(request)
      })
      .expect("serve conditional request");
    rx.recv().expect("observed validator")
  });

  (addr, handle)
}

fn assert_partial_response(
  name: &str,
  response: rttp_client::response::Response,
  expected_content_range: &str,
  expected_body: &str,
) {
  assert_eq!(206, response.code(), "{name}");
  assert!(response.is_partial_content(), "{name}");
  assert_eq!(
    Some(&expected_content_range.to_string()),
    response.header_value("Content-Range"),
    "{name}"
  );
  assert_eq!(expected_body, response.body().string().unwrap(), "{name}");
}

fn assert_observed_range(
  handle: thread::JoinHandle<Option<String>>,
  expected_range: &str,
  name: &str,
) {
  assert_eq!(
    Some(expected_range.to_string()),
    handle.join().expect("range server thread"),
    "{name}"
  );
}

fn assert_observed_if_range(
  handle: thread::JoinHandle<(Option<String>, Option<String>)>,
  expected_range: &str,
  expected_if_range: &str,
  name: &str,
) {
  assert_eq!(
    (
      Some(expected_range.to_string()),
      Some(expected_if_range.to_string())
    ),
    handle.join().expect("if-range server thread"),
    "{name}"
  );
}

fn assert_response_cache_control(
  name: &str,
  response: rttp_client::response::Response,
  expected: &fixtures::cache_control::ResponseCase,
) {
  let cache_control = response
    .cache_control()
    .unwrap_or_else(|err| panic!("{name} cache-control should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Cache-Control"));

  assert_eq!(expected.no_cache, cache_control.no_cache(), "{name}");
  assert_eq!(
    expected.no_cache_fields,
    cache_control.no_cache_fields().as_slice(),
    "{name}"
  );
  assert_eq!(expected.no_store, cache_control.no_store(), "{name}");
  assert_eq!(expected.max_age, cache_control.max_age(), "{name}");
  assert_eq!(expected.s_maxage, cache_control.s_maxage(), "{name}");
  assert_eq!(expected.private, cache_control.private(), "{name}");
  assert_eq!(
    expected.private_fields,
    cache_control.private_fields().as_slice(),
    "{name}"
  );
  assert_eq!(expected.public, cache_control.public(), "{name}");
  assert_eq!(
    expected.must_revalidate,
    cache_control.must_revalidate(),
    "{name}"
  );
  assert_eq!(
    expected.proxy_revalidate,
    cache_control.proxy_revalidate(),
    "{name}"
  );
  assert_eq!(expected.immutable, cache_control.immutable(), "{name}");
  assert_eq!(
    expected.stale_while_revalidate,
    cache_control.stale_while_revalidate(),
    "{name}"
  );
  assert_eq!(
    expected.stale_if_error,
    cache_control.stale_if_error(),
    "{name}"
  );
  assert_eq!(
    expected.extensions.len(),
    cache_control.extensions().len(),
    "{name}"
  );
  for ((expected_name, expected_value), observed) in
    expected.extensions.iter().zip(cache_control.extensions())
  {
    assert_eq!(*expected_name, observed.name(), "{name}");
    assert_eq!(*expected_value, observed.value(), "{name}");
  }
}

fn assert_response_vary(
  name: &str,
  response: rttp_client::response::Response,
  expected: &fixtures::vary::ResponseCase,
) {
  let raw_values: Vec<&str> = response
    .header_values("vary")
    .into_iter()
    .map(String::as_str)
    .collect();
  assert_eq!(expected.values, raw_values.as_slice(), "{name}");

  let vary = response
    .vary()
    .unwrap_or_else(|err| panic!("{name} Vary should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Vary"));

  assert_eq!(expected.wildcard, vary.is_any(), "{name}");
  assert_eq!(
    expected.field_names,
    vary.field_names().as_slice(),
    "{name}"
  );
  for field_name in expected.field_names {
    assert!(vary.contains_field_name(field_name), "{name} {field_name}");
    assert!(
      vary.contains_field_name(field_name.to_ascii_uppercase()),
      "{name} uppercase {field_name}"
    );
  }
}

fn assert_cache_control_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-control-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.cache_control().is_err(),
    "{name} helper should reject invalid Cache-Control"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_vary_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/vary-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.vary().is_err(),
    "{name} helper should reject invalid Vary"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_age_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/age-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.age().is_err(),
    "{name} helper should reject invalid Age"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

fn assert_expires_helper_rejects_but_preserves_response(name: &str, raw_response: Vec<u8>) {
  let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/expires-invalid", addr))
    .emit()
    .unwrap_or_else(|err| panic!("{name} response should remain parseable: {err}"));

  assert!(
    response.expires().is_err(),
    "{name} helper should reject invalid Expires"
  );
  assert_eq!("OK", response.body().string().unwrap(), "{name}");

  handle.join().expect("raw response server thread");
}

enum ConditionalHeader {
  IfNoneMatch(&'static str),
  IfMatch(&'static str),
  IfModifiedSince(&'static str),
  IfUnmodifiedSince(&'static str),
  Manual(&'static str, &'static str),
}

#[test]
fn sync_client_parses_shared_cache_control_response_matrix() {
  for case in fixtures::cache_control::response_cases() {
    let raw_response = cache_control_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/cache-control", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_cache_control(case.name, response, case);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_vary_response_matrix() {
  for case in fixtures::vary::response_cases() {
    let raw_response = vary_response(case.values);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/vary", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_response_vary(case.name, response, case);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_age_response_matrix() {
  for case in fixtures::age_expires::age_cases() {
    let raw_response = age_expires_response(
      case.value,
      fixtures::age_expires::EXPIRES_IMF_FIXDATE,
      false,
    );
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/age", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(case.delta_seconds),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(&case.value.to_string()),
      response.header_value("Age"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_shared_expires_response_matrix() {
  for case in fixtures::age_expires::expires_cases() {
    let raw_response = age_expires_response("0", case.value, false);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/expires", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(std::time::UNIX_EPOCH + Duration::from_secs(case.unix_seconds)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(&case.value.to_string()),
      response.header_value("Expires"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_parses_age_expires_with_existing_cache_metadata_helpers() {
  for case in fixtures::age_expires::declaration_cases() {
    let raw_response = age_expires_response(case.age_value, case.expires_value, true);
    let (addr, handle) = fixtures::spawn_socket2_owned_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/cache-metadata", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{} response should parse: {err}", case.name));

    assert_eq!(
      Some(case.age),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(std::time::UNIX_EPOCH + Duration::from_secs(case.expires_unix_seconds)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
    assert_eq!(
      Some(60),
      response
        .cache_control()
        .expect("Cache-Control should parse")
        .expect("Cache-Control should be present")
        .max_age(),
      "{}",
      case.name
    );
    assert!(
      response
        .vary()
        .expect("Vary should parse")
        .expect("Vary should be present")
        .contains_field_name("accept-encoding"),
      "{}",
      case.name
    );
    assert_eq!("OK", response.body().string().unwrap(), "{}", case.name);
    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_cache_control_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::cache_control::invalid_response_cases() {
    assert_cache_control_helper_rejects_but_preserves_response(
      case.name,
      cache_control_response(&[case.value]),
    );
  }
}

#[test]
fn sync_client_vary_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::vary::invalid_cases() {
    assert_vary_helper_rejects_but_preserves_response(case.name, vary_response(&[case.value]));
  }
}

#[test]
fn sync_client_age_and_expires_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::age_expires::invalid_age_cases() {
    assert_age_helper_rejects_but_preserves_response(
      case.name,
      age_expires_response(
        case.value,
        fixtures::age_expires::EXPIRES_IMF_FIXDATE,
        false,
      ),
    );
  }

  for case in fixtures::age_expires::invalid_expires_cases() {
    assert_expires_helper_rejects_but_preserves_response(
      case.name,
      age_expires_response("0", case.value, false),
    );
  }
}

#[test]
fn sync_client_cache_control_helper_enforces_shared_bounds() {
  assert_cache_control_helper_rejects_but_preserves_response(
    "too many response Cache-Control directives",
    cache_control_response(&[&fixtures::cache_control::too_many_directives_value()]),
  );
  assert_cache_control_helper_rejects_but_preserves_response(
    "oversized response Cache-Control value",
    cache_control_response(&[&fixtures::cache_control::oversized_value()]),
  );
}

#[test]
fn sync_client_vary_helper_enforces_shared_bounds() {
  assert_vary_helper_rejects_but_preserves_response(
    "too many Vary field names",
    vary_response(&[&fixtures::vary::too_many_field_names_value()]),
  );
  assert_vary_helper_rejects_but_preserves_response(
    "oversized Vary value",
    vary_response(&[&fixtures::vary::oversized_value()]),
  );
}

#[test]
fn sync_client_cache_control_matrix_keeps_cache_engine_non_goals_explicit() {
  let raw_response = concat!(
    "HTTP/1.1 200 OK\r\n",
    "ETag: \"representation\"\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  )
  .as_bytes();
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

  let response = client()
    .get()
    .url(format!("http://{}/matrix/cache-control-non-goals", addr))
    .emit()
    .expect("response without Cache-Control should parse");

  assert!(response
    .cache_control()
    .expect("missing header is valid")
    .is_none());
  assert_eq!(
    Some(&"\"representation\"".to_string()),
    response.header_value("ETag")
  );
  assert_eq!("OK", response.body().string().unwrap());

  handle.join().expect("raw response server thread");
}

impl ConditionalHeader {
  fn apply(&self, client: &mut HttpClient) {
    match self {
      Self::IfNoneMatch(value) => {
        client
          .if_none_match(value)
          .expect("If-None-Match helper should accept test validator");
      }
      Self::IfMatch(value) => {
        client
          .if_match(value)
          .expect("If-Match helper should accept test validator");
      }
      Self::IfModifiedSince(value) => {
        client
          .if_modified_since(value)
          .expect("If-Modified-Since helper should accept test date");
      }
      Self::IfUnmodifiedSince(value) => {
        client
          .if_unmodified_since(value)
          .expect("If-Unmodified-Since helper should accept test date");
      }
      Self::Manual(name, value) => {
        client.header((*name, *value));
      }
    }
  }
}

struct ConditionalCase {
  name: &'static str,
  method: &'static str,
  header: ConditionalHeader,
  expected_validator: &'static str,
  expected_code: u32,
  expected_body: &'static str,
}

const ORDINARY_200_FRAMING_CASES: &[(&str, &[u8], &str, &str, &str)] = &[
  (
    "200 content-length",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 2\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes(),
    "Content-Length",
    "2",
    "OK",
  ),
  (
    "200 chunked",
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nchunked\r\n",
      "6\r\n body!\r\n",
      "0\r\n\r\n"
    )
    .as_bytes(),
    "Transfer-Encoding",
    "chunked",
    "chunked body!",
  ),
];

#[test]
fn sync_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  let response = client()
    .get()
    .url(format!("http://{}/matrix/chunked", addr))
    .emit()
    .expect("sync response should parse");

  assert_eq!(200, response.code());
  assert_eq!("chunked body!", response.body().string().unwrap());
  assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
  assert_eq!(
    Some(&"signed".to_string()),
    response.trailer_value("X-SIGNATURE")
  );

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
fn sync_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/no-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(*status, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!("", response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    let response = client()
      .get()
      .url(format!("http://{}/matrix/ordinary-body", addr))
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(200, response.code(), "{name}");
    assert_eq!(
      Some(&framing_value.to_string()),
      response.header_value(framing_header),
      "{name}"
    );
    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!(*body, response.body().string().unwrap(), "{name}");

    handle.join().expect("raw response server thread");
  }
}

#[test]
fn sync_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  let error = client()
    .get()
    .url(format!("http://{}/matrix/ambiguous", addr))
    .emit()
    .expect_err("ambiguous response should be rejected");

  assert!(
    error.to_string().contains("Content-Length"),
    "unexpected error: {error}"
  );
  handle.join().expect("raw response server thread");
}

#[test]
fn sync_client_waits_for_shared_expect_continue_fixture() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  let response = client()
    .post()
    .url(format!("http://{}{}", addr, fixture.target))
    .header(("Expect", "100-continue"))
    .raw(String::from_utf8_lossy(fixture.body).as_ref())
    .emit()
    .expect("expect-continue response should parse");

  assert_eq!(200, response.code());
  assert_eq!("accepted", response.body().string().unwrap());

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}

#[test]
fn sync_client_range_helpers_interoperate_with_server_partial_content_helper() {
  for (name, expected_range, expected_content_range, expected_body, request) in [
    (
      "bounded range",
      "bytes=3-7",
      "bytes 3-7/16",
      "34567",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted");
      }) as Box<dyn Fn(&mut HttpClient)>,
    ),
    (
      "open-ended range",
      "bytes=12-",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client.range_from(12);
      }),
    ),
    (
      "suffix range",
      "bytes=-4",
      "bytes 12-15/16",
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_suffix(4)
          .expect("suffix range should be accepted");
      }),
    ),
  ] {
    let (addr, handle) = spawn_range_server();
    let mut client = client();
    client.get().url(format!("http://{}/matrix/range", addr));
    request(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_partial_response(name, response, expected_content_range, expected_body);
    assert_observed_range(handle, expected_range, name);
  }
}

#[test]
fn sync_client_if_range_helpers_interoperate_with_server_range_validator_evaluation() {
  for (name, metadata, expected_range, expected_if_range, expected_code, expected_body, request) in [
    (
      "matching strong ETag returns partial content",
      conditional_metadata(),
      "bytes=3-7",
      r#""abc""#,
      206,
      "34567",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""abc""#)
          .expect("matching strong etag should be accepted");
      }) as Box<dyn Fn(&mut HttpClient)>,
    ),
    (
      "non-matching strong ETag falls back to full response",
      conditional_metadata(),
      "bytes=3-7",
      r#""other""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""other""#)
          .expect("non-matching strong etag should be accepted");
      }),
    ),
    (
      "matching HTTP-date returns partial content",
      conditional_metadata(),
      "bytes=12-",
      CONDITIONAL_LAST_MODIFIED,
      206,
      "cdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(12)
          .if_range_date(CONDITIONAL_LAST_MODIFIED)
          .expect("matching date should be accepted");
      }),
    ),
    (
      "stale HTTP-date falls back to full response",
      conditional_metadata(),
      "bytes=12-",
      CONDITIONAL_STALE_DATE,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(12)
          .if_range_date(CONDITIONAL_STALE_DATE)
          .expect("stale date should be accepted");
      }),
    ),
    (
      "missing metadata falls back to full response",
      HttpConditionalMetadata::new(),
      "bytes=3-7",
      r#""abc""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .if_range_etag(r#""abc""#)
          .expect("strong etag should be accepted");
      }),
    ),
    (
      "matching validator preserves unsatisfied range response",
      conditional_metadata(),
      "bytes=16-",
      r#""abc""#,
      416,
      "",
      Box::new(|client: &mut HttpClient| {
        client
          .range_from(RANGE_BODY.len() as u64)
          .if_range_etag(r#""abc""#)
          .expect("matching strong etag should be accepted");
      }),
    ),
    (
      "manual If-Range header remains available",
      conditional_metadata(),
      "bytes=3-7",
      r#"W/"abc""#,
      200,
      "0123456789abcdef",
      Box::new(|client: &mut HttpClient| {
        client
          .range(3, 7)
          .expect("bounded range should be accepted")
          .header(("If-Range", r#"W/"abc""#));
      }),
    ),
  ] {
    let (addr, handle) = spawn_if_range_server(metadata);
    let mut client = client();
    client.get().url(format!("http://{}/matrix/if-range", addr));
    request(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

    assert_eq!(expected_code, response.code(), "{name}");
    assert_eq!(expected_body, response.body().string().unwrap(), "{name}");
    if expected_code == 206 {
      assert!(response.is_partial_content(), "{name}");
    }
    if expected_code == 416 {
      assert!(response.is_range_not_satisfiable(), "{name}");
      assert_eq!(
        Some(&format!("bytes */{}", RANGE_BODY.len())),
        response.header_value("Content-Range"),
        "{name}"
      );
    }
    assert_observed_if_range(handle, expected_range, expected_if_range, name);
  }
}

#[test]
fn sync_client_conditional_helpers_interoperate_with_server_validator_evaluation() {
  let cases = [
    ConditionalCase {
      name: "GET If-None-Match strong match returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#""abc""#),
      expected_validator: r#"If-None-Match: "abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-None-Match weak match returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#"W/"abc""#),
      expected_validator: r#"If-None-Match: W/"abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-None-Match miss proceeds",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch(r#""different""#),
      expected_validator: r#"If-None-Match: "different""#,
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-None-Match wildcard returns 304",
      method: "GET",
      header: ConditionalHeader::IfNoneMatch("*"),
      expected_validator: "If-None-Match: *",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "PUT If-None-Match wildcard returns 412",
      method: "PUT",
      header: ConditionalHeader::IfNoneMatch("*"),
      expected_validator: "If-None-Match: *",
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match strong match proceeds",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#""abc""#),
      expected_validator: r#"If-Match: "abc""#,
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Match weak comparison miss returns 412",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#"W/"abc""#),
      expected_validator: r#"If-Match: W/"abc""#,
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match non-match returns 412",
      method: "GET",
      header: ConditionalHeader::IfMatch(r#""different""#),
      expected_validator: r#"If-Match: "different""#,
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Match wildcard proceeds",
      method: "GET",
      header: ConditionalHeader::IfMatch("*"),
      expected_validator: "If-Match: *",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Modified-Since fresh returns 304",
      method: "GET",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Modified-Since stale proceeds",
      method: "GET",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_STALE_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:36 GMT",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "GET If-Unmodified-Since stale returns 412",
      method: "GET",
      header: ConditionalHeader::IfUnmodifiedSince(CONDITIONAL_STALE_DATE),
      expected_validator: "If-Unmodified-Since: Sun, 06 Nov 1994 08:49:36 GMT",
      expected_code: 412,
      expected_body: "",
    },
    ConditionalCase {
      name: "GET If-Unmodified-Since fresh proceeds",
      method: "GET",
      header: ConditionalHeader::IfUnmodifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Unmodified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 200,
      expected_body: CONDITIONAL_BODY,
    },
    ConditionalCase {
      name: "HEAD If-None-Match match returns bodyless 304",
      method: "HEAD",
      header: ConditionalHeader::IfNoneMatch(r#""abc""#),
      expected_validator: r#"If-None-Match: "abc""#,
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "HEAD If-Modified-Since fresh returns bodyless 304",
      method: "HEAD",
      header: ConditionalHeader::IfModifiedSince(CONDITIONAL_FRESH_DATE),
      expected_validator: "If-Modified-Since: Sun, 06 Nov 1994 08:49:38 GMT",
      expected_code: 304,
      expected_body: "",
    },
    ConditionalCase {
      name: "manual If-None-Match list remains available",
      method: "GET",
      header: ConditionalHeader::Manual("If-None-Match", r#""different", "abc""#),
      expected_validator: r#"If-None-Match: "different", "abc""#,
      expected_code: 304,
      expected_body: "",
    },
  ];

  for case in cases {
    let (addr, handle) = spawn_conditional_server();
    let mut client = client();
    client
      .method(case.method)
      .url(format!("http://{}/matrix/conditional", addr));
    case.header.apply(&mut client);

    let response = client
      .emit()
      .unwrap_or_else(|err| panic!("{} should parse: {err}", case.name));

    assert_eq!(case.expected_code, response.code(), "{}", case.name);
    assert_eq!(
      case.expected_body,
      response.body().string().unwrap(),
      "{}",
      case.name
    );
    assert_eq!(
      Some(case.expected_validator.to_string()),
      handle.join().expect("conditional server thread"),
      "{}",
      case.name
    );
  }
}

#[test]
fn sync_client_unsatisfied_range_maps_to_server_416_response() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_from(RANGE_BODY.len() as u64)
    .emit()
    .expect("unsatisfied range response should parse");

  assert_eq!(416, response.code());
  assert!(response.is_range_not_satisfiable());
  assert_eq!(
    Some(&format!("bytes */{}", RANGE_BODY.len())),
    response.header_value("Content-Range")
  );
  assert_eq!("", response.body().string().unwrap());
  assert_observed_range(
    handle,
    &format!("bytes={}-", RANGE_BODY.len()),
    "unsatisfied range",
  );
}

#[test]
fn sync_client_malformed_range_helpers_reject_before_reaching_server() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind range server");
  let addr = server.local_addr().expect("range server addr");
  let (tx, rx) = mpsc::channel();
  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.header("range").map(str::to_string))
          .expect("send unexpected range");
        HttpResponse::ok("unexpected")
      })
      .expect("serve optional range request");
  });

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range(7, 3)
    .expect_err("inverted range should be rejected");
  assert!(error.is_builder());

  let mut range_client = client();
  let error = range_client
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .range_suffix(0)
    .expect_err("empty suffix should be rejected");
  assert!(error.is_builder());

  assert!(
    rx.recv_timeout(Duration::from_millis(100)).is_err(),
    "malformed helper input should not reach the range server"
  );

  let mut stream = TcpStream::connect(addr).expect("release range server");
  stream
    .write_all(b"GET /matrix/release HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    .expect("write release request");
  assert_eq!(None, rx.recv().expect("release request observed range"));
  handle.join().expect("range server thread");
}

#[test]
fn sync_client_manual_range_header_interoperates_with_server_partial_content_helper() {
  let (addr, handle) = spawn_range_server();

  let response = client()
    .get()
    .url(format!("http://{}/matrix/range", addr))
    .header(("Range", "bytes=5-9"))
    .emit()
    .expect("manual range response should parse");

  assert_partial_response("manual range", response, "bytes 5-9/16", "56789");
  assert_observed_range(handle, "bytes=5-9", "manual range");
}

#[test]
#[cfg(feature = "async")]
fn async_client_decodes_shared_chunk_extensions_and_trailers_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::CHUNKED_WITH_EXTENSIONS_AND_TRAILERS,
  );

  block_on(async {
    let response = client()
      .get()
      .url(format!("http://{}/matrix/chunked", addr))
      .rasync()
      .await
      .expect("async response should parse");

    assert_eq!(200, response.code());
    assert_eq!("chunked body!", response.body().string().unwrap());
    assert_eq!(Some(&"abc".to_string()), response.trailer_value("x-trace"));
    assert_eq!(
      Some(&"signed".to_string()),
      response.trailer_value("X-SIGNATURE")
    );
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).starts_with("GET /matrix/chunked HTTP/1.1"));
}

#[test]
#[cfg(feature = "async")]
fn async_client_treats_204_and_304_as_bodyless_despite_framing_headers() {
  for (name, raw_response, status, framing_header, framing_value) in
    NO_BODY_STATUS_WITH_FRAMING_CASES
  {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/no-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(*status, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!("", response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_preserves_ordinary_200_framed_bodies() {
  for (name, raw_response, framing_header, framing_value, body) in ORDINARY_200_FRAMING_CASES {
    let (addr, handle) = fixtures::spawn_socket2_raw_response_server(raw_response);

    block_on(async {
      let response = client()
        .get()
        .url(format!("http://{}/matrix/ordinary-body", addr))
        .rasync()
        .await
        .unwrap_or_else(|err| panic!("{name} should parse: {err}"));

      assert_eq!(200, response.code(), "{name}");
      assert_eq!(
        Some(&framing_value.to_string()),
        response.header_value(framing_header),
        "{name}"
      );
      assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
      assert_eq!(*body, response.body().string().unwrap(), "{name}");
    });

    handle.join().expect("raw response server thread");
  }
}

#[test]
#[cfg(feature = "async")]
fn async_client_rejects_shared_response_framing_ambiguity_fixture() {
  let (addr, handle) = fixtures::spawn_socket2_raw_response_server(
    fixtures::response::TRANSFER_ENCODING_WITH_CONTENT_LENGTH,
  );

  block_on(async {
    let error = client()
      .get()
      .url(format!("http://{}/matrix/ambiguous", addr))
      .rasync()
      .await
      .expect_err("ambiguous response should be rejected");

    assert!(
      error.to_string().contains("Content-Length"),
      "unexpected error: {error}"
    );
  });

  handle.join().expect("raw response server thread");
}

#[test]
#[cfg(feature = "async")]
fn async_client_waits_for_shared_expect_continue_fixture() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let (addr, handle) = fixtures::spawn_socket2_expect_continue_server(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Length: 8\r\n",
      "Connection: close\r\n",
      "\r\n",
      "accepted"
    )
    .as_bytes(),
  );

  block_on(async {
    let response = client()
      .post()
      .url(format!("http://{}{}", addr, fixture.target))
      .header(("Expect", "100-continue"))
      .raw(String::from_utf8_lossy(fixture.body).as_ref())
      .rasync()
      .await
      .expect("expect-continue response should parse");

    assert_eq!(200, response.code());
    assert_eq!("accepted", response.body().string().unwrap());
  });

  let request = handle.join().expect("raw response server thread");
  assert!(String::from_utf8_lossy(&request).contains("Expect: 100-continue"));
  assert!(request.ends_with(fixture.body));
}
