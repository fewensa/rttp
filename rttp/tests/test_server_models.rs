use rttp::server::{
  HttpByteRange, HttpByteRangeError, HttpConditionalMetadata, HttpEntityTag,
  HttpIfRangeRequestOutcome, HttpRequest, HttpRequestCacheControl, HttpResponse,
  HttpResponseCacheControl,
};

fn parse_request(raw: &str) -> HttpRequest {
  HttpRequest::parse(raw.as_bytes()).expect("request should parse")
}

#[test]
fn parses_request_cache_control_directives() {
  let request = parse_request(concat!(
    "GET /cached HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Cache-Control: no-cache, no-store, max-age=60, max-stale=120\r\n",
    "Cache-Control: min-fresh=30, no-transform, only-if-cached, ext=\"a,b\"\r\n",
    "\r\n"
  ));

  let cache_control = request
    .cache_control()
    .expect("valid cache-control should parse")
    .expect("cache-control header should be present");

  assert!(cache_control.no_cache());
  assert!(cache_control.no_store());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(Some(Some(120)), cache_control.max_stale());
  assert_eq!(Some(30), cache_control.min_fresh());
  assert!(cache_control.no_transform());
  assert!(cache_control.only_if_cached());
  assert_eq!(1, cache_control.extensions().len());
  assert_eq!("ext", cache_control.extensions()[0].name());
  assert_eq!(Some("a,b"), cache_control.extensions()[0].value());
}

#[test]
fn parses_request_cache_control_max_stale_without_value() {
  let cache_control =
    HttpRequestCacheControl::parse("max-stale").expect("max-stale without delta-seconds is valid");

  assert_eq!(Some(None), cache_control.max_stale());
}

#[test]
fn parses_response_cache_control_directives() {
  let response = HttpResponse::new(200, "OK")
    .header(
      "Cache-Control",
      "no-cache=\"Set-Cookie, Authorization\", no-store, max-age=60",
    )
    .header(
      "Cache-Control",
      "s-maxage=120, private=\"X-User\", public, must-revalidate",
    )
    .header(
      "Cache-Control",
      "proxy-revalidate, immutable, stale-while-revalidate=30, stale-if-error=90",
    )
    .header("Cache-Control", "community=\"u=1, tier=gold\", ext-token");

  let cache_control = response
    .cache_control()
    .expect("valid cache-control should parse")
    .expect("cache-control header should be present");

  assert!(cache_control.no_cache());
  assert_eq!(
    vec!["Set-Cookie", "Authorization"],
    cache_control.no_cache_fields()
  );
  assert!(cache_control.no_store());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(Some(120), cache_control.s_maxage());
  assert!(cache_control.private());
  assert_eq!(vec!["X-User"], cache_control.private_fields());
  assert!(cache_control.public());
  assert!(cache_control.must_revalidate());
  assert!(cache_control.proxy_revalidate());
  assert!(cache_control.immutable());
  assert_eq!(Some(30), cache_control.stale_while_revalidate());
  assert_eq!(Some(90), cache_control.stale_if_error());
  assert_eq!(2, cache_control.extensions().len());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(
    Some("u=1, tier=gold"),
    cache_control.extensions()[0].value()
  );
  assert_eq!("ext-token", cache_control.extensions()[1].name());
  assert_eq!(None, cache_control.extensions()[1].value());
}

#[test]
fn parses_response_cache_control_from_raw_values() {
  let cache_control = HttpResponseCacheControl::parse("public, max-age=15")
    .expect("standalone cache-control value should parse");

  assert!(cache_control.public());
  assert_eq!(Some(15), cache_control.max_age());
}

#[test]
fn cache_control_helpers_reject_invalid_numbers_and_quoted_strings() {
  for value in [
    "max-age=-1",
    "s-maxage=abc",
    "stale-while-revalidate=1.5",
    "stale-if-error=\"60\"",
    "private=\"unterminated",
    "extension=\"bad\\\"",
  ] {
    assert!(
      HttpResponseCacheControl::parse(value).is_err(),
      "response helper should reject {value:?}"
    );
  }

  for value in [
    "max-age=abc",
    "max-stale=-1",
    "min-fresh=\"60\"",
    "extension=\"bad\\\"",
  ] {
    assert!(
      HttpRequestCacheControl::parse(value).is_err(),
      "request helper should reject {value:?}"
    );
  }
}

#[test]
fn parses_http_request_target_headers_and_body() {
  let raw = concat!(
    "POST /submit?name=Rttp&debug=true HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain\r\n",
    "X-Trace-Id: abc-123\r\n",
    "Content-Length: 11\r\n",
    "\r\n",
    "hello=world"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("POST", request.method());
  assert_eq!("/submit", request.path());
  assert_eq!(Some("name=Rttp&debug=true"), request.query());
  assert_eq!("HTTP/1.1", request.version());
  assert_eq!(Some("example.test"), request.header("host"));
  assert_eq!(Some("text/plain"), request.header("Content-Type"));
  assert_eq!(Some("abc-123"), request.header("x-trace-id"));
  assert_eq!(b"hello=world", request.body());
}

#[test]
fn parses_absolute_form_request_target_as_origin_path_and_query() {
  let raw = concat!(
    "GET http://example.com/a/b?x=1 HTTP/1.1\r\n",
    "Host: proxy.local\r\n",
    "\r\n"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("GET", request.method());
  assert_eq!("/a/b", request.path());
  assert_eq!(Some("x=1"), request.query());
  assert_eq!(Some("proxy.local"), request.header("host"));
}

#[test]
fn parses_body_only_when_content_length_matches() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!(b"hello", request.body());
}

#[test]
fn parses_chunked_transfer_coded_request_body() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Transfer-Encoding: chunked\r\n",
    "\r\n",
    "5;foo=bar\r\n",
    "hello\r\n",
    "6\r\n",
    " world\r\n",
    "0\r\n",
    "X-Trace: abc\r\n",
    "\r\n"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("POST", request.method());
  assert_eq!("/submit", request.path());
  assert_eq!(b"hello world", request.body());
}

#[test]
fn parses_fixed_length_request_with_duplicate_matching_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!(b"hello", request.body());
}

#[test]
fn rejects_request_body_shorter_than_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hel"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "request body length does not match Content-Length",
    error.to_string()
  );
}

#[test]
fn rejects_request_body_longer_than_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "helloGET /next HTTP/1.1\r\n\r\n"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "request body length does not match Content-Length",
    error.to_string()
  );
}

#[test]
fn rejects_malformed_request_line_and_request_metadata() {
  for raw in [
    b"GET  HTTP/1.1\r\nHost: example.test\r\n\r\n".as_slice(),
    b"GE(T / HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET /bad path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET http://:80/path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"GET http://example.test:port/path HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"CONNECT example.test HTTP/1.1\r\nHost: example.test\r\n\r\n",
    b"CONNECT example.test:port HTTP/1.1\r\nHost: example.test\r\n\r\n",
  ] {
    let _error = HttpRequest::parse(raw).expect_err("request should be rejected");
  }
}

#[test]
fn rejects_unsupported_and_malformed_http_version_tokens() {
  for raw in [
    b"GET / HTTP/0.9\r\nHost: example.test\r\n\r\n".as_slice(),
    b"GET / HTTP/2.0\r\nHost: example.test\r\n\r\n",
    b"GET / HTP/1.1\r\nHost: example.test\r\n\r\n",
  ] {
    let error = HttpRequest::parse(raw).expect_err("request should be rejected");

    assert_eq!("invalid request version", error.to_string());
  }
}

#[test]
fn rejects_malformed_absolute_form_request_target() {
  let error = HttpRequest::parse(
    b"GET http://example.test:port/a/b?x=1 HTTP/1.1\r\nHost: proxy.local\r\n\r\n",
  )
  .expect_err("request should be rejected");

  assert_eq!("invalid request target", error.to_string());
}

#[test]
fn rejects_invalid_and_folded_request_headers() {
  for raw in [
    b"GET / HTTP/1.1\r\nBad Header: value\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: bad\rvalue\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test\r\n folded: value\r\n\r\n",
  ] {
    let _error = HttpRequest::parse(raw).expect_err("request should be rejected");
  }
}

#[test]
fn rejects_http_11_request_without_host_header() {
  let error =
    HttpRequest::parse(b"GET / HTTP/1.1\r\n\r\n").expect_err("request should be rejected");

  assert_eq!(
    "HTTP/1.1 request requires exactly one Host header",
    error.to_string()
  );
}

#[test]
fn rejects_http_11_request_with_multiple_host_headers() {
  let error = HttpRequest::parse(
    concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "hOSt: other.test\r\n",
      "\r\n"
    )
    .as_bytes(),
  )
  .expect_err("request should be rejected");

  assert_eq!(
    "HTTP/1.1 request requires exactly one Host header",
    error.to_string()
  );
}

#[test]
fn rejects_http_11_request_with_invalid_host_header_value() {
  for raw in [
    b"GET / HTTP/1.1\r\nHost: \r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: http://example.test\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test/path\r\n\r\n",
    b"GET / HTTP/1.1\r\nHost: example.test:port\r\n\r\n",
  ] {
    let error = HttpRequest::parse(raw).expect_err("request should be rejected");

    assert_eq!("invalid Host header", error.to_string());
  }
}

#[test]
fn rejects_connect_request_when_host_does_not_match_authority_target() {
  for raw in [
    b"CONNECT example.test:443 HTTP/1.1\r\nHost: other.test\r\n\r\n".as_slice(),
    b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test\r\n\r\n",
  ] {
    let error = HttpRequest::parse(raw).expect_err("request should be rejected");

    assert_eq!("invalid Host header", error.to_string());
  }
}

#[test]
fn accepts_http_10_request_without_host_header() {
  let request = HttpRequest::parse(b"GET /legacy HTTP/1.0\r\n\r\n").expect("request should parse");

  assert_eq!("HTTP/1.0", request.version());
  assert_eq!(None, request.header("host"));
}

#[test]
fn rejects_conflicting_duplicate_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Length: 5\r\n",
    "Content-Length: 6\r\n",
    "\r\n",
    "hello"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!("conflicting Content-Length headers", error.to_string());
}

#[test]
fn rejects_transfer_encoding_request_even_with_content_length() {
  let raw = concat!(
    "POST /submit HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "hello"
  );

  let error = HttpRequest::parse(raw.as_bytes()).expect_err("request should be rejected");

  assert_eq!(
    "Transfer-Encoding conflicts with Content-Length",
    error.to_string()
  );
}

#[test]
fn parses_http_request_without_query_or_body() {
  let raw = concat!(
    "GET /health HTTP/1.0\r\n",
    "Host: example.test\r\n",
    "Connection: close\r\n",
    "\r\n"
  );

  let request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");

  assert_eq!("GET", request.method());
  assert_eq!("/health", request.path());
  assert_eq!(None, request.query());
  assert_eq!("HTTP/1.0", request.version());
  assert_eq!(Some("close"), request.header("connection"));
  assert!(request.body().is_empty());
}

#[test]
fn serializes_http_response_status_headers_content_length_and_body() {
  let response = HttpResponse::new(201, "Created")
    .header("Content-Type", "application/json")
    .header("Connection", "close")
    .body(r#"{"ok":true}"#);

  let serialized = response.to_bytes();

  assert_eq!(
    concat!(
      "HTTP/1.1 201 Created\r\n",
      "Content-Type: application/json\r\n",
      "Connection: close\r\n",
      "Content-Length: 11\r\n",
      "\r\n",
      r#"{"ok":true}"#
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn serializes_at_most_one_connection_header() {
  let response = HttpResponse::new(200, "OK")
    .header("Connection", "keep-alive")
    .header("Connection", "close")
    .body("ok");

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
  let connection_headers = serialized
    .lines()
    .filter(|line| line.to_ascii_lowercase().starts_with("connection:"))
    .count();

  assert_eq!(1, connection_headers);
  assert!(serialized.contains("\r\nConnection: close\r\n"));
  assert!(!serialized.contains("\r\nConnection: keep-alive\r\n"));
}

#[test]
fn write_to_preserves_explicit_connection_header() {
  let response = HttpResponse::ok("ok").header("Connection", "keep-alive");
  let mut serialized = Vec::new();

  response
    .write_to(&mut serialized)
    .expect("response should serialize");

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Connection: keep-alive\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "ok"
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn parses_single_bounded_byte_ranges_against_entity_length() {
  assert_eq!(
    HttpByteRange::new(2, 5),
    HttpByteRange::parse("bytes=2-5", 10).expect("closed range should parse")
  );
  assert_eq!(
    HttpByteRange::new(7, 9),
    HttpByteRange::parse("bytes=7-", 10).expect("open range should parse")
  );
  assert_eq!(
    HttpByteRange::new(6, 9),
    HttpByteRange::parse("bytes=-4", 10).expect("suffix range should parse")
  );
}

#[test]
fn rejects_unsupported_multiple_invalid_and_unsatisfied_byte_ranges() {
  for (header, entity_length, expected) in [
    ("items=0-1", 10, HttpByteRangeError::UnsupportedUnit),
    ("bytes=0-1,4-5", 10, HttpByteRangeError::MultipleRanges),
    ("bytes=5-2", 10, HttpByteRangeError::InvalidRange),
    ("bytes=10-5", 10, HttpByteRangeError::InvalidRange),
    ("bytes=-0", 10, HttpByteRangeError::InvalidRange),
    ("bytes=10-", 10, HttpByteRangeError::UnsatisfiedRange),
    ("bytes=-5", 0, HttpByteRangeError::UnsatisfiedRange),
  ] {
    let error = HttpByteRange::parse(header, entity_length).expect_err("range should reject");

    assert_eq!(expected, error);
  }
}

#[test]
fn serializes_partial_content_response_for_parsed_byte_range() {
  let body = b"0123456789";
  let range = HttpByteRange::parse("bytes=3-6", body.len()).expect("range should parse");
  let response = HttpResponse::partial_content(body, range);

  assert_eq!(
    concat!(
      "HTTP/1.1 206 Partial Content\r\n",
      "Content-Range: bytes 3-6/10\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "3456"
    )
    .as_bytes(),
    response.to_bytes().as_slice()
  );
}

#[test]
fn serializes_range_not_satisfiable_response() {
  let response = HttpResponse::range_not_satisfiable(10);

  assert_eq!(
    concat!(
      "HTTP/1.1 416 Range Not Satisfiable\r\n",
      "Content-Range: bytes */10\r\n",
      "Content-Length: 0\r\n",
      "\r\n"
    )
    .as_bytes(),
    response.to_bytes().as_slice()
  );
}

#[test]
fn if_range_allows_partial_content_for_matching_strong_etag() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=2-5\r\n",
    "If-Range: \"abc123\"\r\n",
    "\r\n"
  ));
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("abc123"));

  assert_eq!(
    Ok(HttpIfRangeRequestOutcome::PartialContent(
      HttpByteRange::new(2, 5)
    )),
    request.evaluate_if_range(&metadata, 10)
  );
}

#[test]
fn if_range_falls_back_to_full_response_for_non_matching_or_weak_etag() {
  for if_range in [r#""other""#, r#"W/"abc123""#] {
    let request = parse_request(&format!(
      concat!(
        "GET /asset HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Range: bytes=2-5\r\n",
        "If-Range: {if_range}\r\n",
        "\r\n"
      ),
      if_range = if_range
    ));
    let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("abc123"));

    assert_eq!(
      Ok(HttpIfRangeRequestOutcome::FullResponse),
      request.evaluate_if_range(&metadata, 10)
    );
  }
}

#[test]
fn if_range_allows_partial_content_for_exact_http_date_match() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=7-\r\n",
    "If-Range: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "\r\n"
  ));
  let metadata = HttpConditionalMetadata::new().last_modified(
    httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").expect("metadata date"),
  );

  assert_eq!(
    Ok(HttpIfRangeRequestOutcome::PartialContent(
      HttpByteRange::new(7, 9)
    )),
    request.evaluate_if_range(&metadata, 10)
  );
}

#[test]
fn if_range_falls_back_to_full_response_for_stale_invalid_or_missing_validator_metadata() {
  for (if_range, metadata) in [
    (
      "Sun, 06 Nov 1994 08:49:36 GMT",
      HttpConditionalMetadata::new().last_modified(
        httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").expect("metadata date"),
      ),
    ),
    ("not a validator", HttpConditionalMetadata::new()),
    (r#""abc123""#, HttpConditionalMetadata::new()),
  ] {
    let request = parse_request(&format!(
      concat!(
        "GET /asset HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Range: bytes=2-5\r\n",
        "If-Range: {if_range}\r\n",
        "\r\n"
      ),
      if_range = if_range
    ));

    assert_eq!(
      Ok(HttpIfRangeRequestOutcome::FullResponse),
      request.evaluate_if_range(&metadata, 10)
    );
  }
}

#[test]
fn if_range_without_if_range_header_uses_existing_range_parser_outcomes() {
  let partial = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=-4\r\n",
    "\r\n"
  ));
  let unsatisfied = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=10-\r\n",
    "\r\n"
  ));
  let invalid = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=5-2\r\n",
    "\r\n"
  ));
  let metadata = HttpConditionalMetadata::new();

  assert_eq!(
    Ok(HttpIfRangeRequestOutcome::PartialContent(
      HttpByteRange::new(6, 9)
    )),
    partial.evaluate_if_range(&metadata, 10)
  );
  assert_eq!(
    Ok(HttpIfRangeRequestOutcome::RangeNotSatisfiable),
    unsatisfied.evaluate_if_range(&metadata, 10)
  );
  assert_eq!(
    Err(HttpByteRangeError::InvalidRange),
    invalid.evaluate_if_range(&metadata, 10)
  );
}

#[test]
fn if_range_without_range_header_falls_back_to_full_response() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "If-Range: \"abc123\"\r\n",
    "\r\n"
  ));
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("abc123"));

  assert_eq!(
    Ok(HttpIfRangeRequestOutcome::FullResponse),
    request.evaluate_if_range(&metadata, 10)
  );
}

#[test]
fn serializes_chunked_response_body_when_transfer_encoding_is_chunked() {
  let response = HttpResponse::new(200, "OK")
    .header("Transfer-Encoding", "chunked")
    .body("hello");

  let serialized = response.to_bytes();

  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\n",
      "hello\r\n",
      "0\r\n",
      "\r\n"
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn serializes_chunked_response_trailers() {
  let response = HttpResponse::new(200, "OK")
    .header("Transfer-Encoding", "chunked")
    .trailer("X-Trace", "abc")
    .trailer("X-Signature", "signed")
    .body("hello");

  let serialized = response.to_bytes();

  assert_eq!(2, response.trailers().len());
  assert_eq!(Some("abc"), response.trailer_value("x-trace"));
  assert_eq!(Some("signed"), response.trailer_value("X-SIGNATURE"));
  assert_eq!(
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Transfer-Encoding: chunked\r\n",
      "Trailer: X-Trace, X-Signature\r\n",
      "\r\n",
      "5\r\n",
      "hello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "X-Signature: signed\r\n",
      "\r\n"
    )
    .as_bytes(),
    serialized.as_slice()
  );
}

#[test]
fn rejects_response_headers_with_crlf() {
  let result = std::panic::catch_unwind(|| {
    let _response = HttpResponse::new(302, "Found").header("Location", "/safe\r\nX-Evil: true");
  });

  assert!(result.is_err());
}

#[test]
fn rejects_response_trailers_with_crlf() {
  let result = std::panic::catch_unwind(|| {
    let _response = HttpResponse::new(200, "OK").trailer("X-Trace", "safe\r\nX-Evil: true");
  });

  assert!(result.is_err());
}

#[test]
fn rejects_response_trailers_with_malformed_names() {
  for name in ["", "Bad Name", "Bad:Name"] {
    let result = std::panic::catch_unwind(|| {
      let _response = HttpResponse::new(200, "OK").trailer(name, "unsafe");
    });

    assert!(result.is_err(), "{name:?} trailer should be rejected");
  }
}

#[test]
fn rejects_forbidden_response_trailer_names() {
  for name in [
    "Content-Length",
    "transfer-encoding",
    "Host",
    "Authorization",
    "Proxy-Authorization",
    "WWW-Authenticate",
    "Proxy-Authenticate",
    "Connection",
    "Cookie",
    "Set-Cookie",
    "TE",
    "Trailer",
    "Upgrade",
  ] {
    let result = std::panic::catch_unwind(|| {
      let _response = HttpResponse::new(200, "OK").trailer(name, "unsafe");
    });

    assert!(result.is_err(), "{name} trailer should be rejected");
  }
}

#[test]
fn serializes_empty_http_response_without_content_length_for_204() {
  let response = HttpResponse::new(204, "No Content");

  let serialized = response.to_bytes();

  assert_eq!(b"HTTP/1.1 204 No Content\r\n\r\n", serialized.as_slice());
}

#[test]
fn omits_chunked_trailer_declaration_for_bodyless_response() {
  let response = HttpResponse::new(204, "No Content")
    .header("Transfer-Encoding", "chunked")
    .trailer("X-Trace", "abc")
    .body("ignored");

  let serialized = response.to_bytes();

  assert_eq!(b"HTTP/1.1 204 No Content\r\n\r\n", serialized.as_slice());
}

#[test]
fn serializes_empty_http_response_without_content_length_for_1xx() {
  let response = HttpResponse::new(101, "Switching Protocols");

  let serialized = response.to_bytes();

  assert_eq!(
    b"HTTP/1.1 101 Switching Protocols\r\n\r\n",
    serialized.as_slice()
  );
}
