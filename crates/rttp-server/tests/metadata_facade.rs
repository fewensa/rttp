use rttp_server::server::{
  HttpAcceptCh, HttpAcceptCharset, HttpAcceptCharsetParseError, HttpAcceptLanguageParseError,
  HttpAcceptLanguages, HttpAccessControlAllowCredentials,
  HttpAccessControlAllowCredentialsParseError, HttpAccessControlAllowHeaders,
  HttpAccessControlAllowMethods, HttpAccessControlRequestHeaders,
  HttpAccessControlRequestHeadersParseError, HttpAccessControlRequestMethod,
  HttpAccessControlRequestMethodParseError, HttpAccessControlRequestPrivateNetwork,
  HttpAccessControlRequestPrivateNetworkParseError, HttpAltUsed, HttpAltUsedParseError,
  HttpAuthorization, HttpAuthorizationParseError, HttpBaggage, HttpBaggageMember,
  HttpBaggageParseError, HttpBaggageProperty, HttpCacheStatus, HttpCacheStatusParseError,
  HttpCdnCacheControl, HttpConditionalMetadata, HttpConnection, HttpConnectionParseError,
  HttpContentDisposition, HttpContentDispositionParseError, HttpContentDpr,
  HttpContentDprParseError, HttpContentLength, HttpContentLocation, HttpContentLocationParseError,
  HttpContentRange, HttpContentRangeParseError, HttpContentSecurityPolicyReportOnly,
  HttpContentSecurityPolicyReportOnlyParseError, HttpCrossOriginEmbedderPolicyReportOnly,
  HttpCrossOriginOpenerPolicy, HttpCrossOriginOpenerPolicyReportOnly,
  HttpCrossOriginResourcePolicy, HttpDeprecation, HttpDeprecationParseError, HttpEntityTag,
  HttpExpectParseError, HttpExpectations, HttpHost, HttpIdempotencyKey,
  HttpIdempotencyKeyParseError, HttpIfModifiedSince, HttpIfModifiedSinceParseError,
  HttpIfUnmodifiedSince, HttpIfUnmodifiedSinceParseError, HttpKeepAlive, HttpMaxForwards,
  HttpMaxForwardsParseError, HttpMementoDatetime, HttpMementoDatetimeParseError, HttpNoVarySearch,
  HttpNoVarySearchParams, HttpOriginTrialParseError, HttpOriginTrials, HttpPermissionsPolicy,
  HttpPermissionsPolicyAllowlist, HttpPermissionsPolicyAllowlistMember,
  HttpPermissionsPolicyDirective, HttpPermissionsPolicyParseError, HttpPragma, HttpPragmaDirective,
  HttpPragmaParseError, HttpPreferenceKind, HttpProxyAuthorization, HttpProxyStatus,
  HttpProxyStatusParseError, HttpRequest, HttpRequestAcceptCharsets, HttpRequestAcceptEncodings,
  HttpResponse, HttpSaveData, HttpSaveDataParseError, HttpSecGpc, HttpSecGpcParseError,
  HttpServiceWorkerAllowed, HttpServiceWorkerAllowedParseError, HttpSignature, HttpSignatureInput,
  HttpSignatureInputBareItem, HttpSignatureInputComponent, HttpSignatureInputEntry,
  HttpSignatureInputParameter, HttpSignatureInputParseError, HttpSignatureParseError,
  HttpSpeculationRules, HttpSpeculationRulesParseError, HttpSupportsLoadingMode,
  HttpSupportsLoadingModeParseError, HttpTraceParent, HttpTraceParentParseError, HttpTraceState,
  HttpTraceStateMember, HttpTraceStateParseError, HttpTransferEncoding,
  HttpTransferEncodingParseError, HttpUpgrade, HttpUpgradeInsecureRequests,
  HttpUpgradeInsecureRequestsParseError, HttpUpgradeParseError, HttpWantContentDigest,
  HttpWantReprDigest, SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};

#[test]
fn server_facade_exports_representative_bounded_metadata_types() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let accept_charsets: HttpRequestAcceptCharsets =
    HttpRequestAcceptCharsets::parse("utf-8, iso-8859-1;q=0.5, *;q=0")
      .expect("Accept-Charset should parse");
  let _: HttpAcceptCharsetParseError = HttpRequestAcceptCharsets::parse("utf-8, UTF-8")
    .expect_err("duplicate Accept-Charset should be rejected");
  let accept_languages: HttpAcceptLanguages =
    HttpAcceptLanguages::parse("en-US, fr-CA; q=0.8").expect("Accept-Language should parse");
  let _: HttpAcceptLanguageParseError = HttpAcceptLanguages::parse("en; q=1.001")
    .expect_err("malformed Accept-Language should be rejected");
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
  let alt_used: HttpAltUsed =
    HttpAltUsed::parse("[2001:db8::1]:8443").expect("Alt-Used should parse");
  let _: HttpAltUsedParseError =
    HttpAltUsed::parse("https://alt.example").expect_err("invalid Alt-Used should be rejected");
  let origin_trials: HttpOriginTrials =
    HttpOriginTrials::parse_values(["token-one", "token-two"]).expect("Origin-Trial should parse");
  let _: HttpOriginTrialParseError = HttpOriginTrials::parse("token\r\nX-Injected: 1")
    .expect_err("injected Origin-Trial should be rejected");
  let speculation_rules: HttpSpeculationRules =
    HttpSpeculationRules::parse("https://example.test/speculation-rules.json")
      .expect("Speculation-Rules should parse");
  let _: HttpSpeculationRulesParseError =
    HttpSpeculationRules::parse("https://example.test/rules.json\r\nX-Injected: 1")
      .expect_err("injected Speculation-Rules should be rejected");
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
  let sec_gpc: HttpSecGpc = HttpSecGpc::parse("1").expect("Sec-GPC should parse");
  let sec_gpc_error: Result<HttpSecGpc, HttpSecGpcParseError> = HttpSecGpc::parse("0");
  let upgrade_insecure_requests: HttpUpgradeInsecureRequests =
    HttpUpgradeInsecureRequests::parse("1").expect("Upgrade-Insecure-Requests should parse");
  let upgrade_insecure_requests_error: Result<
    HttpUpgradeInsecureRequests,
    HttpUpgradeInsecureRequestsParseError,
  > = HttpUpgradeInsecureRequests::parse("0");
  let authorization: HttpAuthorization =
    HttpAuthorization::parse("Bearer origin-token").expect("Authorization should parse");
  let authorization_error: Result<HttpAuthorization, HttpAuthorizationParseError> =
    HttpAuthorization::parse("Bearer \rsecret");
  let proxy_authorization: HttpProxyAuthorization =
    HttpProxyAuthorization::parse("Basic cHJveHk6c2VjcmV0")
      .expect("Proxy-Authorization should parse");
  let max_forwards: HttpMaxForwards =
    HttpMaxForwards::parse("0").expect("Max-Forwards should parse");
  let max_forwards_error: Result<HttpMaxForwards, HttpMaxForwardsParseError> =
    HttpMaxForwards::parse("4294967296");
  let expectations: HttpExpectations =
    HttpExpectations::parse("100-continue, preview").expect("Expect should parse");
  let expectations_error: Result<HttpExpectations, HttpExpectParseError> =
    HttpExpectations::parse("100-continue, 100-CONTINUE");
  let if_modified_since: HttpIfModifiedSince =
    HttpIfModifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("If-Modified-Since should parse");
  let if_modified_since_error: Result<HttpIfModifiedSince, HttpIfModifiedSinceParseError> =
    HttpIfModifiedSince::parse("not-a-date");
  let if_unmodified_since: HttpIfUnmodifiedSince =
    HttpIfUnmodifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("If-Unmodified-Since should parse");
  let if_unmodified_since_error: Result<HttpIfUnmodifiedSince, HttpIfUnmodifiedSinceParseError> =
    HttpIfUnmodifiedSince::parse("not-a-date");
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
  let opener_policy_report_only: HttpCrossOriginOpenerPolicyReportOnly =
    HttpCrossOriginOpenerPolicyReportOnly::parse(r#"same-origin; report-to="coop""#)
      .expect("Cross-Origin-Opener-Policy-Report-Only should parse");
  let content_security_policy_report_only: HttpContentSecurityPolicyReportOnly =
    HttpContentSecurityPolicyReportOnly::parse("default-src 'self'; report-to csp-endpoint")
      .expect("Content-Security-Policy-Report-Only should parse");
  let _: HttpContentSecurityPolicyReportOnlyParseError =
    HttpContentSecurityPolicyReportOnly::parse("")
      .expect_err("empty Content-Security-Policy-Report-Only should be rejected");
  let signature_input: HttpSignatureInput =
    HttpSignatureInput::parse(r#"sig1=("@method");created=1700000000"#)
      .expect("Signature-Input should parse");
  let signature_input_error: Result<HttpSignatureInput, HttpSignatureInputParseError> =
    HttpSignatureInput::parse("");
  let content_location = HttpContentLocation::parse("../representations/current.json")
    .expect("Content-Location should parse");
  let _: HttpContentLocationParseError = HttpContentLocation::parse("not valid")
    .expect_err("invalid Content-Location should be rejected");
  let service_worker_allowed =
    HttpServiceWorkerAllowed::parse("/").expect("Service-Worker-Allowed should parse");
  let _: HttpServiceWorkerAllowedParseError =
    HttpServiceWorkerAllowed::parse("http://example.test/scope")
      .expect_err("absolute URI Service-Worker-Allowed should be rejected");
  let content_disposition = HttpContentDisposition::parse(
    "attachment; filename=\"report.txt\"; filename*=UTF-8''report.txt",
  )
  .expect("Content-Disposition should parse");
  let _: HttpContentDispositionParseError = HttpContentDisposition::parse("attachment;")
    .expect_err("invalid Content-Disposition should be rejected");
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
  let memento_datetime = HttpMementoDatetime::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("Memento-Datetime should parse");
  let _: HttpMementoDatetimeParseError =
    HttpMementoDatetime::parse("").expect_err("empty Memento-Datetime should be rejected");
  let memento_response = HttpResponse::ok("")
    .with_memento_datetime(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777));
  let keep_alive_response = HttpResponse::ok("")
    .with_keep_alive("timeout=5, max=100")
    .expect("Keep-Alive should be accepted");
  let proxy_status: HttpProxyStatus =
    HttpProxyStatus::parse("ExampleCDN; error=connection_timeout")
      .expect("Proxy-Status should parse");
  let _: HttpProxyStatusParseError =
    HttpProxyStatus::parse("").expect_err("empty Proxy-Status should be rejected");
  let proxy_status_response = HttpResponse::ok("")
    .with_proxy_status("ExampleCDN; error=connection_timeout")
    .expect("Proxy-Status should be accepted");
  let permissions_policy: HttpPermissionsPolicy =
    HttpPermissionsPolicy::parse(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
      .expect("Permissions-Policy should parse");
  let _: HttpPermissionsPolicyParseError =
    HttpPermissionsPolicy::parse("geolocation=src").expect_err("src should be rejected");
  let permissions_policy_response = HttpResponse::ok("")
    .with_permissions_policy(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
    .expect("Permissions-Policy should be accepted");
  let supports_loading_mode: HttpSupportsLoadingMode =
    HttpSupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
      .expect("Supports-Loading-Mode should parse");
  let _: HttpSupportsLoadingModeParseError =
    HttpSupportsLoadingMode::parse("?1").expect_err("non-token should be rejected");
  let supports_loading_mode_response = HttpResponse::ok("")
    .with_supports_loading_mode(["fenced-frame", "credentialed-prerender"])
    .expect("Supports-Loading-Mode should be accepted");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let sec_purpose = SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let upgrade: HttpUpgrade = HttpUpgrade::parse("websocket").expect("Upgrade should parse");
  let _: HttpUpgradeParseError = HttpUpgrade::parse("").expect_err("empty Upgrade should fail");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  let first_charset: &HttpAcceptCharset = &accept_charsets.charsets()[0];
  assert_eq!(first_charset.charset(), "utf-8");
  assert_eq!(first_charset.quality(), 1000);
  assert_eq!(accept_charsets.charsets()[1].charset(), "iso-8859-1");
  assert_eq!(accept_charsets.charsets()[1].quality(), 500);
  assert_eq!(
    accept_charsets.header_value(),
    "utf-8, iso-8859-1;q=0.5, *;q=0"
  );
  assert_eq!(accept_languages.ranges(), ["en-US", "fr-CA"]);
  assert_eq!(accept_languages.qualities(), [None, Some("0.8")]);
  assert_eq!(accept_languages.header_value(), "en-US, fr-CA; q=0.8");
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(allow_methods.methods(), ["GET"]);
  assert_eq!(allow_headers.field_names(), ["x-request-id"]);
  assert_eq!(alt_used.host(), "[2001:db8::1]");
  assert_eq!(alt_used.port(), Some("8443"));
  assert_eq!(origin_trials.tokens(), ["token-one", "token-two"]);
  assert!(!format!("{origin_trials:?}").contains("token-one"));
  assert_eq!(
    speculation_rules.header_value(),
    "https://example.test/speculation-rules.json"
  );
  assert!(!format!("{speculation_rules:?}").contains("speculation-rules.json"));
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
  assert_eq!(sec_gpc.header_value(), "1");
  assert!(sec_gpc_error.is_err());
  assert_eq!(upgrade_insecure_requests.header_value(), "1");
  assert!(upgrade_insecure_requests_error.is_err());
  assert_eq!(authorization.scheme(), "Bearer");
  assert_eq!(authorization.header_value(), "Bearer origin-token");
  assert!(authorization_error.is_err());
  assert_eq!(proxy_authorization.scheme(), "Basic");
  assert_eq!(proxy_authorization.header_value(), "Basic cHJveHk6c2VjcmV0");
  assert_eq!(max_forwards.value(), 0);
  assert_eq!(max_forwards.header_value(), "0");
  assert!(max_forwards_error.is_err());
  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
  assert_eq!(expectations.header_value(), "100-continue, preview");
  assert!(expectations_error.is_err());
  assert_eq!(
    if_modified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert!(if_modified_since_error.is_err());
  assert_eq!(
    if_unmodified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert!(if_unmodified_since_error.is_err());
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(
    cache_status.members()[0].identifier().as_str(),
    "OriginCache"
  );
  assert_eq!(cache_status.members()[0].ttl(), Some(1100));
  assert_eq!(cdn_cache_control.directives()[1].name(), "cdn-example");
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
  assert_eq!(report_only_policy.header_value(), "require-corp");
  assert_eq!(
    HttpCrossOriginOpenerPolicy::SameOrigin,
    opener_policy_report_only.policy()
  );
  assert_eq!(Some("coop"), opener_policy_report_only.report_to());
  assert_eq!(
    opener_policy_report_only.header_value(),
    r#"same-origin; report-to="coop""#
  );
  assert_eq!(
    content_security_policy_report_only.header_value(),
    "default-src 'self'; report-to csp-endpoint"
  );
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
  assert_eq!(service_worker_allowed.header_value(), "/");
  assert_eq!(service_worker_allowed.as_str(), "/");
  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert_eq!(
    content_disposition.parameter("filename"),
    Some("report.txt")
  );
  assert_eq!(
    content_disposition.parameter("filename*"),
    Some("UTF-8''report.txt")
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
  assert_eq!(
    proxy_status.members()[0].identifier().as_str(),
    "ExampleCDN"
  );
  assert_eq!(
    "ExampleCDN",
    proxy_status_response
      .proxy_status()
      .expect("Proxy-Status should parse")
      .expect("Proxy-Status should be present")
      .members()[0]
      .identifier()
      .as_str()
  );
  let geolocation: &HttpPermissionsPolicyDirective =
    permissions_policy.directive("geolocation").unwrap();
  assert_eq!(geolocation.feature(), "geolocation");
  let geolocation_allowlist: &HttpPermissionsPolicyAllowlist = geolocation.allowlist();
  assert!(!geolocation_allowlist.is_all_origins());
  let first_member: &HttpPermissionsPolicyAllowlistMember = &geolocation_allowlist.members()[0];
  assert!(first_member.is_self());
  assert_eq!(
    permissions_policy.header_value(),
    r#"geolocation=(self "https://maps.example.test"), camera=()"#
  );
  assert_eq!(
    r#"geolocation=(self "https://maps.example.test"), camera=()"#,
    permissions_policy_response
      .permissions_policy()
      .expect("Permissions-Policy should parse")
      .expect("Permissions-Policy should be present")
      .header_value()
  );
  assert_eq!(
    supports_loading_mode.tokens(),
    ["fenced-frame", "credentialed-prerender"]
  );
  assert!(supports_loading_mode.contains_fenced_frame());
  assert!(supports_loading_mode.contains_credentialed_prerender());
  assert_eq!(
    "fenced-frame, credentialed-prerender",
    supports_loading_mode_response
      .supports_loading_mode()
      .expect("Supports-Loading-Mode should parse")
      .expect("Supports-Loading-Mode should be present")
      .header_value()
  );
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
  assert_eq!(sec_purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(sec_purpose.contains_prefetch());
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert_eq!(
    memento_datetime.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    memento_response
      .memento_datetime()
      .expect("Memento-Datetime should parse")
      .expect("Memento-Datetime should be present")
      .header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
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
fn request_facade_parses_accept_charset_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Charset: utf-8, iso-8859-1;q=0.5, *;q=0\r\n\r\n",
  )
  .expect("request should parse");

  let charsets: HttpRequestAcceptCharsets = request
    .accept_charset()
    .expect("Accept-Charset should parse")
    .expect("Accept-Charset should be present");

  assert_eq!(charsets.charsets()[0].charset(), "utf-8");
  assert_eq!(charsets.charsets()[0].quality(), 1000);
  assert_eq!(charsets.charsets()[1].charset(), "iso-8859-1");
  assert_eq!(charsets.charsets()[1].quality(), 500);
  assert_eq!(charsets.charsets()[2].charset(), "*");
  assert_eq!(charsets.charsets()[2].quality(), 0);
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5, *;q=0");

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert!(absent
    .accept_charset()
    .expect("missing Accept-Charset should be accepted")
    .is_none());
}

#[test]
fn request_facade_rejects_malformed_accept_charset_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Charset: utf-8, UTF-8\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");

  assert_eq!(request.header("Accept-Charset"), Some("utf-8, UTF-8"));
  assert!(request.accept_charset().is_err());
}

#[test]
fn request_facade_parses_accept_encoding_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Encoding: gzip, br;q=0.8, identity;q=0\r\n\r\n",
  )
  .expect("request should parse");

  let encodings: HttpRequestAcceptEncodings = request
    .accept_encoding()
    .expect("Accept-Encoding should parse")
    .expect("Accept-Encoding should be present");

  assert_eq!(encodings.codings()[0].coding(), "gzip");
  assert_eq!(encodings.codings()[0].quality(), 1000);
  assert_eq!(encodings.codings()[1].coding(), "br");
  assert_eq!(encodings.codings()[1].quality(), 800);
  assert_eq!(encodings.codings()[2].coding(), "identity");
  assert_eq!(encodings.codings()[2].quality(), 0);
  assert_eq!(encodings.header_value(), "gzip, br;q=0.8, identity;q=0");
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
fn request_facade_parses_max_forwards_metadata() {
  let request = HttpRequest::parse(
    b"OPTIONS /diagnostics HTTP/1.1\r\nHost: example.test\r\nMax-Forwards: 0\r\n\r\n",
  )
  .expect("request should parse");

  let max_forwards: HttpMaxForwards = request
    .max_forwards()
    .expect("Max-Forwards should parse")
    .expect("Max-Forwards should be present");

  assert_eq!(0, max_forwards.value());
  assert_eq!("0", max_forwards.header_value());
}

#[test]
fn request_facade_parses_expect_metadata() {
  let request = HttpRequest::parse(
    b"POST /upload HTTP/1.1\r\nHost: example.test\r\nExpect: 100-continue\r\nExpect: preview=sha256; chunk=1\r\n\r\n",
  )
  .expect("request should parse");

  let expectations: HttpExpectations = request
    .expectations()
    .expect("Expect should parse")
    .expect("Expect should be present");

  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
  assert_eq!("100-continue, preview", expectations.header_value());

  let absent = HttpRequest::parse(b"POST /upload HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .expectations()
      .expect("absent Expect should be accepted")
  );

  let unsupported =
    HttpRequest::parse(b"POST /upload HTTP/1.1\r\nHost: example.test\r\nExpect: tea-time\r\n\r\n")
      .expect("request should parse");
  let unsupported = unsupported
    .expectations()
    .expect("unsupported Expect should parse")
    .expect("Expect should be present");
  assert!(!unsupported.expects_continue());
  assert_eq!(["tea-time"], unsupported.unsupported());

  let duplicate = HttpRequest::parse(
    b"POST /upload HTTP/1.1\r\nHost: example.test\r\nExpect: 100-continue\r\nExpect: 100-CONTINUE\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.expectations().is_err());

  let malformed = HttpRequest::parse(
    b"POST /upload HTTP/1.1\r\nHost: example.test\r\nExpect: not a token\r\n\r\n",
  )
  .expect("request should parse");
  assert!(malformed.expectations().is_err());

  assert!(HttpExpectations::parse("a".repeat(64 * 1024 + 1)).is_err());
}

#[test]
fn request_facade_parses_idempotency_key_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: charge-2026-08-19-9f3c\r\n\r\n",
  )
  .expect("request should parse");

  let idempotency_key: HttpIdempotencyKey = request
    .idempotency_key()
    .expect("Idempotency-Key should parse")
    .expect("Idempotency-Key should be present");

  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.as_str());
  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.header_value());
  assert!(!format!("{idempotency_key:?}").contains("charge-2026-08-19-9f3c"));

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .idempotency_key()
      .expect("missing Idempotency-Key should be accepted")
  );

  let malformed = HttpRequest::parse(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: key with space\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.idempotency_key().is_err());
  assert_eq!(Some("key with space"), malformed.header("Idempotency-Key"));

  let duplicate = HttpRequest::parse(
    b"POST /charges HTTP/1.1\r\nHost: example.test\r\nIdempotency-Key: first\r\nidempotency-key: second\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.idempotency_key().is_err());
  assert_eq!(Some("first"), duplicate.header("Idempotency-Key"));

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_error: Result<HttpIdempotencyKey, HttpIdempotencyKeyParseError> =
    HttpIdempotencyKey::parse(oversized.as_str());
  assert!(oversized_error.is_err());
  assert!(!format!("{:?}", oversized_error.as_ref().unwrap_err()).contains(&oversized[..16]));
}

#[test]
fn request_facade_parses_trace_context_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"GET /trace HTTP/1.1\r\nHost: example.test\r\ntraceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\ntracestate: rojo=00f067aa0ba902b7\r\n\r\n",
  )
  .expect("request should parse");

  let traceparent: HttpTraceParent = request
    .traceparent()
    .expect("traceparent should parse")
    .expect("traceparent should be present");
  let tracestate: HttpTraceState = request
    .tracestate()
    .expect("tracestate should parse")
    .expect("tracestate should be present");
  let member: &HttpTraceStateMember = &tracestate.members()[0];

  assert_eq!("00", traceparent.version());
  assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", traceparent.trace_id());
  assert_eq!("rojo", member.key());
  assert_eq!("00f067aa0ba902b7", member.value());

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.traceparent().expect("missing traceparent"));
  assert_eq!(None, absent.tracestate().expect("missing tracestate"));

  let malformed = HttpRequest::parse(
    b"GET /trace HTTP/1.1\r\nHost: example.test\r\ntraceparent: invalid\r\ntracestate: rojo=1,rojo=2\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.traceparent().is_err());
  assert!(malformed.tracestate().is_err());
  assert_eq!(Some("invalid"), malformed.header("traceparent"));
  assert_eq!(Some("rojo=1,rojo=2"), malformed.header("tracestate"));

  let traceparent_error: Result<HttpTraceParent, HttpTraceParentParseError> =
    HttpTraceParent::parse("invalid");
  let tracestate_error: Result<HttpTraceState, HttpTraceStateParseError> =
    HttpTraceState::parse("rojo=1,rojo=2");
  assert!(traceparent_error.is_err());
  assert!(tracestate_error.is_err());
}

#[test]
fn request_facade_parses_baggage_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"GET /baggage HTTP/1.1\r\nHost: example.test\r\nbaggage: tenant=acme;source=gateway,release=2026-08-19\r\n\r\n",
  )
  .expect("request should parse");

  let baggage: HttpBaggage = request
    .baggage()
    .expect("baggage should parse")
    .expect("baggage should be present");
  let member: &HttpBaggageMember = &baggage.members()[0];
  let property: &HttpBaggageProperty = &member.properties()[0];

  assert_eq!("tenant", member.key());
  assert_eq!("acme", member.value());
  assert_eq!("source", property.key());
  assert_eq!(Some("gateway"), property.value());
  assert_eq!("release", baggage.members()[1].key());

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.baggage().expect("missing baggage"));

  let malformed = HttpRequest::parse(
    b"GET /baggage HTTP/1.1\r\nHost: example.test\r\nbaggage: tenant=secret,tenant=other\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.baggage().is_err());
  assert_eq!(
    Some("tenant=secret,tenant=other"),
    malformed.header("baggage")
  );

  let baggage_error: Result<HttpBaggage, HttpBaggageParseError> =
    HttpBaggage::parse("tenant=1,tenant=2");
  assert!(baggage_error.is_err());
}

#[test]
fn request_facade_parses_conditional_http_date_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\nIf-Unmodified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\n\r\n",
  )
  .expect("request should parse");

  let if_modified_since: HttpIfModifiedSince = request
    .if_modified_since()
    .expect("If-Modified-Since should parse")
    .expect("If-Modified-Since should be present");
  let if_unmodified_since: HttpIfUnmodifiedSince = request
    .if_unmodified_since()
    .expect("If-Unmodified-Since should parse")
    .expect("If-Unmodified-Since should be present");

  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    if_modified_since.header_value()
  );
  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    if_unmodified_since.header_value()
  );

  let absent = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .if_modified_since()
      .expect("absent value should be valid")
  );
  assert_eq!(
    None,
    absent
      .if_unmodified_since()
      .expect("absent value should be valid")
  );

  let malformed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Modified-Since: not-a-date\r\nIf-Unmodified-Since: not-a-date\r\n\r\n",
  )
  .expect("request should parse");
  assert!(malformed.if_modified_since().is_err());
  assert!(malformed.if_unmodified_since().is_err());

  let duplicate = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nIf-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT\r\nIf-Modified-Since: Sun, 06 Nov 1994 08:49:38 GMT\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.if_modified_since().is_err());

  let oversized = "0".repeat(64 * 1024 + 1);
  assert!(HttpIfModifiedSince::parse(oversized.as_str()).is_err());
  assert!(HttpIfUnmodifiedSince::parse(oversized.as_str()).is_err());
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
fn request_facade_parses_accept_language_metadata() {
  let request = HttpRequest::parse(
    b"GET /localized HTTP/1.1\r\nHost: example.test\r\nAccept-Language: en-US, fr-CA; q=0.8\r\nAccept-Language: *;q=0\r\n\r\n",
  )
  .expect("request should parse");

  let languages = request
    .accept_language()
    .expect("Accept-Language should parse")
    .expect("Accept-Language should be present");

  assert_eq!(languages.ranges(), ["en-US", "fr-CA", "*"]);
  assert_eq!(languages.qualities(), [None, Some("0.8"), Some("0")]);

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert!(absent
    .accept_language()
    .expect("missing Accept-Language should be accepted")
    .is_none());
}

#[test]
fn request_facade_rejects_malformed_accept_language_metadata() {
  let request = HttpRequest::parse(
    b"GET /localized HTTP/1.1\r\nHost: example.test\r\nAccept-Language: en; q=1.001\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");

  assert_eq!(request.header("Accept-Language"), Some("en; q=1.001"));
  assert!(request.accept_language().is_err());
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

#[test]
fn request_facade_parses_pragma_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nPragma: no-cache\r\nPragma: community=private, example=\"quoted, value\"\r\n\r\n",
  )
  .expect("request should parse");

  let pragma = request
    .pragma()
    .expect("Pragma should parse")
    .expect("Pragma should be present");

  assert!(pragma.no_cache());
  assert_eq!(
    pragma.extensions().len(),
    2,
    "extensions must exclude the defined no-cache directive"
  );
  let extensions: Vec<&HttpPragmaDirective> = pragma.extensions();
  assert_eq!("community", extensions[0].name());
  assert_eq!(Some("private"), extensions[0].value());
  assert_eq!("example", extensions[1].name());
  assert_eq!(Some("quoted, value"), extensions[1].value());
  assert_eq!(
    "no-cache, community=private, example=\"quoted, value\"",
    pragma.header_value()
  );

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent.pragma().expect("missing Pragma should be accepted")
  );

  let malformed =
    HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\nPragma: no-cache,\r\n\r\n")
      .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.pragma().is_err());
  assert_eq!(Some("no-cache,"), malformed.header("Pragma"));

  let duplicate = HttpRequest::parse(
    b"GET / HTTP/1.1\r\nHost: example.test\r\nPragma: no-cache\r\npragma: no-cache\r\n\r\n",
  )
  .expect("request should retain duplicate metadata");
  assert!(duplicate.pragma().is_err());
  assert_eq!(Some("no-cache"), duplicate.header("Pragma"));

  let oversized_value = "x".repeat(64 * 1024 + 1);
  let oversized_error: Result<HttpPragma, HttpPragmaParseError> =
    HttpPragma::parse(oversized_value.as_str());
  assert!(oversized_error.is_err());
  assert!(!format!("{:?}", oversized_error.as_ref().unwrap_err()).contains(&oversized_value[..16]));

  let first = "a".repeat(32 * 1024);
  let second = "b".repeat(32 * 1024);
  let combined_error = HttpPragma::parse_values([first.as_str(), second.as_str()]);
  assert!(combined_error.is_err());
  assert!(combined_error
    .unwrap_err()
    .to_string()
    .contains("too large"));
}

#[test]
fn response_facade_builds_and_parses_pragma_metadata() {
  let response = HttpResponse::new(200, "OK")
    .header("Pragma", "raw")
    .with_pragma("no-cache, community=private")
    .expect("Pragma should be accepted");

  let pragma = response
    .pragma()
    .expect("Pragma should parse")
    .expect("Pragma should be present");
  assert!(pragma.no_cache());
  assert_eq!("no-cache, community=private", pragma.header_value());
  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains("\r\nPragma: no-cache, community=private\r\n"));
  assert_eq!(1, serialized.matches("\r\nPragma: ").count());
  assert!(!serialized.contains("\r\nPragma: raw\r\n"));

  assert!(HttpResponse::ok("").with_pragma("no-cache=value").is_err());
  let unchanged = HttpResponse::ok("").header("Pragma", "legacy");
  assert!(unchanged.clone().with_pragma("no-cache,").is_err());
  let mut unchanged_serialized = Vec::new();
  unchanged
    .write_to(&mut unchanged_serialized)
    .expect("response writes");
  let unchanged_serialized = String::from_utf8(unchanged_serialized).expect("response is utf8");
  assert!(
    unchanged_serialized.contains("\r\nPragma: legacy\r\n"),
    "failed with_pragma must leave the response unchanged"
  );

  let absent = HttpResponse::ok("");
  assert!(absent
    .pragma()
    .expect("missing Pragma should be valid")
    .is_none());

  let first = "a".repeat(32 * 1024);
  let second = "b".repeat(32 * 1024);
  let combined = HttpResponse::ok("")
    .header("Pragma", first.as_str())
    .header("Pragma", second.as_str());
  assert!(combined.pragma().is_err());
  let mut serialized = Vec::new();
  combined.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains(&format!("\r\nPragma: {first}\r\n")));
  assert!(serialized.contains(&format!("\r\nPragma: {second}\r\n")));
}

#[test]
fn response_facade_builds_and_parses_origin_trial_metadata() {
  let response = HttpResponse::ok("body")
    .header("Origin-Trial", "stale-token")
    .header("origin-trial", "older-token")
    .with_origin_trials(["token-one", "token-one", "token-two"])
    .expect("valid Origin-Trial metadata should be accepted");
  let origin_trials: HttpOriginTrials = response
    .origin_trials()
    .expect("attached Origin-Trial should parse")
    .expect("Origin-Trial should be present");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(
    origin_trials.tokens(),
    ["token-one", "token-one", "token-two"]
  );
  assert_eq!(3, serialized.matches("\r\nOrigin-Trial: ").count());
  assert!(serialized.contains("\r\nOrigin-Trial: token-one\r\n"));
  assert!(serialized.contains("\r\nOrigin-Trial: token-two\r\n"));
  assert!(!serialized.contains("stale-token"));
  assert!(!format!("{origin_trials:?}").contains("token-one"));
  assert!(!format!("{response:?}").contains("token-one"));
  assert!(format!("{response:?}").contains("[REDACTED]"));

  let absent = HttpResponse::ok("body");
  assert_eq!(
    None,
    absent
      .origin_trials()
      .expect("missing Origin-Trial should be accepted")
  );

  let malformed = HttpResponse::ok("body").header("Origin-Trial", "token\twith-tab");
  assert!(malformed.origin_trials().is_err());
  assert!(String::from_utf8(malformed.to_bytes())
    .expect("response should serialize")
    .contains("\r\nOrigin-Trial: token\twith-tab\r\n"));

  let oversized = "x".repeat(8 * 1024 + 1);
  let raw = HttpResponse::ok("body").header("Origin-Trial", &oversized);
  assert!(raw.origin_trials().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains(&format!("\r\nOrigin-Trial: {oversized}\r\n")));
  assert!(HttpResponse::ok("body")
    .with_origin_trials(["token\r\nX-Injected: 1"])
    .is_err());
}

#[test]
fn response_facade_builds_and_parses_speculation_rules_metadata() {
  let value = "https://example.test/speculation-rules.json";
  let response = HttpResponse::ok("body")
    .header("Speculation-Rules", "https://example.test/stale.json")
    .with_speculation_rules(value)
    .expect("valid Speculation-Rules metadata should be accepted");
  let rules: HttpSpeculationRules = response
    .speculation_rules()
    .expect("attached Speculation-Rules should parse")
    .expect("Speculation-Rules should be present");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(rules.as_str(), value);
  assert_eq!(rules.header_value(), value);
  assert_eq!(1, serialized.matches("\r\nSpeculation-Rules: ").count());
  assert!(serialized.contains(&format!("\r\nSpeculation-Rules: {value}\r\n")));
  assert!(!serialized.contains("stale.json"));
  assert!(!format!("{rules:?}").contains(value));

  let absent = HttpResponse::ok("body");
  assert_eq!(
    None,
    absent
      .speculation_rules()
      .expect("missing Speculation-Rules should be accepted")
  );

  let duplicate = HttpResponse::ok("body")
    .header("Speculation-Rules", "https://example.test/one.json")
    .header("speculation-rules", "https://example.test/two.json");
  assert!(duplicate.speculation_rules().is_err());

  let malformed = HttpResponse::ok("body").header("Speculation-Rules", "");
  assert!(malformed.speculation_rules().is_err());
  assert!(HttpResponse::ok("body")
    .with_speculation_rules("https://example.test/rules.json\r\nX-Injected: 1")
    .is_err());
}
