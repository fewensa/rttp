use std::time::{Duration, UNIX_EPOCH};

use rttp::server::{
  HttpAccept, HttpAcceptCh, HttpAcceptRanges, HttpAccessControlAllowHeaders,
  HttpAccessControlAllowMethods, HttpAccessControlAllowOrigin, HttpAccessControlRequestHeaders,
  HttpAccessControlRequestMethod, HttpAllowedMethods, HttpAuthorization, HttpByteRange,
  HttpByteRangeError, HttpClearSiteData, HttpConditionalMetadata, HttpContentDisposition,
  HttpContentLanguages, HttpContentRange, HttpContentSecurityPolicy, HttpContentType,
  HttpCriticalCh, HttpEntityTag, HttpExpectations, HttpHost, HttpIfNoneMatch, HttpIfRange,
  HttpIfRangeRequestOutcome, HttpLinkValues, HttpNel, HttpPermissionsPolicy, HttpReferrerPolicy,
  HttpReportingEndpoints, HttpRequest, HttpRequestAcceptEncodings, HttpRequestCacheControl,
  HttpRequestTe, HttpResponse, HttpResponseCacheControl, HttpResponseContentEncodings,
  HttpRetryAfter, HttpServerTiming, HttpVary,
};

#[test]
fn response_access_control_allow_origin_helper_validates_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("Access-Control-Allow-Origin", "https://legacy.test")
    .header("access-control-allow-origin", "https://deprecated.test")
    .with_access_control_allow_origin("https://example.test:8443")
    .expect("valid Access-Control-Allow-Origin should be accepted");

  let origin: HttpAccessControlAllowOrigin = response
    .access_control_allow_origin()
    .expect("attached Access-Control-Allow-Origin should parse")
    .expect("Access-Control-Allow-Origin should be present");
  assert_eq!("https://example.test:8443", origin.header_value());
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(
    1,
    serialized
      .matches("\r\nAccess-Control-Allow-Origin: ")
      .count()
  );
  assert!(serialized.contains("\r\nAccess-Control-Allow-Origin: https://example.test:8443\r\n"));

  assert!(HttpResponse::ok("body")
    .with_access_control_allow_origin("https://example.test/path")
    .is_err());
  let raw =
    HttpResponse::ok("body").header("Access-Control-Allow-Origin", "https://example.test/path");
  assert!(raw.access_control_allow_origin().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nAccess-Control-Allow-Origin: https://example.test/path\r\n"));
}

#[test]
fn response_access_control_allow_methods_helper_validates_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("Access-Control-Allow-Methods", "DELETE")
    .header("access-control-allow-methods", "PATCH")
    .with_access_control_allow_methods("get, POST")
    .expect("valid Access-Control-Allow-Methods should be accepted");

  let methods: HttpAccessControlAllowMethods = response
    .access_control_allow_methods()
    .expect("attached Access-Control-Allow-Methods should parse")
    .expect("Access-Control-Allow-Methods should be present");
  assert_eq!(["GET", "POST"], methods.methods());
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(
    1,
    serialized
      .matches("\r\nAccess-Control-Allow-Methods: ")
      .count()
  );
  assert!(serialized.contains("\r\nAccess-Control-Allow-Methods: GET, POST\r\n"));

  assert!(HttpResponse::ok("body")
    .with_access_control_allow_methods("GET POST")
    .is_err());
  let raw = HttpResponse::ok("body").header("Access-Control-Allow-Methods", "GET POST");
  assert!(raw.access_control_allow_methods().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nAccess-Control-Allow-Methods: GET POST\r\n"));
  assert_eq!(
    None,
    HttpResponse::ok("body")
      .access_control_allow_methods()
      .expect("absent Access-Control-Allow-Methods should parse")
  );
}

#[test]
fn request_access_control_request_method_preserves_absent_valid_and_malformed_metadata() {
  let absent = parse_request("OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    absent
      .access_control_request_method()
      .expect("missing Access-Control-Request-Method should be accepted")
  );

  let request = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Method: patch\r\n",
    "\r\n"
  ));
  let method: HttpAccessControlRequestMethod = request
    .access_control_request_method()
    .expect("Access-Control-Request-Method should parse")
    .expect("Access-Control-Request-Method should be present");
  assert_eq!("PATCH", method.method());

  let malformed = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Method: GET, POST\r\n",
    "\r\n"
  ));
  assert!(malformed.access_control_request_method().is_err());
  assert_eq!(
    Some("GET, POST"),
    malformed.header("Access-Control-Request-Method")
  );
}

#[test]
fn request_access_control_request_headers_preserves_absent_valid_and_malformed_metadata() {
  let absent = parse_request("OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    absent
      .access_control_request_headers()
      .expect("missing Access-Control-Request-Headers should be accepted")
  );

  let request = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Headers: X-Request-Id, Authorization\r\n",
    "\r\n"
  ));
  let headers: HttpAccessControlRequestHeaders = request
    .access_control_request_headers()
    .expect("Access-Control-Request-Headers should parse")
    .expect("Access-Control-Request-Headers should be present");
  assert_eq!(["x-request-id", "authorization"], headers.field_names());

  let malformed = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Headers: X-Request Id\r\n",
    "\r\n"
  ));
  assert!(malformed.access_control_request_headers().is_err());
  assert_eq!(
    Some("X-Request Id"),
    malformed.header("Access-Control-Request-Headers")
  );
}

#[test]
fn request_access_control_request_private_network_preserves_absent_valid_and_malformed_metadata() {
  let absent = parse_request("OPTIONS /widgets HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    absent
      .access_control_request_private_network()
      .expect("missing Access-Control-Request-Private-Network should be accepted")
  );

  let request = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: true\r\n",
    "\r\n"
  ));
  let private_network = request
    .access_control_request_private_network()
    .expect("Access-Control-Request-Private-Network should parse")
    .expect("Access-Control-Request-Private-Network should be present");
  assert_eq!("true", private_network.header_value());

  let malformed = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: false\r\n",
    "\r\n"
  ));
  assert!(malformed.access_control_request_private_network().is_err());
  assert_eq!(
    Some("false"),
    malformed.header("Access-Control-Request-Private-Network")
  );

  let duplicate = parse_request(concat!(
    "OPTIONS /widgets HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Access-Control-Request-Private-Network: true\r\n",
    "access-control-request-private-network: true\r\n",
    "\r\n"
  ));
  assert!(duplicate.access_control_request_private_network().is_err());
  assert_eq!(
    Some("true"),
    duplicate.header("Access-Control-Request-Private-Network")
  );
}

#[test]
fn request_representation_metadata_parses_without_applying_policy() {
  let absent = parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n");
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

  let request = parse_request(concat!(
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
  ));

  let content_type = request
    .content_type()
    .expect("Content-Type should parse")
    .expect("Content-Type should be present");
  assert_eq!("application/json", content_type.media_type());
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));

  let encodings = request
    .content_encoding()
    .expect("Content-Encoding should parse")
    .expect("Content-Encoding should be present");
  assert_eq!(vec!["gzip", "br", "zstd"], encodings.codings());

  let languages = request
    .content_language()
    .expect("Content-Language should parse")
    .expect("Content-Language should be present");
  assert_eq!(vec!["fr-CA", "es-419", "en"], languages.languages());

  let accept_encoding = request
    .accept_encoding()
    .expect("Accept-Encoding should parse")
    .expect("Accept-Encoding should be present");
  assert_eq!("gzip", accept_encoding.codings()[0].coding());
  let accept_language = request
    .accept_language()
    .expect("Accept-Language should parse")
    .expect("Accept-Language should be present");
  assert_eq!(vec!["en"], accept_language.ranges());
  assert_eq!(b"body", request.body());
}

#[test]
fn request_representation_metadata_preserves_invalid_headers_and_body() {
  let duplicate = parse_request(concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: application/json\r\n",
    "content-type: text/plain\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ));
  assert!(duplicate.content_type().is_err());
  assert_eq!(Some("application/json"), duplicate.header("Content-Type"));
  assert_eq!(b"body", duplicate.body());

  let malformed = parse_request(concat!(
    "POST /documents HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Content-Type: text/plain;\r\n",
    "Content-Encoding: gzip,\r\n",
    "Content-Language: en,\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "body"
  ));
  assert!(malformed.content_type().is_err());
  assert!(malformed.content_encoding().is_err());
  assert!(malformed.content_language().is_err());
  assert_eq!(Some("text/plain;"), malformed.header("Content-Type"));
  assert_eq!(Some("gzip,"), malformed.header("Content-Encoding"));
  assert_eq!(Some("en,"), malformed.header("Content-Language"));
  assert_eq!(b"body", malformed.body());

  let duplicate_members = parse_request(concat!(
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
  ));
  assert!(duplicate_members.content_type().is_err());
  assert!(duplicate_members.content_encoding().is_err());
  assert!(duplicate_members.content_language().is_err());
  assert_eq!(
    Some("text/plain; charset=utf-8; CHARSET=us-ascii"),
    duplicate_members.header("Content-Type")
  );
  assert_eq!(Some("gzip"), duplicate_members.header("Content-Encoding"));
  assert_eq!(Some("en"), duplicate_members.header("Content-Language"));
  assert_eq!(b"body", duplicate_members.body());

  let too_many_parameters = rttp_test_support::content_type::too_many_server_parameters_value();
  let too_many_codings = rttp_test_support::content_encoding::too_many_server_codings_value();
  let too_many_languages = rttp_test_support::content_language::too_many_server_languages_value();
  let too_many = parse_request(&format!(
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
  ));
  assert!(too_many.content_type().is_err());
  assert!(too_many.content_encoding().is_err());
  assert!(too_many.content_language().is_err());
  assert_eq!(
    Some(too_many_parameters.as_str()),
    too_many.header("Content-Type")
  );
  assert_eq!(
    Some(too_many_codings.as_str()),
    too_many.header("Content-Encoding")
  );
  assert_eq!(
    Some(too_many_languages.as_str()),
    too_many.header("Content-Language")
  );
  assert_eq!(b"body", too_many.body());

  assert!(HttpContentType::parse(rttp_test_support::content_type::oversized_value()).is_err());
  assert!(HttpResponseContentEncodings::parse(
    rttp_test_support::content_encoding::oversized_value()
  )
  .is_err());
  assert!(
    HttpContentLanguages::parse(rttp_test_support::content_language::oversized_value()).is_err()
  );
}

#[test]
fn response_access_control_allow_headers_helper_validates_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("Access-Control-Allow-Headers", "X-Legacy")
    .header("access-control-allow-headers", "X-Deprecated")
    .with_access_control_allow_headers("X-Request-Id, ETag")
    .expect("valid Access-Control-Allow-Headers should be accepted");

  let headers: HttpAccessControlAllowHeaders = response
    .access_control_allow_headers()
    .expect("attached Access-Control-Allow-Headers should parse")
    .expect("Access-Control-Allow-Headers should be present");
  assert_eq!(["x-request-id", "etag"], headers.field_names());
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(
    1,
    serialized
      .matches("\r\nAccess-Control-Allow-Headers: ")
      .count()
  );
  assert!(serialized.contains("\r\nAccess-Control-Allow-Headers: x-request-id, etag\r\n"));

  assert!(HttpResponse::ok("body")
    .with_access_control_allow_headers("X-Request Id")
    .is_err());
  let raw = HttpResponse::ok("body").header("Access-Control-Allow-Headers", "X-Request Id");
  assert!(raw.access_control_allow_headers().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nAccess-Control-Allow-Headers: X-Request Id\r\n"));
  assert!(HttpResponse::ok("body")
    .with_access_control_allow_headers("*")
    .expect("wildcard Access-Control-Allow-Headers should be accepted")
    .access_control_allow_headers()
    .expect("wildcard Access-Control-Allow-Headers should parse")
    .expect("wildcard Access-Control-Allow-Headers should be present")
    .is_wildcard());
  assert_eq!(
    None,
    HttpResponse::ok("body")
      .access_control_allow_headers()
      .expect("absent Access-Control-Allow-Headers should parse")
  );
}

#[test]
fn response_browser_policy_helpers_preserve_metadata_without_enforcing_it() {
  let response = HttpResponse::ok("body")
    .header("Content-Security-Policy", "default-src 'self'")
    .with_content_security_policy("default-src 'none'")
    .expect("Content-Security-Policy metadata should be accepted")
    .with_permissions_policy("geolocation=(), camera=()")
    .expect("Permissions-Policy metadata should be accepted")
    .with_referrer_policy("strict-origin-when-cross-origin")
    .expect("Referrer-Policy metadata should be accepted");

  assert_eq!(
    Some("default-src 'none'"),
    response
      .content_security_policy()
      .expect("Content-Security-Policy metadata should parse")
      .as_ref()
      .map(HttpContentSecurityPolicy::as_str)
  );
  assert_eq!(
    Some("geolocation=(), camera=()"),
    response
      .permissions_policy()
      .expect("Permissions-Policy metadata should parse")
      .as_ref()
      .map(HttpPermissionsPolicy::as_str)
  );
  assert_eq!(
    Some("strict-origin-when-cross-origin"),
    response
      .referrer_policy()
      .expect("Referrer-Policy metadata should parse")
      .as_ref()
      .map(HttpReferrerPolicy::as_str)
  );
  assert!(String::from_utf8(response.to_bytes())
    .expect("response should serialize")
    .contains("\r\nContent-Security-Policy: default-src 'none'\r\n"));

  assert!(HttpContentSecurityPolicy::parse("default-src\r\nblocked").is_err());
  assert!(HttpPermissionsPolicy::parse("").is_err());
  assert!(HttpReferrerPolicy::parse("origin\0").is_err());
}

#[test]
fn response_client_hints_helpers_declare_and_parse_metadata_without_policy() {
  let response = HttpResponse::ok("body")
    .header("Accept-CH", "DPR")
    .with_accept_ch(["Sec-CH-UA", "Viewport-Width"])
    .expect("Accept-CH should be accepted")
    .with_critical_ch(["Sec-CH-UA-Platform", "Downlink"])
    .expect("Critical-CH should be accepted");

  let accept_ch: HttpAcceptCh = response
    .accept_ch()
    .expect("Accept-CH should parse")
    .expect("Accept-CH should be present");
  assert_eq!(&["Sec-CH-UA", "Viewport-Width"], accept_ch.client_hints());
  let critical_ch: HttpCriticalCh = response
    .critical_ch()
    .expect("Critical-CH should parse")
    .expect("Critical-CH should be present");
  assert_eq!(
    &["Sec-CH-UA-Platform", "Downlink"],
    critical_ch.client_hints()
  );
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(1, serialized.matches("\r\nAccept-CH: ").count());
  assert!(serialized.contains("\r\nAccept-CH: Sec-CH-UA, Viewport-Width\r\n"));
  assert!(serialized.contains("\r\nCritical-CH: Sec-CH-UA-Platform, Downlink\r\n"));

  assert!(HttpResponse::ok("body").with_accept_ch(["DPR,"]).is_err());
  assert!(HttpResponse::ok("body")
    .with_critical_ch(["1Downlink"])
    .is_err());
  assert!(HttpAcceptCh::parse("DPR,").is_err());
  assert!(HttpCriticalCh::parse("1Downlink").is_err());
}

#[test]
fn response_clear_site_data_builder_and_parser_preserve_metadata_only_directives() {
  let response = HttpResponse::ok("body")
    .header("Clear-Site-Data", "\"cache\"")
    .with_clear_site_data("\"cookies\", \"executionContexts\"")
    .expect("Clear-Site-Data should be accepted");
  let metadata = response
    .clear_site_data()
    .expect("Clear-Site-Data should parse")
    .expect("Clear-Site-Data should be present");
  assert_eq!(
    vec!["cookies", "executionContexts"],
    metadata
      .directives()
      .iter()
      .map(|directive| directive.as_str())
      .collect::<Vec<_>>()
  );
  assert!(String::from_utf8(response.to_bytes())
    .expect("response should serialize")
    .contains("\r\nClear-Site-Data: \"cookies\", \"executionContexts\"\r\n"));

  assert!(HttpResponse::ok("body")
    .with_clear_site_data("cache")
    .is_err());
  assert!(HttpClearSiteData::parse("\"cache\", \"cache\"").is_err());
}

#[test]
fn response_www_authenticate_helper_validates_and_preserves_raw_headers() {
  let response = HttpResponse::new(401, "Unauthorized")
    .with_www_authenticate("Digest realm=\"apps\", nonce=\"n-1\", Basic")
    .expect("valid challenges should be accepted");
  let challenges = response
    .www_authenticate()
    .expect("attached challenges should parse")
    .expect("WWW-Authenticate should be present");
  assert_eq!(2, challenges.len());
  assert_eq!(Some("apps"), challenges.challenges()[0].parameter("realm"));
  assert_eq!("Basic", challenges.challenges()[1].scheme());
  assert!(String::from_utf8(response.to_bytes())
    .expect("response should serialize")
    .contains("\r\nWWW-Authenticate: Digest realm=\"apps\", nonce=n-1, Basic\r\n"));

  assert!(HttpResponse::ok("body")
    .with_www_authenticate("Basic realm=")
    .is_err());
  let raw = HttpResponse::ok("body").header("WWW-Authenticate", "Basic realm=");
  assert!(raw.www_authenticate().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nWWW-Authenticate: Basic realm=\r\n"));
}

#[test]
fn response_digest_helpers_declare_multiple_algorithms_and_replace_raw_fields() {
  let response = HttpResponse::ok("body")
    .header("Content-Digest", "sha-256=:b2xk:")
    .header("Content-Digest", "sha-512=:b2xk:")
    .header("Repr-Digest", "sha-256=:b2xk:")
    .with_digest("sha-256=:YWJj:, sha-512=:ZGVm:")
    .expect("Digest should be accepted")
    .with_repr_digest("sha-256=:Z2hp:")
    .expect("Repr-Digest should be accepted");
  assert_eq!(
    Some(&b"abc"[..]),
    response
      .digest()
      .expect("Digest should parse")
      .expect("Digest should be present")
      .entry("sha-256")
      .map(|entry| entry.value())
  );
  assert_eq!(
    Some(&b"ghi"[..]),
    response
      .repr_digest()
      .expect("Repr-Digest should parse")
      .expect("Repr-Digest should be present")
      .entry("sha-256")
      .map(|entry| entry.value())
  );
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(1, serialized.matches("\r\nContent-Digest: ").count());
  assert_eq!(1, serialized.matches("\r\nRepr-Digest: ").count());
  assert!(serialized.contains("\r\nContent-Digest: sha-256=:YWJj:, sha-512=:ZGVm:\r\n"));
  assert!(serialized.contains("\r\nRepr-Digest: sha-256=:Z2hp:\r\n"));
}

#[test]
fn response_digest_helpers_reject_invalid_raw_fields_without_mutating_them() {
  for (header, value) in [
    ("Content-Digest", "sha-256=:YWJj:, sha-256=:ZGVm:"),
    ("Repr-Digest", "sha-256=:invalid!:"),
  ] {
    let raw = HttpResponse::ok("body").header(header, value);
    let parsed = if header == "Content-Digest" {
      raw.digest().map(|_| ())
    } else {
      raw.repr_digest().map(|_| ())
    };
    assert!(parsed.is_err());
    assert!(String::from_utf8(raw.to_bytes())
      .expect("response should serialize")
      .contains(&format!("\r\n{header}: {value}\r\n")));
  }

  for header in ["Content-Digest", "Repr-Digest"] {
    let oversized = format!("sha-256=:{}:", "A".repeat(64 * 1024));
    let raw = HttpResponse::ok("body").header(header, &oversized);
    let parsed = if header == "Content-Digest" {
      raw.digest().map(|_| ())
    } else {
      raw.repr_digest().map(|_| ())
    };
    assert!(parsed.is_err());
    assert!(String::from_utf8(raw.to_bytes())
      .expect("response should serialize")
      .contains(&format!("\r\n{header}: {oversized}\r\n")));
  }

  assert!(HttpResponse::ok("body").with_digest("").is_err());
  assert!(HttpResponse::ok("body")
    .with_repr_digest("sha-256=:YWJj:, sha-256=:ZGVm:")
    .is_err());
}

#[test]
fn response_server_timing_helper_validates_formats_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("Server-Timing", "old;dur=1")
    .with_server_timing("db;dur=53.2;desc=\"primary database\";region=us-east, db;cached")
    .expect("valid timing metadata should be accepted");
  let timing = response
    .server_timing()
    .expect("attached timing should parse")
    .expect("Server-Timing should be present");
  assert_eq!(2, timing.len());
  assert_eq!("db", timing.metrics()[0].name());
  assert_eq!(Some(53.2), timing.metrics()[0].duration());
  assert_eq!(Some("primary database"), timing.metrics()[0].description());
  assert_eq!("db", timing.metrics()[1].name());
  assert_eq!(
    "db; dur=53.2; desc=\"primary database\"; region=us-east, db; cached",
    String::from_utf8(response.to_bytes())
      .expect("response should serialize")
      .split("Server-Timing: ")
      .nth(1)
      .expect("Server-Timing should serialize")
      .split("\r\n")
      .next()
      .expect("Server-Timing line should end")
  );

  assert!(HttpResponse::ok("body")
    .with_server_timing("db;dur=not-a-number")
    .is_err());
  let raw = HttpResponse::ok("body").header("Server-Timing", "db;dur=not-a-number");
  assert!(raw.server_timing().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nServer-Timing: db;dur=not-a-number\r\n"));

  assert!(HttpServerTiming::parse(format!("db;desc=\"{}\"", "a".repeat(64 * 1024))).is_err());
}

#[test]
fn response_nel_helper_validates_replaces_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("NEL", r#"{"max_age":1}"#)
    .header("nel", r#"{"max_age":2}"#)
    .with_nel(
      r#"{"report_to":"network-errors","max_age":2592000,"include_subdomains":true,"success_fraction":0.1}"#,
    )
    .expect("valid NEL policy should be accepted");

  let nel: HttpNel = response
    .nel()
    .expect("attached NEL should parse")
    .expect("NEL should be present");
  assert_eq!(2592000, nel.max_age());
  assert_eq!(Some("network-errors"), nel.report_to());
  assert_eq!(Some(true), nel.include_subdomains());
  assert_eq!(Some(0.1), nel.success_fraction());
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(1, serialized.matches("\r\nNEL: ").count());
  assert!(serialized.contains(
    "\r\nNEL: {\"max_age\":2592000,\"report_to\":\"network-errors\",\"include_subdomains\":true,\"success_fraction\":0.1}\r\n"
  ));

  assert!(HttpResponse::ok("body").with_nel("{bad").is_err());
  assert!(HttpResponse::ok("body")
    .with_nel(r#"{"max_age":"1"}"#)
    .is_err());
  let raw = HttpResponse::ok("body").header("NEL", r#"{"max_age":"1"}"#);
  assert!(raw.nel().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nNEL: {\"max_age\":\"1\"}\r\n"));
  assert_eq!(
    None,
    HttpResponse::ok("body")
      .nel()
      .expect("absent NEL should parse")
  );
}

fn parse_request(raw: &str) -> HttpRequest {
  HttpRequest::parse(raw.as_bytes()).expect("request should parse")
}

#[test]
fn request_parses_bounded_range_and_conditional_metadata() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=2-99\r\n",
    "If-Range: \"version-7\"\r\n",
    "If-None-Match: W/\"stale\", \"version-7\"\r\n",
    "If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "\r\n"
  ));

  assert_eq!(
    Some(HttpByteRange::new(2, 9)),
    request.range(10).expect("Range should parse")
  );
  assert_eq!(
    Some(HttpIfRange::EntityTag(HttpEntityTag::strong("version-7"))),
    request.if_range().expect("If-Range should parse")
  );
  assert_eq!(
    Some(HttpIfNoneMatch::Tags(vec![
      HttpEntityTag::weak("stale"),
      HttpEntityTag::strong("version-7"),
    ])),
    request.if_none_match().expect("If-None-Match should parse")
  );
  assert_eq!(
    Some(
      httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").expect("HTTP-date should parse")
    ),
    request
      .if_modified_since()
      .expect("If-Modified-Since should parse")
  );
}

#[test]
fn request_conditional_metadata_helpers_preserve_absent_and_invalid_headers() {
  let absent = parse_request("GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    absent.range(10).expect("missing Range should be valid")
  );
  assert_eq!(
    None,
    absent.if_range().expect("missing If-Range should be valid")
  );
  assert_eq!(
    None,
    absent
      .if_none_match()
      .expect("missing If-None-Match should be valid")
  );
  assert_eq!(
    None,
    absent
      .if_modified_since()
      .expect("missing If-Modified-Since should be valid")
  );

  let invalid = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Range: bytes=9-2\r\n",
    "If-Range: W/\"weak\"\r\n",
    "If-None-Match: *, \"version-7\"\r\n",
    "If-Modified-Since: not-a-date\r\n",
    "\r\n"
  ));
  assert_eq!(Some("bytes=9-2"), invalid.header("Range"));
  assert!(invalid.range(10).is_err());
  assert_eq!(Some(r#"W/"weak""#), invalid.header("If-Range"));
  assert!(invalid.if_range().is_err());
  assert_eq!(Some(r#"*, "version-7""#), invalid.header("If-None-Match"));
  assert!(invalid.if_none_match().is_err());
  assert_eq!(Some("not-a-date"), invalid.header("If-Modified-Since"));
  assert!(invalid.if_modified_since().is_err());
}

#[test]
fn request_authorization_parses_one_bounded_opaque_credential() {
  let request = parse_request(concat!(
    "GET / HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Authorization: Bearer token-123\r\n",
    "\r\n"
  ));
  let authorization = request
    .authorization()
    .expect("Authorization should parse")
    .expect("Authorization should be present");

  assert_eq!("Bearer", authorization.scheme());
  assert_eq!("token-123", authorization.credentials());
  assert!(!format!("{authorization:?}").contains("token-123"));
}

#[test]
fn request_authorization_rejects_duplicate_invalid_and_oversized_values() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .authorization()
      .expect("absent Authorization should be accepted")
  );

  for value in ["Bearer", "bad(scheme credentials", "Bearer \t"] {
    assert!(
      HttpAuthorization::parse(value).is_err(),
      "should reject {value:?}"
    );
  }

  let duplicate = parse_request(concat!(
    "GET / HTTP/1.1\r\nHost: example.test\r\n",
    "Authorization: Bearer first\r\nauthorization: Bearer second\r\n\r\n"
  ));
  assert!(duplicate.authorization().is_err());
  assert!(HttpAuthorization::parse(format!("Bearer {}", "x".repeat(64 * 1024))).is_err());
}

#[test]
fn request_forwarded_parses_standard_parameters_and_multiple_entries() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: internal.test\r\n",
    "Forwarded: for=192.0.2.60;by=203.0.113.43;host=example.test;proto=\"https\"\r\n",
    "Forwarded: for=\"[2001:db8:cafe::17]\"\r\n",
    "\r\n"
  ));

  let forwarded = request
    .forwarded()
    .expect("Forwarded should parse")
    .expect("Forwarded should be present");

  assert_eq!(2, forwarded.len());
  assert_eq!(Some("192.0.2.60"), forwarded.elements()[0].for_value());
  assert_eq!(Some("203.0.113.43"), forwarded.elements()[0].by());
  assert_eq!(Some("example.test"), forwarded.elements()[0].host());
  assert_eq!(Some("https"), forwarded.elements()[0].proto());
  assert_eq!(
    Some("[2001:db8:cafe::17]"),
    forwarded.elements()[1].for_value()
  );
}

#[test]
fn request_forwarded_rejects_duplicate_and_excessive_metadata() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .forwarded()
      .expect("absent Forwarded should be accepted")
  );

  let duplicate = parse_request(concat!(
    "GET / HTTP/1.1\r\nHost: example.test\r\n",
    "Forwarded: for=192.0.2.60;FOR=198.51.100.17\r\n\r\n"
  ));
  assert!(duplicate.forwarded().is_err());

  let excessive = (0..257)
    .map(|index| format!("for=192.0.2.{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let request = parse_request(&format!(
    "GET / HTTP/1.1\r\nHost: example.test\r\nForwarded: {excessive}\r\n\r\n"
  ));
  assert!(request.forwarded().is_err());
}

#[test]
fn request_max_forwards_is_optional_and_rejects_invalid_metadata() {
  let absent = parse_request("OPTIONS / HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    absent
      .max_forwards()
      .expect("missing Max-Forwards should be valid")
  );

  for value in ["0", "256", "999999999999999999999"] {
    let valid = parse_request(&format!(
      "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
    ));
    assert_eq!(
      Some(value.to_owned()),
      valid.max_forwards().expect("value should parse")
    );
  }

  for value in ["abc", "1.0"] {
    let request = parse_request(&format!(
      "OPTIONS / HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: {value}\r\n\r\n"
    ));
    assert!(request.max_forwards().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Max-Forwards"));
  }

  let duplicate = parse_request(concat!(
    "OPTIONS / HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Max-Forwards: 1\r\n",
    "max-forwards: 2\r\n",
    "\r\n"
  ));
  assert!(duplicate.max_forwards().is_err());
  assert_eq!(Some("1"), duplicate.header("Max-Forwards"));
}

#[test]
fn request_te_and_prefer_parse_bounded_metadata_without_enabling_behavior() {
  let request = parse_request(concat!(
    "GET /metadata HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "TE: trailers, deflate;q=0.5\r\n",
    "Prefer: respond-async, return=minimal\r\n",
    "\r\n"
  ));

  let te = request
    .te()
    .expect("TE should parse")
    .expect("TE should exist");
  assert_eq!(2, te.len());
  assert_eq!("trailers", te.codings()[0].coding());
  assert!(te.codings()[0].is_trailers());
  assert_eq!(None, te.codings()[0].quality());
  assert_eq!("deflate", te.codings()[1].coding());
  assert_eq!(Some(500), te.codings()[1].quality());

  let preferences = request
    .prefer()
    .expect("Prefer should parse")
    .expect("Prefer should exist");
  assert_eq!(2, preferences.len());
  assert_eq!("respond-async", preferences.preferences()[0].name());
  assert_eq!(None, preferences.preferences()[0].value());
  assert_eq!("return", preferences.preferences()[1].name());
  assert_eq!(Some("minimal"), preferences.preferences()[1].value());
}

#[test]
fn request_te_and_prefer_reject_invalid_or_duplicate_metadata() {
  for value in ["trailers,, deflate", "gzip;q=1.1", "trailers;q=0.5"] {
    let request = parse_request(&format!(
      "GET /metadata HTTP/1.1\r\nHost: example.test\r\nTE: {value}\r\n\r\n"
    ));
    assert!(request.te().is_err(), "TE should reject {value:?}");
    assert_eq!(Some(value), request.header("TE"));
  }
  for value in ["return=bad value", "respond-async; wait=bad value"] {
    let request = parse_request(&format!(
      "GET /metadata HTTP/1.1\r\nHost: example.test\r\nPrefer: {value}\r\n\r\n"
    ));
    assert!(request.prefer().is_err(), "Prefer should reject {value:?}");
    assert_eq!(Some(value), request.header("Prefer"));
  }

  let duplicate_te = parse_request(concat!(
    "GET / HTTP/1.1\r\nHost: example.test\r\n",
    "TE: trailers\r\nte: TRAILERS;q=0.5\r\n\r\n"
  ));
  assert!(duplicate_te.te().is_err());

  let duplicate_prefer = parse_request(concat!(
    "GET / HTTP/1.1\r\nHost: example.test\r\n",
    "Prefer: return=minimal\r\nprefer: RETURN=representation\r\n\r\n"
  ));
  assert!(duplicate_prefer.prefer().is_err());

  assert!(HttpRequestTe::parse("gzip".repeat(64 * 1024)).is_err());
}

#[test]
fn request_trailer_header_parses_bounded_field_names_separately_from_te_trailers() {
  let request = parse_request(concat!(
    "POST /upload HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "TE: trailers\r\n",
    "Trailer: X-Checksum, x-signature\r\n",
    "Trailer: X-Checksum\r\n",
    "\r\n"
  ));

  let trailer = request
    .trailer_header()
    .expect("Trailer header should parse")
    .expect("Trailer header should be present");
  assert_eq!(vec!["x-checksum", "x-signature"], trailer.field_names());
  assert!(request
    .te()
    .expect("TE should parse")
    .expect("TE should be present")
    .codings()[0]
    .is_trailers());
}

#[test]
fn request_trailer_header_rejects_forbidden_and_invalid_field_names() {
  for value in ["Content-Length", "TE", "bad field"] {
    let request = parse_request(&format!(
      "POST /upload HTTP/1.1\r\nHost: example.test\r\nTrailer: {value}\r\n\r\n"
    ));
    assert!(
      request.trailer_header().is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn parses_request_accept_media_ranges_in_field_order() {
  let request = parse_request(concat!(
    "GET /resource HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept: text/html; level=1; q=0.7, application/json\r\n",
    "Accept: application/*; profile=compact; q=1, */*; q=0\r\n",
    "\r\n"
  ));

  let accept = request
    .accept()
    .expect("valid Accept should parse")
    .expect("Accept header should be present");

  assert_eq!(
    vec!["text/html", "application/json", "application/*", "*/*"],
    accept
      .media_ranges()
      .iter()
      .map(|range| range.media_type())
      .collect::<Vec<_>>()
  );
  assert_eq!(Some(700), accept.media_ranges()[0].quality());
  assert_eq!(vec![("level", "1")], accept.media_ranges()[0].parameters());
  assert_eq!(None, accept.media_ranges()[1].quality());
  assert_eq!(Some(1000), accept.media_ranges()[2].quality());
  assert_eq!(Some(0), accept.media_ranges()[3].quality());
}

#[test]
fn request_accept_helper_is_optional_and_keeps_raw_invalid_headers() {
  let missing = parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n");
  assert_eq!(
    None,
    missing.accept().expect("missing Accept should be valid")
  );

  let malformed = parse_request(concat!(
    "GET / HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept: text/plain; q=1.001\r\n",
    "\r\n"
  ));
  assert!(malformed.accept().is_err());
  assert_eq!(Some("text/plain; q=1.001"), malformed.header("Accept"));

  assert!(HttpAccept::parse("*/json").is_err());
  let oversized = "text/plain,".repeat(257);
  assert!(HttpAccept::parse(&oversized).is_err());
  assert!(HttpAccept::parse("a".repeat(64 * 1024 + 1)).is_err());
}

#[test]
fn request_prefer_preserves_ordered_known_and_extension_metadata() {
  let request = parse_request(concat!(
    "GET /metadata HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Prefer: respond-async, return=representation\r\n",
    "Prefer: wait=15, example-extension=enabled\r\n",
    "\r\n"
  ));

  let preferences = request
    .prefer()
    .expect("Prefer should parse")
    .expect("Prefer should be present");
  assert_eq!(4, preferences.len());
  assert_eq!("respond-async", preferences.preferences()[0].name());
  assert_eq!(None, preferences.preferences()[0].value());
  assert_eq!("return", preferences.preferences()[1].name());
  assert_eq!(Some("representation"), preferences.preferences()[1].value());
  assert_eq!("wait", preferences.preferences()[2].name());
  assert_eq!(Some("15"), preferences.preferences()[2].value());
  assert_eq!("example-extension", preferences.preferences()[3].name());
  assert_eq!(Some("enabled"), preferences.preferences()[3].value());
}

#[test]
fn request_prefer_rejects_malformed_wait_oversized_values_and_excessive_preferences() {
  for value in [
    "wait",
    "wait=-1",
    "wait=1.5",
    "wait=abc",
    "return=bad value",
    "respond-async,",
  ] {
    let request = parse_request(&format!(
      "GET /metadata HTTP/1.1\r\nHost: example.test\r\nPrefer: {value}\r\n\r\n"
    ));
    assert!(request.prefer().is_err(), "Prefer should reject {value:?}");
    assert_eq!(Some(value), request.header("Prefer"));
  }

  assert!(rttp::server::HttpRequestPreferences::parse("a".repeat(64 * 1024 + 1)).is_err());
  let too_many = (0..33)
    .map(|index| format!("extension{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(rttp::server::HttpRequestPreferences::parse(too_many).is_err());
}

#[test]
fn request_accept_ignores_extensions_after_quality() {
  let accept = HttpAccept::parse("text/html; level=1; q=0.8; foo; bar=quoted")
    .expect("Accept extensions after quality should parse");
  let range = &accept.media_ranges()[0];

  assert_eq!("text/html", range.media_type());
  assert_eq!(vec![("level", "1")], range.parameters());
  assert_eq!(Some(800), range.quality());
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
fn request_accept_encoding_parses_codings_and_quality_values() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Encoding: gzip, br;q=0.8\r\n",
    "accept-encoding: identity; q=0\r\n",
    "\r\n"
  ));

  let encodings = request
    .accept_encoding()
    .expect("Accept-Encoding should parse")
    .expect("Accept-Encoding should be present");

  assert_eq!(3, encodings.len());
  assert_eq!("gzip", encodings.codings()[0].coding());
  assert_eq!(1000, encodings.codings()[0].quality());
  assert_eq!("br", encodings.codings()[1].coding());
  assert_eq!(800, encodings.codings()[1].quality());
  assert_eq!("identity", encodings.codings()[2].coding());
  assert_eq!(0, encodings.codings()[2].quality());
}

#[test]
fn request_accept_encoding_rejects_duplicate_invalid_and_oversized_values() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .accept_encoding()
      .expect("absent Accept-Encoding should be accepted")
  );

  for value in [
    "",
    "gzip,",
    ", gzip",
    "gzip,,br",
    "bad coding",
    "gzip;q=1.1",
  ] {
    let request = parse_request(&format!(
      "GET / HTTP/1.1\r\nHost: example.test\r\nAccept-Encoding: {value}\r\n\r\n"
    ));
    assert!(
      request.accept_encoding().is_err(),
      "should reject {value:?}"
    );
  }

  let duplicate = parse_request(concat!(
    "GET / HTTP/1.1\r\nHost: example.test\r\n",
    "Accept-Encoding: gzip\r\naccept-encoding: GZIP;q=0.5\r\n\r\n"
  ));
  assert!(duplicate.accept_encoding().is_err());

  let oversized = "gzip".repeat(64 * 1024);
  assert!(HttpRequestAcceptEncodings::parse(oversized).is_err());

  let too_many = (0..33)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(HttpRequestAcceptEncodings::parse(too_many).is_err());
}

#[test]
fn request_want_content_digest_parses_algorithm_preferences() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Content-Digest: sha-256=10, sha-512=3\r\n",
    "want-content-digest: unixsum=0\r\n",
    "\r\n"
  ));

  let digest = request
    .want_content_digest()
    .expect("Want-Content-Digest should parse")
    .expect("Want-Content-Digest should be present");

  assert_eq!(3, digest.len());
  assert_eq!("sha-256", digest.entries()[0].algorithm());
  assert_eq!(10, digest.entries()[0].preference());
  assert_eq!("sha-512", digest.entries()[1].algorithm());
  assert_eq!(3, digest.entries()[1].preference());
  assert_eq!("unixsum", digest.entries()[2].algorithm());
  assert_eq!(0, digest.entries()[2].preference());
}

#[test]
fn request_want_content_digest_rejects_absent_malformed_and_preserves_raw_headers() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .want_content_digest()
      .expect("absent Want-Content-Digest should be accepted")
  );

  for value in ["", "sha-256", "sha-256=11", "sha-256=10, sha-256=3"] {
    let request = parse_request(&format!(
      "GET / HTTP/1.1\r\nHost: example.test\r\nWant-Content-Digest: {value}\r\n\r\n"
    ));
    assert!(
      request.want_content_digest().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(Some(value), request.header("Want-Content-Digest"));
  }
}

#[test]
fn request_host_parses_http11_authority() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test:8443\r\n",
    "\r\n"
  ));

  let host: HttpHost = request
    .host()
    .expect("Host should parse")
    .expect("Host should be present");

  assert_eq!("example.test", host.host());
  assert_eq!(Some("8443"), host.port());
  assert_eq!("example.test:8443", host.header_value());
}

#[test]
fn request_host_rejects_absent_duplicate_and_malformed_values() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.0\r\n\r\n")
      .host()
      .expect("absent Host should be accepted")
  );

  let duplicate = parse_request(concat!(
    "GET / HTTP/1.0\r\n",
    "Host: example.test\r\n",
    "host: other.test\r\n",
    "\r\n"
  ));
  assert!(duplicate.host().is_err());
  assert_eq!(Some("example.test"), duplicate.header("Host"));

  for value in ["", "example.test/path", "user@example.test"] {
    let request = parse_request(&format!("GET / HTTP/1.0\r\nHost: {value}\r\n\r\n"));
    assert!(request.host().is_err(), "should reject {value:?}");
    assert_eq!(Some(value), request.header("Host"));
  }
}

#[test]
fn request_want_repr_digest_parses_algorithm_preferences() {
  let request = parse_request(concat!(
    "GET /asset HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Want-Repr-Digest: sha-256=10, sha-512=3\r\n",
    "want-repr-digest: unixsum=0\r\n",
    "\r\n"
  ));

  let digest = request
    .want_repr_digest()
    .expect("Want-Repr-Digest should parse")
    .expect("Want-Repr-Digest should be present");

  assert_eq!(3, digest.len());
  assert_eq!("sha-256", digest.entries()[0].algorithm());
  assert_eq!(10, digest.entries()[0].preference());
  assert_eq!("sha-512", digest.entries()[1].algorithm());
  assert_eq!(3, digest.entries()[1].preference());
  assert_eq!("unixsum", digest.entries()[2].algorithm());
  assert_eq!(0, digest.entries()[2].preference());
}

#[test]
fn request_want_repr_digest_rejects_absent_malformed_and_preserves_raw_headers() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .want_repr_digest()
      .expect("absent Want-Repr-Digest should be accepted")
  );

  for value in ["", "sha-256", "sha-256=11", "sha-256=10, sha-256=3"] {
    let request = parse_request(&format!(
      "GET / HTTP/1.1\r\nHost: example.test\r\nWant-Repr-Digest: {value}\r\n\r\n"
    ));
    assert!(
      request.want_repr_digest().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(Some(value), request.header("Want-Repr-Digest"));
  }
}

#[test]
fn request_expectations_distinguish_continue_from_unsupported_extensions() {
  assert_eq!(
    None,
    parse_request("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .expectations()
      .expect("absent Expect should be accepted")
  );

  let request = parse_request(concat!(
    "POST / HTTP/1.1\r\nHost: example.test\r\n",
    "Expect: 100-continue\r\nExpect: preview\r\n\r\n"
  ));
  let expectations = request
    .expectations()
    .expect("Expect should parse")
    .expect("Expect should be present");
  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
}

#[test]
fn request_expectations_preserve_extension_names_with_values_and_parameters() {
  let request = parse_request(concat!(
    "POST / HTTP/1.1\r\nHost: example.test\r\n",
    "Expect: preview=sha256; chunk=1\r\n\r\n"
  ));

  let expectations = request
    .expectations()
    .expect("Expect should parse")
    .expect("Expect should be present");
  assert!(!expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
}

#[test]
fn request_expectations_reject_duplicate_and_oversized_values() {
  let duplicate = parse_request(concat!(
    "POST / HTTP/1.1\r\nHost: example.test\r\n",
    "Expect: 100-continue\r\nExpect: 100-CONTINUE\r\n\r\n"
  ));
  assert!(duplicate.expectations().is_err());

  assert!(HttpExpectations::parse("a".repeat(64 * 1024 + 1)).is_err());
}

#[test]
fn parses_request_cache_control_max_stale_without_value() {
  let cache_control =
    HttpRequestCacheControl::parse("max-stale").expect("max-stale without delta-seconds is valid");

  assert_eq!(Some(None), cache_control.max_stale());
}

#[test]
fn parses_request_accept_language_metadata() {
  let request = parse_request(concat!(
    "GET /localized HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Language: en-US, fr-CA; q=0.8\r\n",
    "Accept-Language: *;q=0\r\n",
    "\r\n"
  ));

  let languages = request
    .accept_language()
    .expect("valid Accept-Language should parse")
    .expect("Accept-Language header should be present");

  assert_eq!(vec!["en-US", "fr-CA", "*"], languages.ranges());
  assert_eq!(vec![None, Some("0.8"), Some("0")], languages.qualities());
}

#[test]
fn request_accept_language_helper_rejects_invalid_wire_values() {
  for value in [
    "en_US",
    "en; q=1.001",
    "en; q=0.1234",
    "en; level=1",
    "en, EN",
  ] {
    let request = parse_request(&format!(
      "GET /localized HTTP/1.1\r\nHost: example.test\r\nAccept-Language: {value}\r\n\r\n"
    ));
    assert!(
      request.accept_language().is_err(),
      "Accept-Language helper should reject {value:?}"
    );
  }

  assert!(
    HttpReportingEndpoints::parse(format!("default=\"{}\"", "x".repeat(64 * 1024))).is_err(),
    "should reject oversized Reporting-Endpoints fields"
  );
  assert!(
    HttpReportingEndpoints::from_endpoints(
      (0..33).map(|index| (format!("endpoint{index}"), "https://reports.example/")),
    )
    .is_err(),
    "should reject excessive Reporting-Endpoints entries"
  );
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
fn parses_vary_field_names_and_normalizes_case() {
  let vary = HttpVary::parse("Accept-Encoding, accept-language, X-User")
    .expect("valid Vary field list should parse");

  assert!(!vary.is_wildcard());
  assert_eq!(
    vec!["accept-encoding", "accept-language", "x-user"],
    vary.field_names()
  );
  assert_eq!(
    "accept-encoding, accept-language, x-user",
    vary.header_value()
  );
}

#[test]
fn parses_vary_wildcard_as_distinct_representation() {
  let vary = HttpVary::parse("*").expect("wildcard Vary should parse");

  assert!(vary.is_wildcard());
  assert!(vary.field_names().is_empty());
  assert_eq!("*", vary.header_value());
}

#[test]
fn vary_helpers_reject_malformed_values() {
  for value in [
    "",
    "Accept-Encoding,",
    ", Accept-Encoding",
    "Accept Encoding",
    "Accept-Encoding, *, Accept-Language",
    "Accept-Encoding, bad:name",
  ] {
    assert!(
      HttpVary::parse(value).is_err(),
      "Vary helper should reject {value:?}"
    );
  }

  assert!(
    HttpResponse::ok("body")
      .with_vary("Accept Encoding")
      .is_err(),
    "response Vary helper should reject invalid field names"
  );
}

#[test]
fn response_vary_helper_enforces_the_field_item_limit_before_deduplicating() {
  let value = std::iter::repeat_n("Accept-Encoding", 257)
    .collect::<Vec<_>>()
    .join(", ");

  assert!(
    HttpResponse::ok("body")
      .header("Vary", value)
      .vary()
      .is_err(),
    "all parsed Vary list members must count toward the bound, including duplicates"
  );
}

#[test]
fn response_vary_helper_combines_multiple_headers_and_deduplicates_names() {
  let response = HttpResponse::ok("body")
    .header("Vary", "Accept-Encoding, User-Agent")
    .header("vArY", "accept-encoding, X-Feature");

  let vary = response
    .vary()
    .expect("attached Vary headers should parse")
    .expect("Vary should be present");

  assert!(!vary.is_wildcard());
  assert_eq!(
    vec!["accept-encoding", "user-agent", "x-feature"],
    vary.field_names()
  );
}

#[test]
fn response_vary_helper_declares_normalized_vary_header() {
  let response = HttpResponse::ok("body")
    .with_vary("Accept-Encoding, X-User")
    .expect("valid Vary should be accepted");

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nVary: accept-encoding, x-user\r\n"));
}

#[test]
fn response_no_vary_search_helper_parses_and_declares_metadata() {
  let response = HttpResponse::ok("body")
    .header("No-Vary-Search", "params")
    .header("no-vary-search", r#"except=("session")"#);

  let no_vary_search = response
    .no_vary_search()
    .expect("attached No-Vary-Search headers should parse")
    .expect("No-Vary-Search should be present");

  assert!(no_vary_search.ignores_all_query_params());
  assert_eq!(no_vary_search.except(), ["session"]);

  let response = HttpResponse::ok("body")
    .header("No-Vary-Search", "params")
    .with_no_vary_search(r#"key-order=?0, params=("utm_source")"#)
    .expect("valid No-Vary-Search should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(!serialized.contains("\r\nNo-Vary-Search: params\r\n"));
  assert!(serialized.contains("\r\nNo-Vary-Search: key-order=?0, params=(\"utm_source\")\r\n"));
}

#[test]
fn parses_allow_methods_and_serializes_single_header_value() {
  let allow =
    HttpAllowedMethods::parse("GET, HEAD, POST").expect("valid Allow header should parse");

  assert_eq!(vec!["GET", "HEAD", "POST"], allow.methods());
  assert_eq!("GET, HEAD, POST", allow.header_value());

  let response = HttpResponse::new(405, "Method Not Allowed")
    .with_allow(["GET", "HEAD", "POST"])
    .expect("valid Allow methods should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAllow: GET, HEAD, POST\r\n"));
  assert_eq!(
    Some(allow),
    response.allow().expect("Allow header should parse")
  );
}

#[test]
fn response_allow_helper_parses_attached_header_fields() {
  let response = HttpResponse::ok("body")
    .header("Allow", "GET, HEAD")
    .header("Allow", "POST");

  let allow = response
    .allow()
    .expect("Allow header should parse")
    .expect("Allow header should be present");

  assert_eq!(vec!["GET", "HEAD", "POST"], allow.methods());
}

#[test]
fn allow_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "GET,",
    ", GET",
    "GET,,POST",
    "G ET",
    "GET, POST, GET",
    "GET, bad:name",
  ] {
    assert!(
      HttpAllowedMethods::parse(value).is_err(),
      "Allow helper should reject {value:?}"
    );
  }

  let oversized = "GET".repeat(64 * 1024);
  assert!(
    HttpAllowedMethods::parse(&oversized).is_err(),
    "Allow helper should reject oversized values"
  );

  let too_many = (0..=256)
    .map(|index| format!("M{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpAllowedMethods::parse(too_many).is_err(),
    "Allow helper should reject too many methods"
  );

  assert!(
    HttpResponse::ok("body").with_allow(["GET", "GET"]).is_err(),
    "response Allow helper should reject duplicate method values"
  );
}

#[test]
fn raw_allow_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Allow", "GET,,POST");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAllow: GET,,POST\r\n"));
  assert!(
    response.allow().is_err(),
    "typed Allow parser should reject malformed raw values"
  );
}

#[test]
fn parses_content_languages_and_serializes_single_header_value() {
  let languages = HttpContentLanguages::parse("en, fr-CA, x-private")
    .expect("valid Content-Language header should parse");

  assert_eq!(vec!["en", "fr-CA", "x-private"], languages.languages());
  assert_eq!("en, fr-CA, x-private", languages.header_value());

  let response = HttpResponse::ok("body")
    .with_content_language(["en", "fr-CA", "x-private"])
    .expect("valid Content-Language values should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Language: en, fr-CA, x-private\r\n"));
  assert_eq!(
    Some(languages),
    response
      .content_language()
      .expect("Content-Language header should parse")
  );
}

#[test]
fn response_content_language_helper_parses_attached_header_fields() {
  let response = HttpResponse::ok("body")
    .header("Content-Language", "en, fr-CA")
    .header("Content-Language", "es-419");

  let languages = response
    .content_language()
    .expect("Content-Language header should parse")
    .expect("Content-Language header should be present");

  assert_eq!(vec!["en", "fr-CA", "es-419"], languages.languages());
}

#[test]
fn content_language_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "en,",
    ", en",
    "en,,fr",
    "en us",
    "en_US",
    "en; q=1",
    "en, fr, en",
    "-en",
    "en-",
    "en--US",
  ] {
    assert!(
      HttpContentLanguages::parse(value).is_err(),
      "Content-Language helper should reject {value:?}"
    );
  }

  let oversized = "en".repeat(64 * 1024);
  assert!(
    HttpContentLanguages::parse(&oversized).is_err(),
    "Content-Language helper should reject oversized values"
  );

  let too_many = (0..33)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpContentLanguages::parse(too_many).is_err(),
    "Content-Language helper should reject too many language tags"
  );

  assert!(
    HttpResponse::ok("body")
      .with_content_language(["en", "en"])
      .is_err(),
    "response Content-Language helper should reject duplicate language tags"
  );
}

#[test]
fn raw_content_language_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Content-Language", "en,,fr");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Language: en,,fr\r\n"));
  assert!(
    response.content_language().is_err(),
    "typed Content-Language parser should reject malformed raw values"
  );
}

#[test]
fn reporting_endpoints_helpers_parse_and_build_bounded_metadata() {
  let endpoints = HttpReportingEndpoints::parse(
    "default=\"https://reports.example/default\", csp=\"https://reports.example/csp\"",
  )
  .expect("valid Reporting-Endpoints header should parse");
  assert_eq!(
    vec![
      ("default", "https://reports.example/default"),
      ("csp", "https://reports.example/csp"),
    ],
    endpoints.endpoints()
  );
  assert_eq!(
    Some("https://reports.example/csp"),
    endpoints.endpoint("csp")
  );

  let response = HttpResponse::ok("body")
    .header(
      "Reporting-Endpoints",
      "default=\"https://reports.example/default\"",
    )
    .with_reporting_endpoints([("csp", "https://reports.example/csp")])
    .expect("valid Reporting-Endpoints values should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
  assert_eq!(1, serialized.matches("\r\nReporting-Endpoints: ").count());
  assert!(serialized.contains("csp=\"https://reports.example/csp\""));
  let parsed = response
    .reporting_endpoints()
    .expect("Reporting-Endpoints header should parse")
    .expect("Reporting-Endpoints should be present");
  assert_eq!(
    vec![("csp", "https://reports.example/csp")],
    parsed.endpoints()
  );

  for value in [
    "default=https://reports.example/default",
    "Default=\"https://reports.example/default\"",
    "default=\"https://reports.example/default\", default=\"https://reports.example/other\"",
  ] {
    assert!(
      HttpReportingEndpoints::parse(value).is_err(),
      "should reject {value:?}"
    );
  }
}

#[test]
fn parses_accept_ranges_and_serializes_single_header_value() {
  let accept_ranges =
    HttpAcceptRanges::parse("bytes, custom-unit").expect("valid Accept-Ranges should parse");

  assert!(!accept_ranges.is_none());
  assert_eq!(vec!["bytes", "custom-unit"], accept_ranges.units());
  assert_eq!("bytes, custom-unit", accept_ranges.header_value());

  let response = HttpResponse::ok("body")
    .header("Accept-Ranges", "old-unit")
    .with_accept_ranges(["bytes", "custom-unit"])
    .expect("valid Accept-Ranges units should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAccept-Ranges: bytes, custom-unit\r\n"));
  assert_eq!(1, serialized.matches("\r\nAccept-Ranges: ").count());
  assert_eq!(
    Some(accept_ranges),
    response
      .accept_ranges()
      .expect("Accept-Ranges should parse")
  );
}

#[test]
fn response_accept_ranges_none_declares_none_sentinel() {
  let response = HttpResponse::ok("body")
    .header("Accept-Ranges", "bytes")
    .with_accept_ranges_none();
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");
  let accept_ranges = response
    .accept_ranges()
    .expect("Accept-Ranges should parse")
    .expect("Accept-Ranges should be present");

  assert!(accept_ranges.is_none());
  assert!(accept_ranges.units().is_empty());
  assert_eq!("none", accept_ranges.header_value());
  assert!(serialized.contains("\r\nAccept-Ranges: none\r\n"));
  assert_eq!(1, serialized.matches("\r\nAccept-Ranges: ").count());
}

#[test]
fn response_accept_ranges_helper_parses_attached_header_fields() {
  let response = HttpResponse::ok("body")
    .header("Accept-Ranges", "bytes")
    .header("Accept-Ranges", "custom-unit");

  let accept_ranges = response
    .accept_ranges()
    .expect("Accept-Ranges should parse")
    .expect("Accept-Ranges should be present");

  assert_eq!(vec!["bytes", "custom-unit"], accept_ranges.units());
}

#[test]
fn accept_ranges_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "bytes,",
    ", bytes",
    "bytes,,custom",
    "bad unit",
    "bytes, bytes",
    "none, bytes",
    "bytes, none",
    "bad:name",
  ] {
    assert!(
      HttpAcceptRanges::parse(value).is_err(),
      "Accept-Ranges helper should reject {value:?}"
    );
  }

  let oversized = "bytes".repeat(64 * 1024);
  assert!(
    HttpAcceptRanges::parse(&oversized).is_err(),
    "Accept-Ranges helper should reject oversized values"
  );

  let too_many = (0..257)
    .map(|index| format!("unit{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpAcceptRanges::parse(too_many).is_err(),
    "Accept-Ranges helper should reject too many range units"
  );

  assert!(
    HttpResponse::ok("body")
      .with_accept_ranges(["bytes", "bytes"])
      .is_err(),
    "response Accept-Ranges helper should reject duplicate range units"
  );
  assert!(
    HttpResponse::ok("body")
      .with_accept_ranges(["none"])
      .is_err(),
    "response Accept-Ranges unit helper should reject the none sentinel"
  );
}

#[test]
fn raw_accept_ranges_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Accept-Ranges", "bytes,,custom");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAccept-Ranges: bytes,,custom\r\n"));
  assert!(
    response.accept_ranges().is_err(),
    "typed Accept-Ranges parser should reject malformed raw values"
  );
}

#[test]
fn response_content_location_helper_declares_single_header_value() {
  let response = HttpResponse::ok("body")
    .header("Content-Location", "/old")
    .with_content_location(" /representations/current ")
    .expect("valid Content-Location should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Location: /representations/current\r\n"));
  assert_eq!(1, serialized.matches("\r\nContent-Location: ").count());
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
fn response_content_location_helper_parses_attached_singleton_header() {
  let response = HttpResponse::ok("body").header("Content-Location", "../variant.en");

  assert_eq!(
    "../variant.en",
    response
      .content_location()
      .expect("Content-Location should parse")
      .expect("Content-Location should be present")
      .as_str()
  );
}

#[test]
fn response_etag_helper_declares_and_parses_singleton_metadata() {
  let absent = HttpResponse::ok("body");
  assert_eq!(None, absent.etag().expect("absent ETag should parse"));

  let response = HttpResponse::ok("body")
    .header("ETag", "\"old\"")
    .with_etag(HttpEntityTag::weak("asset-v7"));
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nETag: W/\"asset-v7\"\r\n"));
  assert_eq!(1, serialized.matches("\r\nETag: ").count());
  assert_eq!(
    Some(HttpEntityTag::weak("asset-v7")),
    response.etag().expect("ETag should parse")
  );

  let response = HttpResponse::ok("body").header("ETag", "\"asset-v7\"");
  assert_eq!(
    Some(HttpEntityTag::strong("asset-v7")),
    response.etag().expect("ETag should parse")
  );
}

#[test]
fn response_etag_helper_rejects_malformed_duplicate_and_oversized_values_without_losing_raw_headers(
) {
  for value in ["abc", "W/abc", "\"bad space\""] {
    let response = HttpResponse::ok("body").header("ETag", value);
    let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

    assert!(
      response.etag().is_err(),
      "ETag helper should reject {value:?}"
    );
    assert!(
      serialized.contains(&format!("\r\nETag: {value}\r\n")),
      "raw ETag header should be preserved"
    );
  }

  let duplicate = HttpResponse::ok("body")
    .header("ETag", "\"one\"")
    .header("etag", "W/\"two\"");
  let serialized = String::from_utf8(duplicate.to_bytes()).expect("response is UTF-8");

  assert!(
    duplicate.etag().is_err(),
    "ETag helper should reject duplicate singleton headers"
  );
  assert!(serialized.contains("\r\nETag: \"one\"\r\n"));
  assert!(serialized.contains("\r\netag: W/\"two\"\r\n"));

  let oversized = format!("\"{}\"", "a".repeat(64 * 1024));
  let response = HttpResponse::ok("body").header("ETag", &oversized);
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(
    response.etag().is_err(),
    "ETag helper should reject oversized values"
  );
  assert!(
    serialized.contains(&format!("\r\nETag: {oversized}\r\n")),
    "raw oversized ETag header should be preserved"
  );
}

#[test]
fn content_location_helper_rejects_empty_control_duplicate_and_oversized_values() {
  for value in [
    "",
    " ",
    "/safe\u{7f}",
    "/safe\u{1f}",
    "/safe\r\nX-Evil: true",
  ] {
    assert!(
      HttpResponse::ok("body")
        .with_content_location(value)
        .is_err(),
      "Content-Location helper should reject {value:?}"
    );
  }

  for value in ["", " ", "/safe\u{7f}", "/safe\u{1f}"] {
    let response = HttpResponse::ok("body").header("Content-Location", value);
    assert!(
      response.content_location().is_err(),
      "Content-Location parser should reject {value:?}"
    );
  }

  let response = HttpResponse::ok("body")
    .header("Content-Location", "/one")
    .header("Content-Location", "/two");
  assert!(
    response.content_location().is_err(),
    "Content-Location parser should reject duplicate header fields"
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024 + 1));
  assert!(
    HttpResponse::ok("body")
      .with_content_location(&oversized)
      .is_err(),
    "Content-Location helper should reject oversized values"
  );

  let response = HttpResponse::ok("body").header("Content-Location", oversized);
  assert!(
    response.content_location().is_err(),
    "Content-Location parser should reject oversized raw values"
  );
}

#[test]
fn parses_content_disposition_and_serializes_single_header_value() {
  let disposition = HttpContentDisposition::parse(
    "attachment; filename=\"report \\\"Q1\\\".txt\"; creation-date=2026-07-09",
  )
  .expect("valid Content-Disposition should parse");

  assert_eq!("attachment", disposition.disposition_type());
  assert_eq!(Some("report \"Q1\".txt"), disposition.parameter("filename"));
  assert_eq!(Some("2026-07-09"), disposition.parameter("creation-date"));

  let response = HttpResponse::ok("body")
    .header("Content-Disposition", "inline")
    .header("content-disposition", "attachment; filename=old.txt")
    .with_content_disposition(disposition)
    .expect("valid Content-Disposition should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains(
    "\r\nContent-Disposition: attachment; filename=\"report \\\"Q1\\\".txt\"; creation-date=2026-07-09\r\n"
  ));
  assert_eq!(1, serialized.matches("\r\nContent-Disposition: ").count());
  assert_eq!(
    Some("report \"Q1\".txt"),
    response
      .content_disposition()
      .expect("Content-Disposition should parse")
      .expect("Content-Disposition should be present")
      .parameter("filename")
  );
}

#[test]
fn response_link_metadata_parses_multiple_values_and_preserves_unknown_parameters() {
  let response = HttpResponse::ok("body")
    .header(
      "Link",
      "</style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload",
    )
    .header(
      "link",
      "<../manifest.json>; type=\"application/manifest+json\"; anchor=\"/app\"",
    );

  let links = response
    .links()
    .expect("Link metadata should parse")
    .expect("Link metadata should be present");

  assert_eq!(3, links.len());
  assert_eq!("/style.css", links.values()[0].target());
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
  assert_eq!(Some("style"), links.values()[0].parameter("as"));
  assert_eq!(
    "https://cdn.example.test/app.js",
    links.values()[1].target()
  );
  assert_eq!(Some("modulepreload"), links.values()[1].parameter("rel"));
  assert_eq!("../manifest.json", links.values()[2].target());
  assert_eq!(
    vec![("type", "application/manifest+json"), ("anchor", "/app")],
    links.values()[2].parameters()
  );
}

#[test]
fn response_link_metadata_preserves_valueless_extensions_and_empty_quoted_values() {
  let response =
    HttpResponse::ok("body").header("Link", "</style.css>; rel=preload; nopush; title=\"\"");

  let links = response
    .links()
    .expect("Link metadata should parse")
    .expect("Link metadata should be present");

  assert_eq!(Some(""), links.values()[0].parameter("nopush"));
  assert_eq!(Some(""), links.values()[0].parameter("title"));
  assert_eq!(
    vec![("rel", "preload"), ("nopush", ""), ("title", "")],
    links.values()[0].parameters()
  );
}

#[test]
fn response_link_metadata_accepts_obs_text_in_quoted_parameter_values() {
  let response = HttpResponse::ok("body").header("Link", r#"</style.css>; title="\é""#);

  let links = response
    .links()
    .expect("Link metadata should parse")
    .expect("Link metadata should be present");

  assert_eq!(Some("é"), links.values()[0].parameter("title"));
}

#[test]
fn response_link_metadata_rejects_invalid_and_bounded_values_without_losing_headers() {
  for value in [
    "style.css; rel=preload",
    "<style.css; rel=preload",
    "</style.css> rel=preload",
    "</style.css>; =preload",
    "</style.css>; bad name=value",
    "</style.css>; rel=\"unterminated",
    "<foo bar>",
    "<foo\tbar>",
    r"<foo\bar>",
    "<a%zz>",
    "<a%2>",
    "<a%>",
    "<foo\"bar>",
    "<foo^bar>",
    "<foo`bar>",
    "<foo|bar>",
    "<caf\u{e9}>",
    "</style.css>; rel=",
    "</style.css>; rel= ",
    "</style.css>; rel =",
    "</style.css>; rel = ",
  ] {
    let response = HttpResponse::ok("body").header("Link", value);
    assert!(
      response.links().is_err(),
      "Link parser should reject {value:?}"
    );
    assert!(
      String::from_utf8(response.to_bytes())
        .expect("response should remain UTF-8")
        .contains(&format!("\r\nLink: {value}\r\n")),
      "raw Link header should remain available"
    );
  }

  let oversized = format!("</{}>", "a".repeat(64 * 1024));
  assert!(HttpLinkValues::parse(oversized).is_err());

  let too_many = (0..257)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(HttpLinkValues::parse(too_many).is_err());

  let too_many_parameters = format!(
    "</asset>{}",
    (0..257)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(HttpLinkValues::parse(too_many_parameters).is_err());

  let oversized_parameter = format!("</asset>; title={}", "a".repeat(64 * 1024 + 1));
  assert!(HttpLinkValues::parse(oversized_parameter).is_err());
}

#[test]
fn content_disposition_helpers_declare_common_dispositions_with_safe_parameters() {
  let attachment = HttpContentDisposition::attachment()
    .with_parameter("filename", "financial report.txt")
    .expect("safe filename parameter should be accepted");
  let inline = HttpContentDisposition::inline();

  assert_eq!(
    "attachment; filename=\"financial report.txt\"",
    attachment.header_value()
  );
  assert_eq!("inline", inline.header_value());

  let response = HttpResponse::ok("body")
    .with_attachment_filename("financial report.txt")
    .expect("attachment filename should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized
    .contains("\r\nContent-Disposition: attachment; filename=\"financial report.txt\"\r\n"));
}

#[test]
fn response_content_disposition_helper_parses_attached_singleton_header() {
  let response =
    HttpResponse::ok("body").header("Content-Disposition", "inline; filename=readme.txt");

  let disposition = response
    .content_disposition()
    .expect("Content-Disposition should parse")
    .expect("Content-Disposition should be present");

  assert_eq!("inline", disposition.disposition_type());
  assert_eq!(Some("readme.txt"), disposition.parameter("filename"));
}

#[test]
fn content_disposition_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "bad type",
    "attachment;",
    "attachment; filename",
    "attachment; filename=",
    "attachment; bad name=value",
    "attachment; filename=\"unterminated",
    "attachment; filename=\"bad\\\"",
    "attachment; filename=\"bad\r\nX-Evil: yes\"",
    "attachment; filename=one; FILENAME=two",
    "attachment; filename=\"bad\u{7f}\"",
  ] {
    assert!(
      HttpContentDisposition::parse(value).is_err(),
      "Content-Disposition helper should reject {value:?}"
    );
  }

  let oversized = format!("attachment; filename=\"{}\"", "a".repeat(64 * 1024));
  assert!(
    HttpContentDisposition::parse(&oversized).is_err(),
    "Content-Disposition helper should reject oversized values"
  );

  let too_many = format!(
    "attachment{}",
    (0..33)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(
    HttpContentDisposition::parse(too_many).is_err(),
    "Content-Disposition helper should reject too many parameters"
  );

  assert!(
    HttpContentDisposition::attachment()
      .with_parameter("bad name", "value")
      .is_err(),
    "Content-Disposition builder should reject invalid parameter names"
  );
  assert!(
    HttpContentDisposition::attachment()
      .with_parameter("filename", "bad\r\nX-Evil: yes")
      .is_err(),
    "Content-Disposition builder should reject CR/LF injection"
  );
  assert!(
    HttpContentDisposition::attachment()
      .with_parameter("filename", "caf\u{e9}.txt")
      .is_err(),
    "Content-Disposition builder should reject values that cannot be safely quoted"
  );
  assert!(
    HttpContentDisposition::attachment()
      .with_parameter("filename", "one")
      .and_then(|disposition| disposition.with_parameter("FILENAME", "two"))
      .is_err(),
    "Content-Disposition builder should reject duplicate parameters"
  );

  let response = HttpResponse::ok("body")
    .header("Content-Disposition", "inline")
    .header("Content-Disposition", "attachment");
  assert!(
    response.content_disposition().is_err(),
    "Content-Disposition parser should reject duplicate header fields"
  );
}

#[test]
fn raw_content_disposition_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Content-Disposition", "attachment;");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Disposition: attachment;\r\n"));
  assert!(
    response.content_disposition().is_err(),
    "typed Content-Disposition parser should reject malformed raw values"
  );
}

#[test]
fn parses_content_encoding_and_serializes_single_header_value() {
  let encodings =
    HttpResponseContentEncodings::parse("gzip, br").expect("valid Content-Encoding should parse");

  assert_eq!(vec!["gzip", "br"], encodings.codings());
  assert_eq!("gzip, br", encodings.header_value());

  let response = HttpResponse::ok("body")
    .header("Content-Encoding", "old")
    .with_content_encoding(["gzip", "br"])
    .expect("valid Content-Encoding should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Encoding: gzip, br\r\n"));
  assert_eq!(1, serialized.matches("\r\nContent-Encoding: ").count());
  assert_eq!(
    Some(encodings),
    response
      .content_encoding()
      .expect("Content-Encoding should parse")
  );
}

#[test]
fn parses_content_type_and_serializes_single_header_value() {
  let content_type = HttpContentType::parse("Text/HTML; Charset=\"utf-8\"; boundary=abc-123")
    .expect("valid Content-Type should parse");

  assert_eq!("text/html", content_type.media_type());
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));
  assert_eq!(Some("abc-123"), content_type.parameter("boundary"));
  assert_eq!(
    "text/html; charset=utf-8; boundary=abc-123",
    content_type.header_value()
  );

  let response = HttpResponse::ok("body")
    .header("Content-Type", "text/plain")
    .header("content-type", "application/octet-stream")
    .with_content_type(content_type)
    .expect("valid Content-Type should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Type: text/html; charset=utf-8; boundary=abc-123\r\n"));
  assert_eq!(1, serialized.matches("\r\nContent-Type: ").count());
  assert_eq!(
    Some("utf-8"),
    response
      .content_type()
      .expect("Content-Type should parse")
      .expect("Content-Type should be present")
      .parameter("charset")
  );
}

#[test]
fn response_content_encoding_helper_parses_attached_header_fields_in_order() {
  let response = HttpResponse::ok("body")
    .header("Content-Encoding", "gzip, br")
    .header("content-encoding", "identity");

  let encodings = response
    .content_encoding()
    .expect("Content-Encoding should parse")
    .expect("Content-Encoding should be present");

  assert_eq!(vec!["gzip", "br", "identity"], encodings.codings());
}

#[test]
fn content_encoding_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "gzip,",
    ", gzip",
    "gzip,,br",
    "bad coding",
    "gzip, g:zip",
  ] {
    assert!(
      HttpResponseContentEncodings::parse(value).is_err(),
      "Content-Encoding helper should reject {value:?}"
    );
  }

  assert!(
    HttpResponseContentEncodings::parse("gzip, br, GZIP").is_err(),
    "Content-Encoding helper should reject duplicate codings"
  );

  let oversized = "gzip".repeat(64 * 1024);
  assert!(
    HttpResponseContentEncodings::parse(&oversized).is_err(),
    "Content-Encoding helper should reject oversized values"
  );

  let too_many = (0..33)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    HttpResponseContentEncodings::parse(too_many).is_err(),
    "Content-Encoding helper should reject excessive codings"
  );

  assert!(
    HttpResponse::ok("body")
      .with_content_encoding(["gzip", "bad coding"])
      .is_err(),
    "Content-Encoding declaration helper should reject invalid codings"
  );
}

#[test]
fn content_type_helpers_declare_common_media_types_with_safe_parameters() {
  let content_type = HttpContentType::new("application", "json")
    .expect("valid media type should be accepted")
    .with_parameter("charset", "utf-8")
    .expect("safe parameter should be accepted");

  assert_eq!(
    "application/json; charset=utf-8",
    content_type.header_value()
  );

  let response = HttpResponse::ok("{}")
    .with_content_type("Application/JSON; Charset=utf-8")
    .expect("Content-Type string should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Type: application/json; charset=utf-8\r\n"));
}

#[test]
fn response_content_type_helper_parses_attached_singleton_header() {
  let response = HttpResponse::ok("body").header("Content-Type", "text/plain; charset=utf-8");

  let content_type = response
    .content_type()
    .expect("Content-Type should parse")
    .expect("Content-Type should be present");

  assert_eq!("text/plain", content_type.media_type());
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));
}

#[test]
fn content_type_helpers_reject_malformed_duplicate_oversized_and_excessive_values() {
  for value in [
    "",
    " ",
    "text",
    "text/",
    "/plain",
    "text /plain",
    "text/plain;",
    "text/plain; charset",
    "text/plain; charset=",
    "text/plain; bad name=value",
    "text/plain; charset=\"unterminated",
    "text/plain; charset=\"bad\\\"",
    "text/plain; charset=\"bad\r\nX-Evil: yes\"",
    "text/plain; charset=utf-8; CHARSET=us-ascii",
    "text/plain; charset=\"bad\u{7f}\"",
  ] {
    assert!(
      HttpContentType::parse(value).is_err(),
      "Content-Type helper should reject {value:?}"
    );
  }

  let oversized = format!("text/plain; charset=\"{}\"", "a".repeat(64 * 1024));
  assert!(
    HttpContentType::parse(&oversized).is_err(),
    "Content-Type helper should reject oversized values"
  );

  let too_many = format!(
    "text/plain{}",
    (0..33)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(
    HttpContentType::parse(too_many).is_err(),
    "Content-Type helper should reject too many parameters"
  );

  assert!(
    HttpContentType::new("bad type", "plain").is_err(),
    "Content-Type builder should reject invalid type tokens"
  );
  assert!(
    HttpContentType::new("text", "plain")
      .and_then(|content_type| content_type.with_parameter("bad name", "value"))
      .is_err(),
    "Content-Type builder should reject invalid parameter names"
  );
  assert!(
    HttpContentType::new("text", "plain")
      .and_then(|content_type| content_type.with_parameter("charset", "bad\r\nX-Evil: yes"))
      .is_err(),
    "Content-Type builder should reject CR/LF injection"
  );
  assert!(
    HttpContentType::new("text", "plain")
      .and_then(|content_type| content_type.with_parameter("charset", "caf\u{e9}"))
      .is_err(),
    "Content-Type builder should reject values that cannot be safely quoted"
  );
  assert!(
    HttpContentType::new("text", "plain")
      .and_then(|content_type| content_type.with_parameter("charset", "utf-8"))
      .and_then(|content_type| content_type.with_parameter("CHARSET", "us-ascii"))
      .is_err(),
    "Content-Type builder should reject duplicate parameters"
  );

  let response = HttpResponse::ok("body")
    .header("Content-Type", "text/plain")
    .header("Content-Type", "application/json");
  assert!(
    response.content_type().is_err(),
    "Content-Type parser should reject duplicate header fields"
  );
}

#[test]
fn raw_content_encoding_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Content-Encoding", "gzip,");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Encoding: gzip,\r\n"));
  assert!(
    response.content_encoding().is_err(),
    "typed Content-Encoding parser should reject malformed raw values"
  );
}

#[test]
fn raw_content_type_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body").header("Content-Type", "text/plain;");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-Type: text/plain;\r\n"));
  assert!(
    response.content_type().is_err(),
    "typed Content-Type parser should reject malformed raw values"
  );
}

#[test]
fn response_age_and_expires_helpers_declare_metadata_headers() {
  let expires = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let response = HttpResponse::ok("body").with_age(60).with_expires(expires);

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAge: 60\r\n"));
  assert!(serialized.contains("\r\nExpires: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
  assert_eq!(Some(60), response.age().expect("Age should parse"));
  assert_eq!(
    Some(expires),
    response.expires().expect("Expires should parse")
  );
}

#[test]
fn response_sunset_helper_declares_and_parses_metadata() {
  let sunset = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let response = HttpResponse::ok("body").with_sunset(sunset);

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nSunset: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
  assert_eq!(
    Some(sunset),
    response.sunset().expect("Sunset should parse")
  );
}

#[test]
fn response_sunset_helper_replaces_existing_metadata() {
  let initial_sunset = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let sunset = initial_sunset + Duration::from_secs(1);
  let response = HttpResponse::ok("body")
    .header("Sunset", httpdate::fmt_http_date(initial_sunset))
    .with_sunset(initial_sunset)
    .with_sunset(sunset);

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert_eq!(1, serialized.matches("\r\nSunset: ").count());
  assert_eq!(
    Some(sunset),
    response.sunset().expect("Sunset should parse")
  );
}

#[test]
fn response_sunset_helper_rejects_invalid_and_duplicate_raw_values() {
  for response in [
    HttpResponse::ok("body").header("Sunset", "not a date"),
    HttpResponse::ok("body")
      .header("Sunset", "Sun, 06 Nov 1994 08:49:37 GMT")
      .header("Sunset", "Sun, 06 Nov 1994 08:49:38 GMT"),
  ] {
    assert!(
      response.sunset().is_err(),
      "Sunset helper should reject invalid metadata"
    );
  }
}

#[test]
fn response_age_and_expires_helpers_parse_raw_metadata_headers() {
  let response = HttpResponse::ok("body")
    .header("Age", "0")
    .header("Expires", "Sunday, 06-Nov-94 08:49:37 GMT");

  assert_eq!(Some(0), response.age().expect("Age should parse"));
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784_111_777)),
    response.expires().expect("Expires should parse")
  );
}

#[test]
fn response_retry_after_helpers_declare_delta_seconds_and_http_date() {
  let retry_at = UNIX_EPOCH + Duration::from_secs(784_111_777);

  let delta_response = HttpResponse::new(503, "Service Unavailable").with_retry_after_delta(120);
  let date_response = HttpResponse::new(503, "Service Unavailable").with_retry_after_date(retry_at);

  let delta_serialized = String::from_utf8(delta_response.to_bytes()).expect("response is UTF-8");
  let date_serialized = String::from_utf8(date_response.to_bytes()).expect("response is UTF-8");

  assert!(delta_serialized.contains("\r\nRetry-After: 120\r\n"));
  assert!(date_serialized.contains("\r\nRetry-After: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
  assert_eq!(
    Some(HttpRetryAfter::DeltaSeconds(120)),
    delta_response
      .retry_after()
      .expect("Retry-After delta should parse")
  );
  assert_eq!(
    Some(HttpRetryAfter::HttpDate(retry_at)),
    date_response
      .retry_after()
      .expect("Retry-After date should parse")
  );
}

#[test]
fn response_retry_after_helper_parses_raw_delta_seconds_and_http_date() {
  let delta_response = HttpResponse::ok("body").header("Retry-After", "0");
  let date_response =
    HttpResponse::ok("body").header("Retry-After", "Sunday, 06-Nov-94 08:49:37 GMT");

  assert_eq!(
    Some(HttpRetryAfter::DeltaSeconds(0)),
    delta_response
      .retry_after()
      .expect("Retry-After delta should parse")
  );
  assert_eq!(
    Some(HttpRetryAfter::HttpDate(
      UNIX_EPOCH + Duration::from_secs(784_111_777)
    )),
    date_response
      .retry_after()
      .expect("Retry-After date should parse")
  );
}

#[test]
fn response_retry_after_helper_rejects_malformed_overflowing_or_oversized_values() {
  for value in [
    "",
    " ",
    "-1",
    "+1",
    "1.5",
    "abc",
    "0, 60",
    "18446744073709551616",
    "Sun, 06 Nov 1994 08:49:37 PST",
  ] {
    let response = HttpResponse::ok("body").header("Retry-After", value);

    assert!(
      response.retry_after().is_err(),
      "Retry-After helper should reject {value:?}"
    );
  }

  let response = HttpResponse::ok("body").header("Retry-After", "1".repeat(64 * 1024 + 1));
  assert!(
    response.retry_after().is_err(),
    "Retry-After helper should reject oversized values"
  );
}

#[test]
fn response_retry_after_helper_rejects_duplicate_values() {
  let response = HttpResponse::ok("body")
    .header("Retry-After", "60")
    .header("Retry-After", "120");

  assert!(
    response.retry_after().is_err(),
    "Retry-After helper should reject duplicate header fields"
  );
}

#[test]
fn response_age_helper_rejects_malformed_or_overflowing_values() {
  for value in ["", " ", "-1", "+1", "1.5", "abc", "18446744073709551616"] {
    let response = HttpResponse::ok("body").header("Age", value);

    assert!(
      response.age().is_err(),
      "Age helper should reject {value:?}"
    );
  }
}

#[test]
fn response_expires_helper_rejects_malformed_values() {
  for value in ["", "not a date", "Sun, 06 Nov 1994 08:49:37 PST"] {
    let response = HttpResponse::ok("body").header("Expires", value);

    assert!(
      response.expires().is_err(),
      "Expires helper should reject {value:?}"
    );
  }
}

#[test]
fn raw_age_and_expires_headers_are_preserved_without_helper_validation() {
  let response = HttpResponse::ok("body")
    .header("Age", "not-a-delta")
    .header("Expires", "not-a-date")
    .header("Retry-After", "not-a-delta-or-date");

  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nAge: not-a-delta\r\n"));
  assert!(serialized.contains("\r\nExpires: not-a-date\r\n"));
  assert!(serialized.contains("\r\nRetry-After: not-a-delta-or-date\r\n"));
}

#[test]
fn selects_request_headers_named_by_vary_case_insensitively() {
  let request = parse_request(concat!(
    "GET /cached HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Encoding: gzip\r\n",
    "accept-encoding: br\r\n",
    "X-User: 123\r\n",
    "\r\n"
  ));
  let vary =
    HttpVary::parse("ACCEPT-ENCODING, x-user, accept-language").expect("Vary should parse");

  let selection = request.vary_selection(&vary);

  assert!(!selection.is_wildcard());
  assert_eq!(
    vec!["accept-encoding", "x-user", "accept-language"],
    selection.field_names()
  );
  assert_eq!(vec!["gzip", "br"], selection.values("Accept-Encoding"));
  assert_eq!(vec!["123"], selection.values("x-user"));
  assert!(selection.values("accept-language").is_empty());
}

#[test]
fn wildcard_vary_selection_does_not_read_specific_request_headers() {
  let request = parse_request(concat!(
    "GET /cached HTTP/1.1\r\n",
    "Host: example.test\r\n",
    "Accept-Encoding: gzip\r\n",
    "\r\n"
  ));
  let vary = HttpVary::wildcard();

  let selection = request.vary_selection(&vary);

  assert!(selection.is_wildcard());
  assert!(selection.fields().is_empty());
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
    Some(HttpContentRange::Bytes {
      start: 3,
      end: 6,
      complete_length: Some(10),
    }),
    response
      .content_range()
      .expect("Content-Range should parse")
  );
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
    Some(HttpContentRange::Unsatisfied {
      complete_length: 10,
    }),
    response
      .content_range()
      .expect("Content-Range should parse")
  );
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

#[test]
fn early_hints_serializes_link_metadata_without_body_or_content_length() {
  let response = HttpResponse::early_hints([
    r#"</style.css>; rel=preload; as=style"#,
    r#"</app.js>; rel=preload; as=script"#,
  ])
  .expect("early hints should build");

  assert_eq!(
    concat!(
      "HTTP/1.1 103 Early Hints\r\n",
      "Link: </style.css>; rel=preload; as=style\r\n",
      "Link: </app.js>; rel=preload; as=script\r\n",
      "\r\n"
    )
    .as_bytes(),
    response.body("ignored").to_bytes().as_slice()
  );
}

#[test]
fn early_hints_accepts_safe_metadata_headers() {
  let response = HttpResponse::early_hints_with_headers(
    [r#"</style.css>; rel=preload; as=style"#],
    [("Server", "rttp"), ("Cache-Control", "public, max-age=60")],
  )
  .expect("safe metadata should build");

  assert_eq!(
    concat!(
      "HTTP/1.1 103 Early Hints\r\n",
      "Link: </style.css>; rel=preload; as=style\r\n",
      "Server: rttp\r\n",
      "Cache-Control: public, max-age=60\r\n",
      "\r\n"
    )
    .as_bytes(),
    response.to_bytes().as_slice()
  );
}

#[test]
fn early_hints_rejects_invalid_injected_forbidden_and_oversized_headers() {
  assert!(HttpResponse::early_hints([r#"</style.css>; rel=preload; as=style"#]).is_ok());
  assert!(HttpResponse::early_hints([""]).is_err());
  assert!(HttpResponse::early_hints(["/safe\r\nX-Evil: true"]).is_err());
  assert!(HttpResponse::early_hints(["x".repeat(64 * 1024 + 1)]).is_err());

  for name in [
    "",
    "Bad Name",
    "Content-Length",
    "Transfer-Encoding",
    "Connection",
    "TE",
    "Trailer",
    "Upgrade",
    "Keep-Alive",
    "Proxy-Connection",
  ] {
    assert!(
      HttpResponse::early_hints_with_headers(
        [r#"</style.css>; rel=preload; as=style"#],
        [(name, "safe")]
      )
      .is_err(),
      "{name:?} metadata header should reject"
    );
  }

  assert!(HttpResponse::early_hints_with_headers(
    [r#"</style.css>; rel=preload; as=style"#],
    [("X-Trace", "safe\r\nX-Evil: true")]
  )
  .is_err());
  assert!(HttpResponse::early_hints_with_headers(
    [r#"</style.css>; rel=preload; as=style"#],
    [("X-Trace", "x".repeat(64 * 1024 + 1))]
  )
  .is_err());
}

#[test]
fn early_hints_rejects_links_that_links_parser_rejects() {
  for value in [
    "style.css; rel=preload",
    "<foo bar>",
    "<foo\tbar>",
    "<a%zz>",
    "<a%2>",
    "<a%>",
    "<foo\"bar>",
    "<caf\u{e9}>",
    "</style.css>; rel=",
    "</style.css>; rel= ",
  ] {
    assert!(
      HttpResponse::early_hints([value]).is_err(),
      "early_hints should reject {value:?}"
    );
  }

  let response = HttpResponse::early_hints([r#"</style.css>; rel=preload; as=style"#])
    .expect("valid RFC 8288 link should build");
  let links = response
    .links()
    .expect("early-hints Link should parse")
    .expect("Link metadata should be present");
  assert_eq!(1, links.len());
  assert_eq!("/style.css", links.values()[0].target());
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
}
