use rttp_client::response::{
  AltSvc, AltUsed, AuthenticationInfo, ContentDisposition, ContentDpr, ContentEncoding,
  ContentLocation, ContentRange, ContentSecurityPolicy, ContentSecurityPolicyReportOnly,
  ContentType, CrossOriginEmbedderPolicy, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginOpenerPolicy, CrossOriginResourcePolicy, Deprecation, DocumentPolicy,
  DocumentPolicyReportOnly, DocumentPolicyReportOnlyValue, DocumentPolicyValue, EntityTag,
  HttpClearSiteData, HttpSetCookies, KeepAlive, LinkValues, Location, MementoDatetime,
  OriginTrials, PermissionsPolicy, ProxyAuthenticate, ProxyAuthenticationInfo, ProxyStatus,
  ProxyStatusBareItem, ReferrerPolicy, ReferrerPolicyToken, Response, RetryAfter,
  SecWebSocketAccept, SecWebSocketExtensions, SecWebSocketProtocol, SecWebSocketVersion,
  ServerTiming, ServiceWorkerAllowed, SignatureInput, SpeculationRules, StrictTransportSecurity,
  SupportsLoadingMode, Via, Warning, XContentTypeOptions, XFrameOptions,
};
use rttp_client::types::{Cookie, RoUrl};
use rttp_protocol::sec_websocket_key::SecWebSocketKey;
use std::io::Write;
use std::time::{Duration, UNIX_EPOCH};

fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
  let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
  encoder.write_all(bytes).expect("write gzip fixture");
  encoder.finish().expect("finish gzip fixture")
}

#[test]
fn test_parse_cookie_name_can_match_attribute_name() {
  let same_site = Cookie::parse("SameSite=choice; Path=/").unwrap();
  assert_eq!("SameSite", same_site.name());
  assert_eq!("choice", same_site.value());
  assert_eq!(Some(&"/".to_string()), same_site.path().as_ref());

  let path = Cookie::parse("Path=value; HttpOnly").unwrap();
  assert_eq!("Path", path.name());
  assert_eq!("value", path.value());
  assert!(path.http_only());
}

#[test]
fn sec_websocket_accept_response_helper_parses_and_verifies_metadata() {
  let key =
    SecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");
  let response = Response::new(
    RoUrl::with("https://example.test/chat"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let accept = response
    .sec_websocket_accept()
    .expect("Sec-WebSocket-Accept should parse")
    .expect("Sec-WebSocket-Accept should be present");
  assert_eq!("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", accept.as_str());
  assert_eq!("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", accept.header_value());
  assert!(accept.verify_key(&key));
  assert!(response
    .verify_sec_websocket_accept(&key)
    .expect("Sec-WebSocket-Accept should verify"));
  assert!(!format!("{accept:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert!(!format!("{accept:?}").contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

  let derived = SecWebSocketAccept::derive_from_key(&key);
  assert_eq!(derived, accept);
}

#[test]
fn sec_websocket_accept_response_helper_handles_absent_mismatch_and_invalid_metadata() {
  let key =
    SecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");

  let absent = Response::new(
    RoUrl::with("https://example.test/chat"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");
  assert_eq!(
    None,
    absent.sec_websocket_accept().expect("absent should parse")
  );
  assert!(!absent
    .verify_sec_websocket_accept(&key)
    .expect("absent accept should not verify"));

  let mismatch = Response::new(
    RoUrl::with("https://example.test/chat"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Accept: AAAAAAAAAAAAAAAAAAAAAAAAAAA=\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");
  assert!(!mismatch
    .verify_sec_websocket_accept(&key)
    .expect("mismatched accept should parse"));

  let duplicate = Response::new(
    RoUrl::with("https://example.test/chat"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n",
      "sec-websocket-accept: AAAAAAAAAAAAAAAAAAAAAAAAAAA=\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should preserve duplicate metadata");
  let error = duplicate
    .sec_websocket_accept()
    .expect_err("duplicate Sec-WebSocket-Accept should fail");
  let message = error.to_string();
  assert!(message.contains("Sec-WebSocket-Accept"));
  assert!(!message.contains("dGhlIHNhbXBsZSBub25jZQ=="));
}

#[test]
fn clear_site_data_metadata_parses_quoted_directives_and_wildcard_without_clearing_state() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Clear-Site-Data: \"cache\", \"cookies\"\r\n",
      "Clear-Site-Data: \"storage\", \"executionContexts\"\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");
  let metadata = response
    .clear_site_data()
    .expect("Clear-Site-Data should parse")
    .expect("Clear-Site-Data should be present");
  assert_eq!(
    vec!["cache", "cookies", "storage", "executionContexts"],
    metadata
      .directives()
      .iter()
      .map(|directive| directive.as_str())
      .collect::<Vec<_>>()
  );
  assert!(metadata.clears_cache());
  assert!(metadata.clears_cookies());
  assert!(metadata.clears_storage());
  assert!(metadata.clears_execution_contexts());

  let wildcard = HttpClearSiteData::parse("\"*\"").expect("wildcard should parse");
  assert!(wildcard.is_wildcard());
  assert!(wildcard.clears_cache());
  assert!(wildcard.clears_cookies());
  assert!(wildcard.clears_storage());
  assert!(wildcard.clears_execution_contexts());
}

#[test]
fn signature_input_metadata_parses_without_verifying_signatures() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Signature-Input: sig1=(\"@method\" \"@path\");created=1700000000\r\n",
      "Signature-Input: sig2=(\"content-digest\";sf);keyid=\"test-key\"\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .signature_input()
    .expect("Signature-Input should parse")
    .expect("Signature-Input should be present");

  assert_eq!(metadata.members()[0].label(), "sig1");
  assert_eq!(
    metadata.members()[1].covered_components()[0].identifier(),
    "content-digest"
  );
  assert_eq!(
    response.header_values("Signature-Input").len(),
    2,
    "raw headers should remain available"
  );
}

#[test]
fn signature_input_metadata_errors_without_hiding_raw_headers() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nSignature-Input: sig1=(content-digest)\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("response should parse");

  assert!(response.signature_input().is_err());
  assert_eq!(
    response.header_value("Signature-Input"),
    Some(&"sig1=(content-digest)".to_string())
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");
  let _: Option<SignatureInput> = absent
    .signature_input()
    .expect("absent Signature-Input should not fail");
}

#[test]
fn clear_site_data_metadata_rejects_invalid_duplicate_and_unbounded_values() {
  for value in [
    "cache",
    "\"unknown\"",
    "\"cache\", \"cache\"",
    "\"unterminated",
  ] {
    assert!(
      HttpClearSiteData::parse(value).is_err(),
      "should reject {value:?}"
    );
  }
  assert!(HttpClearSiteData::parse(format!("\"{}\"", "x".repeat(64 * 1024))).is_err());
  assert!(HttpClearSiteData::parse(
    std::iter::repeat_n("\"cache\"", 257)
      .collect::<Vec<_>>()
      .join(","),
  )
  .is_err());
}

#[test]
fn referrer_policy_metadata_preserves_repeated_declarations_and_raw_headers() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Referrer-Policy: origin\r\n",
      "Referrer-Policy: origin\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response
      .referrer_policy()
      .expect("Referrer-Policy should parse")
      .expect("Referrer-Policy should be present")
      .policies(),
    &[ReferrerPolicyToken::Origin, ReferrerPolicyToken::Origin]
  );
  assert_eq!(
    response.header_values("Referrer-Policy"),
    [&"origin".to_string(), &"origin".to_string()]
  );
}

#[test]
fn referrer_policy_metadata_rejects_malformed_and_oversized_values_without_hiding_headers() {
  for value in ["", "invalid", "origin,"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nReferrer-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.referrer_policy().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Referrer-Policy"),
      Some(&value.to_string())
    );
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nReferrer-Policy: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.referrer_policy().is_err());
  assert_eq!(response.header_value("Referrer-Policy"), Some(&oversized));
}

#[test]
fn referrer_policy_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(response.referrer_policy().expect("header is absent"), None);
  let _: Option<ReferrerPolicy> = response.referrer_policy().expect("header is absent");
}

#[test]
fn strict_transport_security_metadata_parses_flags_without_applying_policy() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Strict-Transport-Security: max-age=31536000; includeSubDomains; preload\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .strict_transport_security()
    .expect("Strict-Transport-Security should parse")
    .expect("Strict-Transport-Security should be present");

  assert_eq!(metadata.max_age(), 31_536_000);
  assert!(metadata.include_sub_domains());
  assert!(metadata.preload());
  assert_eq!(
    response.header_value("Strict-Transport-Security"),
    Some(&"max-age=31536000; includeSubDomains; preload".to_string())
  );
}

#[test]
fn strict_transport_security_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    "includeSubDomains",
    "max-age=abc",
    "max-age=60; preload=true",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nStrict-Transport-Security: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.strict_transport_security().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Strict-Transport-Security"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Strict-Transport-Security: max-age=60\r\n",
      "Strict-Transport-Security: max-age=120\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.strict_transport_security().is_err());
  assert_eq!(
    response.header_values("Strict-Transport-Security"),
    [&"max-age=60".to_string(), &"max-age=120".to_string()]
  );
}

#[test]
fn strict_transport_security_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response
      .strict_transport_security()
      .expect("header is absent"),
    None
  );
  let _: Option<StrictTransportSecurity> = response
    .strict_transport_security()
    .expect("header is absent");
}

#[test]
fn x_content_type_options_metadata_parses_nosniff_without_applying_policy() {
  for value in ["nosniff", "NoSniff"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nX-Content-Type-Options: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    let metadata = response
      .x_content_type_options()
      .expect("X-Content-Type-Options should parse")
      .expect("X-Content-Type-Options should be present");

    assert_eq!(metadata, XContentTypeOptions::Nosniff);
    assert_eq!(metadata.header_value(), "nosniff");
    assert_eq!(
      response.header_value("X-Content-Type-Options"),
      Some(&value.to_string())
    );
  }
}

#[test]
fn content_security_policy_metadata_preserves_opaque_value_without_enforcing_policy() {
  let value = "default-src 'self'; object-src 'none'";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nContent-Security-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .content_security_policy()
    .expect("Content-Security-Policy should parse")
    .expect("Content-Security-Policy should be present");

  assert_eq!(metadata.as_str(), value);
  assert_eq!(metadata.header_value(), value);
  assert_eq!(metadata.header_values(), [value]);
  assert_eq!(
    response.header_value("Content-Security-Policy"),
    Some(&value.to_string())
  );
}

#[test]
fn content_security_policy_metadata_preserves_layered_policy_fields() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Security-Policy: default-src 'self'\r\n",
      "content-security-policy: object-src 'none'\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .content_security_policy()
    .expect("Content-Security-Policy should parse")
    .expect("Content-Security-Policy should be present");

  assert_eq!(metadata.as_str(), "default-src 'self'");
  assert_eq!(
    metadata.header_values(),
    ["default-src 'self'", "object-src 'none'"]
  );
  assert_eq!(
    response.header_value("Content-Security-Policy"),
    Some(&"default-src 'self'".to_string())
  );
}

#[test]
fn content_security_policy_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in ["", "default-src 'self'\u{7f}"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nContent-Security-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.content_security_policy().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Content-Security-Policy"),
      Some(&value.to_string())
    );
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nContent-Security-Policy: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");
  assert!(response.content_security_policy().is_err());
  assert_eq!(
    response.header_value("Content-Security-Policy"),
    Some(&oversized)
  );
}

#[test]
fn content_security_policy_metadata_rejects_too_many_repeated_fields() {
  let mut bytes = String::from("HTTP/1.1 200 OK\r\n");
  for _ in 0..=rttp_protocol::content_security_policy::MAX_CONTENT_SECURITY_POLICY_FIELDS {
    bytes.push_str("Content-Security-Policy: default-src 'self'\r\n");
  }
  bytes.push_str("Content-Length: 0\r\n\r\n");

  let response = Response::new(RoUrl::with("https://example.test"), bytes.into_bytes())
    .expect("response should parse");

  assert!(response.content_security_policy().is_err());
  assert_eq!(
    response.header_values("Content-Security-Policy").len(),
    rttp_protocol::content_security_policy::MAX_CONTENT_SECURITY_POLICY_FIELDS + 1
  );
}

#[test]
fn content_security_policy_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response
      .content_security_policy()
      .expect("header is absent"),
    None
  );
  let _: Option<ContentSecurityPolicy> = response
    .content_security_policy()
    .expect("header is absent");
}

#[test]
fn content_security_policy_report_only_metadata_preserves_layered_policy_fields() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Security-Policy-Report-Only: default-src 'self'\r\n",
      "content-security-policy-report-only: object-src 'none'\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .content_security_policy_report_only()
    .expect("Content-Security-Policy-Report-Only should parse")
    .expect("Content-Security-Policy-Report-Only should be present");

  assert_eq!(metadata.as_str(), "default-src 'self'");
  assert_eq!(
    metadata.header_values(),
    ["default-src 'self'", "object-src 'none'"]
  );
  assert_eq!(
    response.header_value("Content-Security-Policy-Report-Only"),
    Some(&"default-src 'self'".to_string())
  );
}

#[test]
fn content_security_policy_report_only_metadata_rejects_invalid_values_without_hiding_raw_headers()
{
  for value in ["", "default-src 'self'\u{7f}"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!(
        "HTTP/1.1 200 OK\r\nContent-Security-Policy-Report-Only: {value}\r\nContent-Length: 0\r\n\r\n"
      )
      .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.content_security_policy_report_only().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Content-Security-Policy-Report-Only"),
      Some(&value.to_string())
    );
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 200 OK\r\nContent-Security-Policy-Report-Only: {oversized}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");
  assert!(response.content_security_policy_report_only().is_err());
  assert_eq!(
    response.header_value("Content-Security-Policy-Report-Only"),
    Some(&oversized)
  );
}

#[test]
fn content_security_policy_report_only_metadata_rejects_too_many_repeated_fields() {
  let mut bytes = String::from("HTTP/1.1 200 OK\r\n");
  for _ in 0..=rttp_protocol::content_security_policy_report_only::MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS {
    bytes.push_str("Content-Security-Policy-Report-Only: default-src 'self'\r\n");
  }
  bytes.push_str("Content-Length: 0\r\n\r\n");

  let response = Response::new(RoUrl::with("https://example.test"), bytes.into_bytes())
    .expect("response should parse");

  assert!(response.content_security_policy_report_only().is_err());
  assert_eq!(
    response.header_values("Content-Security-Policy-Report-Only").len(),
    rttp_protocol::content_security_policy_report_only::MAX_CONTENT_SECURITY_POLICY_REPORT_ONLY_FIELDS
      + 1
  );
}

#[test]
fn content_security_policy_report_only_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  let _: Option<ContentSecurityPolicyReportOnly> = response
    .content_security_policy_report_only()
    .expect("Content-Security-Policy-Report-Only helper should parse absent header");
}

#[test]
fn x_content_type_options_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in ["", "unknown", "nosniff, nosniff"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nX-Content-Type-Options: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.x_content_type_options().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("X-Content-Type-Options"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "X-Content-Type-Options: nosniff\r\n",
      "X-Content-Type-Options: nosniff\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.x_content_type_options().is_err());
  assert_eq!(
    response.header_values("X-Content-Type-Options"),
    [&"nosniff".to_string(), &"nosniff".to_string()]
  );
}

#[test]
fn x_content_type_options_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response.x_content_type_options().expect("header is absent"),
    None
  );
  let _: Option<XContentTypeOptions> = response.x_content_type_options().expect("header is absent");
}

#[test]
fn x_frame_options_metadata_parses_tokens_without_applying_policy() {
  for (value, expected) in [
    ("DENY", XFrameOptions::Deny),
    ("deny", XFrameOptions::Deny),
    ("SAMEORIGIN", XFrameOptions::SameOrigin),
    ("SameOrigin", XFrameOptions::SameOrigin),
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nX-Frame-Options: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    let metadata = response
      .x_frame_options()
      .expect("X-Frame-Options should parse")
      .expect("X-Frame-Options should be present");

    assert_eq!(metadata, expected);
    assert_eq!(metadata.header_value(), expected.header_value());
    assert_eq!(
      response.header_value("X-Frame-Options"),
      Some(&value.to_string())
    );
  }
}

#[test]
fn x_frame_options_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    "unknown",
    "ALLOW-FROM https://example.test",
    "DENY, SAMEORIGIN",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nX-Frame-Options: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.x_frame_options().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("X-Frame-Options"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "X-Frame-Options: DENY\r\n",
      "X-Frame-Options: SAMEORIGIN\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.x_frame_options().is_err());
  assert_eq!(
    response.header_values("X-Frame-Options"),
    [&"DENY".to_string(), &"SAMEORIGIN".to_string()]
  );
}

#[test]
fn x_frame_options_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(response.x_frame_options().expect("header is absent"), None);
  let _: Option<XFrameOptions> = response.x_frame_options().expect("header is absent");
}

#[test]
fn permissions_policy_metadata_parses_dictionary_without_enforcing_policy() {
  let value = r#"geolocation=(self "https://maps.example.test"), camera=()"#;
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nPermissions-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .permissions_policy()
    .expect("Permissions-Policy should parse")
    .expect("Permissions-Policy should be present");

  assert_eq!(metadata.directives().len(), 2);
  let geolocation = metadata
    .directive("geolocation")
    .expect("geolocation present");
  assert_eq!(geolocation.feature(), "geolocation");
  assert!(!geolocation.allowlist().is_all_origins());
  assert_eq!(geolocation.allowlist().members().len(), 2);
  assert!(geolocation.allowlist().members()[0].is_self());
  assert_eq!(
    geolocation.allowlist().members()[1].origin(),
    Some("https://maps.example.test")
  );
  let camera = metadata.directive("camera").expect("camera present");
  assert!(camera.allowlist().is_empty());
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Permissions-Policy"),
    Some(&value.to_string())
  );
}

#[test]
fn permissions_policy_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    "geolocation=src",
    "geolocation=('none')",
    "geolocation=*;unknown=1",
    "geolocation=(* \"https://example.test\")",
    "geolocation=\"https://example.test/path\"",
    "geolocation=5",
    "geolocation=(\"https://example.test\" \"https://example.test\")",
    "geolocation=(self), geolocation=(self)",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nPermissions-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.permissions_policy().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Permissions-Policy"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Permissions-Policy: geolocation=(self)\r\n",
      "Permissions-Policy: geolocation=(self)\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.permissions_policy().is_err());
  assert_eq!(
    response.header_values("Permissions-Policy"),
    [
      &"geolocation=(self)".to_string(),
      &"geolocation=(self)".to_string()
    ]
  );
}

#[test]
fn permissions_policy_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = format!(
    "geolocation=(\"{}\")",
    "https://example.test/".repeat(64 * 1024)
  );
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nPermissions-Policy: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.permissions_policy().is_err());
  assert_eq!(
    response.header_value("Permissions-Policy"),
    Some(&oversized)
  );
}

#[test]
fn permissions_policy_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response.permissions_policy().expect("header is absent"),
    None
  );
  let _: Option<PermissionsPolicy> = response.permissions_policy().expect("header is absent");
}

#[test]
fn document_policy_metadata_parses_dictionary_without_enforcing_policy() {
  let value = "oversized-images=2.0, unsized-media=?0, *;report-to=default";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nDocument-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .document_policy()
    .expect("Document-Policy should parse")
    .expect("Document-Policy should be present");

  assert_eq!(metadata.directives().len(), 3);
  assert_eq!(
    metadata.directive("oversized-images").unwrap().value(),
    &DocumentPolicyValue::Decimal("2.0".to_string())
  );
  assert_eq!(
    metadata.directive("unsized-media").unwrap().value(),
    &DocumentPolicyValue::Boolean(false)
  );
  assert_eq!(
    metadata.directive("*").unwrap().report_to(),
    Some("default")
  );
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Document-Policy"),
    Some(&value.to_string())
  );
}

#[test]
fn document_policy_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    "=()",
    "=(1 2)",
    "=\"2.0\"",
    "=:MjA=:",
    "=@123",
    "=%\"2.0\"",
    "=+2.0",
    "=1.2345",
    "unsized-media=src;foo=bar",
    "oversized-images=1;report-to=first;report-to=second",
    "oversized-images=1.0, oversized-images=2.0",
    "UnSized-Media=?0",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nDocument-Policy: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert_eq!(response.code(), 200);
    assert!(response.body().binary().is_empty());
    assert!(
      response.document_policy().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Document-Policy"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Document-Policy: oversized-images=1.0\r\n",
      "Document-Policy: oversized-images=2.0\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert_eq!(response.code(), 200);
  assert!(response.body().binary().is_empty());
  assert!(response.document_policy().is_err());
  assert_eq!(
    response.header_values("Document-Policy"),
    [
      &"oversized-images=1.0".to_string(),
      &"oversized-images=2.0".to_string()
    ]
  );
}

#[test]
fn document_policy_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = format!("x={}", "a".repeat(64 * 1024));
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nDocument-Policy: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  assert_eq!(response.code(), 200);
  assert!(response.body().binary().is_empty());
  assert!(response.document_policy().is_err());
  assert_eq!(response.header_value("Document-Policy"), Some(&oversized));
}

#[test]
fn document_policy_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(response.document_policy().expect("header is absent"), None);
  let _: Option<DocumentPolicy> = response.document_policy().expect("header is absent");
}

#[test]
fn document_policy_report_only_metadata_parses_without_enforcing_or_reporting() {
  let value = "oversized-images=2.0, unsized-media=?0, *;report-to=default";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nDocument-Policy-Report-Only: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .document_policy_report_only()
    .expect("Document-Policy-Report-Only should parse")
    .expect("Document-Policy-Report-Only should be present");

  assert_eq!(metadata.directives().len(), 3);
  assert_eq!(
    metadata.directive("oversized-images").unwrap().value(),
    &DocumentPolicyReportOnlyValue::Decimal("2.0".to_string())
  );
  assert_eq!(
    metadata.directive("*").unwrap().report_to(),
    Some("default")
  );
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Document-Policy-Report-Only"),
    Some(&value.to_string())
  );
}

#[test]
fn document_policy_report_only_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    "=()",
    "=1.2345",
    "unsized-media=src;foo=bar",
    "oversized-images=1;report-to=first;report-to=second",
    "oversized-images=1.0, oversized-images=2.0",
    "UnSized-Media=?0",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!(
        "HTTP/1.1 200 OK\r\nDocument-Policy-Report-Only: {value}\r\nContent-Length: 0\r\n\r\n"
      )
      .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.document_policy_report_only().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Document-Policy-Report-Only"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Document-Policy-Report-Only: oversized-images=1.0\r\n",
      "Document-Policy-Report-Only: oversized-images=2.0\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.document_policy_report_only().is_err());
  assert_eq!(
    response.header_values("Document-Policy-Report-Only"),
    [
      &"oversized-images=1.0".to_string(),
      &"oversized-images=2.0".to_string()
    ]
  );
}

#[test]
fn document_policy_report_only_metadata_rejects_oversized_values_and_absent_headers() {
  let oversized = format!("x={}", "a".repeat(64 * 1024));
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 200 OK\r\nDocument-Policy-Report-Only: {oversized}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.document_policy_report_only().is_err());
  assert_eq!(
    response.header_value("Document-Policy-Report-Only"),
    Some(&oversized)
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    absent
      .document_policy_report_only()
      .expect("header is absent"),
    None
  );
  let _: Option<DocumentPolicyReportOnly> = absent
    .document_policy_report_only()
    .expect("header is absent");
}

#[test]
fn supports_loading_mode_metadata_parses_tokens_without_applying_loading_policy() {
  let value = "fenced-frame, credentialed-prerender";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nSupports-Loading-Mode: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .supports_loading_mode()
    .expect("Supports-Loading-Mode should parse")
    .expect("Supports-Loading-Mode should be present");

  assert_eq!(
    metadata.tokens(),
    ["fenced-frame", "credentialed-prerender"]
  );
  assert!(metadata.contains_fenced_frame());
  assert!(metadata.contains_credentialed_prerender());
  assert!(!metadata.contains_prerender_cross_origin_frames());
  assert!(metadata.contains("CREDENTIALED-PRERENDER"));
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Supports-Loading-Mode"),
    Some(&value.to_string())
  );
}

#[test]
fn supports_loading_mode_metadata_retains_unknown_tokens_and_combines_fields() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Supports-Loading-Mode: uncredentialed-prerender\r\n",
      "Supports-Loading-Mode: fenced-frame\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .supports_loading_mode()
    .expect("Supports-Loading-Mode should parse")
    .expect("Supports-Loading-Mode should be present");

  assert_eq!(
    metadata.tokens(),
    ["uncredentialed-prerender", "fenced-frame"]
  );
  assert_eq!(
    metadata.header_value(),
    "uncredentialed-prerender, fenced-frame"
  );
  assert_eq!(
    response.header_values("Supports-Loading-Mode"),
    [
      &"uncredentialed-prerender".to_string(),
      &"fenced-frame".to_string()
    ]
  );
}

#[test]
fn supports_loading_mode_metadata_rejects_invalid_values_without_hiding_raw_headers() {
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
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!("HTTP/1.1 200 OK\r\nSupports-Loading-Mode: {value}\r\nContent-Length: 0\r\n\r\n")
        .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.supports_loading_mode().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Supports-Loading-Mode"),
      Some(&value.to_string())
    );
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Supports-Loading-Mode: fenced-frame\r\n",
      "Supports-Loading-Mode: Fenced-Frame\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(response.supports_loading_mode().is_err());
  assert_eq!(
    response.header_values("Supports-Loading-Mode"),
    [&"fenced-frame".to_string(), &"Fenced-Frame".to_string()]
  );
}

#[test]
fn supports_loading_mode_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = format!("fenced-frame{}", "x".repeat(64 * 1024));
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nSupports-Loading-Mode: {oversized}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.supports_loading_mode().is_err());
  assert_eq!(
    response.header_value("Supports-Loading-Mode"),
    Some(&oversized)
  );
}

#[test]
fn sec_websocket_version_metadata_parses_version_13_without_switching_protocols() {
  let value = "13";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Version: {value}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .sec_websocket_version()
    .expect("Sec-WebSocket-Version should parse")
    .expect("Sec-WebSocket-Version should be present");

  assert_eq!(metadata.versions(), ["13"]);
  assert!(metadata.contains("13"));
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Sec-WebSocket-Version"),
    Some(&value.to_string())
  );
  assert_eq!(response.header_value("Connection"), None);
  assert_eq!(response.header_value("Upgrade"), None);
}

#[test]
fn sec_websocket_version_metadata_combines_fields_in_wire_order() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 400 Bad Request\r\n",
      "Sec-WebSocket-Version: 13\r\n",
      "Sec-WebSocket-Version: 8, 7\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let metadata = response
    .sec_websocket_version()
    .expect("Sec-WebSocket-Version should parse")
    .expect("Sec-WebSocket-Version should be present");

  assert_eq!(metadata.versions(), ["13", "8", "7"]);
  assert_eq!(metadata.header_value(), "13, 8, 7");
  assert_eq!(
    response.header_values("Sec-WebSocket-Version"),
    [&"13".to_string(), &"8, 7".to_string()]
  );
}

#[test]
fn sec_websocket_version_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in ["", "13,", "v13", "013", "8, 13", "13, 13", "300"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!(
        "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Version: {value}\r\nContent-Length: 0\r\n\r\n"
      )
      .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.sec_websocket_version().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Sec-WebSocket-Version"),
      Some(&value.to_string())
    );
  }
}

#[test]
fn sec_websocket_version_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = "1".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Version: {oversized}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.sec_websocket_version().is_err());
  assert_eq!(
    response.header_value("Sec-WebSocket-Version"),
    Some(&oversized)
  );
}

#[test]
fn sec_websocket_version_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response.sec_websocket_version().expect("header is absent"),
    None
  );
  let _: Option<SecWebSocketVersion> = response.sec_websocket_version().expect("header is absent");
}

#[test]
fn sec_websocket_protocol_metadata_parses_selected_token_without_switching_protocols() {
  let value = "graphql-transport-ws";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 101 Switching Protocols\r\nSec-WebSocket-Protocol: {value}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .sec_websocket_protocol()
    .expect("Sec-WebSocket-Protocol should parse")
    .expect("Sec-WebSocket-Protocol should be present");

  assert_eq!(metadata.protocols(), ["graphql-transport-ws"]);
  assert_eq!(metadata.selected(), Some("graphql-transport-ws"));
  assert!(metadata.contains("graphql-transport-ws"));
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Sec-WebSocket-Protocol"),
    Some(&value.to_string())
  );
  assert_eq!(response.header_value("Connection"), None);
  assert_eq!(response.header_value("Upgrade"), None);
}

#[test]
fn sec_websocket_protocol_metadata_rejects_multi_token_selections() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Protocol: chat, superchat\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(
    response.sec_websocket_protocol().is_err(),
    "a selection must be a singleton"
  );
  assert_eq!(
    response.header_value("Sec-WebSocket-Protocol"),
    Some(&"chat, superchat".to_string())
  );

  let combined = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Protocol: chat\r\n",
      "Sec-WebSocket-Protocol: superchat\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");
  assert!(
    combined.sec_websocket_protocol().is_err(),
    "combined selection fields must still be a singleton"
  );
  assert_eq!(
    combined.header_values("Sec-WebSocket-Protocol"),
    [&"chat".to_string(), &"superchat".to_string()]
  );
}

#[test]
fn sec_websocket_protocol_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in ["", ",", "not a token", "chat;foo", "chat/1", "chat, chat"] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!(
        "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Protocol: {value}\r\nContent-Length: 0\r\n\r\n"
      )
      .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.sec_websocket_protocol().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Sec-WebSocket-Protocol"),
      Some(&value.to_string())
    );
  }
}

#[test]
fn sec_websocket_protocol_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = "a".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Protocol: {oversized}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.sec_websocket_protocol().is_err());
  assert_eq!(
    response.header_value("Sec-WebSocket-Protocol"),
    Some(&oversized)
  );
}

#[test]
fn sec_websocket_protocol_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response.sec_websocket_protocol().expect("header is absent"),
    None
  );
  let _: Option<SecWebSocketProtocol> =
    response.sec_websocket_protocol().expect("header is absent");
}

#[test]
fn sec_websocket_extensions_metadata_parses_selected_extension_without_switching_protocols() {
  let value = r#"permessage-deflate; client_no_context_takeover; mode="safe""#;
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 101 Switching Protocols\r\nSec-WebSocket-Extensions: {value}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  let metadata = response
    .sec_websocket_extensions()
    .expect("Sec-WebSocket-Extensions should parse")
    .expect("Sec-WebSocket-Extensions should be present");
  let selected = metadata.selected().expect("selected extension");

  assert_eq!(selected.token(), "permessage-deflate");
  assert_eq!(selected.parameters().len(), 2);
  assert_eq!(metadata.header_value(), value);
  assert_eq!(
    response.header_value("Sec-WebSocket-Extensions"),
    Some(&value.to_string())
  );
  assert_eq!(response.header_value("Connection"), None);
  assert_eq!(response.header_value("Upgrade"), None);
}

#[test]
fn sec_websocket_extensions_metadata_rejects_multi_extension_selections() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 101 Switching Protocols\r\n",
      "Sec-WebSocket-Extensions: permessage-deflate, x-test\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert!(
    response.sec_websocket_extensions().is_err(),
    "a response selection must be a singleton extension"
  );
  assert_eq!(
    response.header_value("Sec-WebSocket-Extensions"),
    Some(&"permessage-deflate, x-test".to_string())
  );
}

#[test]
fn sec_websocket_extensions_metadata_rejects_invalid_values_without_hiding_raw_headers() {
  for value in [
    "",
    ",",
    "permessage deflate",
    "permessage-deflate;",
    "permessage-deflate; p=",
    "permessage-deflate; p=1; p=2",
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test"),
      format!(
        "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Extensions: {value}\r\nContent-Length: 0\r\n\r\n"
      )
      .into_bytes(),
    )
    .expect("response should parse");

    assert!(
      response.sec_websocket_extensions().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      response.header_value("Sec-WebSocket-Extensions"),
      Some(&value.to_string())
    );
  }
}

#[test]
fn sec_websocket_extensions_metadata_rejects_oversized_values_without_hiding_raw_headers() {
  let oversized = "a".repeat(64 * 1024 + 1);
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Extensions: {oversized}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("response should parse");

  assert!(response.sec_websocket_extensions().is_err());
  assert_eq!(
    response.header_value("Sec-WebSocket-Extensions"),
    Some(&oversized)
  );
}

#[test]
fn sec_websocket_extensions_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response
      .sec_websocket_extensions()
      .expect("header is absent"),
    None
  );
  let _: Option<SecWebSocketExtensions> = response
    .sec_websocket_extensions()
    .expect("header is absent");
}

#[test]
fn supports_loading_mode_metadata_is_absent_without_a_header() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    response.supports_loading_mode().expect("header is absent"),
    None
  );
  let _: Option<SupportsLoadingMode> = response.supports_loading_mode().expect("header is absent");
}

#[test]
fn test_parse_response() {
  let s = "HTTP/1.1 200 OK\r\n\
        Content-Length: 18\r\n\
        Server: GWS/2.0\r\n\
        Date: Sat, 11 Jan 2003 02:44:04 GMT\r\n\
        Content-Type: text/html\r\n\
        Cache-control: private\r\n\
        Set-Cookie: 1P_JAR=2019-11-21-07; expires=Sat, 21-Dec-2019 07:23:44 GMT; path=/; domain=.example.test; SameSite=none\r\n\
        Connection: keep-alive\r\n\
        \r\n\
        <html>hello</html>";
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec());
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
  let cookies = response.cookies();
  println!("{:#?}", cookies);
}

#[test]
fn response_success_status_boundaries() {
  for (code, expected_success, expected_ok) in [
    (199, false, false),
    (200, true, true),
    (204, true, false),
    (299, true, false),
    (300, false, false),
  ] {
    let raw = format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("response should parse");

    assert_eq!(expected_success, response.is_success(), "status {code}");
    assert_eq!(expected_ok, response.ok(), "status {code}");
  }
}

#[test]
fn response_error_status_boundaries() {
  for (code, expected_client_error, expected_server_error) in [
    (199, false, false),
    (200, false, false),
    (300, false, false),
    (399, false, false),
    (400, true, false),
    (404, true, false),
    (499, true, false),
    (500, false, true),
    (599, false, true),
    (600, false, false),
  ] {
    let raw = format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("response should parse");

    assert_eq!(
      expected_client_error,
      response.is_client_error(),
      "status {code}"
    );
    assert_eq!(
      expected_server_error,
      response.is_server_error(),
      "status {code}"
    );
  }
}

#[test]
#[rustfmt::skip]
fn response_status_family_boundaries() {
  for (code, expected_informational, expected_redirection, expected_error) in [(99, false, false, false), (100, true, false, false), (199, true, false, false), (200, false, false, false), (299, false, false, false), (300, false, true, false), (304, false, true, false), (399, false, true, false), (400, false, false, true), (499, false, false, true), (500, false, false, true), (599, false, false, true), (600, false, false, false)] {
    let raw = format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes()).expect("response should parse");
    assert_eq!((expected_informational, expected_redirection, expected_error), (response.is_informational(), response.is_redirection(), response.is_error()), "status {code}");
  }
}

#[test]
fn response_existing_status_helpers_reject_out_of_range_codes() {
  for code in [99, 600] {
    let raw = format!("HTTP/1.1 {code} Test\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("response framing should remain inspectable");

    assert_eq!(
      (false, false, false),
      (
        response.is_success(),
        response.is_client_error(),
        response.is_server_error()
      ),
      "status {code}"
    );
  }
}

#[test]
fn response_redirection_family_is_broader_than_redirect_following() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert!(response.is_redirection() && !response.is_redirect());
}

#[test]
fn test_parse_response_preserves_duplicate_headers_with_case_insensitive_lookup() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Set-Cookie: session=abc; Path=/; HttpOnly\r\n",
    "cache-control: no-cache\r\n",
    "SET-COOKIE: theme=dark; Path=/; SameSite=Lax\r\n",
    "Cache-Control: max-age=60\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse raw duplicate header response");

  let header_lines = response
    .headers()
    .iter()
    .map(|header| (header.name().as_str(), header.value().as_str()))
    .collect::<Vec<_>>();
  assert_eq!(
    vec![
      ("Set-Cookie", "session=abc; Path=/; HttpOnly"),
      ("cache-control", "no-cache"),
      ("SET-COOKIE", "theme=dark; Path=/; SameSite=Lax"),
      ("Cache-Control", "max-age=60"),
      ("Content-Length", "2")
    ],
    header_lines
  );

  assert_eq!(
    vec![
      "session=abc; Path=/; HttpOnly",
      "theme=dark; Path=/; SameSite=Lax"
    ],
    response
      .headers_of_name("set-cookie")
      .iter()
      .map(|header| header.value().as_str())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec!["no-cache", "max-age=60"],
    response
      .headers_of_name("CACHE-CONTROL")
      .iter()
      .map(|header| header.value().as_str())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec![
      &"session=abc; Path=/; HttpOnly".to_string(),
      &"theme=dark; Path=/; SameSite=Lax".to_string()
    ],
    response.header_values("SET-cookie")
  );
  assert_eq!(2, response.cookies().len());
  assert_eq!(
    Some("abc"),
    response
      .cookie("session")
      .map(|cookie| cookie.value().as_str())
  );
  assert_eq!(
    Some("dark"),
    response
      .cookie("theme")
      .map(|cookie| cookie.value().as_str())
  );
}

#[test]
fn test_parse_response_rejects_folded_and_invalid_line_break_headers() {
  for raw in [
    b"HTTP/1.1 200 OK\r\nX-Test: first\r\n second\r\nContent-Length: 0\r\n\r\n".as_slice(),
    b"HTTP/1.1 200 OK\r\nX-Test: first\r\n\tsecond\r\nContent-Length: 0\r\n\r\n".as_slice(),
    b"HTTP/1.1 200 OK\r\nX-Test: first\nsecond\r\nContent-Length: 0\r\n\r\n".as_slice(),
    b"HTTP/1.1 200 OK\r\nX-Test: first\rsecond\r\nContent-Length: 0\r\n\r\n".as_slice(),
  ] {
    let error = Response::new(RoUrl::with("https://example.test"), raw.to_vec())
      .expect_err("folded and invalid response header line breaks must be rejected");
    assert!(error.to_string().contains("Invalid response header"));
  }
}

#[test]
fn test_parse_response_preserves_obs_text_header_values_as_latin1_code_points() {
  let raw = b"HTTP/1.1 200 OK\r\nX-Obs: \x80\xc3\xa9\xff\r\nContent-Length: 0\r\n\r\n";
  let response = Response::new(RoUrl::with("https://example.test"), raw.to_vec())
    .expect("obs-text response header should parse");

  // Header values are exposed as `String`, so each accepted raw obs-text byte is
  // represented by the matching Latin-1 code point rather than returned as bytes.
  assert_eq!(
    Some(&"\u{0080}\u{00c3}\u{00a9}\u{00ff}".to_string()),
    response.header_value("X-Obs")
  );
}

#[test]
fn test_parse_response_preserves_non_ows_obs_text_header_value_edges() {
  let raw = b"HTTP/1.1 200 OK\r\nX-Obs: \xa0value\xa0\r\nContent-Length: 0\r\n\r\n";
  let response = Response::new(RoUrl::with("https://example.test"), raw.to_vec())
    .expect("obs-text response header should parse");

  assert_eq!(
    Some(&"\u{00a0}value\u{00a0}".to_string()),
    response.header_value("X-Obs")
  );
}

#[test]
fn test_parse_reporting_endpoints_response_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Reporting-Endpoints: default=\"https://reports.example/default\"\r\n",
    "Reporting-Endpoints: csp=\"https://reports.example/csp\"\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("response should parse");
  let endpoints = response
    .reporting_endpoints()
    .expect("Reporting-Endpoints metadata should parse")
    .expect("Reporting-Endpoints should be present");
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

  let invalid = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nReporting-Endpoints: Default=\"https://reports.example/default\"\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response should remain inspectable");
  assert!(invalid.reporting_endpoints().is_err());
  assert_eq!(
    Some(&r#"Default="https://reports.example/default""#.to_string()),
    invalid.header_value("Reporting-Endpoints")
  );
}

#[test]
fn test_parse_reporting_endpoints_escaped_duplicate_and_bounded_metadata() {
  let escaped = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Reporting-Endpoints: default=\"https://reports.example/a\\\"b\\\\c\"\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("escaped Reporting-Endpoints response should parse");
  let endpoints = escaped
    .reporting_endpoints()
    .expect("escaped Reporting-Endpoints should parse")
    .expect("Reporting-Endpoints should be present");
  assert_eq!(
    Some(r#"https://reports.example/a"b\c"#),
    endpoints.endpoint("default")
  );

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Reporting-Endpoints: default=\"https://reports.example/default\"\r\n",
      "Reporting-Endpoints: default=\"https://reports.example/other\"\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain inspectable");
  assert!(duplicate.reporting_endpoints().is_err());
  assert_eq!(
    vec![
      r#"default="https://reports.example/default""#,
      r#"default="https://reports.example/other""#,
    ],
    duplicate
      .header_values("Reporting-Endpoints")
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
  );

  let oversized_value = format!(r#"default="{}""#, "x".repeat(64 * 1024));
  let oversized = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 200 OK\r\nReporting-Endpoints: {oversized_value}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("raw response should remain inspectable");
  assert!(oversized.reporting_endpoints().is_err());
  assert_eq!(
    Some(&oversized_value),
    oversized.header_value("Reporting-Endpoints")
  );

  let excessive_value = (0..33)
    .map(|index| format!(r#"endpoint{index}="https://reports.example/""#))
    .collect::<Vec<_>>()
    .join(", ");
  let excessive = Response::new(
    RoUrl::with("https://example.test"),
    format!(
      "HTTP/1.1 200 OK\r\nReporting-Endpoints: {excessive_value}\r\nContent-Length: 0\r\n\r\n"
    )
    .into_bytes(),
  )
  .expect("raw response should remain inspectable");
  assert!(excessive.reporting_endpoints().is_err());
  assert_eq!(
    Some(&excessive_value),
    excessive.header_value("Reporting-Endpoints")
  );
}

#[test]
fn test_parse_speculation_rules_response_metadata_preserves_singleton_value() {
  let value = "https://example.test/speculation-rules.json";
  let response = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nSpeculation-Rules: {value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("response should parse");
  let rules = response
    .speculation_rules()
    .expect("Speculation-Rules metadata should parse")
    .expect("Speculation-Rules should be present");

  assert_eq!(rules.as_str(), value);
  assert_eq!(
    rules,
    SpeculationRules::parse(value).expect("canonical Speculation-Rules should parse")
  );
  assert_eq!(
    Some(&value.to_string()),
    response.header_value("Speculation-Rules")
  );
  assert!(!format!("{rules:?}").contains(value));
  let headers_debug = format!("{:?}", response.headers());
  assert!(headers_debug.contains("[REDACTED]"));
  assert!(!headers_debug.contains(value));
  let response_debug = format!("{response:?}");
  assert!(response_debug.contains("[REDACTED]"));
  assert!(!response_debug.contains(value));
}

#[test]
fn test_parse_speculation_rules_rejects_duplicate_and_unsafe_values_without_hiding_headers() {
  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Speculation-Rules: https://example.test/one.json\r\n",
      "speculation-rules: https://example.test/two.json\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain inspectable");
  assert!(duplicate.speculation_rules().is_err());
  assert_eq!(
    vec![
      "https://example.test/one.json",
      "https://example.test/two.json"
    ],
    duplicate
      .header_values("Speculation-Rules")
      .iter()
      .map(|value| value.as_str())
      .collect::<Vec<_>>()
  );

  let oversized_value = "x".repeat(64 * 1024 + 1);
  let oversized = Response::new(
    RoUrl::with("https://example.test"),
    format!("HTTP/1.1 200 OK\r\nSpeculation-Rules: {oversized_value}\r\nContent-Length: 0\r\n\r\n")
      .into_bytes(),
  )
  .expect("raw response should remain inspectable");
  assert!(oversized.speculation_rules().is_err());
  assert_eq!(
    Some(&oversized_value),
    oversized.header_value("Speculation-Rules")
  );
  let headers_debug = format!("{:?}", oversized.headers());
  assert!(headers_debug.contains("[REDACTED]"));
  assert!(!headers_debug.contains(&oversized_value));
  let response_debug = format!("{oversized:?}");
  assert!(response_debug.contains("[REDACTED]"));
  assert!(!response_debug.contains(&oversized_value));
}

#[test]
fn set_cookie_metadata_is_bounded_and_preserves_unknown_attributes() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Set-Cookie: session=abc; Path=/; Priority=high; Partitioned\r\n",
    "SET-COOKIE: theme=dark; SameSite=Lax\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("response should parse");
  let cookies = response
    .set_cookies()
    .expect("Set-Cookie metadata should parse")
    .expect("Set-Cookie headers should be present");
  assert_eq!(2, cookies.cookies().len());
  assert_eq!("session", cookies.cookies()[0].name());
  assert_eq!("abc", cookies.cookies()[0].value());
  assert_eq!(
    vec![
      ("Path", Some("/")),
      ("Priority", Some("high")),
      ("Partitioned", None),
    ],
    cookies.cookies()[0]
      .attributes()
      .iter()
      .map(|attribute| (attribute.name(), attribute.value()))
      .collect::<Vec<_>>()
  );

  assert!(HttpSetCookies::parse("session=abc\x01").is_err());
}

#[test]
fn test_parse_partial_content_range_metadata() {
  let s = concat!(
    "HTTP/1.1 206 Partial Content\r\n",
    "Content-Range: bytes 10-19/200\r\n",
    "Content-Length: 10\r\n",
    "\r\n",
    "0123456789"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse partial content response");
  let content_range = response
    .content_range()
    .expect("Content-Range should parse")
    .expect("partial content response should expose content range");

  assert!(response.is_partial_content());
  assert!(!response.is_range_not_satisfiable());
  assert_eq!(
    ContentRange::Bytes {
      start: 10,
      end: 19,
      complete_length: Some(200),
    },
    content_range
  );
  assert_eq!("bytes", content_range.unit());
  assert_eq!(Some(10), content_range.start());
  assert_eq!(Some(19), content_range.end());
  assert_eq!(Some(200), content_range.complete_length());
  assert!(!content_range.is_unsatisfied());
  assert_eq!("0123456789", response.body().string().unwrap());
}

#[test]
fn test_parse_range_not_satisfiable_metadata_preserves_body_and_headers() {
  let s = concat!(
    "HTTP/1.1 416 Range Not Satisfiable\r\n",
    "Content-Range: bytes */200\r\n",
    "Content-Type: text/plain\r\n",
    "Content-Length: 17\r\n",
    "\r\n",
    "range unavailable"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse range not satisfiable response");
  let content_range = response
    .content_range()
    .expect("Content-Range should parse")
    .expect("416 response should expose unsatisfied content range");

  assert!(!response.is_partial_content());
  assert!(response.is_range_not_satisfiable());
  assert_eq!(
    ContentRange::Unsatisfied {
      complete_length: 200,
    },
    content_range
  );
  assert_eq!("bytes", content_range.unit());
  assert_eq!(None, content_range.start());
  assert_eq!(None, content_range.end());
  assert_eq!(Some(200), content_range.complete_length());
  assert!(content_range.is_unsatisfied());
  assert_eq!(
    Some(&"text/plain".to_string()),
    response.header_value("Content-Type")
  );
  assert_eq!("range unavailable", response.body().string().unwrap());
}

#[test]
fn test_invalid_content_range_metadata_is_checked_without_removing_raw_header() {
  let s = concat!(
    "HTTP/1.1 206 Partial Content\r\n",
    "Content-Range: bytes 10-20/20\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse response with invalid metadata");

  assert!(response.content_range().is_err());
  assert_eq!(
    Some(&"bytes 10-20/20".to_string()),
    response.header_value("Content-Range")
  );
}

#[test]
fn test_duplicate_content_range_metadata_is_checked() {
  let s = concat!(
    "HTTP/1.1 206 Partial Content\r\n",
    "Content-Range: bytes 0-0/2\r\n",
    "Content-Range: bytes 1-1/2\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse response with duplicate metadata");

  assert!(response.content_range().is_err());
}

#[test]
fn test_response_too_early_status_helper_matches_only_425() {
  for (status, reason, expected) in [
    (425, "Too Early", true),
    (424, "Failed Dependency", false),
    (426, "Upgrade Required", false),
    (200, "OK", false),
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test/asset"),
      format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
    )
    .expect("response should parse");

    assert_eq!(expected, response.is_too_early(), "{status} {reason}");
  }
}

#[test]
fn test_response_not_extended_status_helper_matches_only_510() {
  for (status, reason, expected) in [
    (510, "Not Extended", true),
    (509, "Bandwidth Limit Exceeded", false),
    (511, "Network Authentication Required", false),
    (200, "OK", false),
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test/asset"),
      format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
    )
    .expect("response should parse");

    assert_eq!(expected, response.is_not_extended(), "{status} {reason}");
  }
}

#[test]
fn test_response_network_authentication_required_helper_matches_only_511() {
  for (status, reason, expected) in [
    (511, "Network Authentication Required", true),
    (510, "Not Extended", false),
    (512, "Internal Server Error-ish", false),
    (200, "OK", false),
  ] {
    let response = Response::new(
      RoUrl::with("https://example.test/asset"),
      format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\n\r\n").into_bytes(),
    )
    .expect("response should parse");

    assert_eq!(
      expected,
      response.is_network_authentication_required(),
      "{status} {reason}"
    );
  }
}

#[test]
fn test_parse_content_type_response_helper_normalizes_media_type_and_preserves_parameters() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: Text/Plain; charset=utf-8; boundary=\"AaB03x\"; format=flowed\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with content-type");

  let content_type = response
    .content_type()
    .expect("valid content-type should parse")
    .expect("content-type header should be present");

  assert_eq!("text", content_type.type_());
  assert_eq!("plain", content_type.subtype());
  assert_eq!("text/plain", content_type.essence());
  assert!(content_type.is("TEXT", "PLAIN"));
  assert_eq!(Some("utf-8"), content_type.parameter("charset"));
  assert_eq!(Some("AaB03x"), content_type.parameter("BOUNDARY"));
  assert_eq!(
    vec![
      ("charset", "utf-8"),
      ("boundary", "AaB03x"),
      ("format", "flowed")
    ],
    content_type
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some(&"Text/Plain; charset=utf-8; boundary=\"AaB03x\"; format=flowed".to_string()),
    response.header_value("Content-Type")
  );
}

#[test]
fn test_parse_content_type_response_helper_accepts_common_application_json() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: application/json\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "{}"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with application/json content-type");
  let content_type = response
    .content_type()
    .expect("valid content-type should parse")
    .expect("content-type header should be present");

  assert_eq!("application", content_type.type_());
  assert_eq!("json", content_type.subtype());
  assert!(content_type.parameters().is_empty());
}

#[test]
fn test_parse_content_type_response_helper_returns_none_when_absent() {
  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response without content-type");

  assert_eq!(
    None,
    response
      .content_type()
      .expect("absent content-type should parse")
  );
}

#[test]
fn test_parse_content_type_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "text",
    "text/",
    "/plain",
    "te xt/plain",
    "text/pla in",
    "text/plain;",
    "text/plain; charset",
    "text/plain; char set=utf-8",
    "text/plain; charset=utf 8",
    "text/plain; charset=\"unterminated",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nContent-Type: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_type().is_err(),
      "content-type helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Type")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn test_parse_content_type_rejects_duplicate_singleton_duplicate_parameter_and_bounds() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Type: text/plain\r\n",
    "content-type: application/json\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-type remains usable");

  assert!(
    response.content_type().is_err(),
    "content-type helper should reject duplicate singleton fields"
  );
  assert_eq!(
    vec![&"text/plain".to_string(), &"application/json".to_string()],
    response.header_values("Content-Type")
  );

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Content-Type: text/plain; charset=utf-8; CHARSET=iso-8859-1\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response with duplicate content-type parameter remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject duplicate parameters"
  );

  let oversized = format!("text/plain; charset={}", "a".repeat(64 * 1024));
  let raw = format!("HTTP/1.1 200 OK\r\nContent-Type: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-type remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject oversized values"
  );

  let too_many = (0..257)
    .map(|ix| format!("p{ix}=v"))
    .collect::<Vec<_>>()
    .join("; ");
  let raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/plain; {too_many}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-type parameters remains usable");
  assert!(
    response.content_type().is_err(),
    "content-type helper should reject too many parameters"
  );
}

#[test]
fn test_content_type_parse_rejects_crlf_injection() {
  let error = ContentType::parse("text/plain; charset=\"bad\r\nX-Evil: yes\"")
    .expect_err("content-type helper should reject CR/LF injection");

  assert!(
    error.to_string().contains("Content-Type"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_conditional_response_metadata() {
  let s = concat!(
    "HTTP/1.1 304 Not Modified\r\n",
    "ETag: \"abc123\"\r\n",
    "Last-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse not modified response");

  assert!(response.is_not_modified());
  assert!(!response.is_precondition_failed());
  assert_eq!(Some(&"\"abc123\"".to_string()), response.etag_value());
  assert_eq!(
    Some(EntityTag::strong("abc123")),
    response.etag().expect("ETag should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.last_modified()
  );

  let s = concat!(
    "HTTP/1.1 412 Precondition Failed\r\n",
    "ETag: W/\"stale\"\r\n",
    "Content-Length: 5\r\n",
    "\r\n",
    "stale"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    s.as_bytes().to_vec(),
  )
  .expect("parse precondition failed response");

  assert!(!response.is_not_modified());
  assert!(response.is_precondition_failed());
  assert_eq!(Some(&"W/\"stale\"".to_string()), response.etag_value());
  assert_eq!(
    Some(EntityTag::weak("stale")),
    response.etag().expect("ETag should parse")
  );
  assert_eq!(None, response.last_modified());
}

#[test]
fn test_parse_etag_response_helper_handles_singleton_metadata() {
  let absent = Response::new(
    RoUrl::with("https://example.test/asset"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_vec(),
  )
  .expect("response without ETag should parse");
  assert_eq!(None, absent.etag().expect("absent ETag should parse"));

  for (value, expected) in [
    ("\"asset-v7\"", EntityTag::strong("asset-v7")),
    ("W/\"asset-v7\"", EntityTag::weak("asset-v7")),
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nETag: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test/asset"), raw.into_bytes())
      .expect("response with ETag should parse");

    assert_eq!(Some(expected), response.etag().expect("ETag should parse"));
    assert_eq!(Some(&value.to_string()), response.etag_value());
    assert_eq!(vec![&value.to_string()], response.header_values("ETag"));
  }
}

#[test]
fn test_parse_etag_rejects_malformed_duplicate_and_oversized_values_without_losing_raw_headers() {
  for value in ["abc", "W/abc", "\"bad space\"", "\"bad\"value\""] {
    let raw = format!("HTTP/1.1 200 OK\r\nETag: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test/asset"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.etag().is_err(),
      "ETag helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("ETag"));
  }

  let duplicate = Response::new(
    RoUrl::with("https://example.test/asset"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "ETag: \"one\"\r\n",
      "etag: W/\"two\"\r\n",
      "Content-Length: 2\r\n",
      "\r\n",
      "OK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response with duplicate ETags remains usable");

  assert!(
    duplicate.etag().is_err(),
    "ETag helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"\"one\"".to_string(), &"W/\"two\"".to_string()],
    duplicate.header_values("ETag")
  );

  let oversized = format!("\"{}\"", "a".repeat(64 * 1024));
  let raw = format!("HTTP/1.1 200 OK\r\nETag: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test/asset"), raw.into_bytes())
    .expect("raw response with oversized ETag remains usable");

  assert!(
    response.etag().is_err(),
    "ETag helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("ETag"));
}

#[test]
fn last_modified_date_parses_valid_singleton_and_absent_values() {
  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Last-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    response
      .last_modified_date()
      .expect("valid Last-Modified should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.last_modified()
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Last-Modified")
  );

  let response = Response::new(
    RoUrl::with("https://example.test/asset"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    None,
    response
      .last_modified_date()
      .expect("absent Last-Modified should parse")
  );
  assert_eq!(None, response.last_modified());
}

#[test]
fn last_modified_date_rejects_malformed_and_duplicate_values_without_hiding_headers() {
  for value in ["", "not a date", "Sun, 06 Nov 1994 08:49:37 PST"] {
    let raw = format!("HTTP/1.1 200 OK\r\nLast-Modified: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.last_modified_date().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Last-Modified")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Last-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "last-modified: Mon, 07 Nov 1994 08:49:37 GMT\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response with duplicate Last-Modified remains usable");

  assert!(
    response.last_modified_date().is_err(),
    "should reject duplicate singleton fields"
  );
  assert_eq!(
    vec![
      &"Sun, 06 Nov 1994 08:49:37 GMT".to_string(),
      &"Mon, 07 Nov 1994 08:49:37 GMT".to_string()
    ],
    response.header_values("Last-Modified")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.last_modified()
  );
}

#[test]
fn test_parse_content_location_response_helper_accepts_uri_references() {
  let cases = [
    (
      "https://cdn.example.test/images/logo.png?size=small#v1",
      "absolute URI",
    ),
    (
      "http://[::1]/images/logo.png",
      "absolute URI with IPv6 authority",
    ),
    ("/images/logo.png?size=small#v1", "absolute path"),
    ("images/logo.png?size=small#v1", "relative path reference"),
    ("../images/logo.png", "relative dot segment reference"),
    ("?variant=small", "query-only relative reference"),
  ];

  for (value, name) in cases {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Location: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .unwrap_or_else(|err| panic!("{name} response should parse: {err}"));
    let content_location = response
      .content_location()
      .unwrap_or_else(|err| panic!("{name} content-location should parse: {err}"))
      .unwrap_or_else(|| panic!("{name} content-location should be present"));

    assert_eq!(value, content_location.as_str());
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Location")
    );
  }
}

#[test]
fn test_parse_content_disposition_response_helper_preserves_ordered_parameters() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=\"report \\\"final\\\".txt\"; filename*=UTF-8''report-final.txt; preview=yes\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/download"),
    s.as_bytes().to_vec(),
  )
  .expect("parse response with content-disposition");

  let content_disposition = response
    .content_disposition()
    .expect("valid content-disposition should parse")
    .expect("content-disposition header should be present");

  assert_eq!("attachment", content_disposition.disposition_type());
  assert_eq!(Some("report \"final\".txt"), content_disposition.filename());
  assert_eq!(
    Some("UTF-8''report-final.txt"),
    content_disposition.filename_ext()
  );
  assert_eq!(
    vec![
      ("filename", "report \"final\".txt"),
      ("filename*", "UTF-8''report-final.txt"),
      ("preview", "yes")
    ],
    content_disposition
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some(
      &"attachment; filename=\"report \\\"final\\\".txt\"; filename*=UTF-8''report-final.txt; preview=yes"
        .to_string()
    ),
    response.header_value("Content-Disposition")
  );
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_parse_content_disposition_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without content-disposition");

  assert_eq!(
    None,
    response
      .content_disposition()
      .expect("absent content-disposition should parse")
  );
}

#[test]
fn test_www_authenticate_response_helper_parses_bounded_challenges() {
  let raw = concat!(
    "HTTP/1.1 401 Unauthorized\r\n",
    "WWW-Authenticate: Basic realm=\"users\"\r\n",
    "WWW-Authenticate: Bearer mF_9.B5f-4.1JqM=, Digest realm=\"apps\", nonce=\"a\\\\b\"\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let challenges = response
    .www_authenticate()
    .expect("valid challenges should parse")
    .expect("WWW-Authenticate should be present");

  assert_eq!(3, challenges.len());
  assert_eq!("Basic", challenges.challenges()[0].scheme());
  assert_eq!(Some("users"), challenges.challenges()[0].parameter("realm"));
  assert_eq!("Bearer", challenges.challenges()[1].scheme());
  assert_eq!(
    Some("mF_9.B5f-4.1JqM="),
    challenges.challenges()[1].token68()
  );
  assert_eq!("Digest", challenges.challenges()[2].scheme());
  assert_eq!(Some("a\\b"), challenges.challenges()[2].parameter("nonce"));
}

#[test]
fn test_www_authenticate_response_helper_combines_repeated_field_parameters() {
  let raw = concat!(
    "HTTP/1.1 401 Unauthorized\r\n",
    "WWW-Authenticate: Digest realm=apps\r\n",
    "WWW-Authenticate: nonce=abc\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let challenges = response
    .www_authenticate()
    .expect("valid challenges should parse")
    .expect("WWW-Authenticate should be present");

  assert_eq!(1, challenges.len());
  let digest = &challenges.challenges()[0];
  assert_eq!("Digest", digest.scheme());
  assert_eq!(Some("apps"), digest.parameter("realm"));
  assert_eq!(Some("abc"), digest.parameter("nonce"));
}

#[test]
fn test_www_authenticate_preserves_quoted_parameter_wire_bytes() {
  let raw = concat!(
    "HTTP/1.1 401 Unauthorized\r\n",
    "WWW-Authenticate: Digest realm=\"caf\u{e9}\"\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let challenges = response
    .www_authenticate()
    .expect("valid challenge should parse")
    .expect("WWW-Authenticate should be present");

  assert_eq!(
    Some("caf\u{00c3}\u{00a9}"),
    challenges.challenges()[0].parameter("realm")
  );
}

#[test]
fn test_www_authenticate_rejects_malformed_duplicate_and_bounded_values() {
  for value in [
    "Basic realm=\"unterminated",
    "Basic realm=one, REALM=two",
    "Basic @",
    "Basic token===more",
  ] {
    let raw = format!(
      "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: {value}\r\nContent-Length: 2\r\n\r\nOK"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(
      response.www_authenticate().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("WWW-Authenticate")
    );
  }

  let oversized = "Basic realm=".to_string() + &"a".repeat(64 * 1024);
  let too_many_challenges = (0..257)
    .map(|index| format!("Scheme{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let too_many_parameters = format!(
    "Digest {}",
    (0..257)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(", ")
  );
  let oversized_parameter = format!("Basic realm={}", "a".repeat(64 * 1024 + 1));

  for value in [
    oversized,
    too_many_challenges,
    too_many_parameters,
    oversized_parameter,
  ] {
    let raw = format!(
      "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");

    assert!(
      response.www_authenticate().is_err(),
      "should reject bounded value"
    );
    assert_eq!(
      Some(&value),
      response.header_value("WWW-Authenticate"),
      "raw header should remain available after a parse failure"
    );
  }
}

#[test]
fn test_authentication_info_response_helper_parses_bounded_auth_params() {
  let digest_field = concat!(
    r#"nextnonce="6629fae49393a05397450978507c4ef1", "#,
    r#"qop=auth, "#,
    r#"rspauth="6629fae49393a05397450978507c4ef1", "#,
    r#"cnonce="0a4f113b", "#,
    "nc=00000001"
  );
  let raw = format!(
    "HTTP/1.1 200 OK\r\nAuthentication-Info: {digest_field}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");

  let info = response
    .authentication_info()
    .expect("valid Authentication-Info should parse")
    .expect("Authentication-Info should be present");

  assert_eq!(
    info.parameter("nextnonce"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(
    info.parameter("rspauth"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("cnonce"), Some("0a4f113b"));
  assert_eq!(info.parameter("nc"), Some("00000001"));
  assert_eq!(
    Some(&digest_field.to_string()),
    response.header_value("Authentication-Info")
  );

  let combined = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Authentication-Info: nextnonce=abc\r\n",
      "Authentication-Info: qop=auth\r\n",
      "Content-Length: 2\r\n\r\nOK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain usable");
  let combined_info = combined
    .authentication_info()
    .expect("combined fields should parse")
    .expect("Authentication-Info should be present");
  assert_eq!(combined_info.parameter("nextnonce"), Some("abc"));
  assert_eq!(combined_info.parameter("qop"), Some("auth"));
  assert_eq!(
    combined.header_values("Authentication-Info"),
    [&"nextnonce=abc".to_string(), &"qop=auth".to_string()]
  );
}

#[test]
fn test_proxy_authenticate_response_helper_parses_bounded_challenges() {
  let raw = concat!(
    "HTTP/1.1 407 Proxy Authentication Required\r\n",
    "Proxy-Authenticate: Basic realm=\"corp\"\r\n",
    "Proxy-Authenticate: Bearer mF_9.B5f-4.1JqM, Digest realm=\"apps\", nonce=\"n-1\"\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let challenges = response
    .proxy_authenticate()
    .expect("valid challenges should parse")
    .expect("Proxy-Authenticate should be present");

  assert_eq!(3, challenges.len());
  assert_eq!("Basic", challenges.challenges()[0].scheme());
  assert_eq!(Some("corp"), challenges.challenges()[0].parameter("realm"));
  assert_eq!("Bearer", challenges.challenges()[1].scheme());
  assert_eq!(
    Some("mF_9.B5f-4.1JqM"),
    challenges.challenges()[1].token68()
  );
  assert_eq!("Digest", challenges.challenges()[2].scheme());
  assert_eq!(Some("apps"), challenges.challenges()[2].parameter("realm"));
  assert_eq!(Some("n-1"), challenges.challenges()[2].parameter("nonce"));
  assert_eq!(
    response.header_values("Proxy-Authenticate"),
    [
      &"Basic realm=\"corp\"".to_string(),
      &"Bearer mF_9.B5f-4.1JqM, Digest realm=\"apps\", nonce=\"n-1\"".to_string()
    ]
  );
}

#[test]
fn test_authentication_info_rejects_invalid_and_absent_values() {
  for value in ["", "nextnonce=", "Bearer mF_9.B5f-4.1JqM"] {
    let raw =
      format!("HTTP/1.1 200 OK\r\nAuthentication-Info: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(
      response.authentication_info().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Authentication-Info")
    );
  }

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Authentication-Info: qop=auth\r\n",
      "Authentication-Info: QOP=auth\r\n",
      "Content-Length: 2\r\n\r\nOK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain usable");
  assert!(duplicate.authentication_info().is_err());
  assert_eq!(
    Some(&"qop=auth".to_string()),
    duplicate.header_value("Authentication-Info")
  );
  assert_eq!(
    duplicate.header_values("Authentication-Info"),
    [&"qop=auth".to_string(), &"QOP=auth".to_string()]
  );

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw =
    format!("HTTP/1.1 200 OK\r\nAuthentication-Info: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.authentication_info().is_err());
  assert_eq!(
    Some(&oversized),
    oversized_response.header_value("Authentication-Info")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Authentication-Info should parse");
  assert_eq!(
    None,
    absent
      .authentication_info()
      .expect("absent Authentication-Info should parse")
  );
  let _: Option<AuthenticationInfo> = absent
    .authentication_info()
    .expect("absent Authentication-Info should parse");
}

#[test]
fn test_proxy_authenticate_response_helper_combines_repeated_fields() {
  let raw = concat!(
    "HTTP/1.1 407 Proxy Authentication Required\r\n",
    "Proxy-Authenticate: Digest realm=corp\r\n",
    "Proxy-Authenticate: nonce=abc\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let challenges = response
    .proxy_authenticate()
    .expect("valid repeated fields should parse")
    .expect("Proxy-Authenticate should be present");

  assert_eq!(1, challenges.len());
  let digest = &challenges.challenges()[0];
  assert_eq!("Digest", digest.scheme());
  assert_eq!(Some("corp"), digest.parameter("realm"));
  assert_eq!(Some("abc"), digest.parameter("nonce"));
}

#[test]
fn test_proxy_authenticate_rejects_invalid_and_absent_values() {
  for value in ["", "Basic @", "Basic realm=", "Basic realm=one, REALM=two"] {
    let raw = format!(
      "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: {value}\r\nContent-Length: 2\r\n\r\nOK"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(
      response.proxy_authenticate().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Proxy-Authenticate")
    );
  }

  let oversized = "Basic realm=".to_string() + &"a".repeat(64 * 1024);
  let raw = format!(
    "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: {oversized}\r\nContent-Length: 0\r\n\r\n"
  );
  let oversized_response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");
  assert!(oversized_response.proxy_authenticate().is_err());
  assert_eq!(
    Some(&oversized),
    oversized_response.header_value("Proxy-Authenticate")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Proxy-Authenticate should parse");
  assert_eq!(
    None,
    absent
      .proxy_authenticate()
      .expect("absent Proxy-Authenticate should parse")
  );
  let _: Option<ProxyAuthenticate> = absent
    .proxy_authenticate()
    .expect("absent Proxy-Authenticate should parse");
}

#[test]
fn test_proxy_authentication_info_response_helper_parses_bounded_auth_params() {
  let digest_field = concat!(
    r#"nextnonce="6629fae49393a05397450978507c4ef1", "#,
    r#"qop=auth, "#,
    r#"rspauth="6629fae49393a05397450978507c4ef1", "#,
    r#"cnonce="0a4f113b", "#,
    "nc=00000001"
  );
  let raw = format!(
    "HTTP/1.1 200 OK\r\nProxy-Authentication-Info: {digest_field}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");

  let info = response
    .proxy_authentication_info()
    .expect("valid Proxy-Authentication-Info should parse")
    .expect("Proxy-Authentication-Info should be present");

  assert_eq!(
    info.parameter("nextnonce"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(
    info.parameter("rspauth"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("cnonce"), Some("0a4f113b"));
  assert_eq!(info.parameter("nc"), Some("00000001"));
  assert_eq!(
    Some(&digest_field.to_string()),
    response.header_value("Proxy-Authentication-Info")
  );

  let combined = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Proxy-Authentication-Info: nextnonce=abc\r\n",
      "Proxy-Authentication-Info: qop=auth\r\n",
      "Content-Length: 2\r\n\r\nOK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain usable");
  let combined_info = combined
    .proxy_authentication_info()
    .expect("combined fields should parse")
    .expect("Proxy-Authentication-Info should be present");
  assert_eq!(combined_info.parameter("nextnonce"), Some("abc"));
  assert_eq!(combined_info.parameter("qop"), Some("auth"));
  assert_eq!(
    combined.header_values("Proxy-Authentication-Info"),
    [&"nextnonce=abc".to_string(), &"qop=auth".to_string()]
  );
}

#[test]
fn test_via_response_helper_parses_repeated_hops() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Via: 1.1 edge-a (TLS terminator)\r\n",
    "Via: HTTP/2 upstream\r\n",
    "Content-Length: 2\r\n\r\nok"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let via = response
    .via()
    .expect("valid Via should parse")
    .expect("Via should be present");

  assert_eq!(2, via.len());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("TLS terminator"), via.members()[0].comment());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!("2", via.members()[1].protocol_version());
  assert_eq!("upstream", via.members()[1].received_by());
  assert_eq!(
    response.header_values("Via"),
    [
      &"1.1 edge-a (TLS terminator)".to_string(),
      &"HTTP/2 upstream".to_string()
    ]
  );
}

#[test]
fn test_via_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Via should parse");
  assert_eq!(None, absent.via().expect("absent Via should parse"));
}

#[test]
fn test_via_rejects_malformed_and_oversized_values_without_hiding_headers() {
  for value in ["", "1.1", "1.1 hop extra", "1.1 hop("] {
    let raw = format!("HTTP/1.1 200 OK\r\nVia: {value}\r\nContent-Length: 2\r\n\r\nok");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.via().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("Via"));
    assert_eq!("ok", response.body().string().unwrap());
  }

  let oversized = format!("1.1 {}", "a".repeat(64 * 1024));
  let oversized_raw = format!("HTTP/1.1 200 OK\r\nVia: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.via().is_err());
  assert_eq!(Some(&oversized), oversized_response.header_value("Via"));
  assert!(Via::parse(
    (0..257)
      .map(|index| format!("1.1 hop{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
}

#[test]
fn test_proxy_status_response_helper_parses_rfc9209_and_combined_fields() {
  let raw = concat!(
    "HTTP/1.1 504 Gateway Timeout\r\n",
    "Proxy-Status: ExampleCDN; error=connection_timeout\r\n",
    "Proxy-Status: OtherProxy; extra-param\r\n",
    "Content-Length: 7\r\n\r\ntimeout"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let status = response
    .proxy_status()
    .expect("valid Proxy-Status should parse")
    .expect("Proxy-Status should be present");

  assert_eq!(2, status.len());
  assert_eq!("ExampleCDN", status.members()[0].identifier().as_str());
  assert_eq!(
    Some(&ProxyStatusBareItem::Token(
      "connection_timeout".to_string()
    )),
    status.members()[0]
      .parameter("error")
      .map(|parameter| parameter.value())
  );
  assert_eq!(
    Some(&ProxyStatusBareItem::Boolean(true)),
    status.members()[1]
      .parameter("extra-param")
      .map(|parameter| parameter.value())
  );
  assert_eq!(
    response.header_values("Proxy-Status"),
    [
      &"ExampleCDN; error=connection_timeout".to_string(),
      &"OtherProxy; extra-param".to_string()
    ]
  );
}

#[test]
fn test_proxy_status_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Proxy-Status should parse");
  assert_eq!(
    None,
    absent
      .proxy_status()
      .expect("absent Proxy-Status should parse")
  );
}

#[test]
fn test_proxy_status_rejects_malformed_and_oversized_values_without_hiding_headers() {
  for value in [
    "",
    "(ExampleCDN)",
    "ExampleCDN; error=timeout; error=reset",
    "ExampleCDN;\x01bad",
  ] {
    let raw = format!(
      "HTTP/1.1 504 Gateway Timeout\r\nProxy-Status: {value}\r\nContent-Length: 7\r\n\r\ntimeout"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.proxy_status().is_err(), "should reject {value:?}");
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Proxy-Status")
    );
    assert_eq!("timeout", response.body().string().unwrap());
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw =
    format!("HTTP/1.1 200 OK\r\nProxy-Status: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.proxy_status().is_err());
  assert_eq!(
    Some(&oversized),
    oversized_response.header_value("Proxy-Status")
  );
  assert!(ProxyStatus::parse(
    (0..257)
      .map(|index| format!("Proxy{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
}

#[test]
fn test_proxy_authentication_info_rejects_invalid_and_absent_values() {
  for value in ["", "nextnonce=", "Bearer mF_9.B5f-4.1JqM"] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nProxy-Authentication-Info: {value}\r\nContent-Length: 2\r\n\r\nOK"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(
      response.proxy_authentication_info().is_err(),
      "should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Proxy-Authentication-Info")
    );
  }

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Proxy-Authentication-Info: qop=auth\r\n",
      "Proxy-Authentication-Info: QOP=auth\r\n",
      "Content-Length: 2\r\n\r\nOK"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("raw response should remain usable");
  assert!(duplicate.proxy_authentication_info().is_err());
  assert_eq!(
    Some(&"qop=auth".to_string()),
    duplicate.header_value("Proxy-Authentication-Info")
  );
  assert_eq!(
    duplicate.header_values("Proxy-Authentication-Info"),
    [&"qop=auth".to_string(), &"QOP=auth".to_string()]
  );

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw = format!(
    "HTTP/1.1 200 OK\r\nProxy-Authentication-Info: {oversized}\r\nContent-Length: 0\r\n\r\n"
  );
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.proxy_authentication_info().is_err());
  assert_eq!(
    Some(&oversized),
    oversized_response.header_value("Proxy-Authentication-Info")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Proxy-Authentication-Info should parse");
  assert_eq!(
    None,
    absent
      .proxy_authentication_info()
      .expect("absent Proxy-Authentication-Info should parse")
  );
  let _: Option<ProxyAuthenticationInfo> = absent
    .proxy_authentication_info()
    .expect("absent Proxy-Authentication-Info should parse");
}

#[test]
fn test_digest_response_helpers_parse_bounded_digest_fields() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Digest: sha-256=:YWJj:, sha-512=:ZGVm:\r\n",
    "Repr-Digest: sha-256=:Z2hp:;foo=bar\r\n",
    "Repr-Digest: sha-512=:amts:\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let digest = response
    .digest()
    .expect("Digest should parse")
    .expect("Digest should be present");
  assert_eq!(2, digest.len());
  assert_eq!(
    Some(&b"abc"[..]),
    digest.entry("sha-256").map(|entry| entry.value())
  );
  assert_eq!(
    Some(&b"def"[..]),
    digest.entry("sha-512").map(|entry| entry.value())
  );

  let repr_digest = response
    .repr_digest()
    .expect("Repr-Digest should parse")
    .expect("Repr-Digest should be present");
  assert_eq!(2, repr_digest.len());
  assert_eq!(
    Some(&b"ghi"[..]),
    repr_digest.entry("sha-256").map(|entry| entry.value())
  );
  assert_eq!(
    Some(&b"jkl"[..]),
    repr_digest.entry("sha-512").map(|entry| entry.value())
  );
  assert_eq!("sha-256=:Z2hp:, sha-512=:amts:", repr_digest.header_value());
}

#[test]
fn test_content_digest_combines_multiple_fields_without_verification() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Digest: sha-256=:YWJj:\r\n",
    "Content-Digest: sha-512=:ZGVm:\r\n",
    "Content-Length: 3\r\n\r\nabc"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  let content_digest = response
    .content_digest()
    .expect("Content-Digest should parse")
    .expect("Content-Digest should be present");
  assert_eq!(2, content_digest.len());
  assert_eq!(
    Some(&b"abc"[..]),
    content_digest.entry("sha-256").map(|entry| entry.value())
  );
  assert_eq!(
    Some(&b"def"[..]),
    content_digest.entry("sha-512").map(|entry| entry.value())
  );
  assert_eq!(
    "sha-256=:YWJj:, sha-512=:ZGVm:",
    content_digest.header_value()
  );

  let digest = response
    .digest()
    .expect("digest() should keep parsing Content-Digest")
    .expect("Content-Digest should be present");
  assert_eq!(content_digest, digest);
  assert_eq!(b"abc", response.body().binary());

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response should remain usable");
  assert_eq!(
    None,
    absent
      .content_digest()
      .expect("absent Content-Digest should parse")
  );
}

#[test]
fn test_content_digest_rejects_malformed_values_without_hiding_headers() {
  for value in [
    "",
    "sha-256=:YWJj:, sha-256=:ZGVm:",
    "sha-256=:not-base64!:",
    "sha-256=:YWJj:;foo=",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nContent-Digest: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(
      response.content_digest().is_err(),
      "Content-Digest should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Digest")
    );
  }

  let oversized = format!("sha-256=:{}:", "A".repeat(64 * 1024 + 1));
  let raw = format!("HTTP/1.1 200 OK\r\nContent-Digest: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");
  assert!(response.content_digest().is_err());
  assert_eq!(Some(&oversized), response.header_value("Content-Digest"));
}

#[test]
fn test_priority_response_helper_parses_known_and_extension_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Priority: u=1, i, x=token\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let priority = response
    .priority()
    .expect("Priority should parse")
    .expect("Priority should be present");
  assert_eq!(Some(1), priority.urgency());
  assert!(priority.incremental());
  assert_eq!(Some("token"), priority.extensions()[0].value());
}

#[test]
fn test_server_timing_response_helper_parses_metrics_extensions_and_duplicates() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Server-Timing: db;dur=53.2;desc=\"primary database\";region=us-east;cached, db;dur=4\r\n",
    "Server-Timing: app;desc=\"render \\\"home\\\"\";build=2026\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let timing = response
    .server_timing()
    .expect("valid Server-Timing should parse")
    .expect("Server-Timing should be present");
  assert_eq!(3, timing.len());
  assert_eq!("db", timing.metrics()[0].name());
  assert_eq!(Some(53.2), timing.metrics()[0].duration());
  assert_eq!(Some("primary database"), timing.metrics()[0].description());
  assert_eq!(
    vec![("region", Some("us-east")), ("cached", None)],
    timing.metrics()[0]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!("db", timing.metrics()[1].name());
  assert_eq!(Some(4.0), timing.metrics()[1].duration());
  assert_eq!(Some("render \"home\""), timing.metrics()[2].description());
  assert_eq!("db; dur=53.2; desc=\"primary database\"; region=us-east; cached, db; dur=4, app; desc=\"render \\\"home\\\"\"; build=2026", timing.header_value());
}

#[test]
fn test_alt_svc_response_helper_parses_and_round_trips_alternatives() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Alt-Svc: h3=\":443\"; ma=3600; persist=1; region=\"us-east\", h2=\"alt.example:8443\"; ma=60\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let alt_svc = response
    .alt_svc()
    .expect("Alt-Svc should parse")
    .expect("Alt-Svc should be present");

  assert!(!alt_svc.is_clear());
  assert_eq!(2, alt_svc.len());
  assert_eq!("h3", alt_svc.alternatives()[0].protocol_id());
  assert_eq!(":443", alt_svc.alternatives()[0].authority());
  assert_eq!(Some(3600), alt_svc.alternatives()[0].max_age());
  assert_eq!(Some(true), alt_svc.alternatives()[0].persist());
  assert_eq!(
    vec![("region", Some("us-east"))],
    alt_svc.alternatives()[0]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    "h3=\":443\"; ma=3600; persist=1; region=us-east, h2=\"alt.example:8443\"; ma=60",
    alt_svc.header_value()
  );
  assert_eq!(
    alt_svc,
    AltSvc::parse(alt_svc.header_value()).expect("round-tripped Alt-Svc should parse")
  );
}

#[test]
fn test_alt_svc_rejects_invalid_or_unbounded_metadata_without_hiding_headers() {
  for value in [
    "h3=:443",
    "h 3=\":443\"",
    "h3=\"unterminated",
    "clear, h3=\":443\"",
    "h3=\":443\"; ma=forever",
    "h3=\":443\"; ma=\"60\"",
    "h3=\":443\"; region",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nAlt-Svc: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.alt_svc().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("Alt-Svc"));
  }

  assert!(AltSvc::parse(format!("h3=\":443\"; x=\"{}\"", "a".repeat(64 * 1024))).is_err());
  assert!(AltSvc::parse(
    (0..257)
      .map(|index| format!("h{index}=\":443\""))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
}

#[test]
fn test_alt_svc_clear_is_an_exclusive_sentinel() {
  let alt_svc = AltSvc::parse("clear").expect("clear should parse");
  assert!(alt_svc.is_clear());
  assert!(alt_svc.is_empty());
  assert_eq!("clear", alt_svc.header_value());
  assert!(AltSvc::parse_values(["clear", "h3=\":443\""]).is_err());
}

#[test]
fn test_alt_used_response_helper_parses_authority_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Alt-Used: [2001:db8::1]:8443\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let alt_used = response
    .alt_used()
    .expect("Alt-Used should parse")
    .expect("Alt-Used should be present");

  assert_eq!("[2001:db8::1]", alt_used.host());
  assert_eq!(Some("8443"), alt_used.port());
  assert_eq!("[2001:db8::1]:8443", alt_used.header_value());
  assert_eq!(
    Some(&"[2001:db8::1]:8443".to_string()),
    response.header_value("Alt-Used")
  );
  assert_eq!(
    alt_used,
    AltUsed::parse(alt_used.header_value()).expect("round-tripped Alt-Used should parse")
  );
}

#[test]
fn test_alt_used_rejects_invalid_duplicate_and_unbounded_metadata_without_hiding_headers() {
  for value in [
    "https://alt.example",
    "user@alt.example",
    "2001:db8::1",
    "alt.example:",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nAlt-Used: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");

    assert!(response.alt_used().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("Alt-Used"));
  }

  let duplicate = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Alt-Used: alt.example:443\r\n",
    "alt-used: other.example:443\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(
    RoUrl::with("https://example.test"),
    duplicate.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate Alt-Used remains usable");
  assert!(response.alt_used().is_err());
  assert_eq!(
    vec![
      &"alt.example:443".to_string(),
      &"other.example:443".to_string()
    ],
    response.header_values("Alt-Used")
  );

  let oversized = "a".repeat(64 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nAlt-Used: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized Alt-Used remains usable");
  assert!(response.alt_used().is_err());
  assert_eq!(Some(&oversized), response.header_value("Alt-Used"));
}

#[test]
fn test_alt_used_response_helper_reports_absent_metadata() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response should parse");

  assert_eq!(
    None,
    response.alt_used().expect("absent Alt-Used should parse")
  );
}

#[test]
fn test_origin_trial_response_helper_parses_multiple_and_duplicate_tokens() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Origin-Trial: token-one\r\n",
    "origin-trial: token-one\r\n",
    "Origin-Trial: token-two\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let origin_trials = response
    .origin_trials()
    .expect("Origin-Trial should parse")
    .expect("Origin-Trial should be present");

  assert_eq!(
    origin_trials.tokens(),
    ["token-one", "token-one", "token-two"]
  );
  assert_eq!(
    vec![
      &"token-one".to_string(),
      &"token-one".to_string(),
      &"token-two".to_string()
    ],
    response.header_values("Origin-Trial")
  );
  assert_eq!(
    origin_trials,
    OriginTrials::parse_values(origin_trials.header_values().iter().map(String::as_str))
      .expect("round-tripped Origin-Trial should parse")
  );
  let debug = format!("{origin_trials:?}");
  assert!(debug.contains("OriginTrials"));
  assert!(!debug.contains("token-one"));
  assert!(!debug.contains("token-two"));
}

#[test]
fn test_origin_trial_rejects_malformed_and_oversized_metadata_without_hiding_headers() {
  let injected = "token\twith-tab";
  let raw = format!("HTTP/1.1 200 OK\r\nOrigin-Trial: {injected}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");
  assert!(
    response.origin_trials().is_err(),
    "should reject {injected:?}"
  );
  assert_eq!(
    Some(&injected.to_string()),
    response.header_value("Origin-Trial")
  );

  let mut obs_text = b"HTTP/1.1 200 OK\r\nOrigin-Trial: token".to_vec();
  obs_text.push(0x80);
  obs_text.extend_from_slice(b"value\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), obs_text)
    .expect("raw response with obs-text Origin-Trial remains usable");
  assert!(response.origin_trials().is_err());
  assert!(response.header_value("Origin-Trial").is_some());

  let oversized = "x".repeat(8 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nOrigin-Trial: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized Origin-Trial remains usable");
  assert!(response.origin_trials().is_err());
  assert_eq!(Some(&oversized), response.header_value("Origin-Trial"));

  let header_debug = format!(
    "{:?}",
    rttp_client::types::Header::new("Origin-Trial", "secret-origin-trial-token")
  );
  assert!(header_debug.contains("Origin-Trial"));
  assert!(header_debug.contains("[REDACTED]"));
  assert!(!header_debug.contains("secret-origin-trial-token"));
}

#[test]
fn test_origin_trial_response_helper_reports_absent_metadata() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response should parse");

  assert_eq!(
    None,
    response
      .origin_trials()
      .expect("absent Origin-Trial should parse")
  );
}

#[test]
fn test_digest_response_helpers_recover_from_empty_duplicate_and_oversized_fields() {
  for (header, value) in [
    ("Content-Digest", ""),
    ("Content-Digest", "sha-256=:YWJj:, sha-256=:ZGVm:"),
    ("Repr-Digest", "sha-256=:YWJj:, sha-256=:ZGVm:"),
    ("Repr-Digest", "sha-256=:not-base64!:"),
    ("Repr-Digest", "sha-256=:YWJj:;foo="),
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\n{header}: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    let result = if header == "Content-Digest" {
      response.digest().map(|_| ())
    } else {
      response.repr_digest().map(|_| ())
    };
    assert!(result.is_err(), "{header} should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value(header));
  }

  let oversized = format!("sha-256=:{}:", "A".repeat(64 * 1024 + 1));
  let raw = format!("HTTP/1.1 200 OK\r\nRepr-Digest: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");
  assert!(response.repr_digest().is_err());
  assert_eq!(Some(&oversized), response.header_value("Repr-Digest"));
}

#[test]
fn test_warning_response_helper_parses_multi_field_quoted_text_and_date() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Warning: 110 - \"Response is Stale\"\r\n",
    "Warning: 299 example.com:80 \"Deprecated \\\"API\\\"\" \"Wed, 21 Oct 2015 07:28:00 GMT\"\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let warning = response
    .warning()
    .expect("valid Warning should parse")
    .expect("Warning should be present");

  assert_eq!(2, warning.len());
  assert_eq!(110, warning.items()[0].code());
  assert_eq!("-", warning.items()[0].agent());
  assert_eq!("Response is Stale", warning.items()[0].text());
  assert_eq!(None, warning.items()[0].date());
  assert_eq!(299, warning.items()[1].code());
  assert_eq!("example.com:80", warning.items()[1].agent());
  assert_eq!("Deprecated \"API\"", warning.items()[1].text());
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(1_445_412_480)),
    warning.items()[1].date()
  );
  assert_eq!(
    response.header_values("Warning"),
    [
      &r#"110 - "Response is Stale""#.to_string(),
      &r#"299 example.com:80 "Deprecated \"API\"" "Wed, 21 Oct 2015 07:28:00 GMT""#.to_string()
    ]
  );
}

#[test]
fn test_warning_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Warning should parse");
  assert_eq!(None, absent.warning().expect("absent Warning should parse"));
}

#[test]
fn test_warning_rejects_malformed_invalid_code_and_bounds_without_hiding_headers() {
  for value in [
    r#"110 - "unterminated"#,
    r#"11 - "too short""#,
    r#"110 - "ok",,"#,
    "",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nWarning: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.warning().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("Warning"));
    assert_eq!("OK", response.body().string().unwrap());
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw =
    format!("HTTP/1.1 200 OK\r\nWarning: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.warning().is_err());
  assert_eq!(Some(&oversized), oversized_response.header_value("Warning"));
  assert!(Warning::parse(
    (0..257)
      .map(|index| format!(r#"110 - "item{index}""#))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
}

#[test]
fn test_keep_alive_response_helper_parses_combined_fields_and_retains_raw_headers() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Keep-Alive: timeout=5\r\n",
    "Keep-Alive: max=100\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let keep_alive = response
    .keep_alive()
    .expect("valid Keep-Alive should parse")
    .expect("Keep-Alive should be present");

  assert_eq!(Some(5), keep_alive.timeout());
  assert_eq!(Some(100), keep_alive.max());
  assert_eq!(
    response.header_values("Keep-Alive"),
    [&"timeout=5".to_string(), &"max=100".to_string()]
  );
}

#[test]
fn test_keep_alive_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Keep-Alive should parse");
  assert_eq!(
    None,
    absent.keep_alive().expect("absent Keep-Alive should parse")
  );
}

#[test]
fn test_keep_alive_rejects_malformed_duplicate_and_bounds_without_hiding_headers() {
  for value in [
    "timeout=abc",
    "timeout=5, timeout=6",
    "timeout=5, max=100, max=200",
    "timeout=18446744073709551616",
    "",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nKeep-Alive: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.keep_alive().is_err(), "should reject {value:?}");
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Keep-Alive")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw =
    format!("HTTP/1.1 200 OK\r\nKeep-Alive: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.keep_alive().is_err());
  assert_eq!(
    Some(&oversized),
    oversized_response.header_value("Keep-Alive")
  );
  assert!(KeepAlive::parse(
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
}

#[test]
fn test_server_timing_rejects_malformed_and_oversized_values_without_hiding_headers() {
  for value in [
    "db;dur=not-a-number",
    "db;dur=-1",
    "db;desc=unterminated value",
    "db;=value",
    "db;dur=1;dur=2",
    "db;desc=\"unterminated",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nServer-Timing: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.server_timing().is_err(), "should reject {value:?}");
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Server-Timing")
    );
  }
  assert!(ServerTiming::parse(format!("db;desc=\"{}\"", "a".repeat(64 * 1024))).is_err());
  assert!(ServerTiming::parse(
    (0..257)
      .map(|index| format!("metric{index}"))
      .collect::<Vec<_>>()
      .join(", ")
  )
  .is_err());
  assert!(ServerTiming::parse(format!(
    "db{}",
    (0..257)
      .map(|index| format!(";ext{index}=value"))
      .collect::<String>()
  ))
  .is_err());
}

#[test]
fn test_pragma_response_helper_parses_combined_fields_and_retains_raw_headers() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Pragma: no-cache\r\n",
    "Pragma: community=private, example=\"quoted, value\"\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let pragma = response
    .pragma()
    .expect("valid Pragma should parse")
    .expect("Pragma should be present");

  assert!(pragma.no_cache());
  assert_eq!(2, pragma.extensions().len());
  assert_eq!("community", pragma.extensions()[0].name());
  assert_eq!(Some("private"), pragma.extensions()[0].value());
  assert_eq!(
    "no-cache, community=private, example=\"quoted, value\"",
    pragma.header_value()
  );
  assert_eq!(
    response.header_values("Pragma"),
    [
      &"no-cache".to_string(),
      &"community=private, example=\"quoted, value\"".to_string()
    ]
  );
}

#[test]
fn test_pragma_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without Pragma should parse");
  assert_eq!(None, absent.pragma().expect("absent Pragma should parse"));
}

#[test]
fn test_pragma_rejects_malformed_duplicate_and_bounds_without_hiding_headers() {
  for value in [
    "",
    "no-cache,",
    "no-cache=value",
    "no-cache, no-cache",
    "community=private, COMMUNITY=public",
    "bad name",
    "x=\"unterminated",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nPragma: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.pragma().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("Pragma"));
    assert_eq!("OK", response.body().string().unwrap());
  }

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_raw =
    format!("HTTP/1.1 200 OK\r\nPragma: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.pragma().is_err());
  assert_eq!(Some(&oversized), oversized_response.header_value("Pragma"));

  let first = "a".repeat(32 * 1024);
  let second = "b".repeat(32 * 1024);
  let combined_oversized =
    format!("HTTP/1.1 200 OK\r\nPragma: {first}\r\nPragma: {second}\r\nContent-Length: 0\r\n\r\n");
  let combined_response = Response::new(
    RoUrl::with("https://example.test"),
    combined_oversized.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(combined_response.pragma().is_err());
  assert_eq!(2, combined_response.header_values("Pragma").len());

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nPragma: no-cache\r\npragma: no-cache\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response should remain usable");
  assert!(duplicate.pragma().is_err());
  assert_eq!(2, duplicate.header_values("Pragma").len());
}

#[test]
fn test_parse_content_disposition_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "attach ment",
    "attachment;",
    "attachment; filename",
    "attachment; file name=report.txt",
    "attachment; filename=report txt",
    "attachment; filename=\"unterminated",
    "attachment; filename*=UTF-8''bad%ZZname",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Disposition: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_disposition().is_err(),
      "content-disposition helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Disposition")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn test_parse_content_disposition_rejects_duplicate_singleton_duplicate_parameter_and_bounds() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=one.txt\r\n",
    "content-disposition: inline; filename=two.txt\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-disposition remains usable");

  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject duplicate singleton fields"
  );
  assert_eq!(
    vec![
      &"attachment; filename=one.txt".to_string(),
      &"inline; filename=two.txt".to_string()
    ],
    response.header_values("Content-Disposition")
  );

  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Disposition: attachment; filename=one.txt; FILENAME=two.txt\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate parameter remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject duplicate parameters"
  );

  let oversized = "a".repeat(64 * 1024 + 1);
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Disposition: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-disposition remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject oversized values"
  );

  let too_many = (0..257)
    .map(|ix| format!("p{ix}=v"))
    .collect::<Vec<_>>()
    .join("; ");
  let raw = format!(
    "HTTP/1.1 200 OK\r\nContent-Disposition: attachment; {too_many}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-disposition parameters remains usable");
  assert!(
    response.content_disposition().is_err(),
    "content-disposition helper should reject too many parameters"
  );
}

#[test]
fn test_parse_link_response_metadata_preserves_multiple_values_and_parameters() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Link: </style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload\r\n",
    "link: <../manifest.json>; type=\"application/manifest+json\"; anchor=\"/app\"\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/app/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with Link metadata remains usable");

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
    Some("application/manifest+json"),
    links.values()[2].parameter("type")
  );
  assert_eq!(Some("/app"), links.values()[2].parameter("anchor"));
  assert_eq!(
    vec![("type", "application/manifest+json"), ("anchor", "/app")],
    links.values()[2]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    vec![
      &"</style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload"
        .to_string(),
      &"<../manifest.json>; type=\"application/manifest+json\"; anchor=\"/app\"".to_string(),
    ],
    response.header_values("Link")
  );
}

#[test]
fn test_parse_link_response_metadata_preserves_valueless_extensions() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Link: </style.css>; rel=preload; nopush\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with Link metadata remains usable");

  let links = response
    .links()
    .expect("Link metadata should parse")
    .expect("Link metadata should be present");

  assert_eq!(Some(""), links.values()[0].parameter("nopush"));
  assert_eq!(
    vec![("rel", "preload"), ("nopush", "")],
    links.values()[0]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn test_link_response_metadata_rejects_invalid_and_bounded_values_without_losing_headers() {
  for value in [
    "style.css; rel=preload",
    "<style.css; rel=preload",
    "</style.css> rel=preload",
    "</style.css>; =preload",
    "</style.css>; bad name=value",
    "</style.css>; rel=\"unterminated",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nLink: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");
    assert!(
      response.links().is_err(),
      "Link parser should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Link"));
  }

  let oversized = format!("</{}>", "a".repeat(64 * 1024));
  assert!(LinkValues::parse(oversized).is_err());

  let too_many = (0..257)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(LinkValues::parse(too_many).is_err());

  let too_many_parameters = format!(
    "</asset>{}",
    (0..257)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(LinkValues::parse(too_many_parameters).is_err());

  let oversized_parameter = format!("</asset>; title={}", "a".repeat(64 * 1024 + 1));
  assert!(LinkValues::parse(oversized_parameter).is_err());
}

#[test]
fn test_parse_content_encoding_response_helper_preserves_order_across_fields() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Encoding: compress, br\r\n",
    "content-encoding: identity\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with content-encoding remains usable");

  let content_encoding = response
    .content_encoding()
    .expect("valid content-encoding should parse")
    .expect("content-encoding header should be present");

  assert_eq!(
    vec!["compress", "br", "identity"],
    content_encoding.codings()
  );
  assert_eq!(
    vec![&"compress, br".to_string(), &"identity".to_string()],
    response.header_values("Content-Encoding")
  );
  assert_eq!("OK", response.body().string().unwrap());

  assert_eq!(
    vec!["gzip", "br"],
    ContentEncoding::parse("gzip, br")
      .expect("common codings should parse")
      .codings()
  );
}

#[test]
fn test_content_encoding_runtime_decodes_only_single_supported_gzip_coding() {
  let body = gzip_bytes(b"OK");
  let mut raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Encoding: gzip\r\n")
    .as_bytes()
    .to_vec();
  raw.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
  raw.extend_from_slice(&body);

  let response = Response::new(RoUrl::with("https://example.test"), raw)
    .expect("single gzip response should decode");

  assert_eq!("OK", response.body().string().unwrap());
  assert!(response.header("Content-Encoding").is_none());
  assert!(response.header("Content-Length").is_none());
  assert!(response.content_encoding().unwrap().is_none());
  assert!(response
    .binary()
    .windows(b"Content-Encoding: gzip".len())
    .any(|window| window == b"Content-Encoding: gzip"));
}

#[test]
fn test_content_encoding_runtime_leaves_empty_gzip_body_and_headers_unchanged() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Encoding: gzip\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("empty gzip response should remain usable");

  assert!(response.body().binary().is_empty());
  assert_eq!(
    Some("gzip"),
    response
      .header_value("Content-Encoding")
      .map(String::as_str)
  );
  assert_eq!(
    Some("0"),
    response.header_value("Content-Length").map(String::as_str)
  );
}

#[test]
fn test_content_encoding_runtime_rejects_malformed_single_gzip_body() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Encoding: gzip\r\n",
    "Content-Length: 8\r\n",
    "\r\n",
    "not-gzip"
  );

  assert!(Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec()).is_err());
}

#[test]
fn test_content_encoding_runtime_leaves_stacked_or_unsupported_codings_undecoded() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Encoding: gzip, br\r\n",
    "Content-Length: 7\r\n",
    "\r\n",
    "encoded"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("stacked unsupported content-encoding should remain usable");

  assert_eq!("encoded", response.body().string().unwrap());
  assert_eq!(
    vec!["gzip", "br"],
    response
      .content_encoding()
      .expect("content-encoding should parse")
      .expect("content-encoding should be present")
      .codings()
  );
}

#[test]
fn test_content_encoding_runtime_leaves_duplicate_gzip_fields_undecoded() {
  let body = gzip_bytes(b"OK");
  let mut raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Encoding: gzip\r\n",
    "Content-Encoding: gzip\r\n"
  )
  .as_bytes()
  .to_vec();
  raw.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
  raw.extend_from_slice(&body);

  let response = Response::new(RoUrl::with("https://example.test"), raw)
    .expect("duplicate gzip fields should remain usable");
  let content_length = body.len().to_string();

  assert_eq!(body, response.body().binary());
  assert_eq!(
    vec![&"gzip".to_string(), &"gzip".to_string()],
    response.header_values("Content-Encoding")
  );
  assert_eq!(
    Some(content_length.as_str()),
    response.header_value("Content-Length").map(String::as_str)
  );
}

#[test]
fn test_parse_content_encoding_rejects_invalid_duplicate_and_excessive_values() {
  for value in [
    "",
    "compress,",
    ", compress",
    "compress,,br",
    "bad coding",
    "compress, g:zip",
  ] {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Encoding: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_encoding().is_err(),
      "content-encoding helper should reject {value:?}"
    );
    assert_eq!("OK", response.body().string().unwrap());
  }

  assert!(
    ContentEncoding::parse("gzip, br, GZIP").is_err(),
    "content-encoding helper should reject duplicate codings"
  );

  let too_many = (0..257)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    ContentEncoding::parse(too_many).is_err(),
    "content-encoding helper should reject excessive codings"
  );
}

#[test]
fn test_content_disposition_parse_rejects_crlf_injection() {
  let error = ContentDisposition::parse("attachment; filename=\"bad\r\nX-Evil: yes\"")
    .expect_err("content-disposition helper should reject CR/LF injection");

  assert!(
    error.to_string().contains("Content-Disposition"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_content_location_response_helper_trims_outer_whitespace_and_allows_absent() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Location:   /representations/current.json   \r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with content-location");
  let content_location = response
    .content_location()
    .expect("valid content-location should parse")
    .expect("content-location header should be present");

  assert_eq!("/representations/current.json", content_location.as_str());
  assert_eq!(
    Some(&"/representations/current.json".to_string()),
    response.header_value("Content-Location")
  );

  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without content-location");
  assert_eq!(
    None,
    response
      .content_location()
      .expect("absent content-location should parse")
  );
}

#[test]
fn test_parse_content_location_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["", "http://[::1", "not valid", "/bad path", "ok\u{7f}"];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Location: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.content_location().is_err(),
      "content-location helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.trim().to_string()),
      response.header_value("Content-Location")
    );
  }
}

#[test]
fn test_parse_content_location_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Location: /one\r\n",
    "content-location: /two\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate content-location remains usable");

  assert!(
    response.content_location().is_err(),
    "content-location helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"/one".to_string(), &"/two".to_string()],
    response.header_values("Content-Location")
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Location: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized content-location remains usable");

  assert!(
    response.content_location().is_err(),
    "content-location helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Content-Location"));
}

#[test]
fn test_parse_content_dpr_response_helper_accepts_present_and_absent_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-DPR:  2.0  \r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/image.png"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with content-dpr");
  let content_dpr = response
    .content_dpr()
    .expect("valid content-dpr should parse")
    .expect("content-dpr header should be present");

  assert_eq!(2.0, content_dpr.ratio());
  assert_eq!("2.0", content_dpr.header_value());
  assert_eq!(
    Some(&"2.0".to_string()),
    response.header_value("Content-DPR")
  );

  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/image.png"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without content-dpr");
  assert_eq!(
    None,
    response
      .content_dpr()
      .expect("absent content-dpr should parse")
  );
}

#[test]
fn test_parse_content_dpr_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["0", "2.", ".5", "+1", "1e1"];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nContent-DPR: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/image.png"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.content_dpr().is_err(),
      "content-dpr helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-DPR")
    );
  }
}

#[test]
fn test_parse_content_dpr_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-DPR: 1\r\n",
    "content-dpr: 2.0\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/image.png"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate content-dpr remains usable");

  assert!(
    response.content_dpr().is_err(),
    "content-dpr helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"1".to_string(), &"2.0".to_string()],
    response.header_values("Content-DPR")
  );

  let oversized = "1".repeat(64 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nContent-DPR: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(
    RoUrl::with("https://example.test/image.png"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized content-dpr remains usable");

  assert!(
    response.content_dpr().is_err(),
    "content-dpr helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Content-DPR"));
}

#[test]
fn test_parse_content_dpr_rejects_control_characters_and_crlf_injection() {
  for value in ["1\r\nX-Evil: yes", "1\r", "1\n", "1\u{7f}"] {
    assert!(
      ContentDpr::parse(value).is_err(),
      "content-dpr parser should reject {value:?}"
    );
  }
}

#[test]
fn test_parse_deprecation_response_helper_accepts_boolean_and_date() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Deprecation: ?1\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/v1/widgets"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with boolean deprecation");
  let deprecation = response
    .deprecation()
    .expect("valid deprecation should parse")
    .expect("deprecation header should be present");

  assert_eq!(Deprecation::Boolean(true), deprecation);
  assert_eq!(
    Some(&"?1".to_string()),
    response.header_value("Deprecation")
  );

  let instant = UNIX_EPOCH + Duration::from_secs(1_688_169_599);
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Deprecation: @1688169599\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/v1/widgets"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with date deprecation");
  let deprecation = response
    .deprecation()
    .expect("valid date deprecation should parse")
    .expect("deprecation header should be present");

  assert_eq!(Deprecation::Date(instant), deprecation);
  assert_eq!(
    Some(&"@1688169599".to_string()),
    response.header_value("Deprecation")
  );
}

#[test]
fn test_parse_deprecation_response_helper_allows_absent() {
  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/v1/widgets"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without deprecation");
  assert_eq!(
    None,
    response
      .deprecation()
      .expect("absent deprecation should parse")
  );
  assert_eq!(None, response.header_value("Deprecation"));
}

#[test]
fn test_parse_deprecation_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["true", "Sun, 06 Nov 1994 08:49:37 GMT", "?1;foo=?1", "?2"];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nDeprecation: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/v1/widgets"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.deprecation().is_err(),
      "deprecation helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Deprecation")
    );
  }
}

#[test]
fn test_parse_deprecation_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Deprecation: ?1\r\n",
    "deprecation: ?0\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/v1/widgets"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate deprecation remains usable");

  assert!(
    response.deprecation().is_err(),
    "deprecation helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"?1".to_string(), &"?0".to_string()],
    response.header_values("Deprecation")
  );

  let oversized = format!("?{}", "1".repeat(64 * 1024));
  let raw = format!("HTTP/1.1 200 OK\r\nDeprecation: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(
    RoUrl::with("https://example.test/v1/widgets"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized deprecation remains usable");

  assert!(
    response.deprecation().is_err(),
    "deprecation helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Deprecation"));
}

#[test]
fn test_parse_content_location_rejects_control_characters_and_crlf_injection() {
  let invalid_values = ["\r\nLocation: /evil", "/ok\r", "/ok\n", "/ok\tinner"];

  for value in invalid_values {
    assert!(
      ContentLocation::parse(value).is_err(),
      "content-location parser should reject {value:?}"
    );
  }
}

#[test]
fn test_parse_service_worker_allowed_response_helper_accepts_paths() {
  let cases = [
    ("/", "absolute path root"),
    ("/app/", "absolute path prefix"),
    ("/static/sw/", "absolute nested path"),
    ("/scope?feature=1", "absolute path with query"),
    ("/scope#section", "absolute path with fragment"),
    ("./", "relative dot segment"),
    ("../scope", "relative parent path"),
    ("scope/nested", "origin-relative path"),
  ];

  for (value, name) in cases {
    let raw =
      format!("HTTP/1.1 200 OK\r\nService-Worker-Allowed: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/static/sw.js"),
      raw.into_bytes(),
    )
    .unwrap_or_else(|err| panic!("{name} response should parse: {err}"));
    let allowed = response
      .service_worker_allowed()
      .unwrap_or_else(|err| panic!("{name} service-worker-allowed should parse: {err}"))
      .unwrap_or_else(|| panic!("{name} service-worker-allowed should be present"));

    assert_eq!(value, allowed.as_str());
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Service-Worker-Allowed")
    );
  }
}

#[test]
fn test_parse_service_worker_allowed_response_helper_trims_outer_whitespace_and_allows_absent() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Service-Worker-Allowed:   /   \r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/static/sw.js"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with service-worker-allowed");
  let allowed = response
    .service_worker_allowed()
    .expect("valid service-worker-allowed should parse")
    .expect("service-worker-allowed header should be present");

  assert_eq!("/", allowed.as_str());
  assert_eq!(
    Some(&"/".to_string()),
    response.header_value("Service-Worker-Allowed")
  );

  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/static/sw.js"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without service-worker-allowed");
  assert_eq!(
    None,
    response
      .service_worker_allowed()
      .expect("absent service-worker-allowed should parse")
  );
}

#[test]
fn test_parse_service_worker_allowed_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    " ",
    "/bad path",
    "/bad%zz",
    "http://example.test/scope",
    "//example.test/scope",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nService-Worker-Allowed: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/static/sw.js"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.service_worker_allowed().is_err(),
      "service-worker-allowed helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.trim().to_string()),
      response.header_value("Service-Worker-Allowed")
    );
  }
}

#[test]
fn test_parse_service_worker_allowed_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Service-Worker-Allowed: /\r\n",
    "service-worker-allowed: /app/\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/static/sw.js"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate service-worker-allowed remains usable");

  assert!(
    response.service_worker_allowed().is_err(),
    "service-worker-allowed helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"/".to_string(), &"/app/".to_string()],
    response.header_values("Service-Worker-Allowed")
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  let raw = format!(
    "HTTP/1.1 200 OK\r\nService-Worker-Allowed: {oversized}\r\nContent-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/static/sw.js"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized service-worker-allowed remains usable");

  assert!(
    response.service_worker_allowed().is_err(),
    "service-worker-allowed helper should reject oversized values"
  );
  assert_eq!(
    Some(&oversized),
    response.header_value("Service-Worker-Allowed")
  );
}

#[test]
fn test_parse_service_worker_allowed_rejects_control_characters_and_crlf_injection() {
  let invalid_values = ["\r\nLocation: /evil", "/ok\r", "/ok\n", "/ok\tinner"];

  for value in invalid_values {
    assert!(
      ServiceWorkerAllowed::parse(value).is_err(),
      "service-worker-allowed parser should reject {value:?}"
    );
  }
}

#[test]
fn test_parse_location_response_helper_allows_absent_absolute_and_relative_targets() {
  for value in [
    "https://example.test/path?q=1#section",
    "/next",
    "../login?next=%2Fdashboard",
    "?page=2",
    "//cdn.example.test/asset.js",
  ] {
    let raw = format!("HTTP/1.1 302 Found\r\nLocation: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");
    let location = response
      .location()
      .expect("Location should parse")
      .expect("Location should be present");

    assert_eq!(value, location.as_str());
    assert_eq!(Some(&value.to_string()), response.header_value("Location"));
  }

  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response without location");
  assert_eq!(
    None,
    response.location().expect("absent Location should parse")
  );
}

#[test]
fn test_parse_location_response_helper_trims_outer_whitespace() {
  let raw = concat!(
    "HTTP/1.1 302 Found\r\n",
    "Location:   /representations/current.json   \r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("parse response with location");
  let location = response
    .location()
    .expect("Location should parse")
    .expect("Location header should be present");

  assert_eq!("/representations/current.json", location.as_str());
  assert_eq!(
    Some(&"/representations/current.json".to_string()),
    response.header_value("Location")
  );
}

#[test]
fn test_parse_location_rejects_invalid_values_without_rejecting_response() {
  let invalid_values = ["", "http://[::1", "/bad path", "/bad%zz", "ok\u{7f}"];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 302 Found\r\nLocation: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(
      RoUrl::with("https://example.test/base/page"),
      raw.into_bytes(),
    )
    .expect("raw response remains usable");

    assert!(
      response.location().is_err(),
      "Location helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.trim().to_string()),
      response.header_value("Location")
    );
  }
}

#[test]
fn test_parse_location_rejects_duplicate_and_oversized_values() {
  let raw = concat!(
    "HTTP/1.1 302 Found\r\n",
    "Location: /one\r\n",
    "location: /two\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.as_bytes().to_vec(),
  )
  .expect("raw response with duplicate Location remains usable");

  assert!(
    response.location().is_err(),
    "Location helper should reject duplicate singleton headers"
  );
  assert_eq!(
    vec![&"/one".to_string(), &"/two".to_string()],
    response.header_values("Location")
  );

  let oversized = format!("/{}", "a".repeat(64 * 1024));
  let raw = format!("HTTP/1.1 302 Found\r\nLocation: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(
    RoUrl::with("https://example.test/base/page"),
    raw.into_bytes(),
  )
  .expect("raw response with oversized Location remains usable");

  assert!(
    response.location().is_err(),
    "Location helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Location"));
}

#[test]
fn test_parse_location_rejects_control_characters_and_crlf_injection() {
  let invalid_values = ["\r\nX-Evil: true", "/ok\r", "/ok\n", "/ok\tinner"];

  for value in invalid_values {
    assert!(
      Location::parse(value).is_err(),
      "Location parser should reject {value:?}"
    );
  }
}

#[test]
fn test_parse_cache_control_response_directives() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cache-Control: no-cache=\"Set-Cookie, Authorization\", no-store, max-age=60\r\n",
    "Cache-Control: s-maxage=120, private=\"X-User\", public, must-revalidate\r\n",
    "Cache-Control: proxy-revalidate, immutable, stale-while-revalidate=30, stale-if-error=90\r\n",
    "Cache-Control: community=\"u=1, tier=gold\", ext-token\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse cache-control response");

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
fn test_parse_cache_control_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "max-age=-1",
    "s-maxage=abc",
    "stale-while-revalidate=1.5",
    "stale-if-error=\"60\"",
    "private=\"unterminated",
    "extension=\"bad\\\"",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nCache-Control: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.cache_control().is_err(),
      "cache-control helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cache-Control")
    );
  }
}

#[test]
fn test_parse_cdn_cache_control_response_metadata() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "CDN-Cache-Control: max-age=600, stale-while-revalidate=30, cdn-example=\"a, b\"\r\n",
    "cdn-cache-control: immutable\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse CDN-Cache-Control response");

  let metadata = response
    .cdn_cache_control()
    .expect("valid CDN-Cache-Control should parse")
    .expect("CDN-Cache-Control should be present");

  assert_eq!(metadata.len(), 4);
  assert_eq!(metadata.directives()[0].name(), "max-age");
  assert_eq!(metadata.directives()[0].value(), Some("600"));
  assert_eq!(metadata.directives()[2].name(), "cdn-example");
  assert_eq!(metadata.directives()[2].value(), Some("a, b"));
  assert_eq!(metadata.directives()[3].name(), "immutable");
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_parse_cache_status_response_metadata() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cache-Status: OriginCache; hit; ttl=1100\r\n",
    "cache-status: \"CDN Company Here\"; hit; ttl=545\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse Cache-Status response");

  let metadata = response
    .cache_status()
    .expect("valid Cache-Status should parse")
    .expect("Cache-Status should be present");

  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.members()[0].identifier().as_str(), "OriginCache");
  assert_eq!(metadata.members()[0].hit(), Some(true));
  assert_eq!(metadata.members()[0].ttl(), Some(1100));
  assert_eq!(
    metadata.members()[1].identifier().as_str(),
    "CDN Company Here"
  );
  assert!(metadata.members()[1].identifier().is_string());
  assert_eq!(metadata.members()[1].ttl(), Some(545));
  assert_eq!(
    Some(&"OriginCache; hit; ttl=1100".to_string()),
    response.header_value("Cache-Status")
  );
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_parse_cache_status_rejects_invalid_helper_values_without_rejecting_response() {
  let oversized = "x".repeat(64 * 1024 + 1);
  let invalid_values = ["OriginCache; hit=yes", "OriginCache,", oversized.as_str()];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nCache-Status: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.cache_status().is_err(),
      "Cache-Status helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cache-Status")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_vec(),
  )
  .expect("raw response without Cache-Status remains usable");

  assert!(response
    .cache_status()
    .expect("missing header should be valid")
    .is_none());
}

#[test]
fn test_parse_cdn_cache_control_rejects_invalid_helper_values_without_rejecting_response() {
  let oversized = "x".repeat(64 * 1024 + 1);
  let invalid_values = [
    "max-age=",
    "max-age=not a token",
    "extension=\"unterminated",
    oversized.as_str(),
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nCDN-Cache-Control: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.cdn_cache_control().is_err(),
      "CDN-Cache-Control helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("CDN-Cache-Control")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }

  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_vec(),
  )
  .expect("raw response without CDN-Cache-Control remains usable");

  assert!(response
    .cdn_cache_control()
    .expect("missing header should be valid")
    .is_none());
}

#[test]
fn test_parse_age_and_expires_response_metadata() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Age: 2147483648\r\n",
    "Expires: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with age and expires metadata");

  assert_eq!(
    Some(2_147_483_648),
    response.age().expect("valid age should parse")
  );
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    response.expires().expect("valid expires should parse")
  );
  assert_eq!(
    Some(&"2147483648".to_string()),
    response.header_value("Age")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Expires")
  );

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without age or expires");
  assert_eq!(None, response.age().expect("absent age should parse"));
  assert_eq!(
    None,
    response.expires().expect("absent expires should parse")
  );
}

#[test]
fn test_parse_date_response_metadata() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Date: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with date metadata");

  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    response.date().expect("valid date should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Date")
  );
  assert_eq!(
    vec![&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()],
    response.header_values("Date")
  );

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without date");
  assert_eq!(None, response.date().expect("absent date should parse"));
}

#[test]
fn test_parse_date_rejects_invalid_duplicate_and_oversized_metadata_without_hiding_headers() {
  let invalid_value = "not a date";
  let raw = format!("HTTP/1.1 200 OK\r\nDate: {invalid_value}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with invalid date remains usable");

  assert!(
    response.date().is_err(),
    "Date helper should reject malformed values"
  );
  assert_eq!(
    Some(&invalid_value.to_string()),
    response.header_value("Date")
  );

  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Date: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "date: Sun, 06 Nov 1994 08:49:38 GMT\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate date remains usable");

  assert!(
    response.date().is_err(),
    "Date helper should reject duplicate values"
  );
  assert_eq!(
    vec![
      &"Sun, 06 Nov 1994 08:49:37 GMT".to_string(),
      &"Sun, 06 Nov 1994 08:49:38 GMT".to_string()
    ],
    response.header_values("Date")
  );

  let oversized = "x".repeat(64 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nDate: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized date remains usable");

  assert_eq!(
    "error receive response: Date header value is too large",
    response
      .date()
      .expect_err("Date helper should reject oversized values")
      .to_string()
  );
  assert_eq!(Some(&oversized), response.header_value("Date"));
}

#[test]
fn test_parse_sunset_response_metadata_and_preserves_raw_value() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Sunset: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784_111_777)),
    response.sunset().expect("Sunset should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.sunset_value()
  );
}

#[test]
fn test_parse_sunset_rejects_invalid_and_duplicate_values_without_rejecting_response() {
  for (header, expected_values) in [
    ("Sunset: not a date\r\n", vec!["not a date"]),
    ("sunset: not a date\r\n", vec!["not a date"]),
    (
      "Sunset: Sun, 06 Nov 1994 08:49:37 GMT\r\nsunset: Sun, 06 Nov 1994 08:49:38 GMT\r\n",
      vec![
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sun, 06 Nov 1994 08:49:38 GMT",
      ],
    ),
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\n{header}Content-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("response should remain usable");

    assert!(
      response.sunset().is_err(),
      "Sunset helper should reject {header:?}"
    );
    assert_eq!(
      expected_values,
      response
        .header_values("Sunset")
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
      "raw Sunset headers must remain inspectable after rejection"
    );
  }
}

#[test]
fn test_parse_memento_datetime_response_metadata_and_preserves_raw_value() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Memento-Datetime: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    Some(MementoDatetime::new(
      UNIX_EPOCH + Duration::from_secs(784_111_777)
    )),
    response
      .memento_datetime()
      .expect("Memento-Datetime should parse")
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Memento-Datetime")
  );
}

#[test]
fn test_parse_memento_datetime_rejects_invalid_and_duplicate_values_without_rejecting_response() {
  for (header, expected_values) in [
    ("Memento-Datetime: not a date\r\n", vec!["not a date"]),
    ("memento-datetime: not a date\r\n", vec!["not a date"]),
    (
      "Memento-Datetime: Sun, 06 Nov 1994 08:49:37 GMT\r\nmemento-datetime: Sun, 06 Nov 1994 08:49:38 GMT\r\n",
      vec![
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sun, 06 Nov 1994 08:49:38 GMT",
      ],
    ),
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\n{header}Content-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("response should remain usable");

    assert!(
      response.memento_datetime().is_err(),
      "Memento-Datetime helper should reject {header:?}"
    );
    assert_eq!(
      expected_values,
      response
        .header_values("Memento-Datetime")
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
      "raw Memento-Datetime headers must remain inspectable after rejection"
    );
  }
}

#[test]
fn test_parse_memento_datetime_returns_none_when_header_is_absent() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(
    None,
    response
      .memento_datetime()
      .expect("absent Memento-Datetime should parse")
  );
}

#[test]
fn test_parse_sunset_returns_none_when_header_is_absent() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(None, response.sunset().expect("absent Sunset should parse"));
  assert_eq!(None, response.sunset_value());
}

#[test]
fn test_parse_retry_after_response_metadata() {
  let s = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: 120\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with retry-after delta metadata");
  let retry_after = response
    .retry_after()
    .expect("valid retry-after should parse")
    .expect("retry-after should be present");

  assert_eq!(Some(120), retry_after.delta_seconds());
  assert_eq!(None, retry_after.http_date());
  assert_eq!(
    Some(&"120".to_string()),
    response.header_value("Retry-After")
  );

  let s = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: Sun, 06 Nov 1994 08:49:37 GMT\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with retry-after date metadata");
  let retry_after = response
    .retry_after()
    .expect("valid retry-after date should parse")
    .expect("retry-after should be present");

  assert_eq!(None, retry_after.delta_seconds());
  assert_eq!(
    Some(UNIX_EPOCH + Duration::from_secs(784111777)),
    retry_after.http_date()
  );
  assert_eq!(
    Some(&"Sun, 06 Nov 1994 08:49:37 GMT".to_string()),
    response.header_value("Retry-After")
  );

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without retry-after");
  assert_eq!(
    None,
    response
      .retry_after()
      .expect("absent retry-after should parse")
  );
}

#[test]
fn test_parse_retry_after_response_metadata_accepts_surrounding_http_whitespace() {
  let raw = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: \t120 \r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with Retry-After whitespace");

  assert_eq!(
    Some(120),
    response
      .retry_after()
      .expect("Retry-After metadata should parse")
      .expect("Retry-After metadata should be present")
      .delta_seconds()
  );
  assert_eq!(
    Some(120),
    RetryAfter::parse("\t120 ")
      .expect("RetryAfter parser should accept HTTP whitespace")
      .delta_seconds()
  );
}

#[test]
fn test_parse_retry_after_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "-1",
    "+1",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    "18446744073709551616",
    "Sun, 06 Nov 1994 08:49:37 PST",
  ];

  for value in invalid_values {
    let raw = format!(
      "HTTP/1.1 503 Service Unavailable\r\nRetry-After: {value}\r\nContent-Length: 4\r\n\r\nbusy"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.retry_after().is_err(),
      "retry-after helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Retry-After")
    );
  }
}

#[test]
fn test_parse_retry_after_rejects_duplicate_and_oversized_helper_values() {
  let raw = concat!(
    "HTTP/1.1 503 Service Unavailable\r\n",
    "Retry-After: 60\r\n",
    "retry-after: 120\r\n",
    "Content-Length: 4\r\n",
    "\r\n",
    "busy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate retry-after remains usable");

  assert!(
    response.retry_after().is_err(),
    "retry-after helper should reject duplicates"
  );
  assert_eq!(
    vec![&"60".to_string(), &"120".to_string()],
    response.header_values("Retry-After")
  );

  let oversized = "1".repeat(64 * 1024 + 1);
  let raw = format!(
    "HTTP/1.1 503 Service Unavailable\r\nRetry-After: {oversized}\r\nContent-Length: 4\r\n\r\nbusy"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized retry-after remains usable");

  assert!(
    response.retry_after().is_err(),
    "retry-after helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Retry-After"));
}

#[test]
fn test_parse_allow_response_helper_preserves_method_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 405 Method Not Allowed\r\n",
    "Allow: GET, HEAD\r\n",
    "allow: POST, OPTIONS\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with allow headers");
  let allow = response
    .allow()
    .expect("valid allow should parse")
    .expect("allow header should be present");

  assert_eq!(vec!["GET", "HEAD", "POST", "OPTIONS"], allow.methods());
  assert!(allow.contains_method("POST"));
  assert!(!allow.contains_method("PATCH"));
  assert_eq!(
    vec![&"GET, HEAD".to_string(), &"POST, OPTIONS".to_string()],
    response.header_values("Allow")
  );
}

#[test]
fn test_parse_allow_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without allow");

  assert_eq!(None, response.allow().expect("absent allow should parse"));
}

#[test]
fn test_parse_allow_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "GET,",
    ",GET",
    "GET,,POST",
    "GET, ,POST",
    "GET POST",
    "GET@POST",
    "GE\tT",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.allow().is_err(),
      "allow helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Allow"));
  }
}

#[test]
fn test_parse_allow_rejects_duplicate_oversized_and_too_many_methods() {
  let raw = concat!(
    "HTTP/1.1 405 Method Not Allowed\r\n",
    "Allow: GET, HEAD\r\n",
    "allow: POST, GET\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate allow remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject duplicate method names"
  );
  assert_eq!(
    vec![&"GET, HEAD".to_string(), &"POST, GET".to_string()],
    response.header_values("Allow")
  );

  let oversized = "GET".repeat(64 * 1024);
  let raw =
    format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized allow remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Allow"));

  let too_many = (0..257)
    .map(|ix| format!("M{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw =
    format!("HTTP/1.1 405 Method Not Allowed\r\nAllow: {too_many}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many allow methods remains usable");

  assert!(
    response.allow().is_err(),
    "allow helper should reject too many methods"
  );
  assert_eq!(Some(&too_many), response.header_value("Allow"));
}

#[test]
fn test_parse_accept_ranges_response_helper_preserves_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: bytes, pages\r\n",
    "accept-ranges: records\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with accept-ranges headers");
  let accept_ranges = response
    .accept_ranges()
    .expect("valid accept-ranges should parse")
    .expect("accept-ranges header should be present");

  assert!(!accept_ranges.is_none());
  assert!(accept_ranges.accepts_bytes());
  assert_eq!(vec!["bytes", "pages", "records"], accept_ranges.units());
  assert_eq!(
    vec![&"bytes, pages".to_string(), &"records".to_string()],
    response.header_values("Accept-Ranges")
  );
}

#[test]
fn test_parse_accept_ranges_response_helper_supports_none_and_absent_header() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: none\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with none accept-ranges");
  let accept_ranges = response
    .accept_ranges()
    .expect("valid none accept-ranges should parse")
    .expect("accept-ranges header should be present");

  assert!(accept_ranges.is_none());
  assert!(!accept_ranges.accepts_bytes());
  assert!(accept_ranges.units().is_empty());

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without accept-ranges");
  assert_eq!(
    None,
    response
      .accept_ranges()
      .expect("absent accept-ranges should parse")
  );
}

#[test]
fn test_parse_accept_ranges_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "bytes,",
    ",bytes",
    "bytes,,pages",
    "bytes, ,pages",
    "byte ranges",
    "bytes@pages",
    "bytes, none",
    "none, bytes",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.accept_ranges().is_err(),
      "accept-ranges helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Accept-Ranges")
    );
  }
}

#[test]
fn test_parse_accept_ranges_rejects_duplicate_oversized_and_too_many_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Ranges: bytes, pages\r\n",
    "accept-ranges: BYTES\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate accept-ranges remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject normalized duplicate units"
  );
  assert_eq!(
    vec![&"bytes, pages".to_string(), &"BYTES".to_string()],
    response.header_values("Accept-Ranges")
  );

  let oversized = "bytes".repeat(16 * 1024);
  let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized accept-ranges remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Accept-Ranges"));

  let too_many = (0..257)
    .map(|ix| format!("unit{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw = format!("HTTP/1.1 200 OK\r\nAccept-Ranges: {too_many}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many accept-ranges values remains usable");

  assert!(
    response.accept_ranges().is_err(),
    "accept-ranges helper should reject too many values"
  );
  assert_eq!(Some(&too_many), response.header_value("Accept-Ranges"));
}

#[test]
fn test_parse_accept_patch_response_helper_preserves_media_types_across_header_fields() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-Patch: application/json; charset=utf-8, application/merge-patch+json\r\n",
    "accept-patch: application/example; profile=\"https://example.test/schema,v1\"\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with Accept-Patch headers");
  let accept_patch = response
    .accept_patch()
    .expect("valid Accept-Patch should parse")
    .expect("Accept-Patch header should be present");

  assert_eq!(
    vec![
      "application/json",
      "application/merge-patch+json",
      "application/example",
    ],
    accept_patch
      .media_types()
      .iter()
      .map(|media_type| media_type.essence())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("https://example.test/schema,v1"),
    accept_patch.media_types()[2].parameter("profile")
  );
  assert_eq!(
    vec![
      &"application/json; charset=utf-8, application/merge-patch+json".to_string(),
      &"application/example; profile=\"https://example.test/schema,v1\"".to_string(),
    ],
    response.header_values("Accept-Patch")
  );
}

#[test]
fn test_parse_accept_post_response_helper_preserves_parameters_across_header_fields() {
  let raw = concat!(
    "HTTP/1.1 201 Created\r\n",
    "Accept-Post: text/plain; charset=utf-8\r\n",
    "accept-post: application/json; profile=\"https://example.test/v1\"\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with Accept-Post headers");
  let accept_post = response
    .accept_post()
    .expect("valid Accept-Post should parse")
    .expect("Accept-Post header should be present");

  assert_eq!(
    vec!["text/plain", "application/json"],
    accept_post
      .media_types()
      .iter()
      .map(|media_type| media_type.essence())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    Some("utf-8"),
    accept_post.media_types()[0].parameter("charset")
  );
  assert_eq!(
    Some("https://example.test/v1"),
    accept_post.media_types()[1].parameter("profile")
  );
}

#[test]
fn test_accept_patch_and_accept_post_helpers_preserve_malformed_headers() {
  for (header, value) in [
    ("Accept-Patch", "application/json,"),
    ("Accept-Post", "application/json; charset=\"unterminated"),
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\n{header}: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    if header == "Accept-Patch" {
      assert!(response.accept_patch().is_err());
    } else {
      assert!(response.accept_post().is_err());
    }
    assert_eq!(Some(&value.to_string()), response.header_value(header));
  }
}

#[test]
fn test_accept_patch_and_accept_post_helpers_return_none_when_absent() {
  let raw = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 0\r\n", "\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response without accept metadata");

  assert_eq!(
    None,
    response
      .accept_patch()
      .expect("absent Accept-Patch should parse")
  );
  assert_eq!(
    None,
    response
      .accept_post()
      .expect("absent Accept-Post should parse")
  );
}

#[test]
fn test_accept_ch_and_critical_ch_response_helpers_parse_metadata_only() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-CH: Sec-CH-UA, DPR\r\n",
    "accept-ch: Viewport-Width\r\n",
    "Critical-CH: Sec-CH-UA-Platform, Downlink\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with client hint metadata");

  assert_eq!(
    ["Sec-CH-UA", "DPR", "Viewport-Width"],
    response
      .accept_ch()
      .expect("Accept-CH should parse")
      .expect("Accept-CH should be present")
      .client_hints()
  );
  assert_eq!(
    ["Sec-CH-UA-Platform", "Downlink"],
    response
      .critical_ch()
      .expect("Critical-CH should parse")
      .expect("Critical-CH should be present")
      .client_hints()
  );
}

#[test]
fn test_timing_allow_origin_response_helper_parses_metadata_and_preserves_raw_headers() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Timing-Allow-Origin: https://example.test, https://api.example.test\r\n",
    "timing-allow-origin: https://static.example.test\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with timing metadata");

  let timing_allow_origin = response
    .timing_allow_origin()
    .expect("Timing-Allow-Origin should parse")
    .expect("Timing-Allow-Origin should be present");
  assert_eq!(
    timing_allow_origin.origins(),
    [
      "https://example.test",
      "https://api.example.test",
      "https://static.example.test",
    ]
  );
  assert_eq!(
    response.header_values("Timing-Allow-Origin"),
    vec![
      &"https://example.test, https://api.example.test".to_string(),
      &"https://static.example.test".to_string(),
    ]
  );
}

#[test]
fn test_timing_allow_origin_response_helper_supports_wildcard_and_absence() {
  let wildcard = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nTiming-Allow-Origin: *\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response with wildcard timing metadata");
  assert!(wildcard
    .timing_allow_origin()
    .expect("wildcard should parse")
    .expect("Timing-Allow-Origin should be present")
    .is_wildcard());

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without timing metadata");
  assert_eq!(
    None,
    absent.timing_allow_origin().expect("absence should parse")
  );
}

#[test]
fn test_timing_allow_origin_response_helper_preserves_invalid_raw_headers() {
  let value = "https://example.test, *";
  let raw = format!("HTTP/1.1 200 OK\r\nTiming-Allow-Origin: {value}\r\nContent-Length: 0\r\n\r\n");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response should remain usable");

  assert!(response.timing_allow_origin().is_err());
  assert_eq!(
    Some(&value.to_string()),
    response.header_value("Timing-Allow-Origin")
  );
}

#[test]
fn test_client_hint_response_helpers_preserve_invalid_or_absent_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Accept-CH: DPR,\r\n",
    "Critical-CH: 1Downlink\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.accept_ch().is_err());
  assert!(response.critical_ch().is_err());
  assert_eq!(
    Some(&"DPR,".to_string()),
    response.header_value("Accept-CH")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without client hint metadata");
  assert_eq!(
    None,
    absent.accept_ch().expect("absent Accept-CH should parse")
  );
  assert_eq!(
    None,
    absent
      .critical_ch()
      .expect("absent Critical-CH should parse")
  );
}

#[test]
fn test_access_control_expose_headers_response_helper_parses_metadata_only() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Access-Control-Expose-Headers: X-Request-Id, ETag\r\n",
    "access-control-expose-headers: X-RateLimit-Remaining\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with Access-Control-Expose-Headers metadata");

  let expose_headers = response
    .access_control_expose_headers()
    .expect("Access-Control-Expose-Headers should parse")
    .expect("Access-Control-Expose-Headers should be present");
  assert_eq!(
    expose_headers.field_names(),
    ["x-request-id", "etag", "x-ratelimit-remaining"]
  );
  assert_eq!(
    vec![
      &"X-Request-Id, ETag".to_string(),
      &"X-RateLimit-Remaining".to_string()
    ],
    response.header_values("access-control-expose-headers")
  );
}

#[test]
fn test_access_control_expose_headers_response_helper_supports_wildcard_and_absence() {
  let wildcard = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nAccess-Control-Expose-Headers: *\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response with wildcard Access-Control-Expose-Headers");
  assert!(wildcard
    .access_control_expose_headers()
    .expect("wildcard Access-Control-Expose-Headers should parse")
    .expect("Access-Control-Expose-Headers should be present")
    .is_wildcard());

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without Access-Control-Expose-Headers");
  assert_eq!(
    None,
    absent
      .access_control_expose_headers()
      .expect("absent Access-Control-Expose-Headers should parse")
  );
}

#[test]
fn test_access_control_expose_headers_response_helper_deduplicates_metadata_and_preserves_raw_header(
) {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Access-Control-Expose-Headers: X-Request-Id, x-request-id\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert_eq!(
    ["x-request-id"],
    response
      .access_control_expose_headers()
      .expect("duplicate Access-Control-Expose-Headers should parse")
      .expect("Access-Control-Expose-Headers should be present")
      .field_names()
  );
  assert_eq!(
    Some(&"X-Request-Id, x-request-id".to_string()),
    response.header_value("Access-Control-Expose-Headers")
  );
}

#[test]
fn test_access_control_allow_methods_response_helper_parses_repeated_metadata_only() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Access-Control-Allow-Methods: get, POST\r\n",
    "access-control-allow-methods: PATCH, get\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("parse response with Access-Control-Allow-Methods metadata");

  assert_eq!(
    response
      .access_control_allow_methods()
      .expect("Access-Control-Allow-Methods should parse")
      .expect("Access-Control-Allow-Methods should be present")
      .methods(),
    ["GET", "POST", "PATCH"]
  );
  assert_eq!(
    response.header_values("access-control-allow-methods"),
    [&"get, POST".to_string(), &"PATCH, get".to_string()]
  );
}

#[test]
fn test_access_control_allow_methods_response_helper_preserves_invalid_or_absent_metadata() {
  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Methods: GET,,\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response should remain usable");

  assert!(malformed.access_control_allow_methods().is_err());
  assert_eq!(
    malformed.header_value("Access-Control-Allow-Methods"),
    Some(&"GET,,".to_string())
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without Access-Control-Allow-Methods");
  assert_eq!(
    absent
      .access_control_allow_methods()
      .expect("absent Access-Control-Allow-Methods should parse"),
    None
  );
}

#[test]
fn test_access_control_allow_origin_response_helper_parses_valid_metadata_and_preserves_invalid_raw_headers(
) {
  for value in ["*", "null", "https://example.test:8443"] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert_eq!(
      value,
      response
        .access_control_allow_origin()
        .expect("Access-Control-Allow-Origin should parse")
        .expect("Access-Control-Allow-Origin should be present")
        .header_value()
    );
  }

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response without metadata should remain usable");
  assert_eq!(
    None,
    absent
      .access_control_allow_origin()
      .expect("absence should parse")
  );

  for value in [
    "https://example.test, https://other.test".to_string(),
    "https://example.test/path".to_string(),
    "x".repeat(64 * 1024 + 1),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.access_control_allow_origin().is_err());
    assert_eq!(
      response.header_value("Access-Control-Allow-Origin"),
      Some(&value)
    );
  }

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Access-Control-Allow-Origin: https://example.test\r\n",
      "access-control-allow-origin: https://other.test\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response with duplicate metadata should remain usable");
  assert!(duplicate.access_control_allow_origin().is_err());
  assert_eq!(
    duplicate.header_values("Access-Control-Allow-Origin"),
    [
      &"https://example.test".to_string(),
      &"https://other.test".to_string()
    ]
  );
}

#[test]
fn test_access_control_allow_credentials_response_helper_parses_valid_metadata_and_preserves_invalid_raw_headers(
) {
  {
    let value = "true";
    let raw = format!(
      "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Credentials: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert_eq!(
      "true",
      response
        .access_control_allow_credentials()
        .expect("Access-Control-Allow-Credentials should parse")
        .expect("Access-Control-Allow-Credentials should be present")
        .header_value()
    );
  }

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("raw response without metadata should remain usable");
  assert_eq!(
    None,
    absent
      .access_control_allow_credentials()
      .expect("absence should parse")
  );

  for value in [
    "TRUE".to_string(),
    "True".to_string(),
    "false".to_string(),
    "true, true".to_string(),
    "x".repeat(64 * 1024 + 1),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Credentials: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.access_control_allow_credentials().is_err());
    assert_eq!(
      response.header_value("Access-Control-Allow-Credentials"),
      Some(&value)
    );
  }

  let duplicate = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Access-Control-Allow-Credentials: true\r\n",
      "access-control-allow-credentials: true\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response with duplicate metadata should remain usable");
  assert!(duplicate.access_control_allow_credentials().is_err());
  assert_eq!(
    duplicate.header_values("Access-Control-Allow-Credentials"),
    [&"true".to_string(), &"true".to_string()]
  );
}

#[test]
fn test_access_control_allow_headers_response_helper_parses_valid_lists_wildcard_and_multiple_fields(
) {
  let listed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Headers: X-Request-Id, ETag\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response with Access-Control-Allow-Headers list");
  assert_eq!(
    listed
      .access_control_allow_headers()
      .expect("Access-Control-Allow-Headers should parse")
      .expect("Access-Control-Allow-Headers should be present")
      .field_names(),
    ["x-request-id", "etag"]
  );

  let wildcard = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Headers: *\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response with wildcard Access-Control-Allow-Headers");
  assert!(wildcard
    .access_control_allow_headers()
    .expect("wildcard Access-Control-Allow-Headers should parse")
    .expect("Access-Control-Allow-Headers should be present")
    .is_wildcard());

  let repeated = Response::new(
    RoUrl::with("https://example.test"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Access-Control-Allow-Headers: X-Request-Id\r\n",
      "access-control-allow-headers: ETag, X-RateLimit-Remaining\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("parse response with repeated Access-Control-Allow-Headers");
  assert_eq!(
    repeated
      .access_control_allow_headers()
      .expect("repeated Access-Control-Allow-Headers should parse")
      .expect("Access-Control-Allow-Headers should be present")
      .field_names(),
    ["x-request-id", "etag", "x-ratelimit-remaining"]
  );
  assert_eq!(
    repeated.header_values("access-control-allow-headers"),
    [
      &"X-Request-Id".to_string(),
      &"ETag, X-RateLimit-Remaining".to_string()
    ]
  );
}

#[test]
fn test_access_control_allow_headers_response_helper_handles_absent_invalid_duplicate_and_bounded_metadata(
) {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without Access-Control-Allow-Headers");
  assert_eq!(
    absent
      .access_control_allow_headers()
      .expect("absent Access-Control-Allow-Headers should parse"),
    None
  );

  for value in [
    "X-Request-Id,,ETag".to_string(),
    "X-Request-Id, x-request-id".to_string(),
    "x".repeat(64 * 1024 + 1),
    (0..=256)
      .map(|index| format!("x{index}"))
      .collect::<Vec<_>>()
      .join(","),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Headers: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");

    assert!(response.access_control_allow_headers().is_err());
    assert_eq!(
      response.header_value("Access-Control-Allow-Headers"),
      Some(&value)
    );
  }
}

#[test]
fn test_access_control_max_age_response_helper_parses_valid_and_maximum_values() {
  for (value, expected_seconds) in [("600", 600), ("18446744073709551615", u64::MAX)] {
    let raw =
      format!("HTTP/1.1 200 OK\r\nAccess-Control-Max-Age: {value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("parse response with Access-Control-Max-Age metadata");

    assert_eq!(
      response
        .access_control_max_age()
        .expect("Access-Control-Max-Age should parse")
        .expect("Access-Control-Max-Age should be present")
        .seconds(),
      expected_seconds
    );
  }
}

#[test]
fn test_access_control_max_age_response_helper_handles_absent_duplicate_and_malformed_metadata() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without Access-Control-Max-Age");
  assert_eq!(
    absent
      .access_control_max_age()
      .expect("absent Access-Control-Max-Age should parse"),
    None
  );

  for value in [
    "Access-Control-Max-Age: 60\r\naccess-control-max-age: 120",
    "Access-Control-Max-Age: 60 seconds",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\n{value}\r\nContent-Length: 0\r\n\r\n");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");

    assert!(response.access_control_max_age().is_err());
  }
}

#[test]
fn test_cross_origin_resource_policy_response_metadata_preserves_raw_headers() {
  for (value, policy) in [
    ("SAME-ORIGIN", CrossOriginResourcePolicy::SameOrigin),
    ("same-site", CrossOriginResourcePolicy::SameSite),
    ("Cross-Origin", CrossOriginResourcePolicy::CrossOrigin),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nCross-Origin-Resource-Policy: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should parse");

    assert_eq!(
      policy,
      response
        .cross_origin_resource_policy()
        .expect("CORP should parse")
        .expect("CORP should be present")
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cross-Origin-Resource-Policy")
    );
  }
}

#[test]
fn test_cross_origin_resource_policy_response_metadata_rejects_invalid_and_absent_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cross-Origin-Resource-Policy: same-origin\r\n",
    "Cross-Origin-Resource-Policy: same-site\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.cross_origin_resource_policy().is_err());
  assert_eq!(
    Some(&"same-origin".to_string()),
    response.header_value("Cross-Origin-Resource-Policy")
  );

  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Resource-Policy: same origin\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with malformed CORP should parse");
  assert!(malformed.cross_origin_resource_policy().is_err());
  assert_eq!(
    Some(&"same origin".to_string()),
    malformed.header_value("Cross-Origin-Resource-Policy")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without CORP should parse");
  assert_eq!(
    None,
    absent
      .cross_origin_resource_policy()
      .expect("absent CORP should parse")
  );
}

#[test]
fn test_cross_origin_embedder_policy_response_metadata_preserves_raw_headers() {
  for (value, policy) in [
    ("unsafe-none", CrossOriginEmbedderPolicy::UnsafeNone),
    (
      r#"require-corp; report-to="coep""#,
      CrossOriginEmbedderPolicy::RequireCorp,
    ),
    ("credentialless", CrossOriginEmbedderPolicy::Credentialless),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should parse");

    assert_eq!(
      policy,
      response
        .cross_origin_embedder_policy()
        .expect("COEP should parse")
        .expect("COEP should be present")
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cross-Origin-Embedder-Policy")
    );
  }
}

#[test]
fn test_cross_origin_embedder_policy_response_metadata_rejects_invalid_and_absent_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cross-Origin-Embedder-Policy: require-corp\r\n",
    "Cross-Origin-Embedder-Policy: credentialless\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.cross_origin_embedder_policy().is_err());
  assert_eq!(
    Some(&"require-corp".to_string()),
    response.header_value("Cross-Origin-Embedder-Policy")
  );

  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy: require corp\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with malformed COEP should parse");
  assert!(malformed.cross_origin_embedder_policy().is_err());
  assert_eq!(
    Some(&"require corp".to_string()),
    malformed.header_value("Cross-Origin-Embedder-Policy")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without COEP should parse");
  assert_eq!(
    None,
    absent
      .cross_origin_embedder_policy()
      .expect("absent COEP should parse")
  );
}

#[test]
fn test_cross_origin_embedder_policy_report_only_response_metadata_preserves_raw_headers() {
  for (value, policy) in [
    (
      "unsafe-none",
      CrossOriginEmbedderPolicyReportOnly::UnsafeNone,
    ),
    (
      r#"require-corp; report-to="coep""#,
      CrossOriginEmbedderPolicyReportOnly::RequireCorp,
    ),
    (
      "credentialless",
      CrossOriginEmbedderPolicyReportOnly::Credentialless,
    ),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy-Report-Only: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should parse");

    assert_eq!(
      policy,
      response
        .cross_origin_embedder_policy_report_only()
        .expect("COEP-Report-Only should parse")
        .expect("COEP-Report-Only should be present")
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cross-Origin-Embedder-Policy-Report-Only")
    );
  }
}

#[test]
fn test_cross_origin_embedder_policy_report_only_response_metadata_rejects_invalid_and_absent_values(
) {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cross-Origin-Embedder-Policy-Report-Only: require-corp\r\n",
    "cross-origin-embedder-policy-report-only: credentialless\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.cross_origin_embedder_policy_report_only().is_err());
  assert_eq!(
    Some(&"require-corp".to_string()),
    response.header_value("Cross-Origin-Embedder-Policy-Report-Only")
  );

  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy-Report-Only: require corp\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with malformed COEP-Report-Only should parse");
  assert!(malformed
    .cross_origin_embedder_policy_report_only()
    .is_err());
  assert_eq!(
    Some(&"require corp".to_string()),
    malformed.header_value("Cross-Origin-Embedder-Policy-Report-Only")
  );

  let uppercase = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy-Report-Only: REQUIRE-CORP\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with uppercase COEP-Report-Only should parse");
  assert!(uppercase
    .cross_origin_embedder_policy_report_only()
    .is_err());

  let oversized = format!(
    "HTTP/1.1 200 OK\r\nCross-Origin-Embedder-Policy-Report-Only: {}\r\nContent-Length: 0\r\n\r\n",
    "x".repeat(64 * 1024 + 1)
  );
  let oversized = Response::new(RoUrl::with("https://example.test"), oversized.into_bytes())
    .expect("raw response with oversized COEP-Report-Only should parse");
  assert!(oversized
    .cross_origin_embedder_policy_report_only()
    .is_err());

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without COEP-Report-Only should parse");
  assert_eq!(
    None,
    absent
      .cross_origin_embedder_policy_report_only()
      .expect("absent COEP-Report-Only should parse")
  );
}

#[test]
fn test_cross_origin_opener_policy_response_metadata_preserves_raw_headers() {
  for (value, policy) in [
    ("unsafe-none", CrossOriginOpenerPolicy::UnsafeNone),
    (
      "same-origin-allow-popups",
      CrossOriginOpenerPolicy::SameOriginAllowPopups,
    ),
    ("same-origin", CrossOriginOpenerPolicy::SameOrigin),
    (
      r#"noopener-allow-popups; report-to="coop""#,
      CrossOriginOpenerPolicy::NoopenerAllowPopups,
    ),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should parse");

    assert_eq!(
      policy,
      response
        .cross_origin_opener_policy()
        .expect("COOP should parse")
        .expect("COOP should be present")
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cross-Origin-Opener-Policy")
    );
  }
}

#[test]
fn test_cross_origin_opener_policy_response_metadata_rejects_invalid_and_absent_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cross-Origin-Opener-Policy: same-origin\r\n",
    "Cross-Origin-Opener-Policy: same-origin-allow-popups\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.cross_origin_opener_policy().is_err());
  assert_eq!(
    Some(&"same-origin".to_string()),
    response.header_value("Cross-Origin-Opener-Policy")
  );

  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy: same origin\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with malformed COOP should parse");
  assert!(malformed.cross_origin_opener_policy().is_err());
  assert_eq!(
    Some(&"same origin".to_string()),
    malformed.header_value("Cross-Origin-Opener-Policy")
  );

  let case_variant = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy: SAME-ORIGIN\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with case-variant COOP should parse");
  assert!(case_variant.cross_origin_opener_policy().is_err());
  assert_eq!(
    Some(&"SAME-ORIGIN".to_string()),
    case_variant.header_value("Cross-Origin-Opener-Policy")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without COOP should parse");
  assert_eq!(
    None,
    absent
      .cross_origin_opener_policy()
      .expect("absent COOP should parse")
  );
}

#[test]
fn test_cross_origin_opener_policy_report_only_response_metadata_preserves_raw_headers() {
  for (value, policy, report_to) in [
    ("unsafe-none", CrossOriginOpenerPolicy::UnsafeNone, None),
    (
      "same-origin-allow-popups",
      CrossOriginOpenerPolicy::SameOriginAllowPopups,
      None,
    ),
    ("same-origin", CrossOriginOpenerPolicy::SameOrigin, None),
    (
      r#"noopener-allow-popups; report-to="coop""#,
      CrossOriginOpenerPolicy::NoopenerAllowPopups,
      Some("coop"),
    ),
  ] {
    let raw = format!(
      "HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy-Report-Only: {value}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should parse");
    let metadata = response
      .cross_origin_opener_policy_report_only()
      .expect("COOP-Report-Only should parse")
      .expect("COOP-Report-Only should be present");

    assert_eq!(policy, metadata.policy());
    assert_eq!(report_to, metadata.report_to());
    assert_eq!(value, metadata.header_value());
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Cross-Origin-Opener-Policy-Report-Only")
    );
  }
}

#[test]
fn test_cross_origin_opener_policy_report_only_response_metadata_rejects_invalid_and_absent_values()
{
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cross-Origin-Opener-Policy-Report-Only: same-origin\r\n",
    "Cross-Origin-Opener-Policy-Report-Only: same-origin-allow-popups\r\n",
    "Content-Length: 0\r\n",
    "\r\n"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");

  assert!(response.cross_origin_opener_policy_report_only().is_err());
  assert_eq!(
    Some(&"same-origin".to_string()),
    response.header_value("Cross-Origin-Opener-Policy-Report-Only")
  );

  let malformed = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy-Report-Only: same origin\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with malformed COOP-Report-Only should parse");
  assert!(malformed.cross_origin_opener_policy_report_only().is_err());
  assert_eq!(
    Some(&"same origin".to_string()),
    malformed.header_value("Cross-Origin-Opener-Policy-Report-Only")
  );

  let case_variant = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy-Report-Only: SAME-ORIGIN\r\nContent-Length: 0\r\n\r\n"
      .to_vec(),
  )
  .expect("raw response with case-variant COOP-Report-Only should parse");
  assert!(case_variant
    .cross_origin_opener_policy_report_only()
    .is_err());
  assert_eq!(
    Some(&"SAME-ORIGIN".to_string()),
    case_variant.header_value("Cross-Origin-Opener-Policy-Report-Only")
  );

  let oversized = format!(
    "HTTP/1.1 200 OK\r\nCross-Origin-Opener-Policy-Report-Only: {}\r\nContent-Length: 0\r\n\r\n",
    "x".repeat(64 * 1024 + 1)
  );
  let oversized = Response::new(RoUrl::with("https://example.test"), oversized.into_bytes())
    .expect("raw response with oversized COOP-Report-Only should parse");
  assert!(oversized.cross_origin_opener_policy_report_only().is_err());

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without COOP-Report-Only should parse");
  assert_eq!(
    None,
    absent
      .cross_origin_opener_policy_report_only()
      .expect("absent COOP-Report-Only should parse")
  );
}

#[test]
fn test_parse_age_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "-1",
    "+1",
    "1.5",
    "6 0",
    "60,61",
    "abc",
    "18446744073709551616",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nAge: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.age().is_err(),
      "age helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Age"));
  }
}

#[test]
fn test_parse_age_rejects_duplicate_and_oversized_helper_values_without_rejecting_response() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Age: 5\r\n",
    "age: 12\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate Age remains usable");

  assert!(
    response.age().is_err(),
    "age helper should reject duplicates"
  );
  assert_eq!(Some(&"5".to_string()), response.header_value("Age"));
  assert_eq!(
    vec![&"5".to_string(), &"12".to_string()],
    response.header_values("Age")
  );

  let oversized = "0".repeat(64 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nAge: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized Age remains usable");

  assert!(
    response.age().is_err(),
    "age helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Age"));
  assert_eq!(vec![&oversized], response.header_values("Age"));
}

#[test]
fn test_parse_expires_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = ["", "not a date", "Sun, 06 Nov 1994 08:49:37 PST"];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nExpires: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.expires().is_err(),
      "expires helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Expires"));
  }
}

#[test]
fn test_parse_vary_response_helper_normalizes_and_deduplicates_field_names() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Vary: Accept-Encoding, User-Agent\r\n",
    "VARY: accept-encoding, X-Feature\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with vary headers");

  let vary = response
    .vary()
    .expect("valid vary should parse")
    .expect("vary header should be present");

  assert!(!vary.is_any());
  assert_eq!(
    vec!["accept-encoding", "user-agent", "x-feature"],
    vary.field_names()
  );
  assert!(vary.contains_field_name("ACCEPT-ENCODING"));
  assert!(vary.contains_field_name("user-agent"));
  assert!(!vary.contains_field_name("authorization"));
  assert_eq!(
    vec![
      &"Accept-Encoding, User-Agent".to_string(),
      &"accept-encoding, X-Feature".to_string()
    ],
    response.header_values("vary")
  );
}

#[test]
fn test_parse_trailer_header_metadata_keeps_te_capability_separate() {
  let response = Response::new(
    RoUrl::with("http://example.test/"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "TE: trailers\r\n",
      "Trailer: X-Checksum, x-signature\r\n",
      "Trailer: X-Checksum\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");
  let trailer = response
    .trailer_header()
    .expect("Trailer metadata should parse")
    .expect("Trailer metadata should be present");
  assert_eq!(vec!["x-checksum", "x-signature"], trailer.field_names());

  let invalid = Response::new(
    RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nTrailer: Content-Length\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response framing should parse");
  assert!(invalid.trailer_header().is_err());
}

#[test]
fn test_parse_vary_response_helper_supports_wildcard_and_absent_header() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Vary: *\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with wildcard vary");
  let vary = response
    .vary()
    .expect("valid wildcard vary should parse")
    .expect("vary header should be present");

  assert!(vary.is_any());
  assert!(vary.field_names().is_empty());
  assert!(!vary.contains_field_name("accept"));

  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without vary");
  assert_eq!(None, response.vary().expect("absent vary should parse"));
}

#[test]
fn test_parse_vary_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "Accept,",
    ",Accept",
    "Accept,,User-Agent",
    "Accept, ,User-Agent",
    "Accept Encoding",
    "Accept@Encoding",
    "*, Accept",
    "Accept, *",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nVary: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.vary().is_err(),
      "vary helper should reject {value:?}"
    );
    assert_eq!(Some(&value.to_string()), response.header_value("Vary"));
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn test_parse_vary_rejects_oversized_and_too_many_field_names() {
  let oversized = "x".repeat(64 * 1024 + 1);
  let raw = format!("HTTP/1.1 200 OK\r\nVary: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized vary remains usable");

  assert!(
    response.vary().is_err(),
    "vary helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Vary"));
  assert_eq!(vec![&oversized], response.header_values("vary"));
  assert_eq!("OK", response.body().string().unwrap());

  let too_many = std::iter::repeat_n("Accept-Encoding", 257)
    .collect::<Vec<_>>()
    .join(",");
  let raw = format!("HTTP/1.1 200 OK\r\nVary: {too_many}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many vary field names remains usable");

  assert!(
    response.vary().is_err(),
    "vary helper should reject too many field names"
  );
  assert_eq!(Some(&too_many), response.header_value("Vary"));
  assert_eq!(vec![&too_many], response.header_values("vary"));
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn test_parse_no_vary_search_response_helper() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "No-Vary-Search: key-order=?0, params\r\n",
    "NO-VARY-SEARCH: except=(\"session\" \"debug\")\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with No-Vary-Search headers");

  let no_vary_search = response
    .no_vary_search()
    .expect("valid No-Vary-Search should parse")
    .expect("No-Vary-Search should be present");

  assert_eq!(Some(false), no_vary_search.key_order());
  assert!(no_vary_search.ignores_all_query_params());
  assert_eq!(no_vary_search.except(), ["session", "debug"]);
  assert_eq!(
    vec![
      &"key-order=?0, params".to_string(),
      &"except=(\"session\" \"debug\")".to_string()
    ],
    response.header_values("no-vary-search")
  );

  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("parse response without No-Vary-Search");
  assert_eq!(
    None,
    absent
      .no_vary_search()
      .expect("absent No-Vary-Search should parse")
  );
}

#[test]
fn test_parse_no_vary_search_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "Params",
    "params=utm",
    r#"params=("utm"), except=("session")"#,
    "key-order=false",
  ];

  for value in invalid_values {
    let raw = format!("HTTP/1.1 200 OK\r\nNo-Vary-Search: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.no_vary_search().is_err(),
      "No-Vary-Search helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("No-Vary-Search")
    );
  }
}

#[test]
fn test_parse_content_language_response_helper_preserves_order_across_header_fields() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Language: en-US, fr\r\n",
    "content-language: zh-Hant-TW, *\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response with content-language headers");

  let content_language = response
    .content_language()
    .expect("valid content-language should parse")
    .expect("content-language header should be present");

  assert_eq!(
    vec!["en-US", "fr", "zh-Hant-TW", "*"],
    content_language.tags()
  );
  assert_eq!(
    vec![&"en-US, fr".to_string(), &"zh-Hant-TW, *".to_string()],
    response.header_values("Content-Language")
  );
}

#[test]
fn test_parse_content_language_response_helper_returns_none_when_absent() {
  let s = concat!("HTTP/1.1 200 OK\r\n", "Content-Length: 2\r\n", "\r\n", "OK");
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect("parse response without content-language");

  assert_eq!(
    None,
    response
      .content_language()
      .expect("absent content-language should parse")
  );
}

#[test]
fn test_parse_content_language_rejects_invalid_helper_values_without_rejecting_response() {
  let invalid_values = [
    "",
    "en-US,",
    ",en-US",
    "en-US,,fr",
    "en-US, ,fr",
    "en_US",
    "en US",
    "en-",
    "-en",
    "englishlong",
    "en-toolongsubtag",
    "en-@",
  ];

  for value in invalid_values {
    let raw =
      format!("HTTP/1.1 200 OK\r\nContent-Language: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response remains usable");

    assert!(
      response.content_language().is_err(),
      "content-language helper should reject {value:?}"
    );
    assert_eq!(
      Some(&value.to_string()),
      response.header_value("Content-Language")
    );
  }
}

#[test]
fn test_parse_content_language_rejects_duplicate_oversized_and_too_many_values() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Content-Language: en-US, fr\r\n",
    "content-language: EN-us\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response with duplicate content-language remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject normalized duplicate tags"
  );
  assert_eq!(
    vec![&"en-US, fr".to_string(), &"EN-us".to_string()],
    response.header_values("Content-Language")
  );

  let oversized = "en".repeat(32 * 1024 + 1);
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Language: {oversized}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with oversized content-language remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject oversized values"
  );
  assert_eq!(Some(&oversized), response.header_value("Content-Language"));

  let too_many = (0..257)
    .map(|ix| format!("x-{ix}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw =
    format!("HTTP/1.1 200 OK\r\nContent-Language: {too_many}\r\nContent-Length: 2\r\n\r\nOK");
  let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response with too many content-language values remains usable");

  assert!(
    response.content_language().is_err(),
    "content-language helper should reject too many values"
  );
  assert_eq!(Some(&too_many), response.header_value("Content-Language"));
}

#[test]
fn test_parse_response_rejects_header_without_colon() {
  let s = concat!(
    "HTTP/1.1 200 OK\r\n",
    "BrokenHeader\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );

  let error = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec())
    .expect_err("malformed response header should be rejected");

  assert!(
    error.to_string().contains("Invalid response header"),
    "unexpected error: {error}"
  );
}

#[test]
fn test_parse_response_1() {
  let s = "HTTP/1.1 200 OK\r\n\
  Access-Control-Allow-Credentials: true\r\n\
  Access-Control-Allow-Origin: *\r\n\
  Content-Type: application/json\r\n\
  Date: Thu, 21 Nov 2019 02:23:24 GMT\r\n\
  Referrer-Policy: no-referrer-when-downgrade\r\n\
  Server: nginx\r\n\
  X-Content-Type-Options: nosniff\r\n\
  X-Frame-Options: DENY\r\n\
  X-XSS-Protection: 1; mode=block\r\n\
  Content-Length: 711\r\n\
  Connection: Close\r\n\
  \r\n\
  {
    \"args\": {
      \"id\": \"1\",
      \"name\": [
        \"jack\",
        \"Julia\"
      ]
    },
    \"data\": \"\",
    \"files\": {
      \"file\": \"[workspace]\\\\nmembers = [\\\\n  \\\"rttp_client\\\",\\\\n]\\\\n\"
    },
    \"form\": {
      \"debug\": \"true\",
      \"id\": \"1\",
      \"name\": [
        \"Chico\",
        \"\\u6587\",
        \"Form\"
      ],
      \"relation\": \"eq\"
    },
    \"headers\": {
      \"Content-Length\": \"863\",
      \"Content-Type\": \"multipart/form-data; boundary=---------------------------5jl1RuC429HeXVP2GOoO\",
      \"Cookie\": \"token=123234;uid=abcdef\",
      \"Host\": \"example.test\",
      \"User-Agent\": \"Mozilla/5.0\"
    },
    \"json\": null,
    \"origin\": \"222.69.134.133, 222.69.134.133\",
    \"url\": \"https://example.test/post?id=1&name=jack&name=Julia\"
  }";
  let response = Response::new(
    RoUrl::with("https://example.test/post"),
    s.as_bytes().to_vec(),
  );
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
fn test_non_chunked_response_exposes_empty_trailers() {
  let s = "HTTP/1.1 200 OK\r\n\
        Content-Length: 2\r\n\
        \r\n\
        OK";
  let response = Response::new(RoUrl::with("https://example.test"), s.as_bytes().to_vec());

  assert!(response.is_ok());
  let response = response.unwrap();
  assert!(response.trailers().is_empty());
  assert!(response.trailer("x-trace").is_none());
}

#[test]
fn test_no_body_status_responses_expose_empty_body_with_illegal_framing_bytes() {
  for raw in [
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    ),
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    ),
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Content-Length: 7\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "ignored"
    ),
    concat!(
      "HTTP/1.1 304 Not Modified\r\n",
      "Transfer-Encoding: chunked\r\n",
      "X-Trace: kept\r\n",
      "\r\n",
      "7\r\nignored\r\n0\r\n\r\n"
    ),
  ] {
    let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
      .expect("no-body status response should parse");

    assert_eq!(Some(&"kept".to_string()), response.header_value("X-Trace"));
    assert_eq!("", response.body().string().unwrap());
  }
}

#[test]
fn test_nel_response_helper_parses_typed_policy_metadata() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "NEL: {\"report_to\":\"network-errors\",\"max_age\":2592000,\"include_subdomains\":true,\"success_fraction\":0.1,\"failure_fraction\":1.0}\r\n",
    "Content-Length: 2\r\n\r\nOK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("raw response should remain usable");
  let nel = response
    .nel()
    .expect("valid NEL should parse")
    .expect("NEL should be present");

  assert_eq!(2592000, nel.max_age());
  assert_eq!(Some("network-errors"), nel.report_to());
  assert_eq!(Some(true), nel.include_subdomains());
  assert_eq!(Some(0.1), nel.success_fraction());
  assert_eq!(Some(1.0), nel.failure_fraction());
  assert_eq!(
    Some(&"{\"report_to\":\"network-errors\",\"max_age\":2592000,\"include_subdomains\":true,\"success_fraction\":0.1,\"failure_fraction\":1.0}".to_string()),
    response.header_value("NEL")
  );
}

#[test]
fn test_nel_response_helper_returns_none_when_absent() {
  let absent = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without NEL should parse");
  assert_eq!(None, absent.nel().expect("absent NEL should parse"));
}

#[test]
fn test_nel_rejects_malformed_duplicate_and_oversized_values_without_hiding_headers() {
  for value in [
    r#"{bad"#,
    r#"{"max_age":"1"}"#,
    r#"{"max_age":1,"max_age":2}"#,
    r#"{"success_fraction":1.5,"max_age":1}"#,
    r#"{"max_age":18446744073709551616}"#,
    r#"{"max_age":1} trailing"#,
    "",
  ] {
    let raw = format!("HTTP/1.1 200 OK\r\nNEL: {value}\r\nContent-Length: 2\r\n\r\nOK");
    let response = Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
      .expect("raw response should remain usable");
    assert!(response.nel().is_err(), "should reject {value:?}");
    assert_eq!(Some(&value.to_string()), response.header_value("NEL"));
    assert_eq!("OK", response.body().string().unwrap());
  }

  let duplicate = concat!(
    "HTTP/1.1 200 OK\r\n",
    "NEL: {\"max_age\":1}\r\n",
    "NEL: {\"max_age\":2}\r\n",
    "Content-Length: 0\r\n\r\n"
  );
  let duplicate_response = Response::new(
    RoUrl::with("https://example.test"),
    duplicate.as_bytes().to_vec(),
  )
  .expect("raw response should remain usable");
  assert!(
    duplicate_response.nel().is_err(),
    "duplicate NEL header fields must be rejected"
  );
  assert_eq!(
    2,
    duplicate_response.header_values("NEL").len(),
    "raw duplicate NEL headers must remain available"
  );

  let oversized = format!("{{\"max_age\":1{}}}", " ".repeat(64 * 1024));
  let oversized_raw = format!("HTTP/1.1 200 OK\r\nNEL: {oversized}\r\nContent-Length: 0\r\n\r\n");
  let oversized_response = Response::new(
    RoUrl::with("https://example.test"),
    oversized_raw.into_bytes(),
  )
  .expect("raw response should remain usable");
  assert!(oversized_response.nel().is_err());
  assert_eq!(Some(&oversized), oversized_response.header_value("NEL"));
}
