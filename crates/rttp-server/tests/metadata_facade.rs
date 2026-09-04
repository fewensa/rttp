use rttp_server::server::{
  HttpAIm, HttpAImMember, HttpAImParameter, HttpAImParseError, HttpAccept, HttpAcceptCh,
  HttpAcceptCharset, HttpAcceptCharsetParseError, HttpAcceptDatetime, HttpAcceptDatetimeParseError,
  HttpAcceptLanguageParseError, HttpAcceptLanguages, HttpAcceptParseError,
  HttpAccessControlAllowCredentials, HttpAccessControlAllowCredentialsParseError,
  HttpAccessControlAllowHeaders, HttpAccessControlAllowMethods, HttpAccessControlRequestHeaders,
  HttpAccessControlRequestHeadersParseError, HttpAccessControlRequestMethod,
  HttpAccessControlRequestMethodParseError, HttpAccessControlRequestPrivateNetwork,
  HttpAccessControlRequestPrivateNetworkParseError, HttpAltUsed, HttpAltUsedParseError,
  HttpAuthorization, HttpAuthorizationParseError, HttpBaggage, HttpBaggageMember,
  HttpBaggageParseError, HttpBaggageProperty, HttpCacheStatus, HttpCacheStatusParseError,
  HttpCdnCacheControl, HttpCdnLoop, HttpCdnLoopMember, HttpCdnLoopParseError,
  HttpConditionalMetadata, HttpConnection, HttpConnectionParseError, HttpContentDisposition,
  HttpContentDispositionParseError, HttpContentDpr, HttpContentDprParseError, HttpContentLength,
  HttpContentLocation, HttpContentLocationParseError, HttpContentRange, HttpContentRangeParseError,
  HttpContentSecurityPolicyReportOnly, HttpContentSecurityPolicyReportOnlyParseError,
  HttpCookieParseError, HttpCrossOriginEmbedderPolicyReportOnly, HttpCrossOriginOpenerPolicy,
  HttpCrossOriginOpenerPolicyReportOnly, HttpCrossOriginResourcePolicy, HttpDeltaBase,
  HttpDeltaBaseParseError, HttpDeprecation, HttpDeprecationParseError, HttpDepth,
  HttpDepthParseError, HttpDnt, HttpDntParseError, HttpDocumentPolicy, HttpDocumentPolicyDirective,
  HttpDocumentPolicyParseError, HttpDocumentPolicyReportOnly,
  HttpDocumentPolicyReportOnlyParseError, HttpDocumentPolicyReportOnlyValue,
  HttpDocumentPolicyValue, HttpEntityTag, HttpExpectParseError, HttpExpectations,
  HttpExpiresParseError, HttpFrom, HttpFromParseError, HttpHost, HttpIdempotencyKey,
  HttpIdempotencyKeyParseError, HttpIf, HttpIfCondition, HttpIfList, HttpIfModifiedSince,
  HttpIfModifiedSinceParseError, HttpIfParseError, HttpIfPredicate, HttpIfResourceTag,
  HttpIfScheduleTagMatch, HttpIfScheduleTagMatchParseError, HttpIfStateToken,
  HttpIfUnmodifiedSince, HttpIfUnmodifiedSinceParseError, HttpIm, HttpImMember, HttpImParameter,
  HttpImParseError, HttpKeepAlive, HttpLockToken, HttpLockTokenParseError, HttpMaxForwards,
  HttpMaxForwardsParseError, HttpMementoDatetime, HttpMementoDatetimeParseError, HttpNegotiate,
  HttpNegotiateDirective, HttpNegotiateParseError, HttpNoVarySearch, HttpNoVarySearchParams,
  HttpOriginTrialParseError, HttpOriginTrials, HttpOverwrite, HttpOverwriteParseError,
  HttpPermissionsPolicy, HttpPermissionsPolicyAllowlist, HttpPermissionsPolicyAllowlistMember,
  HttpPermissionsPolicyDirective, HttpPermissionsPolicyParseError, HttpPragma, HttpPragmaDirective,
  HttpPragmaParseError, HttpPreferenceKind, HttpProxyAuthorization, HttpProxyStatus,
  HttpProxyStatusParseError, HttpRateLimitLimit, HttpRateLimitLimitItem,
  HttpRateLimitLimitParseError, HttpRateLimitParseError, HttpRateLimitRemaining,
  HttpRateLimitRemainingParseError, HttpRateLimitReset, HttpRateLimitResetParseError, HttpReferer,
  HttpRefererParseError, HttpRequest, HttpRequestAcceptCharsets, HttpRequestAcceptEncodings,
  HttpResponse, HttpResponseDate, HttpResponseDateParseError, HttpResponseExpires,
  HttpResponseLastModified, HttpResponseLastModifiedParseError, HttpRetryAfter,
  HttpRetryAfterParseError, HttpSameSite, HttpSaveData, HttpSaveDataParseError, HttpScheduleTag,
  HttpSecGpc, HttpSecGpcParseError, HttpSecWebSocketAccept, HttpSecWebSocketAcceptParseError,
  HttpSecWebSocketExtensions, HttpSecWebSocketExtensionsParseError, HttpSecWebSocketKey,
  HttpSecWebSocketKeyParseError, HttpSecWebSocketProtocol, HttpSecWebSocketProtocolParseError,
  HttpSecWebSocketVersion, HttpSecWebSocketVersionParseError, HttpServiceWorkerAllowed,
  HttpServiceWorkerAllowedParseError, HttpSetCookie, HttpSetCookies, HttpSignature,
  HttpSignatureInput, HttpSignatureInputBareItem, HttpSignatureInputComponent,
  HttpSignatureInputEntry, HttpSignatureInputParameter, HttpSignatureInputParseError,
  HttpSignatureParseError, HttpSpeculationRules, HttpSpeculationRulesParseError,
  HttpSupportsLoadingMode, HttpSupportsLoadingModeParseError, HttpSurrogateControl,
  HttpSurrogateControlParseError, HttpTcn, HttpTcnDirective, HttpTcnParseError, HttpTimeout,
  HttpTimeoutParseError, HttpTimeoutType, HttpTraceParent, HttpTraceParentParseError,
  HttpTraceState, HttpTraceStateMember, HttpTraceStateParseError, HttpTransferEncoding,
  HttpTransferEncodingParseError, HttpUpgrade, HttpUpgradeInsecureRequests,
  HttpUpgradeInsecureRequestsParseError, HttpUpgradeParseError, HttpVariantVary,
  HttpVariantVaryParseError, HttpVia, HttpViaMember, HttpViaParseError, HttpWantContentDigest,
  HttpWantReprDigest, HttpXForwardedFor, HttpXForwardedForParseError, HttpXForwardedHost,
  HttpXForwardedHostParseError, HttpXForwardedProto, HttpXForwardedProtoParseError, SecFetchDest,
  SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};

#[test]
fn server_dav_response_metadata_uses_protocol_representation() {
  let response = HttpResponse::ok("")
    .header("DAV", "legacy")
    .with_dav("1, 2, extended-mkcol, <https://dav.example.test/ns>")
    .expect("valid DAV metadata should be accepted");
  let dav = response
    .dav()
    .expect("DAV metadata should parse")
    .expect("DAV metadata should be present");

  assert_eq!(
    "1, 2, extended-mkcol, <https://dav.example.test/ns>",
    dav.header_value()
  );
  let rendered = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert!(rendered.contains("\r\nDAV: 1, 2, extended-mkcol, <https://dav.example.test/ns>\r\n"));
  assert!(!rendered.contains("\r\nDAV: legacy\r\n"));

  let unchanged = HttpResponse::ok("").header("DAV", "1");
  assert!(unchanged.clone().with_dav("1, 1").is_err());
  assert_eq!(
    "1",
    unchanged
      .dav()
      .expect("original DAV should still parse")
      .expect("original DAV should be present")
      .header_value()
  );

  let oversized = format!("x{}", "a".repeat(64 * 1024));
  let invalid = HttpResponse::ok("").header("DAV", oversized);
  assert!(invalid.dav().is_err());
}

#[test]
fn server_facade_exports_representative_bounded_metadata_types() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let accept: HttpAccept =
    HttpAccept::parse("text/html; level=1; q=0.8").expect("Accept should parse");
  let _: HttpAcceptParseError =
    HttpAccept::parse("*/json").expect_err("invalid Accept should fail");
  let a_im: HttpAIm =
    HttpAIm::parse("diffe, gzip;q=0.3;profile=compact").expect("A-IM should parse");
  let _: HttpAImParseError =
    HttpAIm::parse("diffe, DIFFE").expect_err("duplicate A-IM should be rejected");
  let _: &[HttpAImMember] = a_im.members();
  let _: Option<&HttpAImParameter> = a_im.members()[1].parameters().first();
  let im: HttpIm = HttpIm::parse("diffe, gzip;profile=compact").expect("IM should parse");
  let _: HttpImParseError =
    HttpIm::parse("diffe, DIFFE").expect_err("duplicate IM should be rejected");
  let _: &[HttpImMember] = im.members();
  let _: Option<&HttpImParameter> = im.members()[1].parameters().first();
  let negotiate: HttpNegotiate =
    HttpNegotiate::parse("trans, 1.0, feature-x=preview, *").expect("Negotiate should parse");
  let _: HttpNegotiateParseError =
    HttpNegotiate::parse("trans, TRANS").expect_err("duplicate Negotiate should be rejected");
  let _: &[HttpNegotiateDirective] = negotiate.members();
  let tcn: HttpTcn = HttpTcn::parse("list, choice").expect("TCN should parse");
  let _: HttpTcnParseError =
    HttpTcn::parse("list, LIST").expect_err("duplicate TCN should be rejected");
  let _: &[HttpTcnDirective] = tcn.members();
  let set_cookie: HttpSetCookie =
    HttpSetCookie::parse(r#"session="abc def"; Path=/; SameSite=Lax; Foo=bar"#)
      .expect("Set-Cookie should parse");
  let _: HttpSameSite = set_cookie.same_site().expect("SameSite should parse");
  let _: HttpSetCookies = HttpSetCookies::parse_values([set_cookie.header_value().as_str()])
    .expect("Set-Cookie collection should parse");
  let _: HttpCookieParseError = HttpSetCookie::parse("session=abc; Path=/; path=/other")
    .expect_err("duplicate Set-Cookie attributes should be rejected");
  let variant_vary: HttpVariantVary =
    HttpVariantVary::parse("Accept-Language, Sec-CH-DPR").expect("Variant-Vary should parse");
  let _: HttpVariantVaryParseError = HttpVariantVary::parse("Accept-Language, accept-language")
    .expect_err("duplicate Variant-Vary should be rejected");
  let _: HttpVariantVaryParseError = HttpVariantVary::parse("a".repeat(64 * 1024 + 1))
    .expect_err("oversized Variant-Vary should be rejected");
  assert_eq!(
    vec!["accept-language", "sec-ch-dpr"],
    variant_vary.field_names()
  );
  assert_eq!("accept-language, sec-ch-dpr", variant_vary.header_value());
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
  let dnt: HttpDnt = HttpDnt::parse("1").expect("DNT should parse");
  let dnt_error: Result<HttpDnt, HttpDntParseError> = HttpDnt::parse("on");
  let referer: HttpReferer =
    HttpReferer::parse("https://shop.example/checkout?step=pay").expect("Referer should parse");
  let referer_error: Result<HttpReferer, HttpRefererParseError> =
    HttpReferer::parse("https://example.test/path#frag");
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
  let depth: HttpDepth = HttpDepth::parse("infinity").expect("Depth should parse");
  let depth_error: Result<HttpDepth, HttpDepthParseError> = HttpDepth::parse("2");
  let lock_token: HttpLockToken =
    HttpLockToken::parse("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
      .expect("Lock-Token should parse");
  let lock_token_error: Result<HttpLockToken, HttpLockTokenParseError> =
    HttpLockToken::parse("<relative>");
  let timeout: HttpTimeout =
    HttpTimeout::parse("Second-60, Infinite").expect("Timeout should parse");
  let timeout_error: Result<HttpTimeout, HttpTimeoutParseError> =
    HttpTimeout::parse("Second-60, second-60");
  let overwrite: HttpOverwrite = HttpOverwrite::parse("F").expect("Overwrite should parse");
  let overwrite_error: Result<HttpOverwrite, HttpOverwriteParseError> = HttpOverwrite::parse("t");
  let expectations: HttpExpectations =
    HttpExpectations::parse("100-continue, preview").expect("Expect should parse");
  let expectations_error: Result<HttpExpectations, HttpExpectParseError> =
    HttpExpectations::parse("100-continue, 100-CONTINUE");
  let if_modified_since: HttpIfModifiedSince =
    HttpIfModifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("If-Modified-Since should parse");
  let if_modified_since_error: Result<HttpIfModifiedSince, HttpIfModifiedSinceParseError> =
    HttpIfModifiedSince::parse("not-a-date");
  let if_schedule_tag_match: HttpIfScheduleTagMatch =
    HttpIfScheduleTagMatch::parse("\"sched-17\"").expect("If-Schedule-Tag-Match should parse");
  let if_schedule_tag_match_weak: HttpIfScheduleTagMatch =
    HttpIfScheduleTagMatch::parse("W/\"sched-17\"")
      .expect("weak If-Schedule-Tag-Match should parse");
  let if_schedule_tag_match_error: Result<
    HttpIfScheduleTagMatch,
    HttpIfScheduleTagMatchParseError,
  > = HttpIfScheduleTagMatch::parse("*");
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
  let surrogate_control: HttpSurrogateControl =
    HttpSurrogateControl::parse("max-age=600, content=\"ESI/1.0\"")
      .expect("Surrogate-Control should parse");
  let _: HttpSurrogateControlParseError = HttpSurrogateControl::parse("max-age=60, Max-Age=120")
    .expect_err("duplicate Surrogate-Control directive should be rejected");
  let cdn_loop: HttpCdnLoop =
    HttpCdnLoop::parse(r#"foo123.foocdn.example, barcdn.example; trace="abcdef""#)
      .expect("CDN-Loop should parse");
  let _: HttpCdnLoopParseError =
    HttpCdnLoop::parse("cdn; trace").expect_err("valueless CDN-Loop parameter should be rejected");
  let x_forwarded_for: HttpXForwardedFor =
    HttpXForwardedFor::parse("192.0.2.60, unknown").expect("X-Forwarded-For should parse");
  let _: HttpXForwardedForParseError =
    HttpXForwardedFor::parse("client.example").expect_err("invalid X-Forwarded-For should fail");
  let x_forwarded_host: HttpXForwardedHost =
    HttpXForwardedHost::parse("example.test:443").expect("X-Forwarded-Host should parse");
  let _: HttpXForwardedHostParseError = HttpXForwardedHost::parse("https://example.test")
    .expect_err("invalid X-Forwarded-Host should fail");
  let x_forwarded_proto: HttpXForwardedProto =
    HttpXForwardedProto::parse("https").expect("X-Forwarded-Proto should parse");
  let _: HttpXForwardedProtoParseError =
    HttpXForwardedProto::parse("https://").expect_err("invalid X-Forwarded-Proto should fail");
  let via: HttpVia =
    HttpVia::parse("1.1 edge-a (TLS terminator), HTTP/2 upstream").expect("Via should parse");
  let _: HttpViaParseError =
    HttpVia::parse("1.1").expect_err("incomplete Via hop should be rejected");
  let via_response = HttpResponse::ok("")
    .with_via("1.1 edge-a (TLS terminator), HTTP/2 upstream")
    .expect("Via should be accepted");
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
  let delta_base = HttpDeltaBase::parse("\"revision-42\"").expect("Delta-Base should parse");
  let _: HttpDeltaBaseParseError =
    HttpDeltaBase::parse("\"one\", \"two\"").expect_err("Delta-Base list should be rejected");
  let response = HttpResponse::ok("")
    .with_etag(HttpEntityTag::weak("revision-42"))
    .with_delta_base(delta_base)
    .with_schedule_tag(HttpScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse"))
    .with_deprecation(HttpDeprecation::Boolean(true))
    .with_accept_ch(["Sec-CH-UA"])
    .expect("Accept-CH should be accepted")
    .header("CDN-Cache-Control", "max-age=600, cdn-example=\"a, b\"");
  let keep_alive = HttpKeepAlive::parse("timeout=5, max=100").expect("Keep-Alive should parse");
  let memento_datetime = HttpMementoDatetime::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("Memento-Datetime should parse");
  let _: HttpMementoDatetimeParseError =
    HttpMementoDatetime::parse("").expect_err("empty Memento-Datetime should be rejected");
  let retry_after_delta = HttpRetryAfter::parse("120").expect("Retry-After delta should parse");
  let retry_after_date =
    HttpRetryAfter::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Retry-After date should parse");
  let _: HttpRetryAfterParseError =
    HttpRetryAfter::parse("").expect_err("empty Retry-After should be rejected");
  let retry_after_response = HttpResponse::ok("").with_retry_after_delta(120);
  let rate_limit_limit = HttpRateLimitLimit::new([
    HttpRateLimitLimitItem::new(100),
    HttpRateLimitLimitItem::new(50).with_window(3_600),
  ]);
  let _: HttpRateLimitLimitParseError =
    HttpRateLimitLimit::parse("100, (50)").expect_err("malformed RateLimit-Limit should fail");
  let _: HttpRateLimitRemainingParseError =
    HttpRateLimitRemaining::parse("1, 2").expect_err("duplicate RateLimit-Remaining should fail");
  let _: HttpRateLimitResetParseError =
    HttpRateLimitReset::parse("1, 2").expect_err("duplicate RateLimit-Reset should fail");
  let _: HttpRateLimitParseError =
    HttpRateLimitLimit::parse("100, (50)").expect_err("shared RateLimit parse error should fail");
  let rate_limit_response = HttpResponse::ok("")
    .with_rate_limit_limit(rate_limit_limit)
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(0))
    .with_rate_limit_reset(HttpRateLimitReset::new(0));
  let response_date =
    HttpResponseDate::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Date should parse");
  let _: HttpResponseDateParseError =
    HttpResponseDate::parse("").expect_err("empty Date should be rejected");
  let response_expires =
    HttpResponseExpires::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Expires should parse");
  let _: HttpExpiresParseError =
    HttpResponseExpires::parse("").expect_err("empty Expires should be rejected");
  let response_last_modified = HttpResponseLastModified::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("Last-Modified should parse");
  let _: HttpResponseLastModifiedParseError =
    HttpResponseLastModified::parse("").expect_err("empty Last-Modified should be rejected");
  let memento_response = HttpResponse::ok("")
    .with_memento_datetime(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777));
  let http_date_response = HttpResponse::ok("")
    .with_date(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777))
    .with_expires(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777))
    .with_last_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777));
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
  let document_policy: HttpDocumentPolicy =
    HttpDocumentPolicy::parse("oversized-images=2.0, unsized-media=?0, *;report-to=default")
      .expect("Document-Policy should parse");
  let _: HttpDocumentPolicyParseError = HttpDocumentPolicy::parse("unsized-media=src;foo=bar")
    .expect_err("unknown Document-Policy parameter should be rejected");
  let document_policy_response = HttpResponse::ok("")
    .with_document_policy("oversized-images=2.0, unsized-media=?0, *;report-to=default")
    .expect("Document-Policy should be accepted");
  let document_policy_report_only: HttpDocumentPolicyReportOnly =
    HttpDocumentPolicyReportOnly::parse(
      "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    )
    .expect("Document-Policy-Report-Only should parse");
  let _: HttpDocumentPolicyReportOnlyParseError =
    HttpDocumentPolicyReportOnly::parse("unsized-media=src;foo=bar")
      .expect_err("unknown Document-Policy-Report-Only parameter should be rejected");
  let document_policy_report_only_response = HttpResponse::ok("")
    .with_document_policy_report_only("oversized-images=2.0, unsized-media=?0, *;report-to=default")
    .expect("Document-Policy-Report-Only should be accepted");
  let supports_loading_mode: HttpSupportsLoadingMode =
    HttpSupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
      .expect("Supports-Loading-Mode should parse");
  let _: HttpSupportsLoadingModeParseError =
    HttpSupportsLoadingMode::parse("?1").expect_err("non-token should be rejected");
  let supports_loading_mode_response = HttpResponse::ok("")
    .with_supports_loading_mode(["fenced-frame", "credentialed-prerender"])
    .expect("Supports-Loading-Mode should be accepted");
  let sec_websocket_version: HttpSecWebSocketVersion =
    HttpSecWebSocketVersion::parse("13").expect("Sec-WebSocket-Version should parse");
  let _: HttpSecWebSocketVersionParseError = HttpSecWebSocketVersion::parse("8, 13")
    .expect_err("unordered Sec-WebSocket-Version should be rejected");
  let sec_websocket_version_response = HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_version(["13"])
    .expect("Sec-WebSocket-Version should be accepted");
  let sec_websocket_protocol: HttpSecWebSocketProtocol =
    HttpSecWebSocketProtocol::parse("chat, superchat")
      .expect("Sec-WebSocket-Protocol offers should parse");
  let _: HttpSecWebSocketProtocolParseError =
    HttpSecWebSocketProtocol::parse_selection("chat, superchat")
      .expect_err("multi-token Sec-WebSocket-Protocol selection should be rejected");
  let sec_websocket_protocol_response = HttpResponse::new(400, "Bad Request")
    .with_sec_websocket_protocol("graphql-transport-ws")
    .expect("Sec-WebSocket-Protocol should be accepted");
  let sec_websocket_extensions: HttpSecWebSocketExtensions =
    HttpSecWebSocketExtensions::parse(r#"permessage-deflate; client_max_window_bits; mode="safe""#)
      .expect("Sec-WebSocket-Extensions should parse");
  let _: HttpSecWebSocketExtensionsParseError =
    HttpSecWebSocketExtensions::parse_selection("permessage-deflate, x-test")
      .expect_err("multi-extension Sec-WebSocket-Extensions selection should be rejected");
  let sec_websocket_extensions_response = HttpResponse::new(101, "Switching Protocols")
    .with_sec_websocket_extensions("permessage-deflate; server_max_window_bits=15")
    .expect("Sec-WebSocket-Extensions should be accepted");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let sec_purpose = SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let upgrade: HttpUpgrade = HttpUpgrade::parse("websocket").expect("Upgrade should parse");
  let _: HttpUpgradeParseError = HttpUpgrade::parse("").expect_err("empty Upgrade should fail");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!("text/html", accept.media_ranges()[0].media_type());
  assert_eq!(Some(800), accept.media_ranges()[0].quality());
  assert_eq!(Some("1"), accept.media_ranges()[0].parameter("level"));
  assert_eq!(a_im.members()[0].token(), "diffe");
  assert_eq!(a_im.members()[1].quality(), 300);
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3;profile=compact");
  assert_eq!(negotiate.members()[0], HttpNegotiateDirective::Trans);
  assert_eq!(negotiate.members()[3], HttpNegotiateDirective::Any);
  assert_eq!("trans, 1.0, feature-x=preview, *", negotiate.header_value());
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
  assert_eq!(dnt.header_value(), "1");
  assert!(dnt_error.is_err());
  assert_eq!(
    referer.header_value(),
    "https://shop.example/checkout?step=pay"
  );
  assert!(referer_error.is_err());
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
  assert_eq!(HttpDepth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert!(depth_error.is_err());
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.as_str()
  );
  assert!(!format!("{lock_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(lock_token_error.is_err());
  assert_eq!(
    &[HttpTimeoutType::Second(60), HttpTimeoutType::Infinite],
    timeout.members()
  );
  assert_eq!("second-60, infinite", timeout.header_value());
  assert!(timeout_error.is_err());
  assert_eq!(HttpOverwrite::F, overwrite);
  assert_eq!("F", overwrite.header_value());
  assert!(overwrite_error.is_err());
  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
  assert_eq!(expectations.header_value(), "100-continue, preview");
  assert!(expectations_error.is_err());
  assert_eq!(
    if_modified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert!(if_modified_since_error.is_err());
  assert_eq!(if_schedule_tag_match.header_value(), "\"sched-17\"");
  assert_eq!(if_schedule_tag_match.opaque_tag(), "sched-17");
  assert!(!if_schedule_tag_match.is_weak());
  assert!(if_schedule_tag_match_weak.is_weak());
  assert_eq!(if_schedule_tag_match_weak.header_value(), "W/\"sched-17\"");
  assert!(if_schedule_tag_match_error.is_err());
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
  assert_eq!(surrogate_control.directives()[1].name(), "content");
  assert_eq!(surrogate_control.directives()[1].value(), Some("ESI/1.0"));
  assert_eq!(cdn_loop.members()[0].identifier(), "foo123.foocdn.example");
  assert_eq!(cdn_loop.members()[1].parameter("trace"), Some("abcdef"));
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!(
    "1.1 edge-a (TLS terminator), HTTP/2 upstream",
    via_response
      .via()
      .expect("declared Via should parse")
      .expect("Via should be present")
      .header_value()
  );
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
    response
      .delta_base()
      .expect("Delta-Base should parse")
      .expect("Delta-Base should be present")
      .entity_tag(),
    &HttpEntityTag::strong("revision-42")
  );
  assert_eq!(
    response.schedule_tag().expect("Schedule-Tag should parse"),
    Some(HttpScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse"))
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
  assert_eq!(document_policy.directives().len(), 3);
  assert_eq!(
    document_policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
  let star_directive: &HttpDocumentPolicyDirective = document_policy.directive("*").unwrap();
  assert_eq!(Some("default"), star_directive.report_to());
  let _: &HttpDocumentPolicyValue = star_directive.value();
  assert_eq!(
    "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    document_policy_response
      .document_policy()
      .expect("Document-Policy should parse")
      .expect("Document-Policy should be present")
      .header_value()
  );
  assert_eq!(document_policy_report_only.directives().len(), 3);
  assert_eq!(
    document_policy_report_only
      .directive("oversized-images")
      .unwrap()
      .value(),
    &HttpDocumentPolicyReportOnlyValue::Decimal("2.0".to_string())
  );
  assert_eq!(
    "oversized-images=2.0, unsized-media=?0, *;report-to=default",
    document_policy_report_only_response
      .document_policy_report_only()
      .expect("Document-Policy-Report-Only should parse")
      .expect("Document-Policy-Report-Only should be present")
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
  assert_eq!(sec_websocket_version.versions(), ["13"]);
  assert!(sec_websocket_version.contains("13"));
  assert_eq!(sec_websocket_version.header_value(), "13");
  assert_eq!(
    "13",
    sec_websocket_version_response
      .sec_websocket_version()
      .expect("Sec-WebSocket-Version should parse")
      .expect("Sec-WebSocket-Version should be present")
      .header_value()
  );
  assert_eq!(sec_websocket_protocol.protocols(), ["chat", "superchat"]);
  assert!(sec_websocket_protocol.contains("chat"));
  assert_eq!(sec_websocket_protocol.header_value(), "chat, superchat");
  assert_eq!(
    "graphql-transport-ws",
    sec_websocket_protocol_response
      .sec_websocket_protocol()
      .expect("Sec-WebSocket-Protocol should parse")
      .expect("Sec-WebSocket-Protocol should be present")
      .header_value()
  );
  assert_eq!(
    sec_websocket_extensions.header_value(),
    r#"permessage-deflate; client_max_window_bits; mode="safe""#
  );
  assert_eq!(
    sec_websocket_extensions.extensions()[0].token(),
    "permessage-deflate"
  );
  assert_eq!(
    "permessage-deflate; server_max_window_bits=15",
    sec_websocket_extensions_response
      .sec_websocket_extensions()
      .expect("Sec-WebSocket-Extensions should parse")
      .expect("Sec-WebSocket-Extensions should be present")
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
    response_date.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    response_expires.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    response_last_modified.header_value(),
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
  assert_eq!(retry_after_delta.header_value(), "120");
  assert_eq!(
    retry_after_date.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    retry_after_response
      .retry_after()
      .expect("Retry-After should parse")
      .expect("Retry-After should be present"),
    HttpRetryAfter::DeltaSeconds(120)
  );
  assert_eq!(
    rate_limit_response
      .rate_limit_limit()
      .expect("RateLimit-Limit should parse")
      .expect("RateLimit-Limit should be present")
      .header_value(),
    "100, 50;w=3600"
  );
  assert!(rate_limit_response
    .rate_limit_remaining()
    .expect("RateLimit-Remaining should parse")
    .expect("RateLimit-Remaining should be present")
    .is_exhausted());
  assert!(rate_limit_response
    .rate_limit_reset()
    .expect("RateLimit-Reset should parse")
    .expect("RateLimit-Reset should be present")
    .is_immediate());
  assert_eq!(
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777)),
    http_date_response.date().expect("Date should parse")
  );
  assert_eq!(
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777)),
    http_date_response.expires().expect("Expires should parse")
  );
  assert_eq!(
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777)),
    http_date_response
      .last_modified_date()
      .expect("Last-Modified should parse")
  );
}

#[test]
fn parsed_request_accept_preserves_utf8_quoted_parameter_values() {
  let request = HttpRequest::parse(
    "GET / HTTP/1.1\r\nHost: example.test\r\nAccept: text/plain; title=\"é\"\r\n\r\n".as_bytes(),
  )
  .expect("request should parse");
  let accept = request
    .accept()
    .expect("Accept should parse")
    .expect("Accept should be present");

  assert_eq!(Some("é"), accept.media_ranges()[0].parameter("title"));
  assert_eq!("text/plain; title=\"é\"", accept.header_value());
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
fn response_http_date_setters_replace_existing_singleton_fields() {
  let timestamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(784_111_777);
  let response = HttpResponse::ok("")
    .header("Date", "not a date")
    .header("date", "Sun, 06 Nov 1994 08:49:38 GMT")
    .header("Expires", "not a date")
    .header("expires", "Sun, 06 Nov 1994 08:49:38 GMT")
    .header("Last-Modified", "not a date")
    .header("last-modified", "Sun, 06 Nov 1994 08:49:38 GMT")
    .with_date(timestamp)
    .with_expires(timestamp)
    .with_last_modified(timestamp);

  assert_eq!(
    Some(timestamp),
    response
      .date()
      .expect("Date should parse after replacement")
  );
  assert_eq!(
    Some(timestamp),
    response
      .expires()
      .expect("Expires should parse after replacement")
  );
  assert_eq!(
    Some(timestamp),
    response
      .last_modified_date()
      .expect("Last-Modified should parse after replacement")
  );
  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert_eq!(
    1,
    serialized.matches("\r\nDate: ").count(),
    "Date should be serialized once"
  );
  assert_eq!(
    1,
    serialized.matches("\r\nExpires: ").count(),
    "Expires should be serialized once"
  );
  assert_eq!(
    1,
    serialized.matches("\r\nLast-Modified: ").count(),
    "Last-Modified should be serialized once"
  );
  assert!(serialized.contains("\r\nDate: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
  assert!(serialized.contains("\r\nExpires: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
  assert!(serialized.contains("\r\nLast-Modified: Sun, 06 Nov 1994 08:49:37 GMT\r\n"));
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
fn response_facade_declares_and_parses_surrogate_control_without_policy() {
  let response = HttpResponse::ok("")
    .header("Cache-Control", "max-age=1")
    .header("Surrogate-Control", "legacy=1")
    .with_surrogate_control("max-age=600, content=\"ESI/1.0\"")
    .expect("valid Surrogate-Control should be accepted");

  let metadata = response
    .surrogate_control()
    .expect("Surrogate-Control should parse")
    .expect("Surrogate-Control should be present");

  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.directives()[0].name(), "max-age");
  assert_eq!(metadata.directives()[0].value(), Some("600"));
  assert_eq!(metadata.directives()[1].value(), Some("ESI/1.0"));
  assert_eq!(
    response
      .cache_control()
      .expect("Cache-Control should remain parseable")
      .expect("Cache-Control should remain present")
      .max_age(),
    Some(1)
  );
  let rendered = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert!(rendered.contains("\r\nSurrogate-Control: max-age=600, content=\"ESI/1.0\"\r\n"));
  assert!(!rendered.contains("\r\nSurrogate-Control: legacy=1\r\n"));

  let duplicate = HttpResponse::ok("").header("Surrogate-Control", "max-age=60, Max-Age=120");
  assert!(duplicate.surrogate_control().is_err());
  assert!(HttpResponse::ok("")
    .with_surrogate_control("max-age=60, Max-Age=120")
    .is_err());

  let absent = HttpResponse::ok("");
  assert!(absent
    .surrogate_control()
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
fn request_facade_parses_a_im_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nA-IM: diffe, gzip;q=0.3;profile=compact\r\nA-IM: identity;q=0\r\n\r\n",
  )
  .expect("request should parse");

  let a_im: HttpAIm = request
    .a_im()
    .expect("A-IM should parse")
    .expect("A-IM should be present");

  assert_eq!(a_im.members()[0].token(), "diffe");
  assert_eq!(a_im.members()[1].token(), "gzip");
  assert_eq!(a_im.members()[1].quality(), 300);
  assert_eq!(Some("compact"), a_im.members()[1].parameters()[1].value());
  assert_eq!(a_im.members()[2].token(), "identity");
  assert_eq!(a_im.members()[2].quality(), 0);
  assert_eq!(
    a_im.header_value(),
    "diffe, gzip;q=0.3;profile=compact, identity;q=0"
  );
}

#[test]
fn request_facade_omits_a_im_metadata_when_header_is_absent() {
  let request = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");

  assert_eq!(
    None,
    request.a_im().expect("missing A-IM should be accepted")
  );
}

#[test]
fn request_facade_rejects_malformed_a_im_metadata_without_hiding_headers() {
  let request =
    HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\nA-IM: diffe, DIFFE\r\n\r\n")
      .expect("malformed metadata should not reject raw request parsing");

  assert_eq!(request.header("A-IM"), Some("diffe, DIFFE"));
  assert!(request.a_im().is_err());
}

#[test]
fn request_facade_rejects_oversized_a_im_metadata_without_hiding_headers() {
  let too_many = (0..=32)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw = format!("GET /asset HTTP/1.1\r\nHost: example.test\r\nA-IM: {too_many}\r\n\r\n");
  let request = HttpRequest::parse(raw.as_bytes())
    .expect("over-limit typed metadata should not reject raw request parsing");

  assert_eq!(request.header("A-IM"), Some(too_many.as_str()));
  assert!(request.a_im().is_err());

  let oversized = "x".repeat(64 * 1024 + 1);
  let oversized_error: Result<HttpAIm, HttpAImParseError> = HttpAIm::parse(oversized.as_str());
  assert!(oversized_error.is_err());

  let first = "a".repeat(32 * 1024 + 1);
  let second = "b".repeat(32 * 1024 + 1);
  assert!(HttpAIm::parse_values([first.as_str(), second.as_str()]).is_err());
}

#[test]
fn request_facade_parses_negotiate_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nNegotiate: trans, 1.0, feature-x=preview\r\nNegotiate: *, 2.5\r\n\r\n",
  )
  .expect("request should parse");

  let negotiate: HttpNegotiate = request
    .negotiate()
    .expect("Negotiate should parse")
    .expect("Negotiate should be present");

  assert_eq!(negotiate.members()[0], HttpNegotiateDirective::Trans);
  assert_eq!(
    negotiate.members()[1],
    HttpNegotiateDirective::RvsaVersion { major: 1, minor: 0 }
  );
  assert_eq!(
    negotiate.members()[2],
    HttpNegotiateDirective::Extension {
      name: "feature-x".to_owned(),
      value: Some("preview".to_owned()),
    }
  );
  assert_eq!(negotiate.members()[3], HttpNegotiateDirective::Any);
  assert_eq!(
    negotiate.members()[4],
    HttpNegotiateDirective::RvsaVersion { major: 2, minor: 5 }
  );
  assert_eq!(
    negotiate.header_value(),
    "trans, 1.0, feature-x=preview, *, 2.5"
  );
}

#[test]
fn request_facade_omits_negotiate_metadata_when_header_is_absent() {
  let request = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");

  assert_eq!(
    None,
    request
      .negotiate()
      .expect("missing Negotiate should be accepted")
  );
}

#[test]
fn request_facade_rejects_malformed_negotiate_metadata_without_hiding_headers() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nNegotiate: trans, TRANS\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");

  assert_eq!(request.header("Negotiate"), Some("trans, TRANS"));
  assert!(request.negotiate().is_err());
}

#[test]
fn request_facade_rejects_oversized_negotiate_metadata_without_hiding_headers() {
  let too_many = (0..=32)
    .map(|index| format!("feature-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let raw = format!("GET /asset HTTP/1.1\r\nHost: example.test\r\nNegotiate: {too_many}\r\n\r\n");
  let request = HttpRequest::parse(raw.as_bytes())
    .expect("over-limit typed metadata should not reject raw request parsing");

  assert_eq!(request.header("Negotiate"), Some(too_many.as_str()));
  assert!(request.negotiate().is_err());

  let oversized = format!("feature-x={}", "a".repeat(64 * 1024 + 1));
  let oversized_error: Result<HttpNegotiate, HttpNegotiateParseError> =
    HttpNegotiate::parse(oversized.as_str());
  assert!(oversized_error.is_err());

  let first = "a".repeat(32 * 1024 + 1);
  let second = "b".repeat(32 * 1024 + 1);
  assert!(HttpNegotiate::parse_values([first.as_str(), second.as_str()]).is_err());
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
fn response_facade_builds_and_parses_sec_websocket_accept_metadata() {
  let key =
    HttpSecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");
  let response = HttpResponse::new(101, "Switching Protocols")
    .header("Sec-WebSocket-Accept", "legacy")
    .with_sec_websocket_accept_for_key(&key);

  let accept = response
    .sec_websocket_accept()
    .expect("Sec-WebSocket-Accept should parse")
    .expect("Sec-WebSocket-Accept should be present");

  assert_eq!("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", accept.as_str());
  assert!(accept.verify_key(&key));
  assert!(!format!("{accept:?}").contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
  let _: Result<HttpSecWebSocketKey, HttpSecWebSocketKeyParseError> =
    HttpSecWebSocketKey::parse("the sample nonce");
  let _: Result<HttpSecWebSocketAccept, HttpSecWebSocketAcceptParseError> =
    HttpSecWebSocketAccept::parse("the accept value");

  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains("\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"));
  assert!(!serialized.contains("\r\nSec-WebSocket-Accept: legacy\r\n"));
}

#[test]
fn response_facade_builds_and_parses_lock_token_metadata() {
  let response = HttpResponse::new(200, "OK")
    .header("Lock-Token", "<http://example.test/locks/legacy>")
    .with_lock_token("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
    .expect("Lock-Token should be accepted");

  let lock_token = response
    .lock_token()
    .expect("Lock-Token should parse")
    .expect("Lock-Token should be present");

  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.as_str()
  );
  assert!(!format!("{lock_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  let _: Result<HttpLockToken, HttpLockTokenParseError> = HttpLockToken::parse("<relative>");

  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized
    .contains("\r\nLock-Token: <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\n"));
  assert!(!serialized.contains("\r\nLock-Token: <http://example.test/locks/legacy>\r\n"));
}

#[test]
fn response_facade_builds_and_parses_im_metadata() {
  let response = HttpResponse::ok("")
    .header("IM", "legacy")
    .with_im(["diffe", "gzip;profile=compact"])
    .expect("IM metadata should be accepted");

  let im = response
    .im()
    .expect("IM metadata should parse")
    .expect("IM metadata should be present");

  assert_eq!(im.len(), 2);
  assert_eq!("diffe", im.members()[0].token());
  assert_eq!("gzip", im.members()[1].token());
  assert_eq!(Some("compact"), im.members()[1].parameters()[0].value());
  assert_eq!("diffe, gzip;profile=compact", im.header_value());

  let with_q = HttpResponse::ok("")
    .with_im(["gzip;q=0.3"])
    .expect("q-named IM parameters should be accepted");
  let q_im = with_q
    .im()
    .expect("q-named IM metadata should parse")
    .expect("q-named IM metadata should be present");
  assert_eq!("gzip", q_im.members()[0].token());
  assert_eq!("q", q_im.members()[0].parameters()[0].name());
  assert_eq!(Some("0.3"), q_im.members()[0].parameters()[0].value());
  assert_eq!("gzip;q=0.3", q_im.header_value());

  let mut serialized = Vec::new();
  response.write_to(&mut serialized).expect("response writes");
  let serialized = String::from_utf8(serialized).expect("response is utf8");
  assert!(serialized.contains("\r\nIM: diffe, gzip;profile=compact\r\n"));
  assert!(!serialized.contains("\r\nIM: legacy\r\n"));

  let absent = HttpResponse::ok("");
  assert_eq!(
    absent.im().expect("absent IM metadata should not error"),
    None
  );

  let multi_field = HttpResponse::ok("")
    .header("IM", "diffe")
    .header("IM", "gzip;profile=compact, identity");
  let multi = multi_field
    .im()
    .expect("multi-field IM metadata should parse")
    .expect("multi-field IM metadata should be present");
  assert_eq!(
    multi.header_value(),
    "diffe, gzip;profile=compact, identity"
  );

  let unchanged = HttpResponse::ok("").header("IM", "diffe");
  assert!(unchanged.clone().with_im(["diffe", "DIFFE"]).is_err());
  assert_eq!(
    "diffe",
    unchanged
      .im()
      .expect("original IM should still parse")
      .expect("original IM should be present")
      .header_value()
  );

  let duplicate = HttpResponse::ok("").header("IM", "diffe, DIFFE");
  assert!(duplicate.im().is_err());
  let duplicate_rendered =
    String::from_utf8(duplicate.to_bytes()).expect("response should serialize");
  assert!(duplicate_rendered.contains("\r\nIM: diffe, DIFFE\r\n"));

  let oversized = format!("x{}", "a".repeat(64 * 1024));
  let invalid = HttpResponse::ok("").header("IM", oversized);
  assert!(invalid.im().is_err());
  let invalid_rendered = String::from_utf8(invalid.to_bytes()).expect("response should serialize");
  assert!(invalid_rendered.contains("\r\nIM: "));

  let too_many = (0..=32)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let over_limit = HttpResponse::ok("").header("IM", too_many);
  assert!(over_limit.im().is_err());
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
fn request_facade_parses_from_metadata_without_policy() {
  let bare = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nFrom: ops@example.test\r\n\r\n",
  )
  .expect("request should parse");
  let bare_from: HttpFrom = bare
    .from()
    .expect("From should parse")
    .expect("From should be present");
  assert_eq!("ops@example.test", bare_from.address());
  assert_eq!("ops", bare_from.local_part());
  assert_eq!("example.test", bare_from.domain());
  assert_eq!(None, bare_from.display_name());
  assert_eq!("ops@example.test", bare_from.header_value());

  let named = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nFrom: Ops\t Team  <ops@example.test>\r\n\r\n",
  )
  .expect("request should parse");
  let named_from = named
    .from()
    .expect("From should parse")
    .expect("From should be present");
  assert_eq!(Some("Ops Team"), named_from.display_name());
  assert_eq!("Ops Team <ops@example.test>", named_from.header_value());

  let absent = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.from().expect("missing From should be valid"));

  let malformed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nFrom: Ops Team<ops@example.test>\r\n\r\n",
  )
  .expect("malformed metadata should remain available");
  assert!(malformed.from().is_err());
  assert_eq!(Some("Ops Team<ops@example.test>"), malformed.header("From"));

  let duplicate = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nFrom: ops@example.test\r\nfrom: other@example.test\r\n\r\n",
  )
  .expect("duplicate metadata should remain available");
  assert!(duplicate.from().is_err());
  assert_eq!(Some("ops@example.test"), duplicate.header("From"));

  let _: HttpFromParseError = HttpFrom::parse("ops@example.test\0")
    .expect_err("control-byte From metadata should be rejected");
  assert!(
    HttpFrom::parse("a".repeat(64 * 1024 + 1)).is_err(),
    "oversized From metadata should be rejected"
  );
}

#[test]
fn request_facade_parses_referer_metadata_without_policy() {
  let absolute = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: https://shop.example/checkout?step=pay\r\n\r\n",
  )
  .expect("request should parse");
  let absolute_referer: HttpReferer = absolute
    .referer()
    .expect("Referer should parse")
    .expect("Referer should be present");
  assert_eq!(
    "https://shop.example/checkout?step=pay",
    absolute_referer.header_value()
  );

  let relative = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: /checkout?step=pay\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    "/checkout?step=pay",
    relative
      .referer()
      .expect("Referer should parse")
      .expect("Referer should be present")
      .header_value()
  );

  let scheme_relative = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: //cdn.example/lib.js\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    "//cdn.example/lib.js",
    scheme_relative
      .referer()
      .expect("Referer should parse")
      .expect("Referer should be present")
      .header_value()
  );

  let trimmed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: \thttps://example.test/path?q=1\t\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    "https://example.test/path?q=1",
    trimmed
      .referer()
      .expect("Referer should parse")
      .expect("Referer should be present")
      .header_value()
  );

  let absent = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent.referer().expect("missing Referer should be valid")
  );

  let malformed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: https://example.test/path#frag\r\n\r\n",
  )
  .expect("malformed metadata should remain available");
  assert!(malformed.referer().is_err());
  assert_eq!(
    Some("https://example.test/path#frag"),
    malformed.header("Referer")
  );

  let duplicate = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nReferer: https://example.test/a\r\nreferer: https://example.test/b\r\n\r\n",
  )
  .expect("duplicate metadata should remain available");
  assert!(duplicate.referer().is_err());
  assert_eq!(Some("https://example.test/a"), duplicate.header("Referer"));

  let _: HttpRefererParseError = HttpReferer::parse("https://example.test/%zz")
    .expect_err("malformed percent-encoding Referer should be rejected");
  assert!(
    HttpReferer::parse("a".repeat(64 * 1024 + 1)).is_err(),
    "oversized Referer metadata should be rejected"
  );
  let _: HttpRefererParseError = HttpReferer::parse("https://example.test/path\0")
    .expect_err("control-byte Referer metadata should be rejected");
}

#[test]
fn request_facade_parses_depth_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\nDepth: INFINITY\r\n\r\n",
  )
  .expect("request should parse");
  let depth: HttpDepth = request
    .depth()
    .expect("Depth should parse")
    .expect("Depth should be present");

  assert_eq!(HttpDepth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert_eq!(Some("INFINITY"), request.header("Depth"));

  let absent = HttpRequest::parse(b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent.depth().expect("missing Depth should be accepted")
  );

  let malformed =
    HttpRequest::parse(b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\nDepth: 2\r\n\r\n")
      .expect("request should parse");
  assert!(malformed.depth().is_err());
  assert_eq!(Some("2"), malformed.header("Depth"));

  let duplicate = HttpRequest::parse(
    b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\nDepth: 0\r\ndepth: 1\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.depth().is_err());
  assert_eq!(Some("0"), duplicate.header("Depth"));
}

#[test]
fn request_facade_parses_lock_token_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nLock-Token: <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\n\r\n",
  )
  .expect("request should parse");
  let lock_token: HttpLockToken = request
    .lock_token()
    .expect("Lock-Token should parse")
    .expect("Lock-Token should be present");

  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.as_str()
  );
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.header_value()
  );
  assert!(!format!("{lock_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert_eq!(
    Some("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"),
    request.header("Lock-Token")
  );

  let absent = HttpRequest::parse(b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .lock_token()
      .expect("missing Lock-Token should be accepted")
  );

  let malformed = HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nLock-Token: <relative>\r\n\r\n",
  )
  .expect("request should parse");
  assert!(malformed.lock_token().is_err());
  assert_eq!(Some("<relative>"), malformed.header("Lock-Token"));

  let duplicate = HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nLock-Token: <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\nlock-token: <http://example.test/locks/2>\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.lock_token().is_err());
  assert_eq!(
    Some("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"),
    duplicate.header("Lock-Token")
  );
}

#[test]
fn request_facade_parses_if_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nIf: <http://example.test/src> (<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) (Not [\"etag-one\"])\r\n\r\n",
  )
  .expect("request should parse");
  let if_header: HttpIf = request
    .if_header()
    .expect("WebDAV If should parse")
    .expect("WebDAV If should be present");

  assert!(if_header.is_tagged());
  assert_eq!(2, if_header.lists().len());
  let list: HttpIfList = if_header.lists()[0].clone();
  let tag: HttpIfResourceTag = list.resource_tag().expect("tagged list").clone();
  assert_eq!("<http://example.test/src>", tag.as_str());
  let condition: HttpIfCondition = list.conditions()[0].clone();
  let predicate: HttpIfPredicate = condition.predicate().clone();
  let token: HttpIfStateToken = match predicate {
    HttpIfPredicate::StateToken(token) => token,
    HttpIfPredicate::EntityTag(_) => panic!("expected a state token"),
  };
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    token.as_str()
  );
  assert_eq!(
    if_header.header_value(),
    "<http://example.test/src> (<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) \
     <http://example.test/src> (Not [\"etag-one\"])"
  );
  assert!(!format!("{if_header:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert_eq!(
    Some(
      "<http://example.test/src> (<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) \
       (Not [\"etag-one\"])"
    ),
    request.header("If")
  );
  let _: HttpIfParseError = HttpIf::parse("(junk)").expect_err("malformed WebDAV If should fail");
  let _: HttpIfParseError = HttpIf::parse("(Not<DAV:no-lock>)")
    .expect_err("Not without required whitespace should be rejected");

  let absent = HttpRequest::parse(b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .if_header()
      .expect("missing WebDAV If should be accepted")
  );

  let malformed =
    HttpRequest::parse(b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nIf: (junk)\r\n\r\n")
      .expect("request should parse");
  assert!(malformed.if_header().is_err());
  assert_eq!(Some("(junk)"), malformed.header("If"));

  let duplicate = HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nIf: (<a:b>)\r\nIf: (<b:c>)\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.if_header().is_err());
  assert_eq!(Some("(<a:b>)"), duplicate.header("If"));
}

#[test]
fn request_facade_parses_timeout_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-60, Infinite\r\n\r\n",
  )
  .expect("request should parse");
  let timeout: HttpTimeout = request
    .timeout()
    .expect("Timeout should parse")
    .expect("Timeout should be present");

  assert_eq!(
    &[HttpTimeoutType::Second(60), HttpTimeoutType::Infinite],
    timeout.members()
  );
  assert_eq!("second-60, infinite", timeout.header_value());
  assert_eq!(Some("Second-60, Infinite"), request.header("Timeout"));

  let absent = HttpRequest::parse(b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .timeout()
      .expect("missing Timeout should be accepted")
  );

  let malformed = HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-\r\n\r\n",
  )
  .expect("malformed Timeout request should still parse");
  assert!(malformed.timeout().is_err());
  assert_eq!(Some("Second-"), malformed.header("Timeout"));

  let overflow = HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-18446744073709551616\r\n\r\n",
  )
  .expect("overflow Timeout request should still parse");
  assert!(overflow.timeout().is_err());

  let duplicate = HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-60\r\ntimeout: second-60\r\n\r\n",
  )
  .expect("duplicate Timeout request should still parse");
  assert!(duplicate.timeout().is_err());
  assert_eq!(Some("Second-60"), duplicate.header("Timeout"));

  assert!(
    HttpTimeout::parse(format!("{}Second-1", " ".repeat(64 * 1024 + 1))).is_err(),
    "oversized Timeout values must fail closed"
  );
}

#[test]
fn request_facade_parses_if_schedule_tag_match_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: \"sched-17\"\r\n\r\n",
  )
  .expect("request should parse");
  let validator: HttpIfScheduleTagMatch = request
    .if_schedule_tag_match()
    .expect("If-Schedule-Tag-Match should parse")
    .expect("If-Schedule-Tag-Match should be present");

  assert_eq!("\"sched-17\"", validator.header_value());
  assert_eq!("sched-17", validator.opaque_tag());
  assert!(!validator.is_weak());
  assert_eq!(
    Some("\"sched-17\""),
    request.header("If-Schedule-Tag-Match")
  );

  let weak = HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: W/\"sched-17\"\r\n\r\n",
  )
  .expect("request should parse");
  let weak_validator: HttpIfScheduleTagMatch = weak
    .if_schedule_tag_match()
    .expect("weak If-Schedule-Tag-Match should parse")
    .expect("weak If-Schedule-Tag-Match should be present");
  assert!(weak_validator.is_weak());
  assert_eq!("sched-17", weak_validator.opaque_tag());

  let absent = HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\n\r\n",
  )
  .expect("request should parse");
  assert_eq!(
    None,
    absent
      .if_schedule_tag_match()
      .expect("missing If-Schedule-Tag-Match should be accepted")
  );

  let malformed = HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: *\r\n\r\n",
  )
  .expect("malformed If-Schedule-Tag-Match request should still parse");
  assert!(malformed.if_schedule_tag_match().is_err());
  assert_eq!(Some("*"), malformed.header("If-Schedule-Tag-Match"));

  let duplicate = HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: \"sched-16\"\r\nif-schedule-tag-match: \"sched-17\"\r\n\r\n",
  )
  .expect("duplicate If-Schedule-Tag-Match request should still parse");
  assert!(duplicate.if_schedule_tag_match().is_err());
  assert_eq!(
    Some("\"sched-16\""),
    duplicate.header("If-Schedule-Tag-Match")
  );

  assert!(
    HttpIfScheduleTagMatch::parse(format!("\"{}\"", "a".repeat(64 * 1024 - 1))).is_err(),
    "oversized If-Schedule-Tag-Match values must fail closed"
  );
}

#[test]
fn request_facade_parses_overwrite_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nOverwrite: F\r\n\r\n",
  )
  .expect("request should parse");
  let overwrite: HttpOverwrite = request
    .overwrite()
    .expect("Overwrite should parse")
    .expect("Overwrite should be present");

  assert_eq!(HttpOverwrite::F, overwrite);
  assert_eq!("F", overwrite.header_value());
  assert_eq!(Some("F"), request.header("Overwrite"));

  let absent =
    HttpRequest::parse(b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\n\r\n")
      .expect("request should parse");
  assert_eq!(
    None,
    absent
      .overwrite()
      .expect("missing Overwrite should be accepted")
  );

  let malformed = HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nOverwrite: true\r\n\r\n",
  )
  .expect("malformed Overwrite request should still parse");
  assert!(malformed.overwrite().is_err());
  assert_eq!(Some("true"), malformed.header("Overwrite"));

  let duplicate = HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nOverwrite: T\r\noverwrite: F\r\n\r\n",
  )
  .expect("duplicate Overwrite request should still parse");
  assert!(duplicate.overwrite().is_err());
  assert_eq!(Some("T"), duplicate.header("Overwrite"));

  assert!(
    HttpOverwrite::parse("T".repeat(64 * 1024 + 1)).is_err(),
    "oversized Overwrite values must fail closed"
  );
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
fn request_via_parses_ordered_hops_without_policy() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nVia: 1.1 edge-a (TLS terminator)\r\nVia: HTTP/2 upstream\r\n\r\n",
  )
  .expect("request should parse");

  let via: HttpVia = request
    .via()
    .expect("Via should parse")
    .expect("Via should be present");
  let member: &HttpViaMember = &via.members()[0];

  assert_eq!(2, via.len());
  assert_eq!("edge-a", member.received_by());
  assert_eq!(Some("TLS terminator"), member.comment());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!("upstream", via.members()[1].received_by());

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.via().expect("missing Via"));

  let malformed =
    HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\nVia: 1.1 hop extra\r\n\r\n")
      .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.via().is_err());
  assert_eq!(Some("1.1 hop extra"), malformed.header("Via"));

  let via_error: Result<HttpVia, HttpViaParseError> = HttpVia::parse("1.1");
  assert!(via_error.is_err());
}

#[test]
fn request_via_rejects_malformed_and_oversized_chains() {
  let excessive = (0..257)
    .map(|index| format!("1.1 hop{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let request = HttpRequest::parse(
    format!("GET / HTTP/1.1\r\nHost: example.test\r\nVia: {excessive}\r\n\r\n").as_bytes(),
  )
  .expect("oversized Via should not reject raw request parsing");
  assert!(request.via().is_err());
  assert_eq!(Some(excessive.as_str()), request.header("Via"));
}

#[test]
fn response_via_helper_validates_replaces_and_preserves_raw_headers() {
  let response = HttpResponse::ok("body")
    .header("Via", "1.0 legacy")
    .with_via("1.1 edge-a (TLS terminator), HTTP/2 upstream")
    .expect("valid Via should be accepted");
  let via: HttpVia = response
    .via()
    .expect("attached Via should parse")
    .expect("Via should be present");
  assert_eq!(2, via.len());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!("upstream", via.members()[1].received_by());

  assert!(HttpResponse::ok("body").with_via("1.1").is_err());
  let raw = HttpResponse::ok("body").header("Via", "1.1 hop extra");
  assert!(raw.via().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains("\r\nVia: 1.1 hop extra\r\n"));
}

#[test]
fn request_facade_parses_cdn_loop_metadata_without_policy() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nCDN-Loop: foo123.foocdn.example, barcdn.example; trace=\"abcdef\"\r\nCDN-Loop: AnotherCDN; abc=123\r\n\r\n",
  )
  .expect("request should parse");

  let cdn_loop: HttpCdnLoop = request
    .cdn_loop()
    .expect("CDN-Loop should parse")
    .expect("CDN-Loop should be present");
  let member: &HttpCdnLoopMember = &cdn_loop.members()[1];

  assert_eq!(3, cdn_loop.len());
  assert_eq!("foo123.foocdn.example", cdn_loop.members()[0].identifier());
  assert_eq!(Some("abcdef"), member.parameter("trace"));
  assert_eq!("AnotherCDN", cdn_loop.members()[2].identifier());

  let absent = HttpRequest::parse(b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(None, absent.cdn_loop().expect("missing CDN-Loop"));

  let malformed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nCDN-Loop: cdn; trace\r\n\r\n",
  )
  .expect("malformed metadata should not reject raw request parsing");
  assert!(malformed.cdn_loop().is_err());
  assert_eq!(Some("cdn; trace"), malformed.header("CDN-Loop"));

  let cdn_loop_error: Result<HttpCdnLoop, HttpCdnLoopParseError> =
    HttpCdnLoop::parse("cdn; trace=1; TRACE=2");
  assert!(cdn_loop_error.is_err());
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
fn request_facade_parses_accept_datetime_request_metadata() {
  let request = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Datetime: Sunday, 06-Nov-94 08:49:37 GMT\r\n\r\n",
  )
  .expect("request should parse");

  let accept_datetime: HttpAcceptDatetime = request
    .accept_datetime()
    .expect("Accept-Datetime should parse")
    .expect("Accept-Datetime should be present");
  assert_eq!(
    "Sun, 06 Nov 1994 08:49:37 GMT",
    accept_datetime.header_value(),
    "obsolete HTTP-date forms must canonicalize to IMF-fixdate"
  );
  assert_eq!(
    Some("Sunday, 06-Nov-94 08:49:37 GMT"),
    request.header("Accept-Datetime"),
    "the raw field must remain available"
  );

  let absent = HttpRequest::parse(b"GET /asset HTTP/1.1\r\nHost: example.test\r\n\r\n")
    .expect("request should parse");
  assert_eq!(
    None,
    absent
      .accept_datetime()
      .expect("absent value should be valid")
  );

  let malformed = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Datetime: not-a-date\r\n\r\n",
  )
  .expect("request should parse");
  assert!(malformed.accept_datetime().is_err());
  assert_eq!(
    Some("not-a-date"),
    malformed.header("Accept-Datetime"),
    "raw headers must remain inspectable after a parse error"
  );

  let duplicate = HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Datetime: Sun, 06 Nov 1994 08:49:37 GMT\r\nAccept-Datetime: Sun, 06 Nov 1994 08:49:38 GMT\r\n\r\n",
  )
  .expect("request should parse");
  assert!(duplicate.accept_datetime().is_err());

  let oversized = "0".repeat(64 * 1024 + 1);
  assert!(HttpAcceptDatetime::parse(oversized.as_str()).is_err());
  let _: HttpAcceptDatetimeParseError =
    HttpAcceptDatetime::parse("").expect_err("empty Accept-Datetime should be rejected");
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

#[test]
fn server_set_cookie_response_metadata_uses_protocol_representation() {
  let session = HttpSetCookie::new("session", "abc def")
    .expect("session cookie should be valid")
    .with_path("/")
    .expect("path should be accepted")
    .with_http_only()
    .expect("HttpOnly should be accepted")
    .with_same_site(HttpSameSite::Lax)
    .expect("SameSite should be accepted")
    .with_priority("High")
    .expect("Priority should be accepted")
    .with_partitioned()
    .expect("Partitioned should be accepted");
  let csrf = HttpSetCookie::new("csrf", "token")
    .expect("csrf cookie should be valid")
    .with_path("/form")
    .expect("path should be accepted")
    .with_max_age(60)
    .expect("Max-Age should be accepted")
    .with_extension("Foo", Some("bar"))
    .expect("extension should be accepted");
  let response = HttpResponse::ok("body")
    .header("Set-Cookie", "stale=old")
    .with_set_cookie(session)
    .with_set_cookie(csrf);
  let cookies = response
    .set_cookies()
    .expect("Set-Cookie metadata should parse")
    .expect("Set-Cookie metadata should be present");

  assert_eq!(3, cookies.len());
  assert_eq!("stale", cookies.cookies()[0].name());
  assert_eq!("session", cookies.cookies()[1].name());
  assert!(cookies.cookies()[1].is_value_quoted());
  assert_eq!("csrf", cookies.cookies()[2].name());
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(3, serialized.matches("\r\nSet-Cookie: ").count());
  assert!(serialized.contains("\r\nSet-Cookie: stale=old\r\n"));
  assert!(serialized.contains(
    "\r\nSet-Cookie: session=\"abc def\"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned\r\n"
  ));
  assert!(serialized.contains("\r\nSet-Cookie: csrf=token; Path=/form; Max-Age=60; Foo=bar\r\n"));
  assert!(!format!("{cookies:?}").contains("abc def"));
  assert!(!format!("{cookies:?}").contains("token"));

  let malformed =
    HttpResponse::ok("body").header("Set-Cookie", "session=super-secret; Path=/; path=/other");
  let error = malformed
    .set_cookies()
    .expect_err("duplicate attributes should fail");
  assert!(!error.to_string().contains("super-secret"));
  assert!(String::from_utf8(malformed.to_bytes())
    .expect("response should serialize")
    .contains("\r\nSet-Cookie: session=super-secret; Path=/; path=/other\r\n"));
  let absent = HttpResponse::ok("body");
  assert_eq!(
    None,
    absent
      .set_cookies()
      .expect("missing Set-Cookie should be accepted")
  );
}

#[test]
fn response_facade_builds_and_parses_variant_vary_metadata() {
  let response = HttpResponse::ok("body")
    .header("Variant-Vary", "Accept-Encoding")
    .with_variant_vary("Accept-Language, Sec-CH-DPR")
    .expect("valid Variant-Vary should be accepted");
  let variant_vary: HttpVariantVary = response
    .variant_vary()
    .expect("attached Variant-Vary should parse")
    .expect("Variant-Vary should be present");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(
    vec!["accept-language", "sec-ch-dpr"],
    variant_vary.field_names()
  );
  assert_eq!("accept-language, sec-ch-dpr", variant_vary.header_value());
  assert_eq!(1, serialized.matches("\r\nVariant-Vary: ").count());
  assert!(serialized.contains("\r\nVariant-Vary: accept-language, sec-ch-dpr\r\n"));
  assert!(!serialized.contains("\r\nVariant-Vary: Accept-Encoding\r\n"));

  let unchanged = HttpResponse::ok("body").header("Variant-Vary", "Accept-Language");
  assert!(unchanged
    .clone()
    .with_variant_vary("Accept-Language, accept-language")
    .is_err());
  assert_eq!(
    "accept-language",
    unchanged
      .variant_vary()
      .expect("original Variant-Vary should still parse")
      .expect("original Variant-Vary should be present")
      .header_value()
  );

  let absent = HttpResponse::ok("body");
  assert_eq!(
    None,
    absent
      .variant_vary()
      .expect("missing Variant-Vary should be accepted")
  );

  let malformed = HttpResponse::ok("body").header("Variant-Vary", "Accept Language");
  assert!(malformed.variant_vary().is_err());
  assert!(String::from_utf8(malformed.to_bytes())
    .expect("response should serialize")
    .contains("\r\nVariant-Vary: Accept Language\r\n"));

  let oversized = "a".repeat(64 * 1024 + 1);
  let raw = HttpResponse::ok("body").header("Variant-Vary", &oversized);
  assert!(raw.variant_vary().is_err());
  assert!(String::from_utf8(raw.to_bytes())
    .expect("response should serialize")
    .contains(&format!("\r\nVariant-Vary: {oversized}\r\n")));
  assert!(HttpResponse::ok("body")
    .with_variant_vary(&oversized)
    .is_err());
}
