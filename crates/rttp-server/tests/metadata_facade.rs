use rttp_server::server::{
  HttpAcceptCh, HttpAccessControlAllowCredentials, HttpAccessControlAllowCredentialsParseError,
  HttpAccessControlAllowHeaders, HttpAccessControlAllowMethods, HttpAccessControlRequestHeaders,
  HttpAccessControlRequestHeadersParseError, HttpAccessControlRequestMethod,
  HttpAccessControlRequestMethodParseError, HttpAccessControlRequestPrivateNetwork,
  HttpAccessControlRequestPrivateNetworkParseError, HttpCacheStatus, HttpCacheStatusParseError,
  HttpCdnCacheControl, HttpConditionalMetadata, HttpConnection, HttpConnectionParseError,
  HttpContentDisposition, HttpContentDpr, HttpContentDprParseError, HttpContentLength,
  HttpContentLocation, HttpContentLocationParseError, HttpContentRange, HttpContentRangeParseError,
  HttpCrossOriginEmbedderPolicyReportOnly, HttpCrossOriginResourcePolicy, HttpDeprecation,
  HttpDeprecationParseError, HttpEntityTag, HttpHost, HttpKeepAlive, HttpNoVarySearch,
  HttpNoVarySearchParams, HttpPreferenceKind, HttpRequest, HttpResponse, HttpSaveData,
  HttpSaveDataParseError, HttpSignature, HttpSignatureInput, HttpSignatureInputBareItem,
  HttpSignatureInputComponent, HttpSignatureInputEntry, HttpSignatureInputParameter,
  HttpSignatureInputParseError, HttpSignatureParseError, HttpTransferEncoding,
  HttpTransferEncodingParseError, HttpUpgrade, HttpUpgradeParseError, HttpWantContentDigest,
  HttpWantReprDigest, SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};

#[test]
fn server_facade_exports_representative_bounded_metadata_types() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let allow_credentials: HttpAccessControlAllowCredentials =
    HttpAccessControlAllowCredentials::parse("true")
      .expect("Access-Control-Allow-Credentials should parse");
  let _: Result<HttpAccessControlAllowCredentials, HttpAccessControlAllowCredentialsParseError> =
    HttpAccessControlAllowCredentials::parse("false");
  let allow_methods: HttpAccessControlAllowMethods =
    HttpAccessControlAllowMethods::parse("GET").expect("Access-Control-Allow-Methods should parse");
  let allow_headers: HttpAccessControlAllowHeaders =
    HttpAccessControlAllowHeaders::parse("X-Request-Id")
      .expect("Access-Control-Allow-Headers should parse");
  let request_method: HttpAccessControlRequestMethod =
    HttpAccessControlRequestMethod::parse("patch")
      .expect("Access-Control-Request-Method should parse");
  let request_method_error: Result<
    HttpAccessControlRequestMethod,
    HttpAccessControlRequestMethodParseError,
  > = HttpAccessControlRequestMethod::parse("GET, POST");
  let request_headers: HttpAccessControlRequestHeaders =
    HttpAccessControlRequestHeaders::parse("X-Request-Id, Authorization")
      .expect("Access-Control-Request-Headers should parse");
  let request_headers_error: Result<
    HttpAccessControlRequestHeaders,
    HttpAccessControlRequestHeadersParseError,
  > = HttpAccessControlRequestHeaders::parse("X-Request Id");
  let request_private_network: HttpAccessControlRequestPrivateNetwork =
    HttpAccessControlRequestPrivateNetwork::parse("true")
      .expect("Access-Control-Request-Private-Network should parse");
  let request_private_network_error: Result<
    HttpAccessControlRequestPrivateNetwork,
    HttpAccessControlRequestPrivateNetworkParseError,
  > = HttpAccessControlRequestPrivateNetwork::parse("false");
  let save_data: HttpSaveData = HttpSaveData::parse("on").expect("Save-Data should parse");
  let save_data_error: Result<HttpSaveData, HttpSaveDataParseError> = HttpSaveData::parse("?1");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let no_vary_search: HttpNoVarySearch =
    HttpNoVarySearch::parse(r#"params=("utm_source")"#).expect("No-Vary-Search should parse");
  let policy: HttpCrossOriginResourcePolicy = HttpCrossOriginResourcePolicy::parse("same-origin")
    .expect("Cross-Origin-Resource-Policy should parse");
  let cache_status: HttpCacheStatus =
    HttpCacheStatus::parse("OriginCache; hit; ttl=1100").expect("Cache-Status should parse");
  let _: HttpCacheStatusParseError = HttpCacheStatus::parse("OriginCache; hit=yes")
    .expect_err("invalid Cache-Status should be rejected");
  let cdn_cache_control: HttpCdnCacheControl =
    HttpCdnCacheControl::parse("max-age=600, cdn-example=\"a, b\"")
      .expect("CDN-Cache-Control should parse");
  let content_range = HttpContentRange::parse("bytes */10").expect("Content-Range should parse");
  let content_range_error: Result<HttpContentRange, HttpContentRangeParseError> =
    HttpContentRange::parse("bytes */*");
  let report_only_policy: HttpCrossOriginEmbedderPolicyReportOnly =
    HttpCrossOriginEmbedderPolicyReportOnly::parse("require-corp")
      .expect("Cross-Origin-Embedder-Policy-Report-Only should parse");
  let signature_input: HttpSignatureInput =
    HttpSignatureInput::parse(r#"sig1=("@method");created=1700000000"#)
      .expect("Signature-Input should parse");
  let signature_input_error: Result<HttpSignatureInput, HttpSignatureInputParseError> =
    HttpSignatureInput::parse("");
  let content_location = HttpContentLocation::parse("../representations/current.json")
    .expect("Content-Location should parse");
  let _: HttpContentLocationParseError = HttpContentLocation::parse("not valid")
    .expect_err("invalid Content-Location should be rejected");
  let content_dpr = HttpContentDpr::parse("1.5").expect("Content-DPR should parse");
  let _: HttpContentDprParseError =
    HttpContentDpr::parse("0").expect_err("zero Content-DPR should be rejected");
  let deprecation = HttpDeprecation::parse("?1").expect("Deprecation should parse");
  let _: HttpDeprecationParseError =
    HttpDeprecation::parse("true").expect_err("historical Deprecation token should be rejected");
  let response = HttpResponse::ok("")
    .with_etag(HttpEntityTag::weak("revision-42"))
    .with_deprecation(HttpDeprecation::Boolean(true))
    .with_accept_ch(["Sec-CH-UA"])
    .expect("Accept-CH should be accepted")
    .header("CDN-Cache-Control", "max-age=600, cdn-example=\"a, b\"");
  let keep_alive = HttpKeepAlive::parse("timeout=5, max=100").expect("Keep-Alive should parse");
  let keep_alive_response = HttpResponse::ok("")
    .with_keep_alive("timeout=5, max=100")
    .expect("Keep-Alive should be accepted");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let sec_purpose = SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let upgrade: HttpUpgrade = HttpUpgrade::parse("websocket").expect("Upgrade should parse");
  let _: HttpUpgradeParseError = HttpUpgrade::parse("").expect_err("empty Upgrade should fail");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(allow_methods.methods(), ["GET"]);
  assert_eq!(allow_headers.field_names(), ["x-request-id"]);
  assert_eq!("PATCH", request_method.method());
  assert!(request_method_error.is_err());
  assert_eq!(
    request_headers.field_names(),
    ["x-request-id", "authorization"]
  );
  assert!(request_headers_error.is_err());
  assert_eq!(request_private_network.header_value(), "true");
  assert!(request_private_network_error.is_err());
  assert_eq!(save_data.header_value(), "on");
  assert!(save_data_error.is_err());
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(
    cache_status.members()[0].identifier().as_str(),
    "OriginCache"
  );
  assert_eq!(cache_status.members()[0].ttl(), Some(1100));
  assert_eq!(cdn_cache_control.directives()[1].name(), "cdn-example");
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
  assert_eq!(report_only_policy.header_value(), "require-corp");
  assert_eq!(signature_input.members()[0].label(), "sig1");
  assert!(signature_input_error.is_err());
  assert_eq!(
    HttpContentRange::Unsatisfied {
      complete_length: 10,
    },
    content_range
  );
  assert!(content_range_error.is_err());
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(content_dpr.ratio(), 1.5);
  assert_eq!(content_dpr.header_value(), "1.5");
  assert_eq!(deprecation, HttpDeprecation::Boolean(true));
  assert_eq!(deprecation.header_value(), "?1");
  assert_eq!(
    response
      .deprecation()
      .expect("Deprecation should parse")
      .expect("Deprecation should be present"),
    HttpDeprecation::Boolean(true)
  );
  assert_eq!(
    metadata
      .entity_tag_value()
      .expect("entity tag should be retained")
      .opaque_tag(),
    "revision-42"
  );
  assert_eq!(
    response.etag().expect("ETag should parse"),
    Some(HttpEntityTag::weak("revision-42"))
  );
  assert_eq!(
    no_vary_search.params(),
    Some(&HttpNoVarySearchParams::Names(
      vec!["utm_source".to_owned()]
    ))
  );
  assert_eq!(
    response
      .accept_ch()
      .expect("Accept-CH should parse")
      .expect("Accept-CH should be present")
      .client_hints(),
    ["Sec-CH-UA"]
  );
  assert_eq!(
    response
      .cdn_cache_control()
      .expect("CDN-Cache-Control should parse")
      .expect("CDN-Cache-Control should be present")
      .directives()[0]
      .value(),
    Some("600")
  );
  assert_eq!(Some(5), keep_alive.timeout());
  assert_eq!(Some(100), keep_alive.max());
  assert_eq!(
    Some(5),
    keep_alive_response
      .keep_alive()
      .expect("Keep-Alive should parse")
      .expect("Keep-Alive should be present")
      .timeout()
  );
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
  assert_eq!(sec_purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(sec_purpose.contains_prefetch());
  assert_eq!(upgrade.protocols(), ["websocket"]);
}

#[test]
fn response_facade_parses_cache_status_and_absent_metadata() {
  let response = HttpResponse::ok("")
    .header("Cache-Status", "OriginCache; hit; ttl=1100")
    .header("cache-status", r#""CDN Company Here"; hit; ttl=545"#);

  let metadata = response
    .cache_status()
    .expect("Cache-Status should parse")
    .expect("Cache-Status should be present");

  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.members()[0].identifier().as_str(), "OriginCache");
  assert_eq!(
    metadata.members()[1].identifier().as_str(),
    "CDN Company Here"
  );
  let malformed = HttpResponse::ok("").header("Cache-Status", "OriginCache; hit=yes");
  assert!(malformed.cache_status().is_err());
  let mut serialized = Vec::new();
  malformed
    .write_to(&mut serialized)
    .expect("malformed Cache-Status response still writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains("\r\nCache-Status: OriginCache; hit=yes\r\n"));

  let absent = HttpResponse::ok("");
  assert!(absent
    .cache_status()
    .expect("missing header should be valid")
    .is_none());
}

#[test]
fn parsed_http_request_exposes_sec_purpose_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"GET /prefetch HTTP/1.1\r\nHost: example.test\r\nSec-Purpose: prefetch, vendor-ext\r\n\r\n",
  )
  .expect("request should parse");
  let purpose = request
    .sec_purpose()
    .expect("Sec-Purpose should parse")
    .expect("Sec-Purpose should be present");

  assert_eq!(purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(purpose.contains_prefetch());

  let malformed = HttpRequest::parse(
    b"GET /prefetch HTTP/1.1\r\nHost: example.test\r\nSec-Purpose: prefetch,\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");

  assert_eq!(malformed.header("Sec-Purpose"), Some("prefetch,"));
  assert!(malformed.sec_purpose().is_err());
}

#[test]
fn response_facade_parses_cdn_cache_control_and_absent_metadata() {
  let response = HttpResponse::ok("")
    .header("CDN-Cache-Control", "max-age=600, cdn-example=\"a, b\"")
    .header("cdn-cache-control", "immutable");

  let metadata = response
    .cdn_cache_control()
    .expect("CDN-Cache-Control should parse")
    .expect("CDN-Cache-Control should be present");

  assert_eq!(metadata.len(), 3);
  assert_eq!(metadata.directives()[1].name(), "cdn-example");
  assert_eq!(metadata.directives()[1].value(), Some("a, b"));
  let malformed = HttpResponse::ok("").header("CDN-Cache-Control", "max-age=");
  assert!(malformed.cdn_cache_control().is_err());

  let absent = HttpResponse::ok("");
  assert!(absent
    .cdn_cache_control()
    .expect("missing header should be valid")
    .is_none());
}

#[test]
fn server_facade_parses_signature_input_without_signature_policy() {
  let request = HttpRequest::parse(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nSignature-Input: sig1=(\"@method\" \"@path\");created=1700000000\r\n\r\n",
  )
  .expect("request should parse");

  let request_metadata = request
    .signature_input()
    .expect("request Signature-Input should parse")
    .expect("request Signature-Input should be present");
  assert_eq!(
    request_metadata.members()[0].covered_components()[1].identifier(),
    "@path"
  );

  let response = HttpResponse::ok("")
    .with_signature_input(r#"sig1=("@status");keyid="test-key""#)
    .expect("Signature-Input should be accepted");
  let response_metadata = response
    .signature_input()
    .expect("response Signature-Input should parse")
    .expect("response Signature-Input should be present");
  assert_eq!(
    response_metadata.header_value(),
    r#"sig1=("@status");keyid="test-key""#
  );

  assert!(HttpResponse::ok("")
    .with_signature_input("sig1=(@status)")
    .is_err());
  assert_eq!(
    HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .expect("request should parse")
      .signature_input()
      .expect("absent Signature-Input should parse"),
    None
  );
}

#[test]
fn response_facade_parses_content_range_metadata() {
  let satisfied = HttpResponse::ok("").header("Content-Range", "bytes 3-6/10");
  let unsatisfied = HttpResponse::ok("").header("Content-Range", "bytes */10");
  let duplicate = HttpResponse::ok("")
    .header("Content-Range", "bytes 0-0/2")
    .header("Content-Range", "bytes 1-1/2");

  assert_eq!(
    Some(HttpContentRange::Bytes {
      start: 3,
      end: 6,
      complete_length: Some(10),
    }),
    satisfied
      .content_range()
      .expect("satisfied Content-Range should parse")
  );
  assert_eq!(
    Some(HttpContentRange::Unsatisfied {
      complete_length: 10,
    }),
    unsatisfied
      .content_range()
      .expect("unsatisfied Content-Range should parse")
  );
  assert!(duplicate.content_range().is_err());
}

#[test]
fn request_facade_exposes_validated_content_length_metadata() {
  let request = HttpRequest::parse(
    b"POST /upload HTTP/1.1\r\nHost: example.test\r\nContent-Length: 5\r\n\r\nhello",
  )
  .expect("request should parse");
  let content_length: HttpContentLength = request
    .content_length()
    .expect("validated fixed length should be present");

  assert_eq!(5, content_length.len());
  assert!(!content_length.is_zero());
  assert_eq!("5", content_length.header_value());
}

#[test]
fn request_facade_omits_content_length_metadata_when_header_is_absent() {
  let request = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");

  assert_eq!(None, request.content_length());
}

#[test]
fn request_facade_omits_content_length_metadata_for_chunked_framing() {
  let request = HttpRequest::parse(
    concat!(
      "POST /upload HTTP/1.1\r\n",
      "Host: example.test\r\n",
      "Transfer-Encoding: chunked\r\n",
      "\r\n",
      "5\r\nhello\r\n0\r\n\r\n"
    )
    .as_bytes(),
  )
  .expect("chunked request should parse");

  assert_eq!(None, request.content_length());
}

#[test]
fn request_facade_parses_structured_prefer_metadata() {
  let request = HttpRequest::parse(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nPrefer: handling=strict, vendor=enabled; trace=\"a b\"\r\n\r\n",
  )
  .expect("request should parse");

  let prefer = request
    .prefer()
    .expect("Prefer should parse")
    .expect("Prefer should be present");

  assert_eq!(prefer.preferences()[0].kind(), HttpPreferenceKind::Handling);
  assert_eq!(prefer.preferences()[1].parameters()[0].value(), Some("a b"));
}

#[test]
fn request_facade_parses_want_content_digest_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nWant-Content-Digest: sha-256=10, sha-512=3, unixsum=0\r\n\r\n",
  )
  .expect("request should parse");

  let digest: HttpWantContentDigest = request
    .want_content_digest()
    .expect("Want-Content-Digest should parse")
    .expect("Want-Content-Digest should be present");

  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].preference(), 3);
  assert_eq!(digest.entries()[2].algorithm(), "unixsum");
  assert_eq!(digest.entries()[2].preference(), 0);
}

#[test]
fn request_facade_parses_upgrade_metadata() {
  let request = HttpRequest::parse(
    b"GET /chat HTTP/1.1\r\nHost: example.test\r\nUpgrade: websocket\r\nUpgrade: HTTP/2.0, custom\r\n\r\n",
  )
  .expect("request should parse");

  let upgrade = request
    .upgrade()
    .expect("Upgrade should parse")
    .expect("Upgrade should be present");

  assert_eq!(upgrade.protocols(), ["websocket", "HTTP/2.0", "custom"]);
}

#[test]
fn response_facade_builds_and_parses_upgrade_metadata() {
  let response = HttpResponse::new(101, "Switching Protocols")
    .header("Upgrade", "raw")
    .with_upgrade(["websocket", "TLS/1.3"])
    .expect("Upgrade should be accepted");

  let upgrade = response
    .upgrade()
    .expect("Upgrade should parse")
    .expect("Upgrade should be present");

  assert_eq!(upgrade.protocols(), ["websocket", "TLS/1.3"]);
  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains("\r\nUpgrade: websocket, TLS/1.3\r\n"));
  assert!(!serialized.contains("\r\nUpgrade: raw\r\n"));
  assert!(!serialized.contains("\r\nContent-Length:"));
}

#[test]
fn request_facade_parses_host_authority() {
  let request = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test:8443\r\n\r\n")
    .expect("request should parse");

  let host: HttpHost = request
    .host()
    .expect("Host should parse")
    .expect("Host should be present");

  assert_eq!("example.test", host.host());
  assert_eq!(Some("8443"), host.port());
  assert_eq!("example.test:8443", host.header_value());
}

#[test]
fn request_facade_parses_want_repr_digest_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nWant-Repr-Digest: sha-256=10, sha-512=3, unixsum=0\r\n\r\n",
  )
  .expect("request should parse");

  let digest: HttpWantReprDigest = request
    .want_repr_digest()
    .expect("Want-Repr-Digest should parse")
    .expect("Want-Repr-Digest should be present");

  assert_eq!(digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(digest.entries()[0].preference(), 10);
  assert_eq!(digest.entries()[1].algorithm(), "sha-512");
  assert_eq!(digest.entries()[1].preference(), 3);
  assert_eq!(digest.entries()[2].algorithm(), "unixsum");
  assert_eq!(digest.entries()[2].preference(), 0);
}

#[test]
fn request_facade_parses_signature_metadata_pair() {
  let request = HttpRequest::parse(
    concat!(
      "POST /signed HTTP/1.1\r\n",
      "Host: example.test\r\n",
      r#"Signature-Input: sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#,
      "\r\n",
      "Signature: sig1=:YWJj:\r\n",
      "\r\n"
    )
    .as_bytes(),
  )
  .expect("request should parse");

  let signature: HttpSignature = request
    .signature()
    .expect("Signature should parse")
    .expect("Signature should be present");
  let signature_input: HttpSignatureInput = request
    .signature_input()
    .expect("Signature-Input should parse")
    .expect("Signature-Input should be present");
  let _: Result<HttpSignature, HttpSignatureParseError> = HttpSignature::parse("not-a-signature");
  let _: Result<HttpSignatureInput, HttpSignatureInputParseError> =
    HttpSignatureInput::parse("not-an-input");

  let entry: &HttpSignatureInputEntry = &signature_input.entries()[0];
  let _: &[HttpSignatureInputComponent] = entry.components();
  let _: &[HttpSignatureInputParameter] = entry.parameters();

  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#
  );
  assert!(matches!(
    entry
      .parameter("created")
      .map(HttpSignatureInputParameter::value),
    Some(HttpSignatureInputBareItem::Integer(1_618_884_473))
  ));
}

#[test]
fn request_facade_parses_connection_metadata() {
  let request = HttpRequest::parse(
    b"GET /download HTTP/1.1\r\nHost: files.example.test\r\nConnection: close\r\n\r\n",
  )
  .expect("request should parse");

  let connection: HttpConnection = request
    .connection()
    .expect("Connection should parse")
    .expect("Connection should be present");

  assert_eq!(connection.tokens(), ["close"]);
  assert_eq!(connection.header_value(), "close");
  assert_eq!(request.header("Connection"), Some("close"));
}

#[test]
fn request_facade_returns_none_when_connection_is_absent() {
  let request = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");

  assert!(request
    .connection()
    .expect("missing Connection should be accepted")
    .is_none());
}

#[test]
fn request_facade_rejects_malformed_connection_while_preserving_raw_header() {
  let request =
    HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\nConnection: close,\r\n\r\n")
      .expect("malformed Connection should not reject the request frame");

  assert!(request.connection().is_err());
  assert_eq!(request.header("Connection"), Some("close,"));
}

#[test]
fn request_facade_rejects_invalid_connection_values() {
  let _: HttpConnectionParseError =
    HttpConnection::parse("close; foo").expect_err("parameterized Connection should be rejected");
}

#[test]
fn response_facade_parses_attached_connection_metadata() {
  let response = HttpResponse::ok("").header("Connection", "keep-alive");
  let connection = response
    .connection()
    .expect("Connection should parse")
    .expect("Connection should be present");

  assert_eq!(connection.tokens(), ["keep-alive"]);
  assert_eq!(connection.header_value(), "keep-alive");
}

#[test]
fn request_facade_parses_transfer_encoding_from_validated_chunked_framing() {
  let request = HttpRequest::parse(
    b"POST /upload HTTP/1.1\r\nHost: example.test\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n",
  )
  .expect("chunked request framing should parse");

  let transfer_encoding: HttpTransferEncoding = request
    .transfer_encoding()
    .expect("Transfer-Encoding should parse")
    .expect("Transfer-Encoding should be present");

  assert_eq!(transfer_encoding.codings(), ["chunked"]);
  assert_eq!(transfer_encoding.header_value(), "chunked");
  assert_eq!(request.header("Transfer-Encoding"), Some("chunked"));
}

#[test]
fn request_facade_returns_none_when_transfer_encoding_is_absent() {
  let request = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");

  assert!(request
    .transfer_encoding()
    .expect("missing Transfer-Encoding should be accepted")
    .is_none());
}

#[test]
fn request_facade_rejects_non_sole_chunked_transfer_encoding_values() {
  let _: HttpTransferEncodingParseError = HttpTransferEncoding::parse("gzip, chunked")
    .expect_err("non-sole chunked Transfer-Encoding should be rejected");
}

#[test]
fn response_facade_round_trips_obs_text_content_disposition_parameter_value() {
  let disposition = HttpContentDisposition::parse("attachment; filename=\"é\"")
    .expect("obs-text Content-Disposition parameter should parse");

  assert_eq!(Some("é"), disposition.parameter("filename"));
  assert_eq!("attachment; filename=\"é\"", disposition.header_value());
}

#[test]
fn response_facade_round_trips_escaped_content_disposition_parameter_value() {
  let disposition = HttpContentDisposition::parse(r#"attachment; filename="a\"b\\c""#)
    .expect("escaped Content-Disposition parameter should parse");

  assert_eq!(Some(r#"a"b\c"#), disposition.parameter("filename"));
  assert_eq!(
    r#"attachment; filename="a\"b\\c""#,
    disposition.header_value()
  );
}

#[test]
fn response_content_dpr_helper_declares_and_parses_singleton_metadata() {
  let absent = HttpResponse::ok("body");
  assert_eq!(
    None,
    absent
      .content_dpr()
      .expect("absent Content-DPR should parse")
  );

  let response = HttpResponse::ok("body")
    .header("Content-DPR", "3")
    .with_content_dpr(" 2.0 ")
    .expect("valid Content-DPR should be accepted");
  let serialized = String::from_utf8(response.to_bytes()).expect("response is UTF-8");

  assert!(serialized.contains("\r\nContent-DPR: 2.0\r\n"));
  assert_eq!(1, serialized.matches("\r\nContent-DPR: ").count());
  assert_eq!(
    2.0,
    response
      .content_dpr()
      .expect("Content-DPR should parse")
      .expect("Content-DPR should be present")
      .ratio()
  );

  let attached = HttpResponse::ok("body").header("Content-DPR", "1.5");
  assert_eq!(
    "1.5",
    attached
      .content_dpr()
      .expect("attached Content-DPR should parse")
      .expect("Content-DPR should be present")
      .header_value()
  );
}

#[test]
fn content_dpr_helper_rejects_invalid_duplicate_and_oversized_values() {
  for value in ["0", "2.", ".5", "+1", "1e1", "1\u{7f}"] {
    assert!(
      HttpResponse::ok("body").with_content_dpr(value).is_err(),
      "Content-DPR helper should reject {value:?}"
    );
  }

  let duplicate = HttpResponse::ok("body")
    .header("Content-DPR", "1")
    .header("content-dpr", "2.0");
  assert!(
    duplicate.content_dpr().is_err(),
    "Content-DPR parser should reject duplicate header fields"
  );

  let oversized = "1".repeat(64 * 1024 + 1);
  assert!(
    HttpResponse::ok("body")
      .with_content_dpr(&oversized)
      .is_err(),
    "Content-DPR helper should reject oversized values"
  );
  let response = HttpResponse::ok("body").header("Content-DPR", oversized);
  assert!(
    response.content_dpr().is_err(),
    "Content-DPR parser should reject oversized raw values"
  );
}
