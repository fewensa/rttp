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
fn request_access_control_request_private_network_parses_preflight_metadata_without_policy() {
  let absent_raw = "OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .access_control_request_private_network()
      .expect("missing Access-Control-Request-Private-Network should be accepted")
  );

  let valid_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: true\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    "true",
    valid
      .access_control_request_private_network()
      .expect("Access-Control-Request-Private-Network should parse")
      .expect("Access-Control-Request-Private-Network should be present")
      .header_value()
  );

  let malformed_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: false\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.access_control_request_private_network().is_err());
  assert_eq!(
    Some("false"),
    malformed.header("Access-Control-Request-Private-Network")
  );

  let duplicate_raw = concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: true\r\n",
    "access-control-request-private-network: true\r\n",
    "\r\n"
  );
  let mut duplicate_reader = BufReader::new(Cursor::new(duplicate_raw.as_bytes()));
  let duplicate = Request::read_next_from(&mut duplicate_reader)
    .expect("duplicate metadata should not reject the request frame")
    .expect("duplicate request should be present");
  assert!(duplicate.access_control_request_private_network().is_err());
  assert_eq!(
    Some("true"),
    duplicate.header("Access-Control-Request-Private-Network")
  );
}

#[test]
fn request_te_parses_bounded_codings_without_policy() {
  let absent_raw = "GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent.te().expect("missing TE should be accepted")
  );

  let valid_raw = concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "TE: gzip, deflate;q=0.5, trailers\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  let te = valid
    .te()
    .expect("TE should parse")
    .expect("TE should be present");
  assert_eq!(3, te.len());
  assert_eq!("gzip", te.codings()[0].coding());
  assert_eq!(Some(1000), te.codings()[0].quality());
  assert_eq!("deflate", te.codings()[1].coding());
  assert_eq!(Some(500), te.codings()[1].quality());
  assert_eq!("trailers", te.codings()[2].coding());
  assert_eq!(None, te.codings()[2].quality());
  assert!(te.codings()[2].is_trailers());
}

#[test]
fn request_te_rejects_malformed_or_duplicate_values_while_preserving_raw_headers() {
  for value in ["gzip;q=1.1", "trailers,, deflate", "trailers;q=0.5", "chunked"] {
    let request = Request::from_raw_frame(
      format!(
        "GET /asset HTTP/1.1\r\nHost: example.test\r\nTE: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("malformed TE should not reject the request frame");
    assert!(request.te().is_err(), "TE should reject {value:?}");
    assert_eq!(Some(value), request.header("TE"));
  }

  let duplicate = Request::from_raw_frame(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nTE: trailers\r\nte: TRAILERS;q=0.5\r\n\r\n",
  )
  .expect("duplicate TE should not reject the request frame");
  assert!(duplicate.te().is_err());
}

#[test]
fn request_te_enforces_member_and_value_bounds() {
  let at_limit = (0..32)
    .map(|index| format!("coding-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let at_limit_request = Request::from_raw_frame(
    format!(
      "GET /asset HTTP/1.1\r\nHost: example.test\r\nTE: {at_limit}\r\n\r\n"
    )
    .as_bytes(),
  )
  .expect("32 codings should not reject the request frame");
  assert_eq!(
    32,
    at_limit_request
      .te()
      .expect("TE should parse")
      .expect("TE should be present")
      .len()
  );

  let too_many = (0..=32)
    .map(|index| format!("coding-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_request = Request::from_raw_frame(
    format!(
      "GET /asset HTTP/1.1\r\nHost: example.test\r\nTE: {too_many}\r\n\r\n"
    )
    .as_bytes(),
  )
  .expect("33 codings should not reject the request frame");
  assert!(too_many_request.te().is_err());

  let oversized_value = "x".repeat(64 * 1024 + 1);
  assert!(
    HttpRequestTe::parse(&oversized_value).is_err(),
    "oversized TE values must be rejected"
  );
  assert!(
    HttpRequestTe::parse_values(["gzip", oversized_value.as_str()]).is_err(),
    "an oversized duplicate field must not bypass validation"
  );
}

#[test]
fn request_save_data_parses_request_metadata_without_policy() {
  let absent_raw = "GET /catalog HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .save_data()
      .expect("missing Save-Data should be accepted")
  );
  assert_eq!(None, absent.header("Save-Data"));

  let valid_raw = concat!(
    "GET /catalog HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Save-Data: on\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    "on",
    valid
      .save_data()
      .expect("Save-Data should parse")
      .expect("Save-Data should be present")
      .header_value()
  );

  let malformed_raw = concat!(
    "GET /catalog HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Save-Data: ?1\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.save_data().is_err());
  assert_eq!(Some("?1"), malformed.header("Save-Data"));

  let duplicate_raw = concat!(
    "GET /catalog HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Save-Data: on\r\n",
    "save-data: on\r\n",
    "\r\n"
  );
  let mut duplicate_reader = BufReader::new(Cursor::new(duplicate_raw.as_bytes()));
  let duplicate = Request::read_next_from(&mut duplicate_reader)
    .expect("duplicate metadata should not reject the request frame")
    .expect("duplicate request should be present");
  assert!(duplicate.save_data().is_err());
  assert_eq!(Some("on"), duplicate.header("Save-Data"));
}

#[test]
fn request_sec_gpc_parses_request_metadata_without_policy() {
  let absent_raw = "GET /privacy HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(None, absent.sec_gpc().expect("missing Sec-GPC should be accepted"));
  assert_eq!(None, absent.header("Sec-GPC"));

  let valid_raw = concat!(
    "GET /privacy HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Sec-GPC: 1\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    "1",
    valid
      .sec_gpc()
      .expect("Sec-GPC should parse")
      .expect("Sec-GPC should be present")
      .header_value()
  );

  let malformed_raw = concat!(
    "GET /privacy HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Sec-GPC: 0\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.sec_gpc().is_err());
  assert_eq!(Some("0"), malformed.header("Sec-GPC"));

  let duplicate_raw = concat!(
    "GET /privacy HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Sec-GPC: 1\r\n",
    "sec-gpc: 1\r\n",
    "\r\n"
  );
  let mut duplicate_reader = BufReader::new(Cursor::new(duplicate_raw.as_bytes()));
  let duplicate = Request::read_next_from(&mut duplicate_reader)
    .expect("duplicate metadata should not reject the request frame")
    .expect("duplicate request should be present");
  assert!(duplicate.sec_gpc().is_err());
  assert_eq!(Some("1"), duplicate.header("Sec-GPC"));

  let oversized_value = "1".repeat(rttp_protocol::sec_gpc::MAX_SEC_GPC_VALUE_BYTES + 1);
  let oversized = HttpRequest {
    method: "GET".to_string(),
    path: "/privacy".to_string(),
    query: None,
    version: "HTTP/1.1".to_string(),
    headers: vec![HttpHeader::new("Sec-GPC", oversized_value)],
    body: Vec::new(),
    content_length: None,
  };
  assert!(oversized.sec_gpc().is_err());
  assert!(oversized.header("Sec-GPC").is_some());
}

#[test]
fn request_pragma_parses_request_metadata_without_policy() {
  let absent_raw = "GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .pragma()
      .expect("missing Pragma should be accepted")
  );
  assert_eq!(None, absent.header("Pragma"));

  let valid_raw = concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Pragma: no-cache\r\n",
    "Pragma: community=private, example=\"quoted, value\"\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  let pragma = valid
    .pragma()
    .expect("Pragma should parse")
    .expect("Pragma should be present");
  assert!(pragma.no_cache());
  assert_eq!(2, pragma.extensions().len());
  assert_eq!("community", pragma.extensions()[0].name());
  assert_eq!(Some("private"), pragma.extensions()[0].value());
  assert_eq!(
    "no-cache, community=private, example=\"quoted, value\"",
    pragma.header_value()
  );

  let malformed_raw = concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Pragma: no-cache=\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.pragma().is_err());
  assert_eq!(Some("no-cache="), malformed.header("Pragma"));

  let duplicate_raw = concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Pragma: no-cache\r\n",
    "pragma: no-cache\r\n",
    "\r\n"
  );
  let mut duplicate_reader = BufReader::new(Cursor::new(duplicate_raw.as_bytes()));
  let duplicate = Request::read_next_from(&mut duplicate_reader)
    .expect("duplicate metadata should not reject the request frame")
    .expect("duplicate request should be present");
  assert!(duplicate.pragma().is_err());
  assert_eq!(Some("no-cache"), duplicate.header("Pragma"));

  let oversized_value = "x".repeat(rttp_protocol::pragma::MAX_PRAGMA_VALUE_BYTES + 1);
  let oversized = HttpRequest {
    method: "GET".to_string(),
    path: "/asset".to_string(),
    query: None,
    version: "HTTP/1.1".to_string(),
    headers: vec![HttpHeader::new("Pragma", oversized_value)],
    body: Vec::new(),
    content_length: None,
  };
  assert!(oversized.pragma().is_err());
  assert!(oversized.header("Pragma").is_some());

  let first = "a".repeat(rttp_protocol::pragma::MAX_PRAGMA_VALUE_BYTES / 2);
  let second = "b".repeat(rttp_protocol::pragma::MAX_PRAGMA_VALUE_BYTES / 2);
  let combined = HttpRequest {
    method: "GET".to_string(),
    path: "/asset".to_string(),
    query: None,
    version: "HTTP/1.1".to_string(),
    headers: vec![
      HttpHeader::new("Pragma", first),
      HttpHeader::new("Pragma", second),
    ],
    body: Vec::new(),
    content_length: None,
  };
  assert!(combined.pragma().is_err());
  assert!(combined.header("Pragma").is_some());
}

#[test]
fn request_upgrade_insecure_requests_parses_request_metadata_without_policy() {
  let absent_raw = "GET /page HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .upgrade_insecure_requests()
      .expect("missing Upgrade-Insecure-Requests should be accepted")
  );
  assert_eq!(None, absent.header("Upgrade-Insecure-Requests"));

  let valid_raw = concat!(
    "GET /page HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Upgrade-Insecure-Requests: 1\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  assert_eq!(
    "1",
    valid
      .upgrade_insecure_requests()
      .expect("Upgrade-Insecure-Requests should parse")
      .expect("Upgrade-Insecure-Requests should be present")
      .header_value()
  );

  let malformed_raw = concat!(
    "GET /page HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Upgrade-Insecure-Requests: 0\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.upgrade_insecure_requests().is_err());
  assert_eq!(Some("0"), malformed.header("Upgrade-Insecure-Requests"));

  let duplicate_raw = concat!(
    "GET /page HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Upgrade-Insecure-Requests: 1\r\n",
    "upgrade-insecure-requests: 1\r\n",
    "\r\n"
  );
  let mut duplicate_reader = BufReader::new(Cursor::new(duplicate_raw.as_bytes()));
  let duplicate = Request::read_next_from(&mut duplicate_reader)
    .expect("duplicate metadata should not reject the request frame")
    .expect("duplicate request should be present");
  assert!(duplicate.upgrade_insecure_requests().is_err());
  assert_eq!(Some("1"), duplicate.header("Upgrade-Insecure-Requests"));

  let oversized = "1".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "GET".to_string(),
    target: "/page".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Upgrade-Insecure-Requests".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.upgrade_insecure_requests().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("Upgrade-Insecure-Requests")
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
    "Accept-Charset: utf-8\r\n",
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

  let accept_charset = valid
    .accept_charset()
    .expect("Accept-Charset should parse")
    .expect("Accept-Charset should be present");
  assert_eq!("utf-8", accept_charset.charsets()[0].charset());
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
fn request_accept_charset_parses_ranges_without_selecting_a_charset() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Charset: utf-8, iso-8859-1;q=0.5\r\n",
    "accept-charset: *; q=0\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("request should parse");

  let charsets = request
    .accept_charset()
    .expect("Accept-Charset should parse")
    .expect("Accept-Charset should be present");
  assert_eq!(3, charsets.len());
  assert_eq!("utf-8", charsets.charsets()[0].charset());
  assert_eq!(1000, charsets.charsets()[0].quality());
  assert_eq!("iso-8859-1", charsets.charsets()[1].charset());
  assert_eq!(500, charsets.charsets()[1].quality());
  assert_eq!("*", charsets.charsets()[2].charset());
  assert_eq!(0, charsets.charsets()[2].quality());
  assert_eq!(b"body", request.body());
}

#[test]
fn request_accept_charset_preserves_absent_and_malformed_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .accept_charset()
      .expect("absent Accept-Charset should be accepted")
  );

  let malformed = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Charset: utf-8, UTF-8\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("malformed Accept-Charset should not reject raw request parsing");
  assert_eq!(Some("utf-8, UTF-8"), malformed.header("Accept-Charset"));
  assert!(malformed.accept_charset().is_err());
  assert_eq!(b"body", malformed.body());
}

#[test]
fn request_want_content_digest_parses_preferences_without_selecting_an_algorithm() {
  let request = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Content-Digest: sha-256=10, sha-512=3\r\n",
    "want-content-digest: unixsum=0\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("request should parse");

  let digest = request
    .want_content_digest()
    .expect("Want-Content-Digest should parse")
    .expect("Want-Content-Digest should be present");
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
fn request_want_content_digest_preserves_absent_and_malformed_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .want_content_digest()
      .expect("absent Want-Content-Digest should be accepted")
  );

  let malformed = Request::from_raw_frame(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Content-Digest: sha-256\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ).as_bytes())
  .expect("malformed Want-Content-Digest should not reject the request frame");
  assert!(malformed.want_content_digest().is_err());
  assert_eq!(Some("sha-256"), malformed.header("Want-Content-Digest"));
  assert_eq!(b"body", malformed.body());
}

#[test]
fn request_connection_exposes_retained_http1_tokens() {
  let absent_raw = "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .connection()
      .expect("missing Connection should be accepted")
  );

  let valid_raw = concat!(
    "GET /download HTTP/1.1\r\n",
    "Host: files.example.test\r\n",
    "Connection: close\r\n",
    "\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("valid request should parse")
    .expect("valid request should be present");
  let connection = valid
    .connection()
    .expect("Connection should parse")
    .expect("Connection should be present");
  assert_eq!(vec!["close"], connection.tokens());
  assert_eq!("close", connection.header_value());
  assert_eq!(Some("close"), valid.header("Connection"));

  let malformed_raw = concat!(
    "GET / HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Connection: close,\r\n",
    "\r\n"
  );
  let mut malformed_reader = BufReader::new(Cursor::new(malformed_raw.as_bytes()));
  let malformed = Request::read_next_from(&mut malformed_reader)
    .expect("malformed metadata should not reject the request frame")
    .expect("malformed request should be present");
  assert!(malformed.connection().is_err());
  assert_eq!(Some("close,"), malformed.header("Connection"));
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
fn request_transfer_encoding_exposes_validated_chunked_framing() {
  let absent_raw = "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n";
  let mut absent_reader = BufReader::new(Cursor::new(absent_raw.as_bytes()));
  let absent = Request::read_next_from(&mut absent_reader)
    .expect("absent request should parse")
    .expect("absent request should be present");
  assert_eq!(
    None,
    absent
      .transfer_encoding()
      .expect("missing Transfer-Encoding should be accepted")
  );

  let valid_raw = concat!(
    "POST /upload HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Transfer-Encoding: chunked\r\n",
    "\r\n",
    "5\r\nhello\r\n",
    "0\r\n\r\n"
  );
  let mut valid_reader = BufReader::new(Cursor::new(valid_raw.as_bytes()));
  let valid = Request::read_next_from(&mut valid_reader)
    .expect("chunked request framing should parse")
    .expect("chunked request should be present");
  let transfer_encoding = valid
    .transfer_encoding()
    .expect("Transfer-Encoding should parse")
    .expect("Transfer-Encoding should be present");
  assert_eq!(vec!["chunked"], transfer_encoding.codings());
  assert_eq!("chunked", transfer_encoding.header_value());
  assert_eq!(Some("chunked"), valid.header("Transfer-Encoding"));
  assert_eq!(b"hello", valid.body());
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
  let duplicate_encodings = duplicate_members
    .content_encoding()
    .expect("repeated Content-Encoding should parse")
    .expect("Content-Encoding should be present");
  assert_eq!(vec!["gzip", "GZIP"], duplicate_encodings.codings());
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
    (0..257)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let too_many_codings = (0..257)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_languages = (0..257)
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
    content_length: None,
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
    content_length: None,
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
    content_length: None,
    extended_connect_protocol: None,
  };

  let error = request
    .cache_control()
    .expect_err("too many Cache-Control directives should be rejected");

  assert_eq!("too many Cache-Control directives", error.to_string());
}

#[test]
fn etag_response_helpers_validate_replace_and_parse_singleton_metadata() {
  assert_eq!(
    None,
    HttpResponse::ok([])
      .etag()
      .expect("absent ETag should parse")
  );

  let response = HttpResponse::ok([])
    .header("ETag", "\"old\"")
    .header("etag", "W/\"older\"")
    .with_etag(HttpEntityTag::weak("asset-v7"));
  assert_eq!(
    Some(HttpEntityTag::weak("asset-v7")),
    response.etag().expect("ETag should parse")
  );
  assert_eq!(
    vec![("ETag", "W/\"asset-v7\"")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let strong = HttpResponse::ok([]).header("ETag", "\"asset-v7\"");
  assert_eq!(
    Some(HttpEntityTag::strong("asset-v7")),
    strong.etag().expect("strong ETag should parse")
  );
}

#[test]
fn etag_response_helper_rejects_malformed_duplicate_and_oversized_raw_headers() {
  for value in ["abc", "W/abc", "\"bad space\"", "\"bad\"value\""] {
    let response = HttpResponse::ok([]).header("ETag", value);
    assert!(response.etag().is_err(), "ETag should reject {value:?}");
    assert_eq!(
      vec![("ETag", value)],
      response
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("ETag", "\"one\"")
    .header("etag", "W/\"two\"");
  assert!(duplicate.etag().is_err());
  assert_eq!(
    vec![("ETag", "\"one\""), ("etag", "W/\"two\"")],
    duplicate
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let oversized = format!("\"{}\"", "a".repeat(64 * 1024));
  let response = HttpResponse::ok([]).header("ETag", &oversized);
  assert!(response.etag().is_err());
  assert_eq!(
    vec![("ETag", oversized.as_str())],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn service_worker_allowed_response_helpers_validate_replace_and_parse_singleton_metadata() {
  assert_eq!(
    None,
    HttpResponse::ok([])
      .service_worker_allowed()
      .expect("absent Service-Worker-Allowed should parse")
  );

  let response = HttpResponse::ok([])
    .header("Service-Worker-Allowed", "/old")
    .header("service-worker-allowed", "/older")
    .with_service_worker_allowed(" / ")
    .expect("valid Service-Worker-Allowed should be accepted");
  assert_eq!(
    "/",
    response
      .service_worker_allowed()
      .expect("Service-Worker-Allowed should parse")
      .expect("Service-Worker-Allowed should be present")
      .as_str()
  );
  assert_eq!(
    vec![("Service-Worker-Allowed", "/")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let attached = HttpResponse::ok([]).header("Service-Worker-Allowed", "../scope");
  assert_eq!(
    "../scope",
    attached
      .service_worker_allowed()
      .expect("Service-Worker-Allowed should parse")
      .expect("Service-Worker-Allowed should be present")
      .as_str()
  );
}

#[test]
fn service_worker_allowed_response_helper_rejects_malformed_duplicate_and_oversized_raw_headers() {
  for value in [
    "",
    " ",
    "/bad path",
    "/bad%zz",
    "http://example.test/scope",
    "//example.test/scope",
    "/safe\u{7f}",
  ] {
    assert!(
      HttpResponse::ok([])
        .with_service_worker_allowed(value)
        .is_err(),
      "Service-Worker-Allowed helper should reject {value:?}"
    );

    let response = HttpResponse::ok([]).header("Service-Worker-Allowed", value);
    assert!(
      response.service_worker_allowed().is_err(),
      "Service-Worker-Allowed parser should reject {value:?}"
    );
    assert_eq!(
      vec![("Service-Worker-Allowed", value)],
      response
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Service-Worker-Allowed", "/")
    .header("service-worker-allowed", "/app/");
  assert!(duplicate.service_worker_allowed().is_err());
  assert_eq!(
    vec![
      ("Service-Worker-Allowed", "/"),
      ("service-worker-allowed", "/app/")
    ],
    duplicate
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  assert!(
    HttpResponse::ok([])
      .with_service_worker_allowed(&oversized)
      .is_err(),
    "Service-Worker-Allowed helper should reject oversized values"
  );
  let response = HttpResponse::ok([]).header("Service-Worker-Allowed", &oversized);
  assert!(response.service_worker_allowed().is_err());
  assert_eq!(
    vec![("Service-Worker-Allowed", oversized.as_str())],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
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
fn access_control_allow_credentials_helpers_validate_replace_and_preserve_raw_metadata() {
  let response = HttpResponse::ok([])
    .header("Access-Control-Allow-Credentials", "false")
    .header("access-control-allow-credentials", "false")
    .with_access_control_allow_credentials("true")
    .expect("Access-Control-Allow-Credentials should be accepted");

  assert_eq!(
    "true",
    response
      .access_control_allow_credentials()
      .expect("Access-Control-Allow-Credentials should parse")
      .expect("Access-Control-Allow-Credentials should be present")
      .header_value()
  );
  assert_eq!(
    vec![("Access-Control-Allow-Credentials", "true")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let malformed = HttpResponse::ok([]).header("Access-Control-Allow-Credentials", "false");
  assert!(malformed.access_control_allow_credentials().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_credentials("false")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .access_control_allow_credentials()
      .expect("absent Access-Control-Allow-Credentials should parse")
  );
  for value in ["true", " true "] {
    assert_eq!(
      "true",
      HttpResponse::ok([])
        .with_access_control_allow_credentials(value)
        .expect("valid Access-Control-Allow-Credentials should be accepted")
        .access_control_allow_credentials()
        .expect("Access-Control-Allow-Credentials should parse")
        .expect("Access-Control-Allow-Credentials should be present")
        .header_value()
    );
  }
  assert!(HttpResponse::ok([])
    .with_access_control_allow_credentials("TRUE")
    .is_err());

  let duplicate = HttpResponse::ok([])
    .header("Access-Control-Allow-Credentials", "true")
    .header("access-control-allow-credentials", "true");
  assert!(duplicate.access_control_allow_credentials().is_err());
  assert!(HttpResponse::ok([])
    .with_access_control_allow_credentials("x".repeat(64 * 1024 + 1))
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
fn content_security_policy_report_only_helpers_preserve_order_and_raw_headers() {
  let raw = HttpResponse::ok([])
    .header("Content-Security-Policy-Report-Only", "default-src 'self'")
    .header("content-security-policy-report-only", "object-src 'none'");

  let metadata = raw
    .content_security_policy_report_only()
    .expect("Content-Security-Policy-Report-Only should parse")
    .expect("Content-Security-Policy-Report-Only should be present");

  assert_eq!(metadata.as_str(), "default-src 'self'");
  assert_eq!(
    metadata.header_values(),
    ["default-src 'self'", "object-src 'none'"]
  );
  assert_eq!(
    Some("default-src 'self'"),
    raw
      .headers
      .iter()
      .find(|header| {
        header
          .name
          .eq_ignore_ascii_case("Content-Security-Policy-Report-Only")
      })
      .map(|header| header.value.as_str())
  );

  let replaced = raw
    .with_content_security_policy_report_only("script-src 'none'")
    .expect("Content-Security-Policy-Report-Only should be accepted");
  assert_eq!(
    ["script-src 'none'"],
    replaced
      .content_security_policy_report_only()
      .expect("Content-Security-Policy-Report-Only should parse")
      .expect("Content-Security-Policy-Report-Only should be present")
      .header_values()
  );
  assert_eq!(
    vec![("Content-Security-Policy-Report-Only", "script-src 'none'")],
    replaced
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let malformed =
    HttpResponse::ok([]).header("Content-Security-Policy-Report-Only", "default-src 'self'\u{7f}");
  assert!(malformed.content_security_policy_report_only().is_err());
  assert_eq!(
    Some("default-src 'self'\u{7f}"),
    malformed
      .headers
      .iter()
      .find(|header| {
        header
          .name
          .eq_ignore_ascii_case("Content-Security-Policy-Report-Only")
      })
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_content_security_policy_report_only("default-src\r\nblocked")
    .is_err());

  let mut too_many = HttpResponse::ok([]);
  for _ in 0..=rttp_protocol::content_security_policy_report_only::MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS {
    too_many = too_many.header("Content-Security-Policy-Report-Only", "default-src 'self'");
  }
  assert!(too_many.content_security_policy_report_only().is_err());
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
fn cross_origin_opener_policy_report_only_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Cross-Origin-Opener-Policy-Report-Only", "unsafe-none")
    .header("cross-origin-opener-policy-report-only", "same-origin")
    .with_cross_origin_opener_policy_report_only(r#"same-origin; report-to="coop""#)
    .expect("Cross-Origin-Opener-Policy-Report-Only should be accepted");

  let policy = response
    .cross_origin_opener_policy_report_only()
    .expect("Cross-Origin-Opener-Policy-Report-Only should parse")
    .expect("Cross-Origin-Opener-Policy-Report-Only should be present");
  assert_eq!(HttpCrossOriginOpenerPolicy::SameOrigin, policy.policy());
  assert_eq!(Some("coop"), policy.report_to());
  assert_eq!(r#"same-origin; report-to="coop""#, policy.header_value());
  assert_eq!(
    vec![(
      "Cross-Origin-Opener-Policy-Report-Only",
      r#"same-origin; report-to="coop""#
    )],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn cross_origin_opener_policy_report_only_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header(
    "Cross-Origin-Opener-Policy-Report-Only",
    r#"noopener-allow-popups; report-to="coop""#,
  );
  let policy = raw
    .cross_origin_opener_policy_report_only()
    .expect("raw report-only COOP should parse")
    .expect("Cross-Origin-Opener-Policy-Report-Only should be present");
  assert_eq!(
    HttpCrossOriginOpenerPolicy::NoopenerAllowPopups,
    policy.policy()
  );
  assert_eq!(Some("coop"), policy.report_to());
  assert_eq!(
    Some(r#"noopener-allow-popups; report-to="coop""#),
    raw
      .headers
      .iter()
      .find(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Opener-Policy-Report-Only")
      })
      .map(|header| header.value.as_str())
  );

  let malformed =
    HttpResponse::ok([]).header("Cross-Origin-Opener-Policy-Report-Only", "same origin");
  assert!(malformed
    .cross_origin_opener_policy_report_only()
    .is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_opener_policy_report_only("same origin")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .cross_origin_opener_policy_report_only()
      .expect("absent Cross-Origin-Opener-Policy-Report-Only should parse")
  );
  for value in [
    "unsafe-none",
    "same-origin-allow-popups",
    "same-origin",
    "noopener-allow-popups",
  ] {
    let metadata = HttpResponse::ok([])
      .with_cross_origin_opener_policy_report_only(value)
      .expect("valid Cross-Origin-Opener-Policy-Report-Only should be accepted")
      .cross_origin_opener_policy_report_only()
      .expect("Cross-Origin-Opener-Policy-Report-Only should parse")
      .expect("Cross-Origin-Opener-Policy-Report-Only should be present");
    assert_eq!(value, metadata.header_value());
    assert_eq!(
      HttpCrossOriginOpenerPolicy::parse(value).expect("canonical COOP should parse"),
      metadata.policy()
    );
  }

  let duplicate = HttpResponse::ok([])
    .header("Cross-Origin-Opener-Policy-Report-Only", "same-origin")
    .header(
      "cross-origin-opener-policy-report-only",
      "noopener-allow-popups",
    );
  assert!(duplicate
    .cross_origin_opener_policy_report_only()
    .is_err());
  assert!(HttpResponse::ok([])
    .with_cross_origin_opener_policy_report_only("x".repeat(64 * 1024 + 1))
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
fn permissions_policy_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Permissions-Policy", "geolocation=(self)")
    .header("permissions-policy", "camera=()")
    .with_permissions_policy(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
    .expect("Permissions-Policy should be accepted");

  let policy = response
    .permissions_policy()
    .expect("Permissions-Policy should parse")
    .expect("Permissions-Policy should be present");
  assert_eq!(
    r#"geolocation=(self "https://maps.example.test"), camera=()"#,
    policy.header_value()
  );
  assert_eq!(2, policy.directives().len());
  assert!(policy.directive("camera").unwrap().allowlist().is_empty());
  assert_eq!(
    vec![("Permissions-Policy", r#"geolocation=(self "https://maps.example.test"), camera=()"#)],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn permissions_policy_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header(
    "Permissions-Policy",
    r#"geolocation=(self "https://maps.example.test");report-to="rp""#,
  );
  let policy = raw
    .permissions_policy()
    .expect("raw Permissions-Policy should parse")
    .expect("Permissions-Policy should be present");
  assert_eq!(
    r#"geolocation=(self "https://maps.example.test")"#,
    policy.header_value()
  );
  assert_eq!(
    Some(r#"geolocation=(self "https://maps.example.test");report-to="rp""#),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Permissions-Policy"))
      .map(|header| header.value.as_str())
  );

  for value in [
    "",
    "geolocation=src",
    "geolocation=('none')",
    "geolocation=*;unknown=1",
    "geolocation=(* \"https://example.test\")",
    "geolocation=5",
  ] {
    let malformed = HttpResponse::ok([]).header("Permissions-Policy", value);
    assert!(malformed.permissions_policy().is_err(), "should reject {value:?}");
    assert!(
      HttpResponse::ok([])
        .with_permissions_policy(value)
        .is_err(),
      "should reject {value:?}"
    );
  }

  assert_eq!(
    None,
    HttpResponse::ok([])
      .permissions_policy()
      .expect("absent Permissions-Policy should parse")
  );

  let duplicate = HttpResponse::ok([])
    .header("Permissions-Policy", "geolocation=(self)")
    .header("permissions-policy", "geolocation=(self)");
  assert!(duplicate.permissions_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_permissions_policy(
      format!("geolocation=(\"{}\")", "https://example.test/".repeat(64 * 1024))
    )
    .is_err());
}

#[test]
fn document_policy_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Document-Policy", "oversized-images=1.0")
    .header("document-policy", "unsized-media=?0")
    .with_document_policy("oversized-images=2.0, unsized-media=?0, *;report-to=default")
    .expect("Document-Policy should be accepted");

  let policy = response
    .document_policy()
    .expect("Document-Policy should parse")
    .expect("Document-Policy should be present");
  assert_eq!(
    "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    policy.header_value()
  );
  assert_eq!(3, policy.directives().len());
  assert_eq!(
    Some("default"),
    policy.directive("*").unwrap().report_to()
  );
  assert_eq!(
    vec![("Document-Policy", "oversized-images=2.0, unsized-media=?0, *;report-to=default")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn document_policy_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Document-Policy", "oversized-images=2.0");
  let policy = raw
    .document_policy()
    .expect("raw Document-Policy should parse")
    .expect("Document-Policy should be present");
  assert_eq!("oversized-images=2.0", policy.header_value());
  assert_eq!(
    Some("oversized-images=2.0"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Document-Policy"))
      .map(|header| header.value.as_str())
  );

  for value in [
    "",
    "=()",
    "=1.2345",
    "unsized-media=src;foo=bar",
    "oversized-images=1;report-to=first;report-to=second",
    "oversized-images=1.0, oversized-images=2.0",
  ] {
    let malformed = HttpResponse::ok([]).header("Document-Policy", value);
    assert!(malformed.document_policy().is_err(), "should reject {value:?}");
    assert!(malformed.status_code == 200 && malformed.body.is_empty());
    assert_eq!(
      Some(value),
      malformed
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Document-Policy"))
        .map(|header| header.value.as_str())
    );
    assert!(
      HttpResponse::ok([])
        .with_document_policy(value)
        .is_err(),
      "should reject {value:?}"
    );
  }

  assert_eq!(
    None,
    HttpResponse::ok([])
      .document_policy()
      .expect("absent Document-Policy should parse")
  );

  let duplicate = HttpResponse::ok([])
    .header("Document-Policy", "oversized-images=1.0")
    .header("document-policy", "oversized-images=2.0");
  assert!(duplicate.document_policy().is_err());
  assert!(HttpResponse::ok([])
    .with_document_policy("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn sec_websocket_version_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::new(400, "Bad Request")
    .header("Sec-WebSocket-Version", "12")
    .header("sec-websocket-version", "8")
    .with_sec_websocket_version(["13"])
    .expect("Sec-WebSocket-Version should be accepted");

  let versions = response
    .sec_websocket_version()
    .expect("Sec-WebSocket-Version should parse")
    .expect("Sec-WebSocket-Version should be present");
  assert_eq!("13", versions.header_value());
  assert_eq!(versions.versions(), ["13"]);
  assert!(versions.contains("13"));
  assert_eq!(
    vec![("Sec-WebSocket-Version", "13")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
  assert!(response
    .headers
    .iter()
    .all(|header| !header.name.eq_ignore_ascii_case("Connection")
      && !header.name.eq_ignore_ascii_case("Upgrade")));
}

#[test]
fn sec_websocket_version_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::new(400, "Bad Request")
    .header("Sec-WebSocket-Version", "13")
    .header("sec-websocket-version", "8, 7");
  let versions = raw
    .sec_websocket_version()
    .expect("raw Sec-WebSocket-Version should parse")
    .expect("Sec-WebSocket-Version should be present");
  assert_eq!("13, 8, 7", versions.header_value());

  for value in ["", "v13", "013", "8, 13", "13, 13", "300"] {
    let malformed = HttpResponse::new(400, "Bad Request").header("Sec-WebSocket-Version", value);
    assert!(
      malformed.sec_websocket_version().is_err(),
      "should reject {value:?}"
    );
    assert!(
      HttpResponse::new(400, "Bad Request")
        .with_sec_websocket_version([value])
        .is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(value),
      malformed
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Sec-WebSocket-Version"))
        .map(|header| header.value.as_str())
    );
  }

  assert!(HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_version(["13", "13"])
    .is_err());
  assert!(HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_version(["8", "13"])
    .is_err());
  assert!(HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_version(["1".repeat(64 * 1024 + 1)])
    .is_err());
}

#[test]
fn sec_websocket_protocol_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::new(400, "Bad Request")
    .header("Sec-WebSocket-Protocol", "chat")
    .header("sec-websocket-protocol", "superchat")
    .with_sec_websocket_protocol("graphql-transport-ws")
    .expect("Sec-WebSocket-Protocol should be accepted");

  let protocol = response
    .sec_websocket_protocol()
    .expect("Sec-WebSocket-Protocol should parse")
    .expect("Sec-WebSocket-Protocol should be present");
  assert_eq!("graphql-transport-ws", protocol.header_value());
  assert_eq!(protocol.protocols(), ["graphql-transport-ws"]);
  assert_eq!(protocol.selected(), Some("graphql-transport-ws"));
  assert!(protocol.contains("graphql-transport-ws"));
  assert_eq!(
    vec![("Sec-WebSocket-Protocol", "graphql-transport-ws")],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
  assert!(response
    .headers
    .iter()
    .all(|header| !header.name.eq_ignore_ascii_case("Connection")
      && !header.name.eq_ignore_ascii_case("Upgrade")));
}

#[test]
fn sec_websocket_protocol_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::new(400, "Bad Request")
    .header("Sec-WebSocket-Protocol", "chat")
    .header("sec-websocket-protocol", "superchat");
  assert!(
    raw.sec_websocket_protocol().is_err(),
    "combined response fields must still be a selection singleton"
  );
  assert_eq!(
    vec![
      ("Sec-WebSocket-Protocol", "chat"),
      ("sec-websocket-protocol", "superchat"),
    ],
    raw
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );

  let singleton = HttpResponse::new(400, "Bad Request").header("Sec-WebSocket-Protocol", "chat");
  let protocol = singleton
    .sec_websocket_protocol()
    .expect("raw Sec-WebSocket-Protocol should parse")
    .expect("Sec-WebSocket-Protocol should be present");
  assert_eq!("chat", protocol.header_value());
  assert_eq!(protocol.selected(), Some("chat"));

  for value in ["", "chat, superchat", "not a token", "chat\r\nX-Injected: 1"] {
    // Construct the raw message directly: `HttpResponse::header` asserts on CR/LF values,
    // but the injected value must survive so the typed accessor can reject it.
    let malformed = HttpResponse {
      version: "HTTP/1.1".to_string(),
      status_code: 400,
      reason: "Bad Request".to_string(),
      headers: vec![HttpHeader::new("Sec-WebSocket-Protocol", value)],
      trailers: Vec::new(),
      body: Vec::new(),
    };
    assert!(
      malformed.sec_websocket_protocol().is_err(),
      "should reject {value:?}"
    );
    assert!(
      HttpResponse::new(400, "Bad Request")
        .with_sec_websocket_protocol(value)
        .is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(value),
      malformed
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Sec-WebSocket-Protocol"))
        .map(|header| header.value.as_str())
    );
  }

  let legacy = HttpResponse::new(400, "Bad Request").header("Sec-WebSocket-Protocol", "legacy");
  let error = legacy
    .clone()
    .with_sec_websocket_protocol("chat, superchat")
    .expect_err("multi-token selection should be rejected");
  assert!(error.to_string().contains("Sec-WebSocket-Protocol"));
  assert_eq!(
    Some("legacy"),
    legacy
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Sec-WebSocket-Protocol"))
      .map(|header| header.value.as_str()),
    "a failed builder must leave the response unchanged"
  );
  assert!(HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_protocol("a".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn document_policy_report_only_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Document-Policy-Report-Only", "oversized-images=1.0")
    .header("document-policy-report-only", "unsized-media=?0")
    .with_document_policy_report_only(
      "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    )
    .expect("Document-Policy-Report-Only should be accepted");

  let policy = response
    .document_policy_report_only()
    .expect("Document-Policy-Report-Only should parse")
    .expect("Document-Policy-Report-Only should be present");
  assert_eq!(
    "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    policy.header_value()
  );
  assert_eq!(3, policy.directives().len());
  assert_eq!(
    Some("default"),
    policy.directive("*").unwrap().report_to()
  );
  assert_eq!(
    vec![(
      "Document-Policy-Report-Only",
      "oversized-images=2.0, unsized-media=?0, *;report-to=default"
    )],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn document_policy_report_only_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([]).header("Document-Policy-Report-Only", "oversized-images=2.0");
  let policy = raw
    .document_policy_report_only()
    .expect("raw Document-Policy-Report-Only should parse")
    .expect("Document-Policy-Report-Only should be present");
  assert_eq!("oversized-images=2.0", policy.header_value());
  assert_eq!(
    Some("oversized-images=2.0"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Document-Policy-Report-Only"))
      .map(|header| header.value.as_str())
  );

  for value in [
    "",
    "=()",
    "=1.2345",
    "unsized-media=src;foo=bar",
    "oversized-images=1;report-to=first;report-to=second",
    "oversized-images=1.0, oversized-images=2.0",
  ] {
    let malformed = HttpResponse::ok([]).header("Document-Policy-Report-Only", value);
    assert!(
      malformed.document_policy_report_only().is_err(),
      "should reject {value:?}"
    );
    assert!(malformed.status_code == 200 && malformed.body.is_empty());
    assert_eq!(
      Some(value),
      malformed
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Document-Policy-Report-Only"))
        .map(|header| header.value.as_str())
    );
    assert!(
      HttpResponse::ok([])
        .with_document_policy_report_only(value)
        .is_err(),
      "should reject {value:?}"
    );
  }

  assert_eq!(
    None,
    HttpResponse::ok([])
      .document_policy_report_only()
      .expect("absent Document-Policy-Report-Only should parse")
  );

  let duplicate = HttpResponse::ok([])
    .header("Document-Policy-Report-Only", "oversized-images=1.0")
    .header("document-policy-report-only", "oversized-images=2.0");
  assert!(duplicate.document_policy_report_only().is_err());
  assert!(HttpResponse::ok([])
    .with_document_policy_report_only("x".repeat(64 * 1024 + 1))
    .is_err());
}

#[test]
fn supports_loading_mode_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Supports-Loading-Mode", "fenced-frame")
    .header("supports-loading-mode", "credentialed-prerender")
    .with_supports_loading_mode(["fenced-frame", "credentialed-prerender"])
    .expect("Supports-Loading-Mode should be accepted");

  let modes = response
    .supports_loading_mode()
    .expect("Supports-Loading-Mode should parse")
    .expect("Supports-Loading-Mode should be present");
  assert_eq!(
    "fenced-frame, credentialed-prerender",
    modes.header_value()
  );
  assert_eq!(modes.tokens(), ["fenced-frame", "credentialed-prerender"]);
  assert!(modes.contains_fenced_frame());
  assert!(modes.contains_credentialed_prerender());
  assert_eq!(
    vec![(
      "Supports-Loading-Mode",
      "fenced-frame, credentialed-prerender"
    )],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn supports_loading_mode_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([])
    .header("Supports-Loading-Mode", "fenced-frame")
    .header("supports-loading-mode", "credentialed-prerender");
  let modes = raw
    .supports_loading_mode()
    .expect("raw Supports-Loading-Mode should parse")
    .expect("Supports-Loading-Mode should be present");
  assert_eq!(
    "fenced-frame, credentialed-prerender",
    modes.header_value()
  );
  assert_eq!(
    vec!["fenced-frame", "credentialed-prerender"],
    raw
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Supports-Loading-Mode"))
      .map(|header| header.value.as_str())
      .collect::<Vec<_>>()
  );

  for value in [
    "",
    "fenced-frame credentialed-prerender",
    "fenced-frame,,credentialed-prerender",
    "?1",
    "\"fenced-frame\"",
    "(fenced-frame)",
    "fenced-frame;foo=bar",
    "fenced-frame, fenced-frame",
  ] {
    let malformed = HttpResponse::ok([]).header("Supports-Loading-Mode", value);
    assert!(
      malformed.supports_loading_mode().is_err(),
      "should reject {value:?}"
    );
    assert!(
      HttpResponse::ok([])
        .with_supports_loading_mode([value])
        .is_err(),
      "should reject {value:?}"
    );
  }

  assert_eq!(
    None,
    HttpResponse::ok([])
      .supports_loading_mode()
      .expect("absent Supports-Loading-Mode should parse")
  );

  let duplicate = HttpResponse::ok([])
    .header("Supports-Loading-Mode", "fenced-frame")
    .header("supports-loading-mode", "Fenced-Frame");
  assert!(duplicate.supports_loading_mode().is_err());
  assert!(HttpResponse::ok([])
    .with_supports_loading_mode(["fenced-frame", "FENCED-FRAME"])
    .is_err());
  assert!(HttpResponse::ok([])
    .with_supports_loading_mode([format!("fenced-frame{}", "x".repeat(64 * 1024))])
    .is_err());

  let extension = HttpResponse::ok([])
    .with_supports_loading_mode(["vendor/mode", "vendor:mode"])
    .expect("Structured Fields tokens with ':' and '/' should be accepted");
  let modes = extension
    .supports_loading_mode()
    .expect("Supports-Loading-Mode should parse")
    .expect("Supports-Loading-Mode should be present");
  assert_eq!(modes.tokens(), ["vendor/mode", "vendor:mode"]);
  assert_eq!(modes.header_value(), "vendor/mode, vendor:mode");

  let over_first = format!("a{}", "x".repeat(32_767));
  let over_second = format!("b{}", "x".repeat(32_767));
  assert!(HttpResponse::ok([])
    .with_supports_loading_mode([&over_first, &over_second])
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
fn signature_helpers_validate_replace_and_parse_response_metadata() {
  let response = HttpResponse::ok([])
    .header("Signature", "sig1=:YWJj:")
    .header("signature", "sig-b24=:ZGVm:")
    .header(
      "Signature-Input",
      r#"sig1=("@method")"#,
    )
    .header("signature-input", r#"sig-b24=("@status")"#)
    .with_signature("sig1=:YWJj:")
    .expect("Signature should be accepted")
    .with_signature_input(r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#)
    .expect("Signature-Input should be accepted");

  let signature = response
    .signature()
    .expect("Signature should parse")
    .expect("Signature should be present");
  let signature_input = response
    .signature_input()
    .expect("Signature-Input should parse")
    .expect("Signature-Input should be present");
  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#
  );
  assert_eq!(
    vec![
      ("Signature", "sig1=:YWJj:"),
      (
        "Signature-Input",
        r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#,
      ),
    ],
    response
      .headers
      .iter()
      .map(|header| (header.name.as_str(), header.value.as_str()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn signature_helpers_preserve_raw_metadata_and_report_parse_errors() {
  let raw = HttpResponse::ok([])
    .header("Signature", "sig1=:YWJj:")
    .header(
      "Signature-Input",
      r#"sig1=("@method");created=1618884473"#,
    );
  let signature = raw
    .signature()
    .expect("raw Signature should parse")
    .expect("Signature should be present");
  let signature_input = raw
    .signature_input()
    .expect("raw Signature-Input should parse")
    .expect("Signature-Input should be present");
  assert_eq!("sig1=:YWJj:", signature.header_value());
  assert_eq!(
    r#"sig1=("@method");created=1618884473"#,
    signature_input.header_value()
  );
  assert_eq!(
    Some("sig1=:YWJj:"),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Signature"))
      .map(|header| header.value.as_str())
  );
  assert_eq!(
    Some(r#"sig1=("@method");created=1618884473"#),
    raw
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Signature-Input"))
      .map(|header| header.value.as_str())
  );

  let malformed = HttpResponse::ok([])
    .header("Signature", "not-a-signature")
    .header("Signature-Input", "not-an-input");
  assert!(malformed.signature().is_err());
  assert!(malformed.signature_input().is_err());
  assert_eq!(
    Some("not-a-signature"),
    malformed
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case("Signature"))
      .map(|header| header.value.as_str())
  );
  assert!(HttpResponse::ok([])
    .with_signature("not-a-signature")
    .is_err());
  assert!(HttpResponse::ok([])
    .with_signature_input("not-an-input")
    .is_err());
  assert_eq!(
    None,
    HttpResponse::ok([])
      .signature()
      .expect("absent Signature should parse")
  );
  assert_eq!(
    None,
    HttpResponse::ok([])
      .signature_input()
      .expect("absent Signature-Input should parse")
  );
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

  for value in ["0", "255", "256", "4294967295"] {
    let valid = Request::from_raw_frame(
      format!(
        "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .max_forwards()
      .expect("value should parse")
      .expect("Max-Forwards should be present");
    assert_eq!(value.parse::<u32>().expect("fixture is a u32"), parsed.value());
    assert_eq!(value, parsed.header_value());
  }

  for value in ["", "-1", "+1", "1.0", "4294967296", "999999999999999999999"] {
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

  let oversized = "0".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "OPTIONS".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Max-Forwards".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.max_forwards().is_err());
  assert_eq!(Some(oversized.as_str()), oversized_request.header("Max-Forwards"));

  let duplicate = Request::from_raw_frame(
    b"OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: 1\r\nmax-forwards: 2\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.max_forwards().is_err());
  assert_eq!(Some("1"), duplicate.header("Max-Forwards"));
}

#[test]
fn request_depth_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"PROPFIND / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.depth().expect("missing value should be valid"));

  for (value, expected) in [
    ("0", HttpDepth::Zero),
    ("1", HttpDepth::One),
    ("infinity", HttpDepth::Infinity),
    ("INFINITY", HttpDepth::Infinity),
  ] {
    let valid = Request::from_raw_frame(
      format!("PROPFIND / HTTP/1.1\r\nHost: example.test\r\nDepth: {value}\r\n\r\n").as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .depth()
      .expect("value should parse")
      .expect("Depth should be present");
    assert_eq!(expected, parsed);
    assert_eq!(expected.header_value(), parsed.header_value());
  }

  for value in ["", "2", "-1", "1.0", "0, 1", "infinite"] {
    let request = Request::from_raw_frame(
      format!("PROPFIND / HTTP/1.1\r\nHost: example.test\r\nDepth: {value}\r\n\r\n").as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(request.depth().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Depth"));
  }

  let oversized = "0".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "PROPFIND".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Depth".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.depth().is_err());
  assert_eq!(Some(oversized.as_str()), oversized_request.header("Depth"));

  let duplicate = Request::from_raw_frame(
    b"PROPFIND / HTTP/1.1\r\nHost: example.test\r\nDepth: 0\r\ndepth: 1\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.depth().is_err());
  assert_eq!(Some("0"), duplicate.header("Depth"));
}

#[test]
fn request_idempotency_key_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .idempotency_key()
      .expect("missing value should be valid")
  );

  for value in [
    "charge-2026-08-19-9f3c",
    "urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00",
    "A",
  ] {
    let valid = Request::from_raw_frame(
      format!(
        "POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .idempotency_key()
      .expect("value should parse")
      .expect("Idempotency-Key should be present");
    assert_eq!(value, parsed.as_str());
    assert_eq!(value, parsed.header_value());
  }
  let redacted = Request::from_raw_frame(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: charge-2026-08-19-9f3c\r\n\r\n",
  )
  .expect("request should parse")
  .idempotency_key()
  .expect("value should parse")
  .expect("Idempotency-Key should be present");
  assert!(!format!("{redacted:?}").contains("charge-2026-08-19-9f3c"));

  for value in ["", "key with space"] {
    let request = Request::from_raw_frame(
      format!(
        "POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(request.idempotency_key().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Idempotency-Key"));
  }

  let obs_text = Request::from_raw_frame(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: key\xC3\xA9\r\n\r\n",
  )
  .expect("request should retain obs-text metadata");
  assert!(obs_text.idempotency_key().is_err());
  assert!(obs_text.header("Idempotency-Key").is_some());

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "POST".to_string(),
    target: "/charges".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Idempotency-Key".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.idempotency_key().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("Idempotency-Key")
  );

  let injected_request = Request {
    method: "POST".to_string(),
    target: "/charges".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      (
        "Idempotency-Key".to_string(),
        "key\r\nX-Injected: 1".to_string(),
      ),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(injected_request.idempotency_key().is_err());
  assert_eq!(
    Some("key\r\nX-Injected: 1"),
    injected_request.header("Idempotency-Key")
  );

  let duplicate = Request::from_raw_frame(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: first\r\nidempotency-key: second\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.idempotency_key().is_err());
  assert_eq!(Some("first"), duplicate.header("Idempotency-Key"));
}

#[test]
fn request_sec_websocket_version_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .sec_websocket_version()
      .expect("missing value should be valid")
  );

  for value in ["13", " \t13\t ", "13, 8, 7"] {
    let valid = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Version: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .sec_websocket_version()
      .expect("value should parse")
      .expect("Sec-WebSocket-Version should be present");
    assert_eq!(
      value
        .split(',')
        .map(|member| member.trim_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join(", "),
      parsed.header_value()
    );
    assert!(parsed.contains("13"));
  }

  for value in ["", "v13", "013", "8, 13", "13, 13", "300"] {
    let request = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Version: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(
      request.sec_websocket_version().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(Some(value), request.header("Sec-WebSocket-Version"));
  }

  let oversized = "1".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "GET".to_string(),
    target: "/chat".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Sec-WebSocket-Version".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.sec_websocket_version().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("Sec-WebSocket-Version")
  );

  let unordered = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Version: 13\r\nsec-websocket-version: 8, 13\r\n\r\n",
  )
  .expect("request should retain unordered metadata");
  assert!(unordered.sec_websocket_version().is_err());
  assert_eq!(Some("13"), unordered.header("Sec-WebSocket-Version"));
}

#[test]
fn request_sec_websocket_protocol_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .sec_websocket_protocol()
      .expect("missing value should be valid")
  );

  for value in ["chat", " \tchat\t ", "chat, superchat, graphql-ws"] {
    let valid = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Protocol: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .sec_websocket_protocol()
      .expect("value should parse")
      .expect("Sec-WebSocket-Protocol should be present");
    assert_eq!(
      value
        .split(',')
        .map(|member| member.trim_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join(", "),
      parsed.header_value()
    );
    assert!(parsed.contains("chat"));
  }

  let multi_token = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Protocol: chat, superchat\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    None,
    multi_token
      .sec_websocket_protocol()
      .expect("offers should parse")
      .expect("offers should be present")
      .selected(),
    "a multi-token offer is not a selection"
  );

  let combined = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Protocol: chat\r\nsec-websocket-protocol: superchat, graphql-ws\r\n\r\n",
  )
  .expect("request should parse");
  let parsed = combined
    .sec_websocket_protocol()
    .expect("combined fields should parse")
    .expect("Sec-WebSocket-Protocol should be present");
  assert_eq!("chat, superchat, graphql-ws", parsed.header_value());

  for value in ["", ",", "chat,", "not a token", "chat;foo", "chat/1", "chat, chat"] {
    let request = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Protocol: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(
      request.sec_websocket_protocol().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(Some(value), request.header("Sec-WebSocket-Protocol"));
  }

  let oversized = "a".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "GET".to_string(),
    target: "/chat".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Sec-WebSocket-Protocol".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.sec_websocket_protocol().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("Sec-WebSocket-Protocol")
  );

  let duplicate = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Protocol: chat\r\nsec-websocket-protocol: chat\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.sec_websocket_protocol().is_err());
  assert_eq!(Some("chat"), duplicate.header("Sec-WebSocket-Protocol"));
}

#[test]
fn request_sec_websocket_key_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .sec_websocket_key()
      .expect("missing value should be valid")
  );

  for value in [
    "dGhlIHNhbXBsZSBub25jZQ==",
    "AAAAAAAAAAAAAAAAAAAAAA==",
    " \t+/z9/v8AAQIDBAUGBwgJCg==\t ",
  ] {
    let valid = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Key: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    let parsed = valid
      .sec_websocket_key()
      .expect("value should parse")
      .expect("Sec-WebSocket-Key should be present");
    assert_eq!(value.trim_matches([' ', '\t']), parsed.as_str());
    assert_eq!(value.trim_matches([' ', '\t']), parsed.header_value());
  }
  let redacted = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
  )
  .expect("request should parse")
  .sec_websocket_key()
  .expect("value should parse")
  .expect("Sec-WebSocket-Key should be present");
  assert!(!format!("{redacted:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));

  for value in ["", "the sample nonce", "dGhlIHNhbXBsZSBub25jZQ"] {
    let request = Request::from_raw_frame(
      format!(
        "GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Key: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should retain malformed metadata");
    assert!(request.sec_websocket_key().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Sec-WebSocket-Key"));
  }

  let obs_text = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\xC3\xA9\r\n\r\n",
  )
  .expect("request should retain obs-text metadata");
  assert!(obs_text.sec_websocket_key().is_err());
  assert!(obs_text.header("Sec-WebSocket-Key").is_some());

  let oversized = "A".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "GET".to_string(),
    target: "/chat".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("Sec-WebSocket-Key".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.sec_websocket_key().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("Sec-WebSocket-Key")
  );

  let injected_request = Request {
    method: "GET".to_string(),
    target: "/chat".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      (
        "Sec-WebSocket-Key".to_string(),
        "dGhlIHNhbXBsZSBub25jZQ==\r\nX-Injected: 1".to_string(),
      ),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(injected_request.sec_websocket_key().is_err());
  assert_eq!(
    Some("dGhlIHNhbXBsZSBub25jZQ==\r\nX-Injected: 1"),
    injected_request.header("Sec-WebSocket-Key")
  );

  let duplicate = Request::from_raw_frame(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nsec-websocket-key: AAAAAAAAAAAAAAAAAAAAAA==\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.sec_websocket_key().is_err());
  assert_eq!(
    Some("dGhlIHNhbXBsZSBub25jZQ=="),
    duplicate.header("Sec-WebSocket-Key")
  );
}

#[test]
fn response_sec_websocket_accept_can_parse_and_derive_from_validated_key() {
  let key = rttp_protocol::sec_websocket_key::SecWebSocketKey::parse(
    "dGhlIHNhbXBsZSBub25jZQ==",
  )
  .expect("Sec-WebSocket-Key should parse");

  let response = HttpResponse::new(101, "Switching Protocols")
    .header("Sec-WebSocket-Accept", "legacy")
    .with_sec_websocket_accept_for_key(&key);
  let accept = response
    .sec_websocket_accept()
    .expect("Sec-WebSocket-Accept should parse")
    .expect("Sec-WebSocket-Accept should be present");

  assert_eq!("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", accept.as_str());
  assert!(accept.verify_key(&key));
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert!(serialized.contains("\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"));
  assert!(!serialized.contains("\r\nSec-WebSocket-Accept: legacy\r\n"));
  assert!(!format!("{accept:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert!(!format!("{accept:?}").contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

  let manual = HttpResponse::new(101, "Switching Protocols")
    .with_sec_websocket_accept(" \ts3pPLMBiTxaQ9kYGzzhZRbK+xOo=\t ")
    .expect("manual Sec-WebSocket-Accept should parse");
  assert_eq!(
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
    manual
      .sec_websocket_accept()
      .expect("manual accept should parse")
      .expect("manual accept should be present")
      .header_value()
  );
}

#[test]
fn response_sec_websocket_accept_preserves_invalid_raw_headers() {
  let duplicate = HttpResponse::new(101, "Switching Protocols")
    .header("Sec-WebSocket-Accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
    .header("sec-websocket-accept", "AAAAAAAAAAAAAAAAAAAAAAAAAAA=");
  assert!(duplicate.sec_websocket_accept().is_err());

  let oversized = "A".repeat(64 * 1024 + 1);
  let oversized_response =
    HttpResponse::new(101, "Switching Protocols").header("Sec-WebSocket-Accept", oversized);
  let error = oversized_response
    .sec_websocket_accept()
    .expect_err("oversized Sec-WebSocket-Accept should fail");
  let message = error.to_string();
  assert!(message.contains("Sec-WebSocket-Accept"));
  assert!(!message.contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
}

#[test]
fn request_trace_context_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.traceparent().expect("missing traceparent"));
  assert_eq!(None, absent.tracestate().expect("missing tracestate"));

  let request = Request::from_raw_frame(
    b"GET /trace HTTP/1.1\r\nHost: example.test\r\ntraceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\ntracestate: rojo=00f067aa0ba902b7\r\ntracestate: congo=t61rcWkgMzE\r\n\r\n",
  )
  .expect("request should parse");

  let traceparent = request
    .traceparent()
    .expect("traceparent should parse")
    .expect("traceparent should be present");
  assert_eq!("00", traceparent.version());
  assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", traceparent.trace_id());
  assert!(traceparent.sampled());
  let tracestate = request
    .tracestate()
    .expect("tracestate should parse")
    .expect("tracestate should be present");
  assert_eq!(2, tracestate.members().len());
  assert_eq!("rojo", tracestate.members()[0].key());
  assert_eq!(
    Some("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"),
    request.header("traceparent")
  );

  let invalid = Request::from_raw_frame(
    b"GET /trace HTTP/1.1\r\nHost: example.test\r\ntraceparent: 00-00000000000000000000000000000000-00f067aa0ba902b7-01\r\ntracestate: rojo=1,rojo=2\r\n\r\n",
  )
  .expect("request should retain invalid trace context metadata");
  assert!(invalid.traceparent().is_err());
  assert!(invalid.tracestate().is_err());
  assert_eq!(
    Some("00-00000000000000000000000000000000-00f067aa0ba902b7-01"),
    invalid.header("traceparent")
  );
  assert_eq!(Some("rojo=1,rojo=2"), invalid.header("tracestate"));

  let duplicate = Request::from_raw_frame(
    b"GET /trace HTTP/1.1\r\nHost: example.test\r\ntraceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\nTraceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-00\r\n\r\n",
  )
  .expect("request should retain duplicate traceparent metadata");
  assert!(duplicate.traceparent().is_err());
}

#[test]
fn request_baggage_is_optional_bounded_and_preserves_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.baggage().expect("missing baggage"));

  let request = Request::from_raw_frame(
    b"GET /baggage HTTP/1.1\r\nHost: example.test\r\nbaggage: tenant=acme;source=gateway\r\nbaggage: release=2026-08-19\r\n\r\n",
  )
  .expect("request should parse");

  let baggage = request
    .baggage()
    .expect("baggage should parse")
    .expect("baggage should be present");
  assert_eq!(2, baggage.members().len());
  assert_eq!("tenant", baggage.members()[0].key());
  assert_eq!("acme", baggage.members()[0].value());
  assert_eq!("source", baggage.members()[0].properties()[0].key());
  assert_eq!("release", baggage.members()[1].key());
  assert_eq!(
    Some("tenant=acme;source=gateway"),
    request.header("baggage")
  );

  let invalid = Request::from_raw_frame(
    b"GET /baggage HTTP/1.1\r\nHost: example.test\r\nbaggage: tenant=secret,tenant=other\r\n\r\n",
  )
  .expect("request should retain invalid baggage metadata");
  assert!(invalid.baggage().is_err());
  assert_eq!(
    Some("tenant=secret,tenant=other"),
    invalid.header("baggage")
  );

  let oversized = Request::from_raw_frame(
    format!(
      "GET /baggage HTTP/1.1\r\nHost: example.test\r\nbaggage: k={}\r\n\r\n",
      "v".repeat(8193)
    )
    .as_bytes(),
  )
  .expect("request should retain oversized baggage metadata");
  assert!(oversized.baggage().is_err());
  assert!(oversized.header("baggage").is_some());
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
  assert!(matches!(request.if_unmodified_since(), Ok(None)));
}

#[test]
fn request_conditional_http_date_metadata_is_optional_bounded_and_rejects_invalid_headers() {
  let absent = Request::from_raw_frame(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.if_modified_since().expect("absent should be valid"));
  assert_eq!(None, absent.if_unmodified_since().expect("absent should be valid"));

  let modified = Request::from_raw_frame(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    modified
      .if_modified_since()
      .expect("If-Modified-Since should parse")
      .expect("If-Modified-Since should be present")
      .header_value()
  );

  let unmodified = Request::from_raw_frame(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Unmodified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    unmodified
      .if_unmodified_since()
      .expect("If-Unmodified-Since should parse")
      .expect("If-Unmodified-Since should be present")
      .header_value()
  );

  for value in ["not-a-date", "", "08:49:37 06 Nov 1994"] {
    let request = Request::from_raw_frame(
      format!(
        "GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Modified-Since: {value}\r\n\r\n"
      )
      .as_bytes(),
    )
    .expect("request should parse");
    assert!(request.if_modified_since().is_err());
    assert_eq!(Some(value), request.header("If-Modified-Since"));
  }

  let oversized = "0".repeat(64 * 1024 + 1);
  let oversized_request = Request {
    method: "GET".to_string(),
    target: "/".to_string(),
    version: "HTTP/1.1".to_string(),
    headers: vec![
      ("Host".to_string(), "example.test".to_string()),
      ("If-Modified-Since".to_string(), oversized.clone()),
    ],
    trailers: Vec::new(),
    body: Vec::new(),
    content_length: None,
    extended_connect_protocol: None,
  };
  assert!(oversized_request.if_modified_since().is_err());
  assert_eq!(
    Some(oversized.as_str()),
    oversized_request.header("If-Modified-Since")
  );

  let duplicate = Request::from_raw_frame(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Unmodified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\nif-unmodified-since: Sun, 06 Nov 1994 08:49:38 GMT\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.if_unmodified_since().is_err());
  assert_eq!(
    None,
    duplicate.if_modified_since().expect("absent should be valid")
  );
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
    let content_length = request
      .content_length()
      .expect("matching fixed length should be retained");
    assert_eq!(5, content_length.len());
  }

  #[test]
  fn read_next_from_omits_content_length_metadata_when_header_is_absent() {
    let raw = "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n";
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));

    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");

    assert_eq!("GET", request.method());
    assert_eq!(b"", request.body());
    assert_eq!(None, request.content_length());
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
    assert!(!format!("{proxy_authorization:?}").contains("cHJveHk6c2VjcmV0"));
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("Authorization"));
    assert!(request_debug.contains("Proxy-Authorization"));
    assert!(request_debug.contains("[REDACTED]"));
    assert!(!request_debug.contains("origin-token"));
    assert!(!request_debug.contains("cHJveHk6c2VjcmV0"));

    let http_request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");
    let http_request_debug = format!("{http_request:?}");
    assert!(http_request_debug.contains("Authorization"));
    assert!(http_request_debug.contains("Proxy-Authorization"));
    assert!(http_request_debug.contains("[REDACTED]"));
    assert!(!http_request_debug.contains("origin-token"));
    assert!(!http_request_debug.contains("cHJveHk6c2VjcmV0"));

    let cookie_header_debug = format!("{:?}", HttpHeader::new("Cookie", "session=private"));
    assert!(cookie_header_debug.contains("Cookie"));
    assert!(cookie_header_debug.contains("[REDACTED]"));
    assert!(!cookie_header_debug.contains("session=private"));

    let trace_raw = concat!(
      "GET /trace HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\n",
      "tracestate: rojo=00f067aa0ba902b7\r\n",
      "\r\n"
    );
    let trace_request =
      Request::from_raw_frame(trace_raw.as_bytes()).expect("trace request should parse");
    let traceparent = trace_request
      .traceparent()
      .expect("traceparent should parse")
      .expect("traceparent should be present");
    let tracestate = trace_request
      .tracestate()
      .expect("tracestate should parse")
      .expect("tracestate should be present");
    assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", traceparent.trace_id());
    assert_eq!("00f067aa0ba902b7", tracestate.members()[0].value());
    let trace_debug = format!(
      "{trace_request:?} {:?} {:?}",
      traceparent,
      tracestate.members()[0]
    );
    assert!(!trace_debug.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    assert!(!trace_debug.contains("00f067aa0ba902b7"));

    let trace_http_request =
      HttpRequest::parse(trace_raw.as_bytes()).expect("trace HttpRequest should parse");
    let trace_http_request_debug = format!("{trace_http_request:?}");
    assert!(!trace_http_request_debug.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    assert!(!trace_http_request_debug.contains("00f067aa0ba902b7"));
    let trace_header_debug = format!(
      "{:?} {:?}",
      HttpHeader::new(
        "traceparent",
        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
      ),
      HttpHeader::new("tracestate", "rojo=00f067aa0ba902b7")
    );
    assert!(trace_header_debug.contains("[REDACTED]"));
    assert!(!trace_header_debug.contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    assert!(!trace_header_debug.contains("00f067aa0ba902b7"));

    let baggage_raw = concat!(
      "GET /baggage HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "baggage: tenant=acme-secret;source=gateway\r\n",
      "\r\n"
    );
    let baggage_request =
      Request::from_raw_frame(baggage_raw.as_bytes()).expect("baggage request should parse");
    let baggage = baggage_request
      .baggage()
      .expect("baggage should parse")
      .expect("baggage should be present");
    assert_eq!("acme-secret", baggage.members()[0].value());
    let baggage_debug = format!(
      "{baggage_request:?} {:?} {:?}",
      baggage,
      baggage.members()[0]
    );
    assert!(!baggage_debug.contains("acme-secret"));
    assert!(!baggage_debug.contains("gateway"));

    let baggage_http_request =
      HttpRequest::parse(baggage_raw.as_bytes()).expect("baggage HttpRequest should parse");
    let baggage_http_request_debug = format!("{baggage_http_request:?}");
    assert!(!baggage_http_request_debug.contains("acme-secret"));
    assert!(!baggage_http_request_debug.contains("gateway"));
    let baggage_header_debug = format!("{:?}", HttpHeader::new("baggage", "tenant=acme-secret"));
    assert!(baggage_header_debug.contains("[REDACTED]"));
    assert!(!baggage_header_debug.contains("acme-secret"));

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
    assert!(HttpProxyAuthorization::parse("Basic proxy\rsecret").is_err());
    assert_eq!(
      Some("invalid"),
      malformed.header("Proxy-Authorization")
    );
  }

  #[test]
  fn idempotency_key_debug_redacts_values_in_request_and_http_request() {
    let raw = concat!(
      "POST /charges HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Idempotency-Key: charge-2026-08-19-9f3c\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");

    let idempotency_key = request
      .idempotency_key()
      .expect("Idempotency-Key should parse")
      .expect("Idempotency-Key should be present");
    assert_eq!("charge-2026-08-19-9f3c", idempotency_key.as_str());
    assert!(!format!("{idempotency_key:?}").contains("charge-2026-08-19-9f3c"));
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("Idempotency-Key"));
    assert!(request_debug.contains("[REDACTED]"));
    assert!(!request_debug.contains("charge-2026-08-19-9f3c"));

    let http_request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");
    let http_request_debug = format!("{http_request:?}");
    assert!(http_request_debug.contains("Idempotency-Key"));
    assert!(http_request_debug.contains("[REDACTED]"));
    assert!(!http_request_debug.contains("charge-2026-08-19-9f3c"));

    let header_debug =
      format!("{:?}", HttpHeader::new("Idempotency-Key", "charge-2026-08-19-9f3c"));
    assert!(header_debug.contains("Idempotency-Key"));
    assert!(header_debug.contains("[REDACTED]"));
    assert!(!header_debug.contains("charge-2026-08-19-9f3c"));
  }

  #[test]
  fn sec_websocket_key_debug_redacts_values_in_request_and_http_request() {
    let raw = concat!(
      "GET /chat HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n",
      "\r\n"
    );
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes()));
    let request = Request::read_next_from(&mut reader)
      .expect("request should parse")
      .expect("request should be present");

    let sec_websocket_key = request
      .sec_websocket_key()
      .expect("Sec-WebSocket-Key should parse")
      .expect("Sec-WebSocket-Key should be present");
    assert_eq!(
      "dGhlIHNhbXBsZSBub25jZQ==",
      sec_websocket_key.as_str()
    );
    assert!(!format!("{sec_websocket_key:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("Sec-WebSocket-Key"));
    assert!(request_debug.contains("[REDACTED]"));
    assert!(!request_debug.contains("dGhlIHNhbXBsZSBub25jZQ=="));

    let http_request = HttpRequest::parse(raw.as_bytes()).expect("request should parse");
    let http_request_debug = format!("{http_request:?}");
    assert!(http_request_debug.contains("Sec-WebSocket-Key"));
    assert!(http_request_debug.contains("[REDACTED]"));
    assert!(!http_request_debug.contains("dGhlIHNhbXBsZSBub25jZQ=="));

    let header_debug = format!(
      "{:?}",
      HttpHeader::new("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
    );
    assert!(header_debug.contains("Sec-WebSocket-Key"));
    assert!(header_debug.contains("[REDACTED]"));
    assert!(!header_debug.contains("dGhlIHNhbXBsZSBub25jZQ=="));

    let accept_header_debug = format!(
      "{:?}",
      HttpHeader::new("Sec-WebSocket-Accept", "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
    );
    assert!(accept_header_debug.contains("Sec-WebSocket-Accept"));
    assert!(accept_header_debug.contains("[REDACTED]"));
    assert!(!accept_header_debug.contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
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

  #[test]
  fn origin_trial_helpers_declare_parse_and_redact_response_metadata() {
    let response = HttpResponse::ok([])
      .header("Origin-Trial", "stale-token")
      .with_origin_trials(["secret-token-one", "secret-token-two"])
      .expect("Origin-Trial should be accepted");
    let origin_trials = response
      .origin_trials()
      .expect("response Origin-Trial should parse")
      .expect("response Origin-Trial should be present");

    assert_eq!(
      origin_trials.tokens(),
      ["secret-token-one", "secret-token-two"]
    );
    let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
    assert_eq!(2, serialized.matches("\r\nOrigin-Trial: ").count());
    assert!(serialized.contains("\r\nOrigin-Trial: secret-token-one\r\n"));
    assert!(!serialized.contains("stale-token"));

    let response_debug = format!("{response:?}");
    assert!(response_debug.contains("Origin-Trial"));
    assert!(response_debug.contains("[REDACTED]"));
    assert!(!response_debug.contains("secret-token-one"));
    assert!(!format!("{origin_trials:?}").contains("secret-token-one"));

    let header_debug = format!("{:?}", HttpHeader::new("Origin-Trial", "secret-token-one"));
    assert!(header_debug.contains("Origin-Trial"));
    assert!(header_debug.contains("[REDACTED]"));
    assert!(!header_debug.contains("secret-token-one"));

    let malformed = HttpResponse::ok([]).header("Origin-Trial", "token\twith-tab");
    assert!(malformed.origin_trials().is_err());
    assert!(String::from_utf8(malformed.to_bytes())
      .expect("response should serialize")
      .contains("\r\nOrigin-Trial: token\twith-tab\r\n"));
  }

  #[test]
  fn speculation_rules_helpers_declare_parse_and_redact_response_metadata() {
    let value = "https://example.test/speculation-rules.json";
    let response = HttpResponse::ok([])
      .header("Speculation-Rules", "https://example.test/stale.json")
      .with_speculation_rules(value)
      .expect("Speculation-Rules should be accepted");
    let rules = response
      .speculation_rules()
      .expect("response Speculation-Rules should parse")
      .expect("response Speculation-Rules should be present");

    assert_eq!(rules.as_str(), value);
    assert_eq!(rules.header_value(), value);
    let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
    assert_eq!(1, serialized.matches("\r\nSpeculation-Rules: ").count());
    assert!(serialized.contains(&format!("\r\nSpeculation-Rules: {value}\r\n")));
    assert!(!serialized.contains("stale.json"));

    let response_debug = format!("{response:?}");
    assert!(response_debug.contains("Speculation-Rules"));
    assert!(response_debug.contains("[REDACTED]"));
    assert!(!response_debug.contains(value));
    assert!(!format!("{rules:?}").contains(value));

    let duplicate = HttpResponse::ok([])
      .header("Speculation-Rules", "https://example.test/one.json")
      .header("speculation-rules", "https://example.test/two.json");
    assert!(duplicate.speculation_rules().is_err());

    let malformed = HttpResponse::ok([]).header("Speculation-Rules", "");
    assert!(malformed.speculation_rules().is_err());
    assert!(HttpResponse::ok([])
      .with_speculation_rules("https://example.test/rules.json\r\nX-Injected: 1")
      .is_err());
  }

  #[test]
  fn nel_helpers_reject_raw_crlf_and_serialize_a_single_header_line() {
    assert!(
      HttpResponse::ok([])
        .with_nel("{\"max_age\":1,\"x\":{ \"a\":\r\n1 }}")
        .is_err(),
      "raw CR/LF in a NEL field value must be rejected"
    );

    let response = HttpResponse::ok([])
      .with_nel(r#"{"report_to":"errors","max_age":60,"x":{"a":[1,2]}}"#)
      .expect("valid NEL policy should be accepted");
    let bytes = response.to_bytes();
    let head = std::str::from_utf8(&bytes).expect("serialized head should be UTF-8");
    let nel_start = head
      .find("NEL:")
      .expect("serialized head should contain a NEL header");
    let nel_line_end = head[nel_start..]
      .find("\r\n")
      .expect("NEL header line should end with CRLF");
    let nel_line = &head[nel_start..nel_start + nel_line_end];
    assert!(
      !nel_line.contains('\r') && !nel_line.contains('\n'),
      "serialized NEL header line must not contain raw CR or LF: {nel_line:?}"
    );
    assert_eq!(
      "NEL: {\"max_age\":60,\"report_to\":\"errors\",\"x\":{\"a\":[1,2]}}",
      nel_line
    );

    let parsed = response
      .nel()
      .expect("response NEL should parse")
      .expect("response NEL should be present");
    assert_eq!(60, parsed.max_age());
    assert_eq!(Some("errors"), parsed.report_to());
  }

  #[test]
  fn nel_helper_escapes_del_so_the_wire_header_has_no_raw_0x7f() {
    let response = HttpResponse::ok([])
      .with_nel(r#"{"report_to":"a\u007fb","max_age":1}"#)
      .expect("valid NEL policy with a \\u007f escape should be accepted");
    let bytes = response.to_bytes();
    assert!(
      !bytes.contains(&0x7f),
      "serialized response must not contain a raw DEL byte"
    );

    let head = std::str::from_utf8(&bytes).expect("serialized head should be UTF-8");
    let nel_start = head
      .find("NEL:")
      .expect("serialized head should contain a NEL header");
    let nel_line_end = head[nel_start..]
      .find("\r\n")
      .expect("NEL header line should end with CRLF");
    let nel_line = &head[nel_start..nel_start + nel_line_end];
    assert_eq!(
      r#"NEL: {"max_age":1,"report_to":"a\u007fb"}"#,
      nel_line
    );

    let parsed = response
      .nel()
      .expect("response NEL should parse")
      .expect("response NEL should be present");
    assert_eq!(Some("a\u{7f}b"), parsed.report_to());
  }

  #[test]
  fn keep_alive_helpers_combine_fields_preserve_extensions_and_build_responses() {
    let combined = HttpResponse::ok([])
      .header("Keep-Alive", "timeout=5")
      .header("Keep-Alive", "max=100, vendor=1");
    let keep_alive = combined
      .keep_alive()
      .expect("Keep-Alive should parse")
      .expect("Keep-Alive should be present");

    assert_eq!(Some(5), keep_alive.timeout());
    assert_eq!(Some(100), keep_alive.max());
    assert_eq!(1, keep_alive.extensions().len());
    assert_eq!("vendor", keep_alive.extensions()[0].name());
    assert_eq!("1", keep_alive.extensions()[0].value());
    assert_eq!(
      "timeout=5, max=100, vendor=1",
      keep_alive.header_value(),
      "recognized parameters are emitted before preserved extensions"
    );

    let built = HttpResponse::ok([])
      .with_keep_alive("timeout=5, max=100, vendor=1")
      .expect("Keep-Alive should be accepted");
    assert_eq!(
      vec![("Keep-Alive", "timeout=5, max=100, vendor=1")],
      built
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>()
    );
    let parsed = built
      .keep_alive()
      .expect("built Keep-Alive should parse")
      .expect("built Keep-Alive should be present");
    assert_eq!(Some(5), parsed.timeout());
    assert_eq!(Some(100), parsed.max());
    assert_eq!("vendor", parsed.extensions()[0].name());

    let replaced = HttpResponse::ok([])
      .header("Keep-Alive", "timeout=1")
      .with_keep_alive("max=2")
      .expect("replacement should be accepted");
    assert_eq!(
      vec![("Keep-Alive", "max=2")],
      replaced
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>()
    );
    let replaced_parsed = replaced
      .keep_alive()
      .expect("replaced Keep-Alive should parse")
      .expect("replaced Keep-Alive should be present");
    assert_eq!(None, replaced_parsed.timeout());
    assert_eq!(Some(2), replaced_parsed.max());
    assert_eq!(
      "max=2",
      HttpKeepAlive::parse("max=2")
        .expect("max-only should parse")
        .header_value()
    );
  }

  #[test]
  fn keep_alive_helpers_return_none_when_absent() {
    assert_eq!(
      None,
      HttpResponse::ok([])
        .keep_alive()
        .expect("absent Keep-Alive should parse")
    );
  }

  #[test]
  fn keep_alive_rejects_malformed_duplicate_and_bounds_without_hiding_headers() {
    for value in [
      "timeout=abc",
      "timeout=5, timeout=6",
      "timeout=5, max=100, max=200",
      "timeout=18446744073709551616",
      "",
    ] {
      let response = HttpResponse::ok([]).header("Keep-Alive", value);
      assert!(response.keep_alive().is_err(), "should reject {value:?}");
      assert_eq!(
        Some(value),
        response
          .headers
          .iter()
          .find(|header| header.name.eq_ignore_ascii_case("Keep-Alive"))
          .map(|header| header.value.as_str())
      );
    }

    let oversized = "x".repeat(64 * 1024 + 1);
    let oversized_response = HttpResponse::ok([]).header("Keep-Alive", oversized.as_str());
    assert!(oversized_response.keep_alive().is_err());
    assert_eq!(
      Some(oversized.as_str()),
      oversized_response
        .headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case("Keep-Alive"))
        .map(|header| header.value.as_str())
    );
    assert!(HttpKeepAlive::parse(
      (0..257)
        .map(|index| {
          if index % 2 == 0 {
            "timeout=1".to_string()
          } else {
            "max=2".to_string()
          }
        })
        .collect::<Vec<_>>()
        .join(", ")
    )
    .is_err());
    assert!(HttpResponse::ok([]).with_keep_alive("timeout=abc").is_err());
  }
