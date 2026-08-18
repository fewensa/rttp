use std::net::TcpStream as StdTcpStream;

#[test]
fn request_cache_control_combines_case_insensitive_header_fields() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCache-Control: no-cache, max-age=60\r\ncache-control: min-fresh=30, only-if-cached\r\n\r\n",
  )
  .expect("request should parse");

  let cache_control = request
    .cache_control()
    .expect("Cache-Control should parse")
    .expect("Cache-Control should be present");

  assert!(cache_control.no_cache());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(Some(30), cache_control.min_fresh());
  assert!(cache_control.only_if_cached());
}

#[test]
fn request_access_control_request_method_parses_preflight_metadata_without_policy() {
  let absent_raw = "OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .access_control_request_method()
      .expect("missing Access-Control-Request-Method should be accepted")
  );

  let valid_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Method: patch\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    "PATCH",
    valid
      .access_control_request_method()
      .expect("Access-Control-Request-Method should parse")
      .expect("Access-Control-Request-Method should be present")
      .method()
  );

  let malformed_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Method: GET, POST\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.access_control_request_method().is_err());
  assert_eq!(
    Some("GET, POST"),
    malformed.header("Access-Control-Request-Method")
  );
}

#[test]
fn request_access_control_request_headers_parses_preflight_metadata_without_policy() {
  let absent_raw = "OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .access_control_request_headers()
      .expect("missing Access-Control-Request-Headers should be accepted")
  );

  let valid_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Headers: X-Request-Id, Authorization\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    ["x-request-id", "authorization"],
    valid
      .access_control_request_headers()
      .expect("Access-Control-Request-Headers should parse")
      .expect("Access-Control-Request-Headers should be present")
      .field_names()
  );

  let malformed_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Headers: X-Request Id\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.access_control_request_headers().is_err());
  assert_eq!(
    Some("X-Request Id"),
    malformed.header("Access-Control-Request-Headers")
  );
}

#[test]
fn request_representation_metadata_parses_without_applying_policy() {
  let absent_raw = "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .content_type()
      .expect("missing Content-Type should be accepted")
  );
  assert_eq!(
    None,
    absent
      .content_encoding()
      .expect("missing Content-Encoding should be accepted")
  );
  assert_eq!(
    None,
    absent
      .content_language()
      .expect("missing Content-Language should be accepted")
  );

  let valid_raw = concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: application/json; charset=utf-8\r\n",
    "Content-Encoding: gzip, br\r\n",
    "content-encoding: zstd\r\n",
    "Content-Language: fr-CA, es-419\r\n",
    "content-language: en\r\n",
    "Accept-Encoding: gzip\r\n",
    "Accept-Language: en\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");

  let content_type = valid
    .content_type()
    .expect("Content-Type should parse")
    .expect("Content-Type should be present");
  assert_eq!("application/json", content_type.media_type());
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));

  let encodings = valid
    .content_encoding()
    .expect("Content-Encoding should parse")
    .expect("Content-Encoding should be present");
  assert_eq!(vec!["gzip", "br", "zstd"], encodings.codings());

  let languages = valid
    .content_language()
    .expect("Content-Language should parse")
    .expect("Content-Language should be present");
  assert_eq!(vec!["fr-CA", "es-419", "en"], languages.languages());

  let accept_encoding = valid
    .accept_encoding()
    .expect("Accept-Encoding should parse")
    .expect("Accept-Encoding should be present");
  assert_eq!("gzip", accept_encoding.codings()[0].coding());
  let accept_language = valid
    .accept_language()
    .expect("Accept-Language should parse")
    .expect("Accept-Language should be present");
  assert_eq!(vec!["en"], accept_language.ranges());
  assert_eq!(b"body", valid.body());
}

#[test]
fn request_host_parses_http11_authority_without_routing() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test:8443\r\n",
    "\r\n"
  ).as_bytes())
  .expect("request should parse");

  let host = request
    .host()
    .expect("Host should parse")
    .expect("Host should be present");
  assert_eq!("example.test", host.host());
  assert_eq!(Some("8443"), host.port());
  assert_eq!("example.test:8443", host.header_value());
}

#[test]
fn request_host_preserves_absent_duplicate_and_malformed_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.0\r\n\r\n").expect("request should parse");
  assert_eq!(
    None,
    absent.host().expect("absent Host should be accepted")
  );

  let duplicate = Request::from_raw_frame(concat!(
    "GET / HTTP/1.0\r\n",
    "Host: example.test\r\n",
    "host: other.test\r\n",
    "\r\n"
  ).as_bytes())
  .expect("duplicate Host should not reject the HTTP/1.0 request frame");
  assert!(duplicate.host().is_err());
  assert_eq!(
    vec!["example.test", "other.test"],
    duplicate.headers_named("Host").collect::<Vec<_>>()
  );

  let malformed = Request::from_raw_frame(concat!(
    "GET / HTTP/1.0\r\n",
    "Host: example.test/path\r\n",
    "\r\n"
  ).as_bytes())
  .expect("malformed Host should not reject the HTTP/1.0 request frame");
  assert!(malformed.host().is_err());
  assert_eq!(Some("example.test/path"), malformed.header("Host"));
}

#[test]
fn request_host_parses_http2_authority_mapped_host() {
  let request = DecodedHttp2RequestHeaders {
    method: Some("GET".to_string()),
    target: Some("/asset".to_string()),
    scheme: Some("https".to_string()),
    authority: Some("example.test:8443".to_string()),
    extended_connect_protocol: None,
    headers: Vec::new(),
  }
  .into_request(Vec::new(), Vec::new())
  .expect("HTTP/2 request should build");

  assert_eq!(Some("example.test:8443"), request.header("host"));
  let host = request
    .host()
    .expect("mapped Host should parse")
    .expect("mapped Host should be present");
  assert_eq!("example.test", host.host());
  assert_eq!(Some("8443"), host.port());
}

#[test]
fn request_host_rejects_duplicate_http2_host_and_authority() {
  let request = DecodedHttp2RequestHeaders {
    method: Some("GET".to_string()),
    target: Some("/asset".to_string()),
    scheme: Some("https".to_string()),
    authority: Some("example.test".to_string()),
    extended_connect_protocol: None,
    headers: vec![("host".to_string(), "other.test".to_string())],
  }
  .into_request(Vec::new(), Vec::new())
  .expect("HTTP/2 request should build");

  assert_eq!(
    vec!["other.test", "example.test"],
    request.headers_named("host").collect::<Vec<_>>()
  );
  assert!(request.host().is_err());
}

#[test]
fn request_want_repr_digest_parses_preferences_without_selecting_an_algorithm() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Repr-Digest: sha-256=10, sha-512=3\r\n",
    "want-repr-digest: unixsum=0\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("request should parse");

  let digest = request
    .want_repr_digest()
    .expect("Want-Repr-Digest should parse")
    .expect("Want-Repr-Digest should be present");
  assert_eq!(digest.len(), 3);
  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].preference(), 3);
  assert_eq!(digest.entries()[2].algorithm(), "unixsum");
  assert_eq!(digest.entries()[2].preference(), 0);
  assert_eq!(b"body", request.body());
}

#[test]
fn request_want_repr_digest_preserves_absent_and_malformed_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .want_repr_digest()
      .expect("absent Want-Repr-Digest should be accepted")
  );

  let malformed = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Repr-Digest: sha-256\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("malformed Want-Repr-Digest should not reject the request frame");
  assert!(malformed.want_repr_digest().is_err());
  assert_eq!(Some("sha-256"), malformed.header("Want-Repr-Digest"));
  assert_eq!(b"body", malformed.body());
}

#[test]
fn request_representation_metadata_preserves_invalid_headers_and_body() {
  let duplicate = Request::from_raw_frame(concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: application/json\r\n",
    "content-type: text/plain\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("duplicate Content-Type should not reject the request frame");
  assert!(duplicate.content_type().is_err());
  assert_eq!(
    vec!["application/json", "text/plain"],
    duplicate.headers_named("Content-Type").collect::<Vec<_>>()
  );
  assert_eq!(b"body", duplicate.body());

  let malformed = Request::from_raw_frame(concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain;\r\n",
    "Content-Encoding: gzip,\r\n",
    "Content-Language: en,\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("malformed metadata should not reject the request frame");
  assert!(malformed.content_type().is_err());
  assert!(malformed.content_encoding().is_err());
  assert!(malformed.content_language().is_err());
  assert_eq!(Some("text/plain;"), malformed.header("Content-Type"));
  assert_eq!(Some("gzip,"), malformed.header("Content-Encoding"));
  assert_eq!(Some("en,"), malformed.header("Content-Language"));
  assert_eq!(b"body", malformed.body());

  let duplicate_members = Request::from_raw_frame(concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain; charset=utf-8; CHARSET=us-ascii\r\n",
    "Content-Encoding: gzip\r\n",
    "content-encoding: GZIP\r\n",
    "Content-Language: en\r\n",
    "content-language: EN\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("duplicate members should not reject the request frame");
  assert!(duplicate_members.content_type().is_err());
  assert!(duplicate_members.content_encoding().is_err());
  assert!(duplicate_members.content_language().is_err());
  assert_eq!(
    Some("text/plain; charset=utf-8; CHARSET=us-ascii"),
    duplicate_members.header("Content-Type")
  );
  assert_eq!(
    vec!["gzip", "GZIP"],
    duplicate_members
      .headers_named("Content-Encoding")
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec!["en", "EN"],
    duplicate_members
      .headers_named("Content-Language")
      .collect::<Vec<_>>()
  );
  assert_eq!(b"body", duplicate_members.body());

  let too_many_parameters = format!(
    "text/plain{}",
    (0..33)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let too_many_codings = (0..33)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_languages = (0..33)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many = Request::from_raw_frame(
    format!(
      concat!(
        "POST /documents HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Content-Type: {type}\r\n",
        "Content-Encoding: {encoding}\r\n",
        "Content-Language: {language}\r\n",
        "Content-Length: 4\r\n",
        "\r\n",
        "body"
      ),
      type = too_many_parameters,
      encoding = too_many_codings,
      language = too_many_languages
    )
    .as_bytes(),
  )
  .expect("over-limit metadata should not reject the request frame");
  assert!(too_many.content_type().is_err());
  assert!(too_many.content_encoding().is_err());
  assert!(too_many.content_language().is_err());
  assert_eq!(Some(too_many_parameters.as_str()), too_many.header("Content-Type"));
  assert_eq!(Some(too_many_codings.as_str()), too_many.header("Content-Encoding"));
  assert_eq!(Some(too_many_languages.as_str()), too_many.header("Content-Language"));
  assert_eq!(b"body", too_many.body());

  let oversized_type = format!("text/plain; p={}", "a".repeat(64 * 1024));
  let oversized_encoding = format!("x-{}", "a".repeat(64 * 1024));
  let oversized_language = format!("x-{}", "a".repeat(64 * 1024));
  let oversized = Request {
    method: "POST".to_string(),
    target: "/documents".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Content-Type".to_string(), oversized_type.clone()),
      ("Content-Encoding".to_string(), oversized_encoding.clone()),
      ("Content-Language".to_string(), oversized_language.clone()),
    ],
    trailers: Vec::new(),
    body: b"body".to_vec(),
    extended_connect_protocol: None,
  };
  assert!(oversized.content_type().is_err());
  assert!(oversized.content_encoding().is_err());
  assert!(oversized.content_language().is_err());
  assert_eq!(Some(oversized_type.as_str()), oversized.header("Content-Type"));
  assert_eq!(
    Some(oversized_encoding.as_str()),
    oversized.header("Content-Encoding")
  );
  assert_eq!(
    Some(oversized_language.as_str()),
    oversized.header("Content-Language")
  );
  assert_eq!(b"body", oversized.body());
}

#[test]
fn request_raw_parser_rejects_folded_and_bare_lf_headers() {
  for raw in [
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\n second\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\n\tsecond\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\nsecond\r\n\r\n".as_slice(),
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\rsecond\r\n\r\n".as_slice(),
  ] {
    let error = Request::from_raw_frame(raw)
      .expect_err("folded and bare-LF request headers must be rejected");
    assert_eq!(std::io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid request header", error.to_string());
  }
}

#[test]
fn request_raw_parser_preserves_duplicate_ordinary_headers_in_wire_order() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Test: first\r\nx-test: second\r\n\r\n",
  )
  .expect("duplicate ordinary request headers should parse");

  assert_eq!(
    vec!["first", "second"],
    request.headers_named("X-Test").collect::<Vec<_>>()
  );
}

#[test]
fn request_raw_parser_preserves_obs_text_header_values_as_latin1_code_points() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Obs: \x80\xc3\xa9\xff\r\n\r\n",
  )
  .expect("obs-text request header should parse");

  // Request headers use `String`, so raw obs-text bytes cross the API boundary
  // as their corresponding Latin-1 code points.
  assert_eq!(Some("\u{0080}\u{00c3}\u{00a9}\u{00ff}"), request.header("X-Obs"));
}

#[test]
fn request_raw_parser_preserves_non_ows_obs_text_header_value_edges() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nX-Obs: \xa0value\xa0\r\n\r\n",
  )
  .expect("obs-text request header should parse");

  assert_eq!(Some("\u{00a0}value\u{00a0}"), request.header("X-Obs"));
}

#[test]
fn request_cache_control_preserves_malformed_headers_for_handler_policy() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCache-Control: max-age=invalid\r\n\r\n",
  )
  .expect("request should retain malformed metadata");

  assert!(request.cache_control().is_err());
  assert_eq!(Some("max-age=invalid"), request.header("Cache-Control"));
}

#[test]
fn request_cache_control_rejects_oversized_values_without_panicking() {
  let request = Request {
    method: "GET".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![(
      "Cache-Control".to_string(),
      format!("x={}", "a".repeat(MAX_CACHE_CONTROL_VALUE_BYTES)),
    )],
    trailers: Vec::new(),
    body: Vec::new(),
    extended_connect_protocol: None,
  };

  let error = request
    .cache_control()
    .expect_err("oversized Cache-Control should be rejected");

  assert_eq!("Cache-Control header value is too large", error.to_string());
}

#[test]
fn request_cache_control_rejects_directive_counts_across_header_fields() {
  let directive_field = std::iter::repeat_n("extension", MAX_CACHE_CONTROL_DIRECTIVES)
    .collect::<Vec<_>>()
    .join(", ");
  let request = Request {
    method: "GET".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Cache-Control".to_string(), directive_field),
      ("cache-control".to_string(), "another-extension".to_string()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    extended_connect_protocol: None,
  };

  let error = request
    .cache_control()
    .expect_err("too many Cache-Control directives should be rejected");

  assert_eq!("too many Cache-Control directives", error.to_string());
}

#[test]
fn access_control_allow_methods_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Methods", "DELETE")
    .header("access-control-allow-methods", "PATCH")
    .with_access_control_allow_methods("get, POST")
    .expect("Access-Control-Allow-Methods should be accepted");

  let allow_methods = response
    .access_control_allow_methods()
    .expect("Access-Control-Allow-Methods should parse")
    .expect("Access-Control-Allow-Methods should be present");
  assert_eq!(["GET", "POST"], allow_methods.methods());
  assert_eq!(
    vec![("Access-Control-Allow-Methods", "GET, POST")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn access_control_allow_origin_helpers_validate_replace_and_preserve_raw_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Origin", "https://legacy.test")
    .header("access-control-allow-origin", "https://deprecated.test")
    .with_access_control_allow_origin("https://example.test:8443")
    .expect("Access-Control-Allow-Origin should be accepted");

  assert_eq!(
    "https://example.test:8443",
    response
      .access_control_allow_origin()
      .expect("Access-Control-Allow-Origin should parse")
      .expect("Access-Control-Allow-Origin should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Access-Control-Allow-Origin", "https://example.test:8443")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Origin", "https://example.test/path");
  assert!(malformed.access_control_allow_origin().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_origin("https://example.test/path")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .access_control_allow_origin()
      .expect("absent Access-Control-Allow-Origin should parse")
  );
  for value in ["*", "null"] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_access_control_allow_origin(value)
        .expect("valid Access-Control-Allow-Origin should be accepted")
        .access_control_allow_origin()
        .expect("Access-Control-Allow-Origin should parse")
        .expect("Access-Control-Allow-Origin should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Access-Control-Allow-Origin", "https://example.test")
    .header("access-control-allow-origin", "https://other.test");
  assert!(duplicate.access_control_allow_origin().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_origin("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn cross_origin_resource_policy_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Cross-Origin-Resource-Policy", "same-site")
    .header("cross-origin-resource-policy", "cross-origin")
    .with_cross_origin_resource_policy("SAME-ORIGIN")
    .expect("Cross-Origin-Resource-Policy should be accepted");

  assert_eq!(
    "same-origin",
    response
      .cross_origin_resource_policy()
      .expect("Cross-Origin-Resource-Policy should parse")
      .expect("Cross-Origin-Resource-Policy should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Cross-Origin-Resource-Policy", "same-origin")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn cross_origin_resource_policy_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Cross-Origin-Resource-Policy", "SAME-ORIGIN");
  assert_eq!(
    "same-origin",
    raw
      .cross_origin_resource_policy()
      .expect("raw SAME-ORIGIN should parse")
      .expect("Cross-Origin-Resource-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some("SAME-ORIGIN"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Cross-Origin-Resource-Policy"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Cross-Origin-Resource-Policy", "same origin");
  assert!(malformed.cross_origin_resource_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_resource_policy("same origin")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .cross_origin_resource_policy()
      .expect("absent Cross-Origin-Resource-Policy should parse")
  );
  for value in ["same-origin", "same-site", "cross-origin"] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_cross_origin_resource_policy(value)
        .expect("valid Cross-Origin-Resource-Policy should be accepted")
        .cross_origin_resource_policy()
        .expect("Cross-Origin-Resource-Policy should parse")
        .expect("Cross-Origin-Resource-Policy should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Cross-Origin-Resource-Policy", "same-origin")
    .header("cross-origin-resource-policy", "same-site");
  assert!(duplicate.cross_origin_resource_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_resource_policy("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn cross_origin_embedder_policy_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Cross-Origin-Embedder-Policy", "unsafe-none")
    .header("cross-origin-embedder-policy", "credentialless")
    .with_cross_origin_embedder_policy(r#"require-corp; report-to="coep""#)
    .expect("Cross-Origin-Embedder-Policy should be accepted");

  assert_eq!(
    "require-corp",
    response
      .cross_origin_embedder_policy()
      .expect("Cross-Origin-Embedder-Policy should parse")
      .expect("Cross-Origin-Embedder-Policy should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Cross-Origin-Embedder-Policy", "require-corp")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn cross_origin_embedder_policy_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Cross-Origin-Embedder-Policy", "require-corp");
  assert_eq!(
    "require-corp",
    raw
      .cross_origin_embedder_policy()
      .expect("raw require-corp should parse")
      .expect("Cross-Origin-Embedder-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some("require-corp"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Cross-Origin-Embedder-Policy"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Cross-Origin-Embedder-Policy", "require corp");
  assert!(malformed.cross_origin_embedder_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_embedder_policy("require corp")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .cross_origin_embedder_policy()
      .expect("absent Cross-Origin-Embedder-Policy should parse")
  );
  for value in ["unsafe-none", "require-corp", "credentialless"] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_cross_origin_embedder_policy(value)
        .expect("valid Cross-Origin-Embedder-Policy should be accepted")
        .cross_origin_embedder_policy()
        .expect("Cross-Origin-Embedder-Policy should parse")
        .expect("Cross-Origin-Embedder-Policy should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Cross-Origin-Embedder-Policy", "require-corp")
    .header("cross-origin-embedder-policy", "credentialless");
  assert!(duplicate.cross_origin_embedder_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_embedder_policy("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn cross_origin_embedder_policy_report_only_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Cross-Origin-Embedder-Policy-Report-Only", "unsafe-none")
    .header("cross-origin-embedder-policy-report-only", "credentialless")
    .with_cross_origin_embedder_policy_report_only(r#"require-corp; report-to="coep""#)
    .expect("Cross-Origin-Embedder-Policy-Report-Only should be accepted");

  assert_eq!(
    "require-corp",
    response
      .cross_origin_embedder_policy_report_only()
      .expect("Cross-Origin-Embedder-Policy-Report-Only should parse")
      .expect("Cross-Origin-Embedder-Policy-Report-Only should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Cross-Origin-Embedder-Policy-Report-Only", "require-corp")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn cross_origin_embedder_policy_report_only_helpers_preserve_raw_metadata_and_report_parse_errors()
{
  let raw =
    HttpResponse::ok([]).header("Cross-Origin-Embedder-Policy-Report-Only", "require-corp");
  assert_eq!(
    "require-corp",
    raw
      .cross_origin_embedder_policy_report_only()
      .expect("raw require-corp should parse")
      .expect("Cross-Origin-Embedder-Policy-Report-Only should be present")
      .header_value()
  );
  assert_eq!(
    Some("require-corp"),
    raw
      .headers
      .iter()
      .find(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Embedder-Policy-Report-Only")
      })
      .map(|header| header.value.as_str())
  );

  let malformed =
    HttpResponse::ok([]).header("Cross-Origin-Embedder-Policy-Report-Only", "require corp");
  assert!(malformed.cross_origin_embedder_policy_report_only().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_embedder_policy_report_only("require corp")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .cross_origin_embedder_policy_report_only()
      .expect("absent Cross-Origin-Embedder-Policy-Report-Only should parse")
  );
  for value in ["unsafe-none", "require-corp", "credentialless"] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_cross_origin_embedder_policy_report_only(value)
        .expect("valid Cross-Origin-Embedder-Policy-Report-Only should be accepted")
        .cross_origin_embedder_policy_report_only()
        .expect("Cross-Origin-Embedder-Policy-Report-Only should parse")
        .expect("Cross-Origin-Embedder-Policy-Report-Only should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Cross-Origin-Embedder-Policy-Report-Only", "require-corp")
    .header("cross-origin-embedder-policy-report-only", "credentialless");
  assert!(duplicate.cross_origin_embedder_policy_report_only().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_embedder_policy_report_only("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn cross_origin_opener_policy_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Cross-Origin-Opener-Policy", "unsafe-none")
    .header("cross-origin-opener-policy", "same-origin")
    .with_cross_origin_opener_policy(r#"noopener-allow-popups; report-to="coop""#)
    .expect("Cross-Origin-Opener-Policy should be accepted");

  assert_eq!(
    "noopener-allow-popups",
    response
      .cross_origin_opener_policy()
      .expect("Cross-Origin-Opener-Policy should parse")
      .expect("Cross-Origin-Opener-Policy should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Cross-Origin-Opener-Policy", "noopener-allow-popups")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn cross_origin_opener_policy_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Cross-Origin-Opener-Policy", "same-origin");
  assert_eq!(
    "same-origin",
    raw
      .cross_origin_opener_policy()
      .expect("raw same-origin should parse")
      .expect("Cross-Origin-Opener-Policy should be present")
      .header_value()
  );
  assert_eq!(
    Some("same-origin"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Cross-Origin-Opener-Policy"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Cross-Origin-Opener-Policy", "same origin");
  assert!(malformed.cross_origin_opener_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_opener_policy("same origin")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .cross_origin_opener_policy()
      .expect("absent Cross-Origin-Opener-Policy should parse")
  );
  for value in [
    "unsafe-none",
    "same-origin-allow-popups",
    "same-origin",
    "noopener-allow-popups",
  ] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_cross_origin_opener_policy(value)
        .expect("valid Cross-Origin-Opener-Policy should be accepted")
        .cross_origin_opener_policy()
        .expect("Cross-Origin-Opener-Policy should parse")
        .expect("Cross-Origin-Opener-Policy should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Cross-Origin-Opener-Policy", "same-origin")
    .header("cross-origin-opener-policy", "noopener-allow-popups");
  assert!(duplicate.cross_origin_opener_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_opener_policy("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn strict_transport_security_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Strict-Transport-Security", "max-age=60")
    .header("strict-transport-security", "max-age=120")
    .with_strict_transport_security("max-age=31536000; includeSubDomains")
    .expect("Strict-Transport-Security should be accepted");

  let metadata = response
    .strict_transport_security()
    .expect("Strict-Transport-Security should parse")
    .expect("Strict-Transport-Security should be present");
  assert_eq!(31536000, metadata.max_age());
  assert!(metadata.include_sub_domains());
  assert_eq!(
    "max-age=31536000; includeSubDomains",
    metadata.header_value()
  );
  assert_eq!(
    vec![("Strict-Transport-Security", "max-age=31536000; includeSubDomains")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn strict_transport_security_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Strict-Transport-Security", "max-age=60");
  let metadata = raw
    .strict_transport_security()
    .expect("raw max-age=60 should parse")
    .expect("Strict-Transport-Security should be present");
  assert_eq!(60, metadata.max_age());
  assert_eq!("max-age=60", metadata.header_value());
  assert_eq!(
    Some("max-age=60"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Strict-Transport-Security"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Strict-Transport-Security", "not hsts");
  assert!(malformed.strict_transport_security().is_err());
  assert!(HttpResponse::ok([])
    .with_strict_transport_security("not hsts")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .strict_transport_security()
      .expect("absent Strict-Transport-Security should parse")
  );
  for value in [
    "max-age=60",
    "max-age=0",
    "max-age=31536000; includeSubDomains; preload",
  ] {
    assert_eq!(
      value,
      HttpResponse::ok([])
        .with_strict_transport_security(value)
        .expect("valid Strict-Transport-Security should be accepted")
        .strict_transport_security()
        .expect("Strict-Transport-Security should parse")
        .expect("Strict-Transport-Security should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Strict-Transport-Security", "max-age=60")
    .header("strict-transport-security", "max-age=120");
  assert!(duplicate.strict_transport_security().is_err());
  assert!(HttpResponse::ok([])
    .with_strict_transport_security("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn x_content_type_options_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("X-Content-Type-Options", "nosniff")
    .header("x-content-type-options", "nosniff")
    .with_x_content_type_options("NoSniff")
    .expect("X-Content-Type-Options should be accepted");

  assert_eq!(
    "nosniff",
    response
      .x_content_type_options()
      .expect("X-Content-Type-Options should parse")
      .expect("X-Content-Type-Options should be present")
      .header_value()
  );
  assert_eq!(
    vec![("X-Content-Type-Options", "nosniff")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn x_content_type_options_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("X-Content-Type-Options", "NoSniff");
  assert_eq!(
    "nosniff",
    raw
      .x_content_type_options()
      .expect("raw NoSniff should parse")
      .expect("X-Content-Type-Options should be present")
      .header_value()
  );
  assert_eq!(
    Some("NoSniff"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("X-Content-Type-Options"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("X-Content-Type-Options", "same-origin");
  assert!(malformed.x_content_type_options().is_err());
  assert!(HttpResponse::ok([])
    .with_x_content_type_options("same-origin")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .x_content_type_options()
      .expect("absent X-Content-Type-Options should parse")
  );
  for value in ["nosniff", "NoSniff", "NOSNIFF"] {
    assert_eq!(
      "nosniff",
      HttpResponse::ok([])
        .with_x_content_type_options(value)
        .expect("valid X-Content-Type-Options should be accepted")
        .x_content_type_options()
        .expect("X-Content-Type-Options should parse")
        .expect("X-Content-Type-Options should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("X-Content-Type-Options", "nosniff")
    .header("x-content-type-options", "nosniff");
  assert!(duplicate.x_content_type_options().is_err());
  assert!(HttpResponse::ok([])
    .with_x_content_type_options("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn x_frame_options_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("X-Frame-Options", "deny")
    .header("x-frame-options", "SAMEORIGIN")
    .with_x_frame_options("DENY")
    .expect("X-Frame-Options should be accepted");

  assert_eq!(
    "DENY",
    response
      .x_frame_options()
      .expect("X-Frame-Options should parse")
      .expect("X-Frame-Options should be present")
      .header_value()
  );
  assert_eq!(
    vec![("X-Frame-Options", "DENY")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn x_frame_options_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("X-Frame-Options", "sameorigin");
  assert_eq!(
    "SAMEORIGIN",
    raw
      .x_frame_options()
      .expect("raw sameorigin should parse")
      .expect("X-Frame-Options should be present")
      .header_value()
  );
  assert_eq!(
    Some("sameorigin"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("X-Frame-Options"))
      .map(|header| header.value.as_str())
  );

  let malformed =
    HttpResponse::ok([]).header("X-Frame-Options", "ALLOW-FROM https://example.test");
  assert!(malformed.x_frame_options().is_err());
  assert!(HttpResponse::ok([])
    .with_x_frame_options("ALLOW-FROM https://example.test")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .x_frame_options()
      .expect("absent X-Frame-Options should parse")
  );
  for value in ["DENY", "deny", "SAMEORIGIN", "sameorigin"] {
    assert_eq!(
      value.to_ascii_uppercase(),
      HttpResponse::ok([])
        .with_x_frame_options(value)
        .expect("valid X-Frame-Options should be accepted")
        .x_frame_options()
        .expect("X-Frame-Options should parse")
        .expect("X-Frame-Options should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("X-Frame-Options", "DENY")
    .header("x-frame-options", "SAMEORIGIN");
  assert!(duplicate.x_frame_options().is_err());
  assert!(HttpResponse::ok([])
    .with_x_frame_options("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn authentication_info_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Authentication-Info", "qop=auth")
    .header("authentication-info", r#"rspauth="abc""#)
    .with_authentication_info(
      r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth, rspauth="6629fae49393a05397450978507c4ef1", cnonce="0a4f113b", nc=00000001"#,
    )
    .expect("Authentication-Info should be accepted");

  let metadata = response
    .authentication_info()
    .expect("Authentication-Info should parse")
    .expect("Authentication-Info should be present");
  assert_eq!(
    Some("6629fae49393a05397450978507c4ef1"),
    metadata.parameter("nextnonce")
  );
  assert_eq!(Some("auth"), metadata.parameter("qop"));
  assert_eq!(
    Some("6629fae49393a05397450978507c4ef1"),
    metadata.parameter("rspauth")
  );
  assert_eq!(Some("00000001"), metadata.parameter("nc"));
  assert_eq!(
    "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth, rspauth=6629fae49393a05397450978507c4ef1, cnonce=0a4f113b, nc=00000001",
    metadata.header_value()
  );
  assert_eq!(
    vec![(
      "Authentication-Info",
      "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth, rspauth=6629fae49393a05397450978507c4ef1, cnonce=0a4f113b, nc=00000001",
    )],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn authentication_info_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Authentication-Info", r#"nextnonce="abc""#);
  let metadata = raw
    .authentication_info()
    .expect("raw nextnonce should parse")
    .expect("Authentication-Info should be present");
  assert_eq!(Some("abc"), metadata.parameter("nextnonce"));
  assert_eq!("nextnonce=abc", metadata.header_value());
  assert_eq!(
    Some(r#"nextnonce="abc""#),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Authentication-Info"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Authentication-Info", "nextnonce");
  assert!(malformed.authentication_info().is_err());
  assert!(HttpResponse::ok([])
    .with_authentication_info("nextnonce")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .authentication_info()
      .expect("absent Authentication-Info should parse")
  );
  for (input, canonical) in [
    ("qop=auth", "qop=auth"),
    (
      r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth"#,
      "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth",
    ),
    (r#"msg="say \"hi\"""#, r#"msg="say \"hi\"""#),
  ] {
    assert_eq!(
      canonical,
      HttpResponse::ok([])
        .with_authentication_info(input)
        .expect("valid Authentication-Info should be accepted")
        .authentication_info()
        .expect("Authentication-Info should parse")
        .expect("Authentication-Info should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Authentication-Info", "qop=auth")
    .header("authentication-info", "QOP=auth");
  assert!(duplicate.authentication_info().is_err());
  assert!(HttpResponse::ok([])
    .with_authentication_info("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn proxy_authentication_info_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Proxy-Authentication-Info", "qop=auth")
    .header("proxy-authentication-info", r#"rspauth="abc""#)
    .with_proxy_authentication_info(
      r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth, rspauth="6629fae49393a05397450978507c4ef1", cnonce="0a4f113b", nc=00000001"#,
    )
    .expect("Proxy-Authentication-Info should be accepted");

  let metadata = response
    .proxy_authentication_info()
    .expect("Proxy-Authentication-Info should parse")
    .expect("Proxy-Authentication-Info should be present");
  assert_eq!(
    Some("6629fae49393a05397450978507c4ef1"),
    metadata.parameter("nextnonce")
  );
  assert_eq!(Some("auth"), metadata.parameter("qop"));
  assert_eq!(
    Some("6629fae49393a05397450978507c4ef1"),
    metadata.parameter("rspauth")
  );
  assert_eq!(Some("00000001"), metadata.parameter("nc"));
  assert_eq!(
    "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth, rspauth=6629fae49393a05397450978507c4ef1, cnonce=0a4f113b, nc=00000001",
    metadata.header_value()
  );
  assert_eq!(
    vec![(
      "Proxy-Authentication-Info",
      "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth, rspauth=6629fae49393a05397450978507c4ef1, cnonce=0a4f113b, nc=00000001",
    )],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn proxy_authentication_info_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw =
    HttpResponse::ok([]).header("Proxy-Authentication-Info", r#"nextnonce="abc""#);
  let metadata = raw
    .proxy_authentication_info()
    .expect("raw nextnonce should parse")
    .expect("Proxy-Authentication-Info should be present");
  assert_eq!(Some("abc"), metadata.parameter("nextnonce"));
  assert_eq!("nextnonce=abc", metadata.header_value());
  assert_eq!(
    Some(r#"nextnonce="abc""#),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Proxy-Authentication-Info"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([]).header("Proxy-Authentication-Info", "nextnonce");
  assert!(malformed.proxy_authentication_info().is_err());
  assert!(HttpResponse::ok([])
    .with_proxy_authentication_info("nextnonce")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .proxy_authentication_info()
      .expect("absent Proxy-Authentication-Info should parse")
  );
  for (input, canonical) in [
    ("qop=auth", "qop=auth"),
    (
      r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth"#,
      "nextnonce=6629fae49393a05397450978507c4ef1, qop=auth",
    ),
    (r#"msg="say \"hi\"""#, r#"msg="say \"hi\"""#),
  ] {
    assert_eq!(
      canonical,
      HttpResponse::ok([])
        .with_proxy_authentication_info(input)
        .expect("valid Proxy-Authentication-Info should be accepted")
        .proxy_authentication_info()
        .expect("Proxy-Authentication-Info should parse")
        .expect("Proxy-Authentication-Info should be present")
        .header_value()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Proxy-Authentication-Info", "qop=auth")
    .header("proxy-authentication-info", "QOP=auth");
  assert!(duplicate.proxy_authentication_info().is_err());
  assert!(HttpResponse::ok([])
    .with_proxy_authentication_info("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn access_control_allow_headers_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Headers", "X-Legacy")
    .header("access-control-allow-headers", "X-Deprecated")
    .with_access_control_allow_headers("X-Request-Id, ETag")
    .expect("Access-Control-Allow-Headers should be accepted");

  let allow_headers = response
    .access_control_allow_headers()
    .expect("Access-Control-Allow-Headers should parse")
    .expect("Access-Control-Allow-Headers should be present");
  assert_eq!(["x-request-id", "etag"], allow_headers.field_names());
  assert_eq!(
    vec![("Access-Control-Allow-Headers", "x-request-id, etag")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn access_control_allow_headers_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Headers", "X-Request Id");
  assert!(malformed.access_control_allow_headers().is_err());
  assert_eq!(
    Some("X-Request Id"),
    malformed
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Access-Control-Allow-Headers"))
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_access_control_allow_headers("X-Request Id")
    .is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_headers("*")
    .expect("wildcard Access-Control-Allow-Headers should be accepted")
    .access_control_allow_headers()
    .expect("wildcard Access-Control-Allow-Headers should parse")
    .expect("wildcard Access-Control-Allow-Headers should be present")
    .is_wildcard());
}

#[test]
fn access_control_allow_methods_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Methods", "GET POST");
  assert!(malformed.access_control_allow_methods().is_err());
  assert_eq!(
    Some("GET POST"),
    malformed
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Access-Control-Allow-Methods"))
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_access_control_allow_methods("GET POST")
    .is_err());
}

#[test]
fn access_control_allow_methods_helpers_do_not_apply_cors_policy() {
  assert_eq!(
    None,
    HttpResponse::ok([])
      .access_control_allow_methods()
      .expect("absent Access-Control-Allow-Methods should parse")
  );
}

#[test]
fn request_max_forwards_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.max_forwards().expect("missing value should be valid"));

  for value in ["255", "256", "999999999999999999999"] {
    let valid = Request::from_raw_frame(
      format!(
        "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    assert_eq!(
      Some(value.to_owned()),
      valid.max_forwards().expect("value should parse")
    );
  }

  for value in ["", "-1", "+1", "1.0"] {
    let request = Request::from_raw_frame(
      format!(
        "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(request.max_forwards().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Max-Forwards"));
  }

  let duplicate = Request::from_raw_frame(
    b"OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: 1\r\nmax-forwards: 2\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.max_forwards().is_err());
  assert_eq!(Some("1"), duplicate.header("Max-Forwards"));
}

#[test]
fn request_cookies_are_bounded_and_preserve_pairs() {
  let request = Request::from_raw_frame(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nCookie: session=abc; theme=dark\r\nCookie: flag=\r\n\r\n",
  )
  .expect("request should parse");
  let cookies = request
    .cookies()
    .expect("cookie metadata should parse")
    .expect("Cookie header should be present");
  assert_eq!(
    vec![("session", "abc"), ("theme", "dark"), ("flag", "")],
    cookies
      .pairs()
      .iter()
      .map(|pair| (pair.name(), pair.value()))
      .collect::<Vec<_>>()
  );

  assert!(HttpCookies::parse("session=abc\x01").is_err());
}

#[test]
fn request_exposes_bounded_range_and_conditional_metadata() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=-4\r\n",
    "If-Range: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "If-None-Match: *\r\n",
    "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "\r\n"
  )
  .as_bytes())
  .expect("request should retain metadata");

  assert_eq!(Some(HttpByteRange::new(6, 9)), request.range(10).expect("Range should parse"));
  assert!(matches!(request.if_range(), Ok(Some(HttpIfRange::Date(_)))));
  assert_eq!(Ok(Some(HttpIfNoneMatch::Any)), request.if_none_match());
  assert!(matches!(request.if_modified_since(), Ok(Some(_))));
}

  #[test]
  fn http2_huffman_decode_table_resolves_symbols_without_linear_scan() {
    let table = http2_huffman_decode_table();

    assert_eq!(Some(b'0' as u16), table.decode_symbol(0x00, 5));
    assert_eq!(Some(b'.' as u16), table.decode_symbol(0x17, 6));
    assert_eq!(Some(b'/' as u16), table.decode_symbol(0x18, 6));
    assert_eq!(
      Some(HTTP2_HUFFMAN_EOS),
      table.decode_symbol(0x3fff_ffff, 30)
    );
    assert_eq!(None, table.decode_symbol(0x00, 1));
  }

  #[test]
  fn http2_window_update_rejects_zero_increment() {
    let error = http2_window_update_increment(&[0, 0, 0, 0])
      .expect_err("zero WINDOW_UPDATE increment must be rejected");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid HTTP/2 WINDOW_UPDATE frame", error.to_string());
  }

  #[test]
  fn http2_send_window_rejects_increment_above_maximum() {
    let mut window = Http2SendWindow::new(1);

    let error = window
      .increase(0x7fff_ffff)
      .expect_err("WINDOW_UPDATE must not exceed the HTTP/2 maximum window");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("HTTP/2 flow-control window overflow", error.to_string());
    assert_eq!(1, window.size);
  }

  #[test]
  fn response_flow_control_reads_update_accepted_stream_tracking() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
    let mut client =
      StdTcpStream::connect(listener.local_addr().expect("listener addr")).expect("client connect");
    let (mut server, _) = listener.accept().expect("server accept");
    let header_block = [0x82, 0x84, 0x86];

    write_http2_frame(
      &mut client,
      HTTP2_FRAME_HEADERS,
      HTTP2_FLAG_END_HEADERS | HTTP2_FLAG_END_STREAM,
      3,
      &header_block,
    )
    .expect("client HEADERS frame should write");
    write_http2_window_update(&mut client, 1, 1).expect("client WINDOW_UPDATE should write");
    client.flush().expect("client frames should flush");

    let mut max_frame_size = HTTP2_DEFAULT_MAX_FRAME_SIZE;
    let mut peer_header_table_size = HTTP2_DEFAULT_HEADER_TABLE_SIZE;
    let mut peer_initial_stream_send_window = HTTP2_DEFAULT_INITIAL_WINDOW_SIZE;
    let mut connection_send_window = Http2SendWindow::new(0);
    let mut connection_receive_window = HTTP2_DEFAULT_INITIAL_WINDOW_SIZE;
    let mut stream_send_window = Http2SendWindow::new(0);
    let mut streams = Vec::new();
    let mut reset_streams = Vec::new();
    let mut stream_ids = Http2ClientStreamIds::new();
    let mut request_header_decoder = Http2HeaderDecoder::new(HTTP2_DEFAULT_HEADER_TABLE_SIZE);
    let mut accepted_stream_count = 1;
    let mut last_accepted_stream_id = 1;
    let mut peer_enable_connect_protocol = false;

    let read = {
      let mut flow_control = Http2ResponseFlowControl {
        max_inbound_frame_size: HTTP2_DEFAULT_MAX_FRAME_SIZE,
        max_header_list_size: HTTP2_MAX_HEADER_LIST_SIZE,
        max_frame_size: &mut max_frame_size,
        peer_header_table_size: &mut peer_header_table_size,
        peer_initial_stream_send_window: &mut peer_initial_stream_send_window,
        peer_enable_connect_protocol: &mut peer_enable_connect_protocol,
        connection_send_window: &mut connection_send_window,
        connection_receive_window: &mut connection_receive_window,
        stream_send_window: &mut stream_send_window,
        streams: &mut streams,
        reset_streams: &mut reset_streams,
        stream_ids: &mut stream_ids,
        request_header_decoder: &mut request_header_decoder,
        accepted_stream_count: &mut accepted_stream_count,
        last_accepted_stream_id: &mut last_accepted_stream_id,
      };
      read_http2_response_flow_control_frame(&mut server, 1, &mut flow_control)
        .expect("flow-control read should accept request frames")
    };

    assert!(matches!(
      read,
      Http2ResponseFlowControlRead::WindowAvailable
    ));
    assert_eq!(2, accepted_stream_count);
    assert_eq!(3, last_accepted_stream_id);
    assert_eq!(1, streams.len());
    assert_eq!(3, streams[0].stream_id);
    assert!(streams[0].is_complete());
  }

  #[test]
  fn read_next_from_consumes_one_fully_framed_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello",
      "POST /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "world"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let second = Request::read_next_from(&mut reader)
      .expect("second frame should parse")
      .expect("second request should be present");

    assert_eq!("POST", first.method());
    assert_eq!("/first", first.target());
    assert_eq!(b"hello", first.body());
    assert_eq!("POST", second.method());
    assert_eq!("/second", second.target());
    assert_eq!(b"world", second.body());
    assert!(reader.fill_buf().expect("remaining bytes").is_empty());
  }

  #[test]
  fn read_next_from_rejects_conflicting_duplicate_content_length() {
    let raw = concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 6\r\n",
      "\r\n",
      "hello!"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("conflicting Content-Length should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("conflicting Content-Length headers", error.to_string());
  }

  #[test]
  fn read_next_from_accepts_duplicate_matching_content_length() {
    let raw = concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 5\r\n",
      "Content-Length: 5\r\n",
      "\r\n",
      "hello"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("matching duplicate Content-Length should parse")
      .expect("request should be present");

    assert_eq!("POST", request.method());
    assert_eq!("/upload", request.target());
    assert_eq!(b"hello", request.body());
  }

  #[test]
  fn read_next_from_enforces_request_body_framing_conflict_matrix() {
    let cases = [
      (
        "matching duplicate Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Content-Length: 5\r\n",
          "Content-Length: 5\r\n",
          "\r\n",
          "hello"
        )
        .as_bytes(),
        Ok(b"hello".as_slice()),
      ),
      (
        "conflicting duplicate Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Content-Length: 5\r\n",
          "Content-Length: 6\r\n",
          "\r\n",
          "hello!"
        )
        .as_bytes(),
        Err("conflicting Content-Length headers"),
      ),
      (
        "Transfer-Encoding with Content-Length",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "Content-Length: 0\r\n",
          "\r\n",
          "0\r\n\r\n"
        )
        .as_bytes(),
        Err("Transfer-Encoding conflicts with Content-Length"),
      ),
      (
        "empty chunked body",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "\r\n",
          "0\r\n\r\n"
        )
        .as_bytes(),
        Ok(b"".as_slice()),
      ),
      (
        "malformed chunk terminator",
        concat!(
          "POST /upload HTTP/1.1\r\n",
          "Host: example.test\r\n",
          "Transfer-Encoding: chunked\r\n",
          "\r\n",
          "5\r\nhelloXX0\r\n\r\n"
        )
        .as_bytes(),
        Err("invalid chunk terminator"),
      ),
    ];

    for (name, raw, expected) in cases {
      let mut reader = BufReader::new(Cursor::new(raw));
      match expected {
        Ok(body) => {
          let request = Request::read_next_from(&mut reader)
            .unwrap_or_else(|error| panic!("{name} should parse: {error}"))
            .unwrap_or_else(|| panic!("{name} should include a request"));
          assert_eq!(body, request.body(), "{name}");
        }
        Err(message) => {
          let error = Request::read_next_from(&mut reader)
            .expect_err("{name} should be rejected");
          assert_eq!(io::ErrorKind::InvalidData, error.kind(), "{name}");
          assert_eq!(message, error.to_string(), "{name}");
        }
      }
    }
  }

  #[test]
  fn read_next_from_consumes_one_chunked_request_at_a_time() {
    let raw = concat!(
      "POST /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n",
      "\r\n",
      "GET /second HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let second = Request::read_next_from(&mut reader)
      .expect("second frame should parse")
      .expect("second request should be present");

    assert_eq!("POST", first.method());
    assert_eq!("/first", first.target());
    assert_eq!(b"hello", first.body());
    assert_eq!("GET", second.method());
    assert_eq!("/second", second.target());
    assert!(reader.fill_buf().expect("remaining bytes").is_empty());
  }

  #[test]
  fn read_next_from_accepts_obs_text_in_quoted_chunk_extensions() {
    let raw = b"POST /chunked HTTP/1.1\r\n\
Host: example.test\r\n\
Transfer-Encoding: chunked\r\n\
\r\n\
5;meta=\"\xff\"\r\n\
hello\r\n\
0\r\n\
\r\n";
    let mut reader = BufReader::new(Cursor::new(raw));

    let request = Request::read_next_from(&mut reader)
      .expect("chunk extension with obs-text should parse")
      .expect("request should be present");

    assert_eq!("/chunked", request.target());
    assert_eq!(b"hello", request.body());
  }

  #[test]
  fn read_next_from_rejects_invalid_chunk_size_characters() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5G\r\nhello\r\n",
      "0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("invalid chunk size should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid chunk size", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_oversized_chunk_size_line() {
    let chunk_size = "1".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "{}\r\n",
        "x\r\n",
        "0\r\n",
        "\r\n"
      ),
      chunk_size
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("oversized chunk size line should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_missing_crlf_after_chunk_data() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello",
      "0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("missing chunk data terminator should fail");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid chunk terminator", error.to_string());
  }

  #[test]
  fn read_next_from_rejects_malformed_trailer_termination() {
    let raw = concat!(
      "POST /chunked HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n",
      "0\r\n",
      "X-Trace: abc\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error =
      Request::read_next_from(&mut reader).expect_err("missing trailer terminator should fail");

    assert_eq!(io::ErrorKind::UnexpectedEof, error.kind());
    assert_eq!("incomplete chunked request body", error.to_string());
  }

  #[test]
  fn connection_close_request_marks_keep_alive_loop_terminal() {
    let raw = concat!(
      "POST /final HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Connection: close\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "done",
      "GET /ignored HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("request frame should parse")
      .expect("request should be present");

    assert_eq!("/final", request.target());
    assert_eq!(b"done", request.body());
    assert!(request.closes_connection());
    assert!(reader
      .fill_buf()
      .expect("remaining bytes")
      .starts_with(b"GET /ignored"));
  }

  #[test]
  fn partial_second_request_returns_unexpected_eof_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "POST /partial HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Content-Length: 4\r\n",
      "\r\n",
      "he"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::UnexpectedEof, error.kind());
    assert_eq!("incomplete HTTP request body", error.to_string());
  }

  #[test]
  fn chunk_extension_bytes_count_toward_request_body_limit() {
    let chunk_extension = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0;{}\r\n",
        "\r\n"
      ),
      chunk_extension
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk extension should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn chunk_trailer_bytes_count_toward_request_body_limit() {
    let trailer_value = "a".repeat(MAX_REQUEST_BODY_BYTES);
    let raw = format!(
      concat!(
        "POST /chunked HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Transfer-Encoding: chunked\r\n",
        "\r\n",
        "0\r\n",
        "X-Trace: {}\r\n",
        "\r\n"
      ),
      trailer_value
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let error = Request::read_next_from(&mut reader).expect_err("chunk trailer should be capped");

    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("request body is too large", error.to_string());
  }

  #[test]
  fn malformed_second_request_returns_invalid_data_after_first_frame() {
    let raw = concat!(
      "GET /first HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "\r\n",
      "GET /broken HTTP/1.1\r\n",
      "Host example.test\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let first = Request::read_next_from(&mut reader)
      .expect("first frame should parse")
      .expect("first request should be present");
    let error = Request::read_next_from(&mut reader).expect_err("second frame should fail");

    assert_eq!("/first", first.target());
    assert_eq!(io::ErrorKind::InvalidData, error.kind());
    assert_eq!("invalid request header", error.to_string());
  }

  #[test]
  fn priority_helpers_parse_requests_and_build_responses() {
    let raw = concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Priority: u=1, i, x=token\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");
    let priority = request
      .priority()
      .expect("Priority should parse")
      .expect("Priority should be present");

    assert_eq!(Some(1), priority.urgency());
    assert!(priority.incremental());
    assert_eq!(Some("token"), priority.extensions()[0].value());

    let response = HttpResponse::ok([])
      .with_priority("u=1, i, x=token")
      .expect("Priority should be accepted");
    assert_eq!(
      "u=1, i, x=token",
      response
        .priority()
        .expect("response Priority should parse")
        .expect("response Priority should be present")
        .header_value()
    );
  }

  #[test]
  fn authentication_helpers_parse_bounded_metadata_without_authentication_policy() {
    let raw = concat!(
      "GET / HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Authorization: Bearer origin-token\r\n",
      "Proxy-Authorization: Basic cHJveHk6c2VjcmV0\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");

    assert_eq!(
      "Bearer",
      request
        .authorization()
        .expect("Authorization should parse")
        .expect("Authorization should be present")
        .scheme()
    );
    let proxy_authorization = request
      .proxy_authorization()
      .expect("Proxy-Authorization should parse")
      .expect("Proxy-Authorization should be present");
    assert_eq!("Basic", proxy_authorization.scheme());
    assert_eq!("cHJveHk6c2VjcmV0", proxy_authorization.credentials());

    let response = HttpResponse::new(401, "Unauthorized")
      .header("WWW-Authenticate", "Broken")
      .with_www_authenticate("Basic realm=\"private\", Bearer")
      .expect("WWW-Authenticate should be accepted");
    let challenges = response
      .www_authenticate()
      .expect("WWW-Authenticate should parse")
      .expect("WWW-Authenticate should be present");
    assert_eq!(2, challenges.challenges().len());
    assert_eq!("Basic", challenges.challenges()[0].scheme());
    assert_eq!("Bearer", challenges.challenges()[1].scheme());
    assert!(HttpResponse::ok([])
      .with_www_authenticate("Basic @")
      .is_err());
    let malformed = HttpRequest::parse(
      concat!(
        "GET / HTTP/1.1\r\n",
        "Host: example.test\r\n",
        "Proxy-Authorization: invalid\r\n",
        "\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    assert!(malformed.proxy_authorization().is_err());
    assert_eq!(
      Some("invalid"),
      malformed.header("Proxy-Authorization")
    );
  }

  #[test]
  fn alt_svc_helpers_validate_build_and_parse_response_metadata() {
    let response = HttpResponse::ok([])
      .with_alt_svc("h3=\":443\"; ma=3600; persist=1; region=\"us-east\"")
      .expect("Alt-Svc should be accepted");
    let alt_svc = response
      .alt_svc()
      .expect("response Alt-Svc should parse")
      .expect("response Alt-Svc should be present");

    assert_eq!("h3", alt_svc.alternatives()[0].protocol_id());
    assert_eq!(":443", alt_svc.alternatives()[0].authority());
    assert_eq!(Some(3600), alt_svc.alternatives()[0].max_age());
    assert_eq!(Some(true), alt_svc.alternatives()[0].persist());
    assert_eq!(
      "h3=\":443\"; ma=3600; persist=1; region=us-east",
      alt_svc.header_value()
    );

    let clear = HttpResponse::ok([])
      .with_alt_svc("clear")
      .expect("clear should be accepted");
    assert!(clear
      .alt_svc()
      .expect("clear should parse")
      .expect("clear should be present")
      .is_clear());
  }
