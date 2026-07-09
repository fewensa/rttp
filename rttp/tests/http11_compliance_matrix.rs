use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rttp::server::{
  HttpRequest, HttpRequestCacheControl, HttpResponse, HttpResponseCacheControl, HttpVary,
};
use rttp_http11_test_fixtures as fixtures;

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
fn live_socket2_server_sends_continue_before_reading_shared_body_fixture() {
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

  let mut interim = vec![0; fixtures::response::CONTINUE.len()];
  stream
    .read_exact(&mut interim)
    .expect("read interim response");
  assert_eq!(fixtures::response::CONTINUE, interim.as_slice());

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
fn live_socket2_server_rejects_unsupported_expectation_without_reading_body() {
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

  let mut response = String::new();
  stream
    .read_to_string(&mut response)
    .expect("read expectation failure");

  assert!(response.starts_with("HTTP/1.1 417 Expectation Failed"));
  assert!(
    rx.try_recv().is_err(),
    "unsupported expectation reached the request handler"
  );

  handle.join().expect("server thread");
}
