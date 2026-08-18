use rttp_server::server::{
  HttpAcceptCh, HttpAccessControlAllowHeaders, HttpAccessControlAllowMethods,
  HttpAccessControlRequestHeaders, HttpAccessControlRequestHeadersParseError,
  HttpAccessControlRequestMethod, HttpAccessControlRequestMethodParseError,
  HttpConditionalMetadata, HttpCrossOriginEmbedderPolicyReportOnly, HttpCrossOriginResourcePolicy,
  HttpEntityTag, HttpHost, HttpPreferenceKind, HttpRequest, HttpResponse, HttpTransferEncoding,
  HttpTransferEncodingParseError, HttpWantReprDigest, SecFetchDest, SecFetchMode, SecFetchSite,
  SecFetchUser,
};

#[test]
fn server_facade_exports_representative_bounded_metadata_types() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
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
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let policy: HttpCrossOriginResourcePolicy = HttpCrossOriginResourcePolicy::parse("same-origin")
    .expect("Cross-Origin-Resource-Policy should parse");
  let report_only_policy: HttpCrossOriginEmbedderPolicyReportOnly =
    HttpCrossOriginEmbedderPolicyReportOnly::parse("require-corp")
      .expect("Cross-Origin-Embedder-Policy-Report-Only should parse");
  let response = HttpResponse::ok("")
    .with_accept_ch(["Sec-CH-UA"])
    .expect("Accept-CH should be accepted");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(allow_methods.methods(), ["GET"]);
  assert_eq!(allow_headers.field_names(), ["x-request-id"]);
  assert_eq!("PATCH", request_method.method());
  assert!(request_method_error.is_err());
  assert_eq!(
    request_headers.field_names(),
    ["x-request-id", "authorization"]
  );
  assert!(request_headers_error.is_err());
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(report_only_policy.header_value(), "require-corp");
  assert_eq!(
    metadata
      .entity_tag_value()
      .expect("entity tag should be retained")
      .opaque_tag(),
    "revision-42"
  );
  assert_eq!(
    response
      .accept_ch()
      .expect("Accept-CH should parse")
      .expect("Accept-CH should be present")
      .client_hints(),
    ["Sec-CH-UA"]
  );
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
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
