use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use rttp::server::{
  HttpAcceptRanges, HttpAllowedMethods, HttpContentDisposition, HttpContentLanguages, HttpRequest,
  HttpRequestCacheControl, HttpResponse, HttpResponseCacheControl, HttpRetryAfter, HttpVary,
};
use rttp_test_support as fixtures;

#[derive(Debug)]
struct ParsedResponse<'a> {
  head: &'a str,
  body: &'a str,
}

fn parse_content_length_response(input: &str) -> (ParsedResponse<'_>, &str) {
  let (head, after_head) = input.split_once("\r\n\r\n").expect("response head");
  let content_length = head
    .lines()
    .find_map(|line| line.strip_prefix("Content-Length: "))
    .expect("content length")
    .parse::<usize>()
    .expect("content length value");
  let (body, remaining) = after_head.split_at(content_length);

  (ParsedResponse { head, body }, remaining)
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
  head
    .lines()
    .filter_map(|line| line.split_once(':'))
    .find(|(observed_name, _)| observed_name.eq_ignore_ascii_case(name))
    .map(|(_, value)| value.trim())
}

fn cache_control_request(values: &[&str]) -> Vec<u8> {
  let mut request = String::from("GET /matrix/cache-control HTTP/1.1\r\nHost: example.test\r\n");
  for value in values {
    request.push_str("Cache-Control: ");
    request.push_str(value);
    request.push_str("\r\n");
  }
  request.push_str("\r\n");
  request.into_bytes()
}

fn cache_control_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Cache-Control", value)
    })
}

fn vary_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Vary", value)
    })
}

fn allow_response(values: &[&str]) -> HttpResponse {
  values.iter().fold(
    HttpResponse::new(405, "Method Not Allowed"),
    |response, value| response.header("Allow", value),
  )
}

fn accept_ranges_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Accept-Ranges", value)
    })
}

fn content_language_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Content-Language", value)
    })
}

fn content_location_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Content-Location", value)
    })
}

fn content_disposition_response(values: &[&str]) -> HttpResponse {
  values
    .iter()
    .fold(HttpResponse::new(200, "OK"), |response, value| {
      response.header("Content-Disposition", value)
    })
}

fn age_expires_response(age: u64, expires: std::time::SystemTime) -> HttpResponse {
  HttpResponse::ok("OK")
    .with_age(age)
    .with_expires(expires)
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("test Vary should parse")
}

fn retry_after_response(retry_after: std::time::SystemTime) -> HttpResponse {
  HttpResponse::new(503, "Service Unavailable")
    .with_retry_after_date(retry_after)
    .with_age(5)
    .with_expires(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS))
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("test Vary should parse")
}

fn allow_with_cache_metadata_response(methods: &[&str]) -> HttpResponse {
  HttpResponse::new(405, "Method Not Allowed")
    .with_allow(methods.iter().copied())
    .expect("test Allow should parse")
    .with_retry_after_delta(30)
    .with_age(5)
    .with_expires(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS))
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("test Vary should parse")
}

fn expected_retry_after(kind: &fixtures::retry_after::RetryAfterKind) -> HttpRetryAfter {
  match kind {
    fixtures::retry_after::RetryAfterKind::DeltaSeconds(delta_seconds) => {
      HttpRetryAfter::DeltaSeconds(*delta_seconds)
    }
    fixtures::retry_after::RetryAfterKind::HttpDate(unix_seconds) => {
      HttpRetryAfter::HttpDate(UNIX_EPOCH + Duration::from_secs(*unix_seconds))
    }
  }
}

fn assert_request_cache_control(
  name: &str,
  cache_control: HttpRequestCacheControl,
  expected: &fixtures::cache_control::RequestCase,
) {
  assert_eq!(expected.no_cache, cache_control.no_cache(), "{name}");
  assert_eq!(expected.no_store, cache_control.no_store(), "{name}");
  assert_eq!(expected.max_age, cache_control.max_age(), "{name}");
  assert_eq!(expected.max_stale, cache_control.max_stale(), "{name}");
  assert_eq!(expected.min_fresh, cache_control.min_fresh(), "{name}");
  assert_eq!(
    expected.no_transform,
    cache_control.no_transform(),
    "{name}"
  );
  assert_eq!(
    expected.only_if_cached,
    cache_control.only_if_cached(),
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

fn assert_response_cache_control(
  name: &str,
  cache_control: HttpResponseCacheControl,
  expected: &fixtures::cache_control::ResponseCase,
) {
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

fn assert_response_vary(name: &str, vary: &HttpVary, expected: &fixtures::vary::ResponseCase) {
  assert_eq!(expected.wildcard, vary.is_wildcard(), "{name}");
  assert_eq!(
    expected.field_names,
    vary.field_names().as_slice(),
    "{name}"
  );
}

fn assert_response_allow(
  name: &str,
  allow: &HttpAllowedMethods,
  expected: &fixtures::allow::ResponseCase,
) {
  assert_eq!(expected.methods, allow.methods().as_slice(), "{name}");
}

fn assert_response_accept_ranges(
  name: &str,
  accept_ranges: &HttpAcceptRanges,
  expected: &fixtures::accept_ranges::ResponseCase,
) {
  assert_eq!(expected.none, accept_ranges.is_none(), "{name}");
  if expected.none {
    assert!(accept_ranges.units().is_empty(), "{name}");
    assert_eq!("none", accept_ranges.header_value(), "{name}");
  } else {
    assert_eq!(expected.units, accept_ranges.units().as_slice(), "{name}");
    assert_eq!(
      expected.units.join(", "),
      accept_ranges.header_value(),
      "{name}"
    );
  }
}

fn assert_response_content_language(
  name: &str,
  content_language: &HttpContentLanguages,
  expected: &fixtures::content_language::ResponseCase,
) {
  assert_eq!(
    expected.languages,
    content_language.languages().as_slice(),
    "{name}"
  );
}

#[test]
fn model_parser_accepts_shared_fixed_length_request_fixture() {
  let fixture = fixtures::request::fixed_length_post();

  let request = HttpRequest::parse(fixture.raw).expect("fixed-length request should parse");

  assert_eq!(fixture.method, request.method());
  assert_eq!(fixture.path, request.path());
  assert_eq!(fixture.query, request.query());
  assert_eq!(fixture.version, request.version());
  assert_eq!(Some(fixture.host), request.header("host"));
  assert_eq!(fixture.body, request.body());
}

#[test]
fn model_parser_accepts_shared_cache_control_request_matrix() {
  for case in fixtures::cache_control::request_cases() {
    let request =
      HttpRequest::parse(&cache_control_request(case.values)).expect("request should parse");
    let cache_control = request
      .cache_control()
      .unwrap_or_else(|err| panic!("{} cache-control should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Cache-Control", case.name));

    assert_request_cache_control(case.name, cache_control, case);
  }
}

fn assert_response_content_location(
  name: &str,
  response: &HttpResponse,
  expected: &fixtures::content_location::ResponseCase,
) {
  let content_location = response
    .content_location()
    .unwrap_or_else(|err| panic!("{name} Content-Location should parse: {err}"))
    .unwrap_or_else(|| panic!("{name} should include Content-Location"));
  assert_eq!(
    expected.normalized_value,
    content_location.as_str(),
    "{name}"
  );
}

fn assert_response_content_disposition(
  name: &str,
  content_disposition: &HttpContentDisposition,
  expected: &fixtures::content_disposition::ResponseCase,
) {
  assert_eq!(
    expected.disposition_type,
    content_disposition.disposition_type(),
    "{name}"
  );
  assert_eq!(
    expected.filename,
    content_disposition.parameter("filename"),
    "{name}"
  );
  assert_eq!(
    expected.filename_ext,
    content_disposition.parameter("filename*"),
    "{name}"
  );
  assert_eq!(
    expected.parameters,
    content_disposition.parameters().as_slice(),
    "{name}"
  );
}

#[test]
fn model_parser_cache_control_helper_rejects_shared_invalid_request_matrix() {
  for case in fixtures::cache_control::invalid_request_cases() {
    let request =
      HttpRequest::parse(&cache_control_request(&[case.value])).expect("request should parse");

    assert!(
      request.cache_control().is_err(),
      "{} helper should reject invalid Cache-Control",
      case.name
    );
    assert_eq!(
      Some(case.value),
      request.header("Cache-Control"),
      "{}",
      case.name
    );
  }
}

#[test]
fn model_parser_cache_control_helper_enforces_shared_bounds() {
  let too_many_directives = fixtures::cache_control::too_many_directives_value();
  let request = HttpRequest::parse(&cache_control_request(&[&too_many_directives]))
    .expect("request should parse");

  assert!(
    request.cache_control().is_err(),
    "too many request Cache-Control directives helper should reject invalid Cache-Control"
  );
  assert_eq!(
    Some(too_many_directives.as_str()),
    request.header("Cache-Control"),
    "too many request Cache-Control directives"
  );

  let oversized_value = fixtures::cache_control::oversized_value();
  assert!(
    HttpRequestCacheControl::parse(&oversized_value).is_err(),
    "oversized request Cache-Control value helper should reject invalid Cache-Control"
  );
}

#[test]
fn server_response_helper_accepts_shared_allow_response_matrix() {
  for case in fixtures::allow::response_cases() {
    let response = allow_response(case.values);
    let allow = response
      .allow()
      .unwrap_or_else(|err| panic!("{} Allow should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Allow", case.name));

    assert_response_allow(case.name, &allow, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_accept_ranges_response_matrix() {
  for case in fixtures::accept_ranges::response_cases() {
    let response = accept_ranges_response(case.values);
    let accept_ranges = response
      .accept_ranges()
      .unwrap_or_else(|err| panic!("{} Accept-Ranges should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Accept-Ranges", case.name));

    assert_response_accept_ranges(case.name, &accept_ranges, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_vary_response_matrix() {
  for case in fixtures::vary::response_cases() {
    let response = vary_response(case.values);
    let vary = response
      .vary()
      .unwrap_or_else(|err| panic!("{} Vary should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Vary", case.name));

    assert_response_vary(case.name, &vary, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_content_language_response_matrix() {
  for case in fixtures::content_language::response_cases() {
    let response = content_language_response(case.values);
    let content_language = response
      .content_language()
      .unwrap_or_else(|err| panic!("{} Content-Language should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Content-Language", case.name));

    assert_response_content_language(case.name, &content_language, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_content_location_response_matrix() {
  for case in fixtures::content_location::response_cases() {
    let response = content_location_response(case.values);

    assert_response_content_location(case.name, &response, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_content_disposition_response_matrix() {
  for case in fixtures::content_disposition::response_cases() {
    let response = content_disposition_response(case.values);
    let content_disposition = response
      .content_disposition()
      .unwrap_or_else(|err| panic!("{} Content-Disposition should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Content-Disposition", case.name));

    assert_response_content_disposition(case.name, &content_disposition, case);
  }
}

#[test]
fn server_response_helper_accepts_shared_age_response_matrix() {
  for case in fixtures::age_expires::age_cases() {
    let response = HttpResponse::ok("OK").header("Age", case.value);

    assert_eq!(
      Some(case.delta_seconds),
      response
        .age()
        .unwrap_or_else(|err| panic!("{} Age should parse: {err}", case.name)),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_helper_accepts_shared_expires_response_matrix() {
  for case in fixtures::age_expires::expires_cases() {
    let response = HttpResponse::ok("OK").header("Expires", case.value);

    assert_eq!(
      Some(UNIX_EPOCH + Duration::from_secs(case.unix_seconds)),
      response
        .expires()
        .unwrap_or_else(|err| panic!("{} Expires should parse: {err}", case.name)),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_with_age_and_expires_declares_shared_metadata_matrix() {
  for case in fixtures::age_expires::declaration_cases() {
    let response = age_expires_response(
      case.age,
      UNIX_EPOCH + Duration::from_secs(case.expires_unix_seconds),
    );
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.age_value),
      header_value(&serialized, "Age"),
      "{}",
      case.name
    );
    assert_eq!(
      Some(case.expires_value),
      header_value(&serialized, "Expires"),
      "{}",
      case.name
    );
    assert_eq!(
      Some("public, max-age=60"),
      header_value(&serialized, "Cache-Control"),
      "{}",
      case.name
    );
    assert_eq!(
      Some("accept-encoding"),
      header_value(&serialized, "Vary"),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_age_and_expires_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::age_expires::invalid_age_cases() {
    let response = HttpResponse::ok("OK").header("Age", case.value);

    assert!(
      response.age().is_err(),
      "{} Age helper should reject invalid value",
      case.name
    );
  }

  for case in fixtures::age_expires::invalid_expires_cases() {
    let response = HttpResponse::ok("OK").header("Expires", case.value);

    assert!(
      response.expires().is_err(),
      "{} Expires helper should reject invalid value",
      case.name
    );
  }
}

#[test]
fn server_response_helper_accepts_shared_retry_after_response_matrix() {
  for case in fixtures::retry_after::retry_after_cases() {
    let response = HttpResponse::ok("OK").header("Retry-After", case.value);

    assert_eq!(
      Some(expected_retry_after(&case.kind)),
      response
        .retry_after()
        .unwrap_or_else(|err| panic!("{} Retry-After should parse: {err}", case.name)),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_with_retry_after_declares_shared_metadata_matrix() {
  for case in fixtures::retry_after::declaration_cases() {
    let delta_response =
      HttpResponse::new(503, "Service Unavailable").with_retry_after_delta(case.delta_seconds);
    let date_response = HttpResponse::new(503, "Service Unavailable")
      .with_retry_after_date(UNIX_EPOCH + Duration::from_secs(case.date_unix_seconds));
    let delta_serialized = String::from_utf8(delta_response.to_bytes()).expect("response is UTF-8");
    let date_serialized = String::from_utf8(date_response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.delta_value),
      header_value(&delta_serialized, "Retry-After"),
      "{}",
      case.name
    );
    assert_eq!(
      Some(case.date_value),
      header_value(&date_serialized, "Retry-After"),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_with_retry_after_coexists_with_cache_metadata_helpers() {
  let response = retry_after_response(
    UNIX_EPOCH + Duration::from_secs(fixtures::retry_after::RETRY_AFTER_UNIX_SECONDS),
  );
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some(fixtures::retry_after::RETRY_AFTER_IMF_FIXDATE),
    header_value(&serialized, "Retry-After")
  );
  assert_eq!(Some("5"), header_value(&serialized, "Age"));
  assert_eq!(
    Some(fixtures::age_expires::EXPIRES_IMF_FIXDATE),
    header_value(&serialized, "Expires")
  );
  assert_eq!(
    Some("public, max-age=60"),
    header_value(&serialized, "Cache-Control")
  );
  assert_eq!(Some("accept-encoding"), header_value(&serialized, "Vary"));
  assert_eq!(
    Some(HttpRetryAfter::HttpDate(
      UNIX_EPOCH + Duration::from_secs(fixtures::retry_after::RETRY_AFTER_UNIX_SECONDS)
    )),
    response
      .retry_after()
      .expect("Retry-After should parse with cache metadata")
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .field_names()
    .contains(&"accept-encoding"));
}

#[test]
fn server_response_retry_after_helper_rejects_shared_invalid_matrix() {
  for case in fixtures::retry_after::invalid_cases() {
    let response = HttpResponse::ok("OK").header("Retry-After", case.value);

    assert!(
      response.retry_after().is_err(),
      "{} Retry-After helper should reject invalid value",
      case.name
    );
  }
}

#[test]
fn server_response_retry_after_helper_rejects_duplicate_singleton_and_oversized_values() {
  let duplicate = HttpResponse::ok("OK")
    .header("Retry-After", "60")
    .header("Retry-After", "120");
  assert!(
    duplicate.retry_after().is_err(),
    "duplicate Retry-After header fields should be rejected"
  );

  let oversized = HttpResponse::ok("OK").header(
    "Retry-After",
    fixtures::retry_after::oversized_value().as_str(),
  );
  assert!(
    oversized.retry_after().is_err(),
    "oversized Retry-After value should be rejected"
  );
}

#[test]
fn server_response_with_content_disposition_declares_shared_metadata_matrix() {
  for case in fixtures::content_disposition::response_cases() {
    let disposition = HttpContentDisposition::new(case.disposition_type)
      .unwrap_or_else(|err| panic!("{} disposition type should parse: {err}", case.name));
    let disposition = case
      .parameters
      .iter()
      .fold(disposition, |disposition, (name, value)| {
        disposition
          .with_parameter(name, value)
          .unwrap_or_else(|err| panic!("{} parameter should parse: {err}", case.name))
      });
    let response = HttpResponse::ok("OK")
      .header("Content-Disposition", "inline")
      .with_content_disposition(disposition)
      .unwrap_or_else(|err| panic!("{} declaration should parse: {err}", case.name));
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.normalized_value),
      header_value(&serialized, "Content-Disposition"),
      "{}",
      case.name
    );
    assert_eq!(
      1,
      serialized.matches("\r\nContent-Disposition: ").count(),
      "{}",
      case.name
    );
    let content_disposition = response
      .content_disposition()
      .expect("Content-Disposition should parse")
      .expect("Content-Disposition should be present");
    assert_response_content_disposition(case.name, &content_disposition, case);
  }
}

#[test]
fn server_response_content_disposition_helper_rejects_shared_invalid_matrix() {
  for case in fixtures::content_disposition::invalid_cases() {
    assert!(
      HttpContentDisposition::parse(case.value).is_err(),
      "{} Content-Disposition helper should reject invalid value",
      case.name
    );
    assert!(
      content_disposition_response(&[case.value])
        .content_disposition()
        .is_err(),
      "{} response parser should reject invalid Content-Disposition value",
      case.name
    );
  }
}

#[test]
fn server_response_content_disposition_helper_rejects_duplicates_and_enforces_shared_bounds() {
  let duplicate = content_disposition_response(&["attachment; filename=one.txt", "inline"]);
  assert!(
    duplicate.content_disposition().is_err(),
    "duplicate Content-Disposition header fields should be rejected"
  );

  assert!(
    content_disposition_response(&[fixtures::content_disposition::duplicate_parameter_value()])
      .content_disposition()
      .is_err(),
    "duplicate Content-Disposition parameters should be rejected"
  );

  let oversized = fixtures::content_disposition::oversized_value();
  assert!(
    HttpContentDisposition::parse(&oversized).is_err(),
    "oversized Content-Disposition value should be rejected"
  );
  assert!(
    content_disposition_response(&[&oversized])
      .content_disposition()
      .is_err(),
    "oversized response Content-Disposition value should be rejected"
  );

  let too_many = fixtures::content_disposition::too_many_parameters_value();
  assert!(
    HttpContentDisposition::parse(&too_many).is_err(),
    "too many Content-Disposition parameters should be rejected"
  );
  assert!(
    content_disposition_response(&[&too_many])
      .content_disposition()
      .is_err(),
    "too many response Content-Disposition parameters should be rejected"
  );
}

#[test]
fn server_response_raw_content_disposition_remains_inspectable_after_helper_rejection() {
  let response =
    content_disposition_response(&[fixtures::content_disposition::duplicate_parameter_value()]);
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(
    response.content_disposition().is_err(),
    "typed helper should reject duplicate Content-Disposition parameters"
  );
  assert_eq!(
    Some(fixtures::content_disposition::duplicate_parameter_value()),
    header_value(&serialized, "Content-Disposition")
  );
}

#[test]
fn server_response_with_allow_declares_normalized_shared_allow_matrix() {
  for case in fixtures::allow::response_cases() {
    let response = HttpResponse::new(405, "Method Not Allowed")
      .with_allow(case.methods.iter().copied())
      .unwrap_or_else(|err| panic!("{} Allow declaration should parse: {err}", case.name));
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
    let expected = HttpAllowedMethods::from_methods(case.methods.iter().copied())
      .expect("already parsed by with_allow")
      .header_value();

    assert_eq!(
      Some(expected.as_str()),
      header_value(&serialized, "Allow"),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_response_with_allow_coexists_with_cache_and_retry_metadata_helpers() {
  let response = allow_with_cache_metadata_response(&["GET", "HEAD", "POST"]);
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(Some("GET, HEAD, POST"), header_value(&serialized, "Allow"));
  assert_eq!(
    Some("public, max-age=60"),
    header_value(&serialized, "Cache-Control")
  );
  assert_eq!(Some("5"), header_value(&serialized, "Age"));
  assert_eq!(
    Some(fixtures::age_expires::EXPIRES_IMF_FIXDATE),
    header_value(&serialized, "Expires")
  );
  assert_eq!(Some("accept-encoding"), header_value(&serialized, "Vary"));
  assert_eq!(Some("30"), header_value(&serialized, "Retry-After"));
  assert_eq!(
    &["GET", "HEAD", "POST"],
    response
      .allow()
      .expect("Allow should parse with cache metadata")
      .expect("Allow should be present")
      .methods()
      .as_slice()
  );
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .field_names()
    .contains(&"accept-encoding"));
  assert_eq!(
    Some(HttpRetryAfter::DeltaSeconds(30)),
    response.retry_after().expect("Retry-After should parse")
  );
}

#[test]
fn server_response_accept_patch_and_accept_post_helpers_declare_and_parse_media_types() {
  let response = HttpResponse::ok("OK")
    .with_accept_patch([
      "application/json; charset=utf-8",
      "application/merge-patch+json",
    ])
    .expect("Accept-Patch declaration should parse")
    .with_accept_post(["application/json", "text/plain; profile=summary"])
    .expect("Accept-Post declaration should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some("application/json; charset=utf-8, application/merge-patch+json"),
    header_value(&serialized, "Accept-Patch")
  );
  assert_eq!(
    Some("application/json, text/plain; profile=summary"),
    header_value(&serialized, "Accept-Post")
  );

  let accept_patch = response
    .accept_patch()
    .expect("Accept-Patch should parse")
    .expect("Accept-Patch should be present");
  assert_eq!(
    vec!["application/json", "application/merge-patch+json"],
    accept_patch
      .media_types()
      .iter()
      .map(|media_type| media_type.media_type())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("utf-8"),
    accept_patch.media_types()[0].parameter("charset")
  );

  let accept_post = response
    .accept_post()
    .expect("Accept-Post should parse")
    .expect("Accept-Post should be present");
  assert_eq!(
    vec!["application/json", "text/plain"],
    accept_post
      .media_types()
      .iter()
      .map(|media_type| media_type.media_type())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("summary"),
    accept_post.media_types()[1].parameter("profile")
  );
}

#[test]
fn server_response_accept_patch_and_accept_post_helpers_reject_invalid_raw_metadata() {
  for (header, value) in [
    ("Accept-Patch", "application/json,"),
    ("Accept-Post", "application/json; profile=\"unterminated"),
  ] {
    let response = HttpResponse::ok("OK").header(header, value);

    if header == "Accept-Patch" {
      assert!(
        response.accept_patch().is_err(),
        "{header} should reject {value:?}"
      );
      assert!(
        HttpResponse::ok("OK").with_accept_patch([value]).is_err(),
        "{header} declaration should reject {value:?}"
      );
    } else {
      assert!(
        response.accept_post().is_err(),
        "{header} should reject {value:?}"
      );
      assert!(
        HttpResponse::ok("OK").with_accept_post([value]).is_err(),
        "{header} declaration should reject {value:?}"
      );
    }

    assert_eq!(
      Some(value),
      header_value(&String::from_utf8(response.to_bytes()).unwrap(), header)
    );
  }
}

#[test]
fn server_allow_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::allow::invalid_cases() {
    assert!(
      HttpAllowedMethods::parse(case.value).is_err(),
      "{} Allow helper should reject invalid value",
      case.name
    );
    assert!(
      HttpResponse::new(405, "Method Not Allowed")
        .with_allow([case.value])
        .is_err(),
      "{} response helper should reject invalid Allow value",
      case.name
    );

    let response = allow_response(&[case.value]);
    assert!(
      response.allow().is_err(),
      "{} response parser should reject invalid Allow value",
      case.name
    );
  }
}

#[test]
fn server_allow_helper_rejects_duplicate_methods_and_enforces_shared_bounds() {
  let duplicate = allow_response(&["GET, HEAD", "POST, GET"]);
  assert!(
    duplicate.allow().is_err(),
    "duplicate Allow methods across header fields should be rejected"
  );
  assert!(
    HttpResponse::new(405, "Method Not Allowed")
      .with_allow(["GET", "HEAD", "GET"])
      .is_err(),
    "duplicate Allow methods from declaration helper should be rejected"
  );

  let too_many_methods = fixtures::allow::too_many_methods_value();
  assert!(
    HttpAllowedMethods::parse(&too_many_methods).is_err(),
    "too many Allow methods should be rejected"
  );
  assert!(
    allow_response(&[&too_many_methods]).allow().is_err(),
    "too many response Allow methods should be rejected"
  );

  let oversized_value = fixtures::allow::oversized_value();
  assert!(
    HttpAllowedMethods::parse(&oversized_value).is_err(),
    "oversized Allow value should be rejected"
  );
  assert!(
    allow_response(&[&oversized_value]).allow().is_err(),
    "oversized response Allow value should be rejected"
  );
}

#[test]
fn server_response_with_accept_ranges_declares_shared_matrix() {
  for case in fixtures::accept_ranges::response_cases() {
    let response = if case.none {
      HttpResponse::ok("OK").with_accept_ranges_none()
    } else {
      HttpResponse::ok("OK")
        .with_accept_ranges(case.units.iter().copied())
        .unwrap_or_else(|err| {
          panic!(
            "{} Accept-Ranges declaration should parse: {err}",
            case.name
          )
        })
    };
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.header_value),
      header_value(&serialized, "Accept-Ranges"),
      "{}",
      case.name
    );
    assert_response_accept_ranges(
      case.name,
      &response
        .accept_ranges()
        .expect("Accept-Ranges should parse")
        .expect("Accept-Ranges should be present"),
      case,
    );
  }
}

#[test]
fn server_accept_ranges_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::accept_ranges::invalid_cases() {
    assert!(
      HttpAcceptRanges::parse(case.value).is_err(),
      "{} Accept-Ranges helper should reject invalid value",
      case.name
    );
    assert!(
      accept_ranges_response(&[case.value])
        .accept_ranges()
        .is_err(),
      "{} response parser should reject invalid Accept-Ranges value",
      case.name
    );
  }
}

#[test]
fn server_accept_ranges_helper_rejects_duplicates_and_enforces_shared_bounds() {
  let duplicate = accept_ranges_response(&["bytes, pages", "BYTES"]);
  assert!(
    duplicate.accept_ranges().is_err(),
    "duplicate Accept-Ranges units across header fields should be rejected"
  );
  assert!(
    HttpResponse::ok("OK")
      .with_accept_ranges(["bytes", "BYTES"])
      .is_err(),
    "duplicate Accept-Ranges units from declaration helper should be rejected"
  );
  assert!(
    HttpResponse::ok("OK").with_accept_ranges(["none"]).is_err(),
    "Accept-Ranges none sentinel should use the none helper"
  );

  let too_many_units = fixtures::accept_ranges::too_many_server_units_value();
  assert!(
    HttpAcceptRanges::parse(&too_many_units).is_err(),
    "too many Accept-Ranges units should be rejected"
  );

  let oversized_value = fixtures::accept_ranges::oversized_value();
  assert!(
    HttpAcceptRanges::parse(&oversized_value).is_err(),
    "oversized Accept-Ranges value should be rejected"
  );
}

#[test]
fn server_accept_ranges_raw_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("OK").header("Accept-Ranges", "bytes,,custom");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAccept-Ranges: bytes,,custom\r\n"));
  assert!(
    response.accept_ranges().is_err(),
    "typed Accept-Ranges parser should reject malformed raw values"
  );
}

#[test]
fn server_accept_ranges_helpers_coexist_with_adjacent_metadata_helpers() {
  let response = HttpResponse::new(405, "Method Not Allowed")
    .with_accept_ranges(["bytes", "pages"])
    .expect("Accept-Ranges declaration should parse")
    .with_content_language(["fr-CA", "es-419"])
    .expect("Content-Language declaration should parse")
    .with_allow(["GET", "HEAD"])
    .expect("Allow declaration should parse")
    .with_retry_after_delta(30)
    .with_age(5)
    .with_expires(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS))
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("Vary declaration should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some("bytes, pages"),
    header_value(&serialized, "Accept-Ranges")
  );
  assert_eq!(
    Some("fr-CA, es-419"),
    header_value(&serialized, "Content-Language")
  );
  assert_eq!(Some("GET, HEAD"), header_value(&serialized, "Allow"));
  assert_eq!(
    Some("public, max-age=60"),
    header_value(&serialized, "Cache-Control")
  );
  assert_eq!(Some("5"), header_value(&serialized, "Age"));
  assert_eq!(
    Some(fixtures::age_expires::EXPIRES_IMF_FIXDATE),
    header_value(&serialized, "Expires")
  );
  assert_eq!(Some("accept-encoding"), header_value(&serialized, "Vary"));
  assert_eq!(Some("30"), header_value(&serialized, "Retry-After"));
  assert_eq!(
    &["bytes", "pages"],
    response
      .accept_ranges()
      .expect("Accept-Ranges should parse")
      .expect("Accept-Ranges should be present")
      .units()
      .as_slice()
  );
  assert_eq!(
    &["fr-CA", "es-419"],
    response
      .content_language()
      .expect("Content-Language should parse")
      .expect("Content-Language should be present")
      .languages()
      .as_slice()
  );
  assert!(response
    .allow()
    .expect("Allow should parse")
    .expect("Allow should be present")
    .methods()
    .contains(&"GET"));
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .field_names()
    .contains(&"accept-encoding"));
  assert_eq!(
    Some(HttpRetryAfter::DeltaSeconds(30)),
    response.retry_after().expect("Retry-After should parse")
  );
}

#[test]
fn server_response_with_content_language_declares_single_bounded_header() {
  for case in fixtures::content_language::response_cases() {
    let response = HttpResponse::ok("OK")
      .with_content_language(case.languages.iter().copied())
      .unwrap_or_else(|err| {
        panic!(
          "{} Content-Language declaration should parse: {err}",
          case.name
        )
      });
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.languages.join(", ").as_str()),
      header_value(&serialized, "Content-Language"),
      "{}",
      case.name
    );
    assert_response_content_language(
      case.name,
      &response
        .content_language()
        .expect("Content-Language should parse")
        .expect("Content-Language should be present"),
      case,
    );
  }
}

#[test]
fn server_content_language_helper_parses_multiple_fields_and_enforces_bounds() {
  for case in fixtures::content_language::invalid_cases() {
    assert!(
      HttpContentLanguages::parse(case.value).is_err(),
      "{} Content-Language helper should reject invalid value",
      case.name
    );
    assert!(
      HttpResponse::ok("OK")
        .with_content_language([case.value])
        .is_err(),
      "{} response helper should reject invalid Content-Language value",
      case.name
    );
    assert!(
      content_language_response(&[case.value])
        .content_language()
        .is_err(),
      "{} response parser should reject invalid Content-Language value",
      case.name
    );
  }

  let too_many_languages = fixtures::content_language::too_many_server_languages_value();
  assert!(
    HttpContentLanguages::parse(&too_many_languages).is_err(),
    "too many Content-Language tags should be rejected"
  );

  let oversized_value = fixtures::content_language::oversized_value();
  assert!(
    HttpContentLanguages::parse(&oversized_value).is_err(),
    "oversized Content-Language value should be rejected"
  );
}

#[test]
fn server_content_language_helper_rejects_duplicate_tags_across_fields() {
  let response = content_language_response(&["en-US, fr", "EN-us"]);

  assert!(
    response.content_language().is_err(),
    "duplicate Content-Language tags across header fields should be rejected"
  );
}

#[test]
fn server_content_language_helpers_stay_metadata_only() {
  let response = HttpResponse::new(302, "Found")
    .with_content_language(["fr-CA"])
    .expect("Content-Language declaration should parse")
    .header("Location", "/fallback")
    .header("Cache-Control", "no-store");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(Some("fr-CA"), header_value(&serialized, "Content-Language"));
  assert_eq!(Some("/fallback"), header_value(&serialized, "Location"));
  assert_eq!(Some("no-store"), header_value(&serialized, "Cache-Control"));
  assert!(
    serialized.starts_with("HTTP/1.1 302 Found\r\n"),
    "Content-Language should not alter response status policy"
  );
}

#[test]
fn server_response_with_content_location_declares_single_bounded_header() {
  for case in fixtures::content_location::response_cases() {
    let response = HttpResponse::new(201, "Created")
      .header("Content-Location", "/old")
      .with_content_location(case.declaration_value)
      .expect("Content-Location declaration should parse");
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert_eq!(
      Some(case.normalized_value),
      header_value(&serialized, "Content-Location"),
      "{}",
      case.name
    );
    assert_eq!(1, serialized.matches("\r\nContent-Location: ").count());
    assert_response_content_location(case.name, &response, case);
  }
}

#[test]
fn server_content_location_helper_rejects_duplicate_unsafe_and_oversized_values() {
  for case in fixtures::content_location::invalid_cases() {
    assert!(
      HttpResponse::ok("OK")
        .with_content_location(case.value)
        .is_err(),
      "{} Content-Location declaration should reject invalid value",
      case.name
    );
    assert!(
      HttpResponse::ok("OK")
        .header("Content-Location", case.value)
        .content_location()
        .is_err(),
      "{} Content-Location parser should reject invalid value",
      case.name
    );
  }

  let duplicate = HttpResponse::ok("OK")
    .header("Content-Location", "/one")
    .header("Content-Location", "/two");
  assert!(
    duplicate.content_location().is_err(),
    "duplicate Content-Location header fields should be rejected"
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024 + 1));
  assert!(
    HttpResponse::ok("OK")
      .with_content_location(&oversized)
      .is_err(),
    "oversized Content-Location declaration should be rejected"
  );
  assert!(
    HttpResponse::ok("OK")
      .header("Content-Location", oversized)
      .content_location()
      .is_err(),
    "oversized Content-Location raw value should be rejected"
  );
}

#[test]
fn server_content_location_parser_preserves_invalid_raw_header_values() {
  for case in fixtures::content_location::invalid_cases() {
    let response = content_location_response(&[case.value]);
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert!(
      response.content_location().is_err(),
      "{} Content-Location parser should reject invalid value",
      case.name
    );
    assert_eq!(
      Some(case.value.trim()),
      header_value(&serialized, "Content-Location"),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_content_location_helpers_stay_metadata_only() {
  let response = HttpResponse::new(302, "Found")
    .with_content_location("/representation")
    .expect("Content-Location declaration should parse")
    .header("Location", "/fallback")
    .header("Cache-Control", "no-store");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some("/representation"),
    header_value(&serialized, "Content-Location")
  );
  assert_eq!(Some("/fallback"), header_value(&serialized, "Location"));
  assert_eq!(Some("no-store"), header_value(&serialized, "Cache-Control"));
  assert!(
    serialized.starts_with("HTTP/1.1 302 Found\r\n"),
    "Content-Location should not alter response status policy"
  );
}

#[test]
fn server_content_location_helpers_coexist_with_adjacent_metadata_helpers() {
  let response = HttpResponse::new(405, "Method Not Allowed")
    .with_content_location(" /representations/current ")
    .expect("Content-Location declaration should parse")
    .with_allow(["GET", "HEAD"])
    .expect("Allow declaration should parse")
    .with_retry_after_delta(30)
    .with_age(5)
    .with_expires(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS))
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("Vary declaration should parse")
    .with_content_language(["fr-CA", "es-419"])
    .expect("Content-Language declaration should parse")
    .with_accept_ranges(["bytes", "pages"])
    .expect("Accept-Ranges declaration should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some("/representations/current"),
    header_value(&serialized, "Content-Location")
  );
  assert_eq!(Some("GET, HEAD"), header_value(&serialized, "Allow"));
  assert_eq!(
    Some("public, max-age=60"),
    header_value(&serialized, "Cache-Control")
  );
  assert_eq!(Some("5"), header_value(&serialized, "Age"));
  assert_eq!(
    Some(fixtures::age_expires::EXPIRES_IMF_FIXDATE),
    header_value(&serialized, "Expires")
  );
  assert_eq!(Some("accept-encoding"), header_value(&serialized, "Vary"));
  assert_eq!(Some("30"), header_value(&serialized, "Retry-After"));
  assert_eq!(
    Some("fr-CA, es-419"),
    header_value(&serialized, "Content-Language")
  );
  assert_eq!(
    Some("bytes, pages"),
    header_value(&serialized, "Accept-Ranges")
  );
  assert_eq!(
    "/representations/current",
    response
      .content_location()
      .expect("Content-Location should parse")
      .expect("Content-Location should be present")
      .as_str()
  );
}

#[test]
fn server_content_language_helpers_coexist_with_adjacent_metadata_helpers() {
  let response = HttpResponse::new(405, "Method Not Allowed")
    .with_content_language(["fr-CA", "es-419"])
    .expect("Content-Language declaration should parse")
    .with_allow(["GET", "HEAD"])
    .expect("Allow declaration should parse")
    .with_retry_after_delta(30)
    .with_age(5)
    .with_expires(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS))
    .header("Cache-Control", "public, max-age=60")
    .with_vary("Accept-Encoding")
    .expect("Vary declaration should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(
    Some("fr-CA, es-419"),
    header_value(&serialized, "Content-Language")
  );
  assert_eq!(Some("GET, HEAD"), header_value(&serialized, "Allow"));
  assert_eq!(
    Some("public, max-age=60"),
    header_value(&serialized, "Cache-Control")
  );
  assert_eq!(Some("5"), header_value(&serialized, "Age"));
  assert_eq!(
    Some(fixtures::age_expires::EXPIRES_IMF_FIXDATE),
    header_value(&serialized, "Expires")
  );
  assert_eq!(Some("accept-encoding"), header_value(&serialized, "Vary"));
  assert_eq!(Some("30"), header_value(&serialized, "Retry-After"));
  assert_eq!(
    &["fr-CA", "es-419"],
    response
      .content_language()
      .expect("Content-Language should parse")
      .expect("Content-Language should be present")
      .languages()
      .as_slice()
  );
  assert!(response
    .allow()
    .expect("Allow should parse")
    .expect("Allow should be present")
    .methods()
    .contains(&"GET"));
  assert_eq!(
    Some(60),
    response
      .cache_control()
      .expect("Cache-Control should parse")
      .expect("Cache-Control should be present")
      .max_age()
  );
  assert_eq!(Some(5), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(fixtures::age_expires::EXPIRES_UNIX_SECONDS)),
    response.expires().expect("Expires should parse")
  );
  assert!(response
    .vary()
    .expect("Vary should parse")
    .expect("Vary should be present")
    .field_names()
    .contains(&"accept-encoding"));
  assert_eq!(
    Some(HttpRetryAfter::DeltaSeconds(30)),
    response.retry_after().expect("Retry-After should parse")
  );
}

#[test]
fn server_response_with_vary_declares_normalized_shared_vary_matrix() {
  for case in fixtures::vary::response_cases() {
    let Some(value) = case.values.first() else {
      continue;
    };
    let response = HttpResponse::ok("accepted")
      .with_vary(value)
      .unwrap_or_else(|err| panic!("{} Vary declaration should parse: {err}", case.name));
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
    let expected = HttpVary::parse(value)
      .expect("already parsed by with_vary")
      .header_value();

    assert_eq!(
      Some(expected.as_str()),
      header_value(&serialized, "Vary"),
      "{}",
      case.name
    );
  }
}

#[test]
fn server_request_selection_accepts_shared_vary_matrix() {
  for case in fixtures::vary::selection_cases() {
    let request = HttpRequest::parse(case.request).expect("request should parse");
    let vary = HttpVary::parse(case.value)
      .unwrap_or_else(|err| panic!("{} Vary should parse: {err}", case.name));

    let selection = request.vary_selection(&vary);

    assert_eq!(case.wildcard, selection.is_wildcard(), "{}", case.name);
    assert_eq!(
      case.field_names,
      selection.field_names().as_slice(),
      "{}",
      case.name
    );
    for (field_name, expected_values) in case.selected_values {
      assert_eq!(
        *expected_values,
        selection.values(field_name).as_slice(),
        "{} {field_name}",
        case.name
      );
    }
  }
}

#[test]
fn server_vary_helpers_reject_shared_invalid_matrix() {
  for case in fixtures::vary::invalid_cases() {
    assert!(
      HttpVary::parse(case.value).is_err(),
      "{} Vary helper should reject invalid value",
      case.name
    );
    assert!(
      HttpResponse::ok("body").with_vary(case.value).is_err(),
      "{} response helper should reject invalid Vary value",
      case.name
    );
  }
}

#[test]
fn server_vary_helper_enforces_shared_bounds() {
  let too_many_fields = fixtures::vary::too_many_field_names_value();
  assert!(
    HttpVary::parse(&too_many_fields).is_err(),
    "too many Vary field names should be rejected"
  );

  let oversized_value = fixtures::vary::oversized_value();
  assert!(
    HttpVary::parse(&oversized_value).is_err(),
    "oversized Vary value should be rejected"
  );
}

#[test]
fn live_socket2_server_accepts_shared_cache_control_request_matrix() {
  for case in fixtures::cache_control::request_cases() {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          let cache_control = request
            .cache_control()
            .expect("cache-control should parse")
            .expect("cache-control header should be present");
          tx.send(cache_control).expect("send cache-control");
          HttpResponse::ok("accepted")
        })
        .expect("serve one request");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream
      .write_all(&cache_control_request(case.values))
      .expect(case.name);
    stream.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{}", case.name);
    assert_request_cache_control(case.name, rx.recv().expect("cache-control"), case);

    handle.join().expect("server thread");
  }
}

#[test]
fn server_response_helper_accepts_shared_cache_control_response_matrix() {
  for case in fixtures::cache_control::response_cases() {
    let response = cache_control_response(case.values);
    let cache_control = response
      .cache_control()
      .unwrap_or_else(|err| panic!("{} cache-control should parse: {err}", case.name))
      .unwrap_or_else(|| panic!("{} should include Cache-Control", case.name));

    assert_response_cache_control(case.name, cache_control, case);
  }
}

#[test]
fn server_response_cache_control_helper_rejects_shared_invalid_response_matrix() {
  for case in fixtures::cache_control::invalid_response_cases() {
    let response = cache_control_response(&[case.value]);

    assert!(
      response.cache_control().is_err(),
      "{} helper should reject invalid Cache-Control",
      case.name
    );
  }
}

#[test]
fn server_response_cache_control_helper_enforces_shared_bounds() {
  for (name, value) in [
    (
      "too many response Cache-Control directives",
      fixtures::cache_control::too_many_directives_value(),
    ),
    (
      "oversized response Cache-Control value",
      fixtures::cache_control::oversized_value(),
    ),
  ] {
    let response = cache_control_response(&[&value]);

    assert!(
      response.cache_control().is_err(),
      "{name} helper should reject invalid Cache-Control"
    );
  }
}

#[test]
fn server_cache_control_matrix_keeps_cache_engine_non_goals_explicit() {
  let request = HttpRequest::parse(
    b"GET /matrix/cache-control-non-goals HTTP/1.1\r\nHost: example.test\r\nETag: \"client\"\r\n\r\n",
  )
  .expect("request without Cache-Control should parse");
  let response = HttpResponse::ok("OK").header("ETag", "\"representation\"");

  assert!(request
    .cache_control()
    .expect("missing header is valid")
    .is_none());
  assert!(response
    .cache_control()
    .expect("missing header is valid")
    .is_none());
  assert_eq!(Some("\"client\""), request.header("ETag"));
}

#[test]
fn model_parser_rejects_shared_host_and_target_validation_fixtures() {
  for fixture in fixtures::request::invalid_host_and_target_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn model_parser_rejects_shared_framing_ambiguity_fixtures() {
  for fixture in fixtures::request::framing_ambiguity_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn model_parser_rejects_shared_obsolete_line_folding_fixtures() {
  for fixture in fixtures::request::obsolete_line_folding_cases() {
    let error = HttpRequest::parse(fixture.raw).expect_err(fixture.name);

    assert_eq!(fixture.error, error.to_string(), "{}", fixture.name);
  }
}

#[test]
fn server_serializes_shared_early_hints_link_metadata_fixture() {
  let early_hints = HttpResponse::early_hints_with_headers(
    fixtures::response::EARLY_HINTS_LINKS.iter().copied(),
    fixtures::response::EARLY_HINTS_METADATA.iter().copied(),
  )
  .expect("shared Early Hints fixture should serialize");

  assert_eq!(
    fixtures::response::VALID_EARLY_HINTS_HEAD,
    early_hints.to_bytes().as_slice()
  );
}

#[test]
fn server_serialization_preserves_shared_final_response_after_early_hints() {
  let early_hints = HttpResponse::early_hints_with_headers(
    fixtures::response::EARLY_HINTS_LINKS.iter().copied(),
    fixtures::response::EARLY_HINTS_METADATA.iter().copied(),
  )
  .expect("shared Early Hints fixture should serialize");
  let final_response = HttpResponse::new(200, "OK")
    .header("X-Final", "early-hints")
    .body("OK");
  let mut serialized = early_hints.to_bytes();
  serialized.extend(final_response.to_bytes());

  assert_eq!(
    fixtures::response::VALID_EARLY_HINTS_WITH_FINAL,
    serialized.as_slice()
  );
}

#[test]
fn server_serializes_shared_101_handoff_without_body_framing() {
  let response = HttpResponse::new(101, "Switching Protocols")
    .header("Connection", "Upgrade")
    .header("Upgrade", "websocket")
    .header("Sec-WebSocket-Accept", "shared-accept");

  assert_eq!(
    fixtures::response::SWITCHING_PROTOCOLS_HEAD,
    response.to_bytes().as_slice()
  );
}

#[test]
fn server_early_hints_helper_rejects_shared_invalid_metadata() {
  for case in fixtures::response::invalid_early_hints_metadata_cases() {
    let error = HttpResponse::early_hints_with_headers(
      fixtures::response::EARLY_HINTS_LINKS.iter().copied(),
      [(case.header_name, case.value)],
    )
    .expect_err(case.name);

    assert_eq!(case.error, error.to_string(), "{}", case.name);
  }
}

#[test]
fn live_socket2_server_accepts_shared_chunk_extensions_and_trailers_fixture() {
  let fixture = fixtures::request::chunked_with_extensions_and_trailers();
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        let trailers = fixture
          .trailers
          .iter()
          .map(|(name, _)| (name.to_string(), request.trailer(name).map(str::to_string)))
          .collect::<Vec<_>>();
        tx.send((
          request.method().to_string(),
          request.target().to_string(),
          request.body().to_vec(),
          trailers,
        ))
        .expect("send parsed request");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(fixture.raw)
    .expect("write chunked request");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let (method, target, body, trailers) = rx.recv().expect("parsed request");
  assert_eq!(fixture.method, method);
  assert_eq!(fixture.target, target);
  assert_eq!(fixture.body, body.as_slice());
  for ((name, value), (observed_name, observed_value)) in fixture.trailers.iter().zip(trailers) {
    assert_eq!(*name, observed_name);
    assert_eq!(Some((*value).to_string()), observed_value);
  }
  assert!(response.starts_with("HTTP/1.1 200 OK"));

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_accepts_shared_origin_and_absolute_form_fixtures() {
  for fixture in fixtures::request::valid_origin_and_absolute_form_cases() {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send((request.method().to_string(), request.target().to_string()))
            .expect("send parsed request");
          HttpResponse::ok(format!("served {}", request.target()))
        })
        .expect("serve one request");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream.write_all(fixture.raw).expect(fixture.name);
    stream.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert_eq!(
      (fixture.method.to_string(), fixture.target.to_string()),
      rx.recv().expect("parsed request"),
      "{}",
      fixture.name
    );
    assert!(
      response.starts_with("HTTP/1.1 200 OK"),
      "{} returned {response:?}",
      fixture.name
    );

    handle.join().expect("server thread");
  }
}

#[test]
fn live_socket2_server_rejects_shared_invalid_host_and_target_fixtures_before_handler() {
  for fixture in fixtures::request::invalid_host_and_target_cases() {
    let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
    let addr = server.local_addr().expect("server addr");
    let (tx, rx) = mpsc::channel();

    let handle = thread::spawn(move || {
      server
        .accept_one(|request| {
          tx.send(request.target().to_string())
            .expect("send unexpected request");
          HttpResponse::ok("unexpected")
        })
        .expect("serve invalid request");
    });

    let mut stream = TcpStream::connect(addr).expect("connect server");
    stream.write_all(fixture.raw).expect(fixture.name);
    stream.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");

    assert!(
      rx.try_recv().is_err(),
      "{} should not dispatch to the handler",
      fixture.name
    );
    assert!(
      response.starts_with("HTTP/1.1 400 Bad Request"),
      "{} returned {response:?}",
      fixture.name
    );

    handle.join().expect("server thread");
  }
}

#[test]
fn live_socket2_server_preserves_connection_lifetime_boundaries() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send((request.target().to_string(), request.body().to_vec()))
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(fixtures::request::keep_alive_pipeline())
    .expect("write pipelined requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read responses");

  let (first_response, remaining) = parse_content_length_response(&response);
  let (second_response, remaining) = parse_content_length_response(remaining);

  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(None, header_value(first_response.head, "Connection"));
  assert_eq!("served /matrix/first", first_response.body);
  assert!(second_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(second_response.head, "Connection")
  );
  assert_eq!("served /matrix/second", second_response.body);
  assert_eq!("", remaining);
  assert_eq!(
    ("/matrix/first".to_string(), b"alpha".to_vec()),
    rx.recv().expect("first request")
  );
  assert_eq!(
    ("/matrix/second".to_string(), b"bravo!".to_vec()),
    rx.recv().expect("second request")
  );

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_stops_pipelined_connection_after_request_close() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/close-first HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Connection: close\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/ignored HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined close request");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let (first_response, remaining) = parse_content_length_response(&response);
  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/close-first", first_response.body);
  assert_eq!("", remaining);
  assert_eq!("/matrix/close-first", rx.recv().expect("first request"));
  assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

  let mut next_stream = TcpStream::connect(addr).expect("connect next request");
  next_stream
    .write_all(
      concat!(
        "GET /matrix/next-connection HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write next request");
  next_stream
    .shutdown(Shutdown::Write)
    .expect("shutdown next write");

  let mut next_response = String::new();
  next_stream
    .read_to_string(&mut next_response)
    .expect("read next response");

  let (next_response, next_remaining) = parse_content_length_response(&next_response);
  assert!(next_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!("served /matrix/next-connection", next_response.body);
  assert_eq!("", next_remaining);
  assert_eq!(
    "/matrix/next-connection",
    rx.recv().expect("next connection request")
  );
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_closes_http10_without_keep_alive_before_next_request() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/http10-terminal HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/ignored HTTP/1.0\r\n",
        "Connection: keep-alive\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined HTTP/1.0 requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  let mut next_stream = TcpStream::connect(addr).expect("connect next request");
  next_stream
    .write_all(
      concat!(
        "GET /matrix/http10-next-connection HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write next HTTP/1.0 request");
  next_stream
    .shutdown(Shutdown::Write)
    .expect("shutdown next write");

  let mut next_response = String::new();
  next_stream
    .read_to_string(&mut next_response)
    .expect("read next response");

  let (first_response, remaining) = parse_content_length_response(&response);
  let (next_response, next_remaining) = parse_content_length_response(&next_response);
  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-terminal", first_response.body);
  assert_eq!("", remaining);
  assert!(next_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!("served /matrix/http10-next-connection", next_response.body);
  assert_eq!("", next_remaining);
  assert_eq!("/matrix/http10-terminal", rx.recv().expect("first request"));
  assert_eq!(
    "/matrix/http10-next-connection",
    rx.recv().expect("next connection request")
  );
  assert!(rx.try_recv().is_err());

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_keeps_http10_alive_when_explicitly_requested() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .serve_requests(2, |request| {
        tx.send(request.target().to_string())
          .expect("send parsed request");
        HttpResponse::ok(format!("served {}", request.target()))
      })
      .expect("serve requests");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .write_all(
      concat!(
        "GET /matrix/http10-first HTTP/1.0\r\n",
        "Connection: keep-alive\r\n",
        "Content-Length: 0\r\n",
        "\r\n",
        "GET /matrix/http10-final HTTP/1.0\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write pipelined HTTP/1.0 requests");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read responses");

  let (first_response, remaining) = parse_content_length_response(&response);
  let (second_response, remaining) = parse_content_length_response(remaining);

  assert!(first_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("keep-alive"),
    header_value(first_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-first", first_response.body);
  assert!(second_response.head.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    Some("close"),
    header_value(second_response.head, "Connection")
  );
  assert_eq!("served /matrix/http10-final", second_response.body);
  assert_eq!("", remaining);
  assert_eq!("/matrix/http10-first", rx.recv().expect("first request"));
  assert_eq!("/matrix/http10-final", rx.recv().expect("second request"));

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_reads_shared_expect_body_without_an_interim_response() {
  let fixture = fixtures::request::expect_continue_fixed_length();
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send((request.target().to_string(), request.body().to_vec()))
          .expect("send parsed request");
        HttpResponse::ok("accepted")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(fixture.head)
    .expect("write expect-continue head");

  stream
    .write_all(fixture.body)
    .expect("write expect-continue body");
  stream.shutdown(Shutdown::Write).expect("shutdown write");

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read final response");

  assert!(response.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    (fixture.target.to_string(), fixture.body.to_vec()),
    rx.recv().expect("parsed request")
  );

  handle.join().expect("server thread");
}

#[test]
fn live_socket2_server_exposes_unsupported_expectation_without_rejecting_body() {
  let server = rttp::Http::server("127.0.0.1:0").expect("bind server");
  let addr = server.local_addr().expect("server addr");
  let (tx, rx) = mpsc::channel();

  let handle = thread::spawn(move || {
    server
      .accept_one(|request| {
        tx.send(request.target().to_string()).expect("send request");
        HttpResponse::ok("unexpected")
      })
      .expect("serve one request");
  });

  let mut stream = TcpStream::connect(addr).expect("connect server");
  stream
    .set_read_timeout(Some(Duration::from_millis(250)))
    .expect("set read timeout");
  stream
    .write_all(
      concat!(
        "POST /matrix/unsupported-expect HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Expect: tea-time\r\n",
        "Content-Length: 12\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("write unsupported expectation head");

  stream
    .write_all(b"request body")
    .expect("write request body");
  stream.shutdown(Shutdown::Write).expect("shutdown write");
  let mut response = String::new();
  stream.read_to_string(&mut response).expect("read response");

  assert!(response.starts_with("HTTP/1.1 200 OK"));
  assert_eq!(
    "/matrix/unsupported-expect",
    rx.recv().expect("parsed request")
  );

  handle.join().expect("server thread");
}
