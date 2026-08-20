use rttp::server::{
  HttpAIm, HttpAImParseError, HttpAcceptCh, HttpAcceptCharsetParseError,
  HttpAcceptLanguageParseError, HttpAcceptLanguages, HttpAccessControlRequestMethod,
  HttpAccessControlRequestPrivateNetwork, HttpAltUsed, HttpAltUsedParseError, HttpAlternates,
  HttpAlternatesParseError, HttpAuthorization, HttpBaggage, HttpBaggageMember,
  HttpBaggageParseError, HttpBaggageProperty, HttpCdnLoop, HttpCdnLoopParseError,
  HttpConditionalMetadata, HttpContentDpr, HttpContentDprParseError, HttpContentLocation,
  HttpContentLocationParseError, HttpContentRange, HttpContentRangeParseError,
  HttpCookieParseError, HttpCrossOriginEmbedderPolicy, HttpCrossOriginEmbedderPolicyReportOnly,
  HttpCrossOriginOpenerPolicy, HttpCrossOriginOpenerPolicyReportOnly,
  HttpCrossOriginResourcePolicy, HttpDeprecation, HttpDeprecationParseError, HttpDepth,
  HttpDepthParseError, HttpDestination, HttpDestinationParseError, HttpEntityTag, HttpExpectations,
  HttpIdempotencyKey, HttpIdempotencyKeyParseError, HttpIfModifiedSince, HttpIfScheduleTagMatch,
  HttpIfScheduleTagMatchParseError, HttpIfUnmodifiedSince, HttpLockToken, HttpLockTokenParseError,
  HttpMaxForwards, HttpMementoDatetime, HttpMementoDatetimeParseError, HttpNegotiate,
  HttpNegotiateDirective, HttpNegotiateParseError, HttpNel, HttpOriginTrialParseError,
  HttpOriginTrials, HttpOverwrite, HttpPermissionsPolicy, HttpPermissionsPolicyParseError,
  HttpPragma, HttpPragmaParseError, HttpProxyAuthorization, HttpProxyStatus,
  HttpProxyStatusParseError, HttpRequestAcceptCharsets, HttpResponse, HttpSameSite, HttpSaveData,
  HttpScheduleTag, HttpSecGpc, HttpSecGpcParseError, HttpSecWebSocketAccept,
  HttpSecWebSocketAcceptParseError, HttpSecWebSocketExtensions,
  HttpSecWebSocketExtensionsParseError, HttpSecWebSocketKey, HttpSecWebSocketKeyParseError,
  HttpSecWebSocketProtocol, HttpSecWebSocketProtocolParseError, HttpSecWebSocketVersion,
  HttpSecWebSocketVersionParseError, HttpServiceWorkerAllowed, HttpServiceWorkerAllowedParseError,
  HttpSetCookie, HttpSetCookies, HttpSignature, HttpSignatureInput, HttpSignatureInputBareItem,
  HttpSignatureInputComponent, HttpSignatureInputEntry, HttpSignatureInputParameter,
  HttpSignatureInputParseError, HttpSignatureParseError, HttpSpeculationRules,
  HttpSpeculationRulesParseError, HttpSunsetParseError, HttpSupportsLoadingMode,
  HttpSupportsLoadingModeParseError, HttpTcn, HttpTcnDirective, HttpTcnParseError, HttpTimeout,
  HttpTimeoutParseError, HttpTimeoutType, HttpUpgrade, HttpUpgradeInsecureRequests,
  HttpUpgradeInsecureRequestsParseError, HttpUpgradeParseError, HttpVia, HttpViaParseError,
  HttpXForwardedFor, HttpXForwardedForParseError, HttpXForwardedHost, HttpXForwardedHostParseError,
  HttpXForwardedProto, HttpXForwardedProtoParseError,
};
use std::io::Write;
use std::net::SocketAddr;
use std::thread::{self, JoinHandle};
use std::time::{Duration, UNIX_EPOCH};

#[cfg(feature = "client")]
fn header_value<'a>(message: &'a str, name: &str) -> Option<&'a str> {
  message.lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    if header_name.eq_ignore_ascii_case(name) {
      Some(value.trim())
    } else {
      None
    }
  })
}

#[cfg(feature = "client")]
fn spawn_representation_metadata_response_server(
  response: Vec<u8>,
) -> (SocketAddr, JoinHandle<Vec<u8>>) {
  let listener =
    std::net::TcpListener::bind("127.0.0.1:0").expect("representation metadata server should bind");
  let addr = listener
    .local_addr()
    .expect("representation metadata server addr");
  let handle = thread::spawn(move || {
    let Ok((mut stream, _)) = listener.accept() else {
      return Vec::new();
    };
    let request = rttp_test_support::read_http_request(&mut stream);
    stream
      .write_all(&response)
      .expect("representation metadata response should write");
    request
  });
  (addr, handle)
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_exports_client_metadata_types() {
  let dav: rttp::Dav =
    rttp_client::response::Dav::parse("1, 2, extended-mkcol, <https://dav.example.test/ns>")
      .expect("DAV should parse");
  assert_eq!(
    &[
      rttp::DavClass::One,
      rttp::DavClass::Two,
      rttp::DavClass::ExtensionToken("extended-mkcol".to_string()),
      rttp::DavClass::CodedUrl("https://dav.example.test/ns".to_string()),
    ],
    dav.classes()
  );
  let _: rttp::DavParseError =
    rttp_client::response::Dav::parse("1, 1").expect_err("duplicate DAV should fail");
  let accept_ch: rttp::AcceptCh =
    rttp_client::response::AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let allow_credentials: rttp::AccessControlAllowCredentials =
    rttp_client::response::AccessControlAllowCredentials::parse("true")
      .expect("Access-Control-Allow-Credentials should parse");
  let _: rttp::AccessControlAllowCredentialsParseError =
    rttp_client::response::AccessControlAllowCredentials::parse("false")
      .expect_err("invalid Access-Control-Allow-Credentials should fail");
  let client_sec_websocket_key =
    HttpSecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");
  let client_sec_websocket_accept: rttp::SecWebSocketAccept =
    rttp_client::response::SecWebSocketAccept::derive_from_key(&client_sec_websocket_key);
  let _: rttp::SecWebSocketAcceptParseError =
    rttp_client::response::SecWebSocketAccept::parse("the accept value")
      .expect_err("invalid Sec-WebSocket-Accept should fail");
  let client_sec_websocket_extensions: rttp::SecWebSocketExtensions =
    rttp_client::response::SecWebSocketExtensions::parse(
      r#"permessage-deflate; client_max_window_bits; mode="safe""#,
    )
    .expect("Sec-WebSocket-Extensions should parse");
  let _: rttp::SecWebSocketExtensionsParseError =
    rttp_client::response::SecWebSocketExtensions::parse_selection("permessage-deflate, x-test")
      .expect_err("multi-extension Sec-WebSocket-Extensions selection should fail");
  let critical_ch: rttp::CriticalCh =
    rttp_client::response::CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let cache_status: rttp::CacheStatus =
    rttp_client::response::CacheStatus::parse("OriginCache; hit; ttl=1100")
      .expect("Cache-Status should parse");
  let _: rttp::CacheStatusParseError =
    rttp_client::response::CacheStatus::parse("OriginCache; hit=yes")
      .expect_err("invalid Cache-Status should fail");
  let cdn_cache_control: rttp::CdnCacheControl =
    rttp_client::response::CdnCacheControl::parse("max-age=600, cdn-example=\"a, b\"")
      .expect("CDN-Cache-Control should parse");
  let _: rttp::CdnCacheControlParseError =
    rttp_client::response::CdnCacheControl::parse("max-age=")
      .expect_err("invalid CDN-Cache-Control should fail");
  let surrogate_control: rttp::SurrogateControl =
    rttp_client::response::SurrogateControl::parse("max-age=600, content=\"ESI/1.0\"")
      .expect("Surrogate-Control should parse");
  let _: rttp::SurrogateControlParseError =
    rttp_client::response::SurrogateControl::parse("max-age=60, Max-Age=120")
      .expect_err("duplicate Surrogate-Control should fail");
  let accept_patch: rttp::AcceptPatch =
    rttp_client::response::AcceptPatch::parse("application/json")
      .expect("Accept-Patch should parse");
  let accept_post: rttp::AcceptPost =
    rttp_client::response::AcceptPost::parse("application/json").expect("Accept-Post should parse");
  let content_range_window: rttp::ContentRange =
    rttp_client::response::ContentRange::parse("bytes 3-6/10").expect("Content-Range should parse");
  let _: rttp::ContentRangeParseError = rttp_client::response::ContentRange::parse("bytes */*")
    .expect_err("invalid Content-Range should be rejected");
  let accept_ranges: rttp::AcceptRanges =
    rttp_client::response::AcceptRanges::parse("bytes, pages").expect("Accept-Ranges should parse");
  let accept_charset: rttp::AcceptCharset =
    rttp_client::response::AcceptCharset::parse("utf-8, iso-8859-1;q=0.5, *;q=0")
      .expect("Accept-Charset should parse");
  let accept_encoding: rttp::AcceptEncoding =
    rttp_client::response::AcceptEncoding::parse("gzip, br;q=0.8, identity;q=0")
      .expect("Accept-Encoding should parse");
  let content_location: rttp::ContentLocation =
    rttp_client::response::ContentLocation::parse("../representations/current.json")
      .expect("Content-Location should parse");
  let _: rttp::ContentLocationParseError =
    rttp_client::response::ContentLocation::parse("not valid")
      .expect_err("invalid Content-Location should be rejected");
  let service_worker_allowed: rttp::ServiceWorkerAllowed =
    rttp_client::response::ServiceWorkerAllowed::parse("/")
      .expect("Service-Worker-Allowed should parse");
  let _: rttp::ServiceWorkerAllowedParseError =
    rttp_client::response::ServiceWorkerAllowed::parse("http://example.test/scope")
      .expect_err("absolute URI Service-Worker-Allowed should be rejected");
  let content_dpr: rttp::ContentDpr =
    rttp_client::response::ContentDpr::parse("1.5").expect("Content-DPR should parse");
  let _: rttp::ContentDprParseError =
    rttp_client::response::ContentDpr::parse("0").expect_err("zero Content-DPR should be rejected");
  let deprecation: rttp::Deprecation =
    rttp_client::response::Deprecation::parse("?1").expect("Deprecation should parse");
  let _: rttp::DeprecationParseError = rttp_client::response::Deprecation::parse("true")
    .expect_err("historical Deprecation token should be rejected");
  let destination: rttp::Destination =
    rttp::Destination::parse("https://dav.example.test/archive/report.txt")
      .expect("Destination should parse");
  let _: rttp::DestinationParseError =
    rttp::Destination::parse("/relative").expect_err("relative Destination should be rejected");
  let depth: rttp::Depth = rttp::Depth::parse("infinity").expect("Depth should parse");
  let _: rttp::DepthParseError =
    rttp::Depth::parse("2").expect_err("malformed Depth should be rejected");
  let lock_token: rttp::LockToken =
    rttp::LockToken::parse("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
      .expect("Lock-Token should parse");
  let _: rttp::LockTokenParseError =
    rttp::LockToken::parse("<relative>").expect_err("malformed Lock-Token should be rejected");
  let timeout: rttp::Timeout =
    rttp::Timeout::parse("Second-60, Infinite").expect("Timeout should parse");
  let _: rttp::TimeoutParseError =
    rttp::Timeout::parse("Second-60, second-60").expect_err("duplicate Timeout should be rejected");
  let if_schedule_tag_match: rttp::IfScheduleTagMatch =
    rttp::IfScheduleTagMatch::parse("\"sched-17\"").expect("If-Schedule-Tag-Match should parse");
  let _: rttp::IfScheduleTagMatchParseError =
    rttp::IfScheduleTagMatch::parse("*").expect_err("wildcard If-Schedule-Tag-Match should fail");
  let overwrite: rttp::Overwrite = rttp::Overwrite::parse("F").expect("Overwrite should parse");
  let _: rttp::OverwriteParseError =
    rttp::Overwrite::parse("t").expect_err("lowercase Overwrite should be rejected");
  let x_forwarded_for: rttp::XForwardedFor =
    rttp::XForwardedFor::parse("192.0.2.60, unknown").expect("X-Forwarded-For should parse");
  let _: rttp::XForwardedForParseError =
    rttp::XForwardedFor::parse("client.example").expect_err("invalid X-Forwarded-For should fail");
  let x_forwarded_host: rttp::XForwardedHost =
    rttp::XForwardedHost::parse("example.test:443").expect("X-Forwarded-Host should parse");
  let _: rttp::XForwardedHostParseError = rttp::XForwardedHost::parse("https://example.test")
    .expect_err("invalid X-Forwarded-Host should fail");
  let x_forwarded_proto: rttp::XForwardedProto =
    rttp::XForwardedProto::parse("https").expect("X-Forwarded-Proto should parse");
  let _: rttp::XForwardedProtoParseError =
    rttp::XForwardedProto::parse("https://").expect_err("invalid X-Forwarded-Proto should fail");
  let via: rttp::Via =
    rttp::Via::parse("1.1 edge-a (TLS terminator), HTTP/2 upstream").expect("Via should parse");
  let _: rttp::ViaParseError =
    rttp::Via::parse("1.1").expect_err("incomplete Via hop should be rejected");
  let memento_datetime: rttp::MementoDatetime =
    rttp_client::response::MementoDatetime::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("Memento-Datetime should parse");
  let _: rttp::MementoDatetimeParseError = rttp_client::response::MementoDatetime::parse("")
    .expect_err("empty Memento-Datetime should be rejected");
  let content_security_policy: rttp::ContentSecurityPolicy =
    rttp_client::response::ContentSecurityPolicy::parse("default-src 'self'; object-src 'none'")
      .expect("Content-Security-Policy should parse");
  let _: rttp::ContentSecurityPolicyParseError =
    rttp_client::response::ContentSecurityPolicy::parse("")
      .expect_err("empty Content-Security-Policy should be rejected");
  let content_security_policy_report_only: rttp::ContentSecurityPolicyReportOnly =
    rttp_client::response::ContentSecurityPolicyReportOnly::parse(
      "default-src 'self'; report-to csp-endpoint",
    )
    .expect("Content-Security-Policy-Report-Only should parse");
  let _: rttp::ContentSecurityPolicyReportOnlyParseError =
    rttp_client::response::ContentSecurityPolicyReportOnly::parse("")
      .expect_err("empty Content-Security-Policy-Report-Only should be rejected");
  let content_range: rttp::ContentRange =
    rttp_client::response::ContentRange::parse("bytes 0-4/10").expect("Content-Range should parse");
  let alternates: rttp::Alternates = rttp_client::response::Alternates::parse(
    r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }"#,
  )
  .expect("Alternates should parse");
  let _: rttp::AlternatesParseError =
    rttp_client::response::Alternates::parse(r#"{ "/broken" 1.001 }"#)
      .expect_err("invalid Alternates should fail");
  let _variant: &rttp::AlternateVariant = &alternates.variants()[0];
  let _attribute: &rttp::AlternateAttribute = &alternates.variants()[0].attributes()[0];
  let alt_svc: rttp::AltSvc =
    rttp_client::response::AltSvc::parse("h3=\":443\"; ma=60").expect("Alt-Svc should parse");
  let alt_used: rttp::AltUsed =
    rttp_client::response::AltUsed::parse("alt.example:8443").expect("Alt-Used should parse");
  let _: rttp::AltUsedParseError = rttp_client::response::AltUsed::parse("https://alt.example")
    .expect_err("invalid Alt-Used should be rejected");
  let origin_trials: rttp::OriginTrials =
    rttp_client::response::OriginTrials::parse_values(["token-one", "token-two"])
      .expect("Origin-Trial should parse");
  let _: rttp::OriginTrialParseError =
    rttp_client::response::OriginTrials::parse("token\r\nX-Injected: 1")
      .expect_err("injected Origin-Trial should be rejected");
  let speculation_rules: rttp::SpeculationRules =
    rttp_client::response::SpeculationRules::parse("https://example.test/speculation-rules.json")
      .expect("Speculation-Rules should parse");
  let _: rttp::SpeculationRulesParseError = rttp_client::response::SpeculationRules::parse(
    "https://example.test/rules.json\r\nX-Injected: 1",
  )
  .expect_err("injected Speculation-Rules should be rejected");
  let authentication_info: rttp::AuthenticationInfo =
    rttp_client::response::AuthenticationInfo::parse("nextnonce=\"n-2\"")
      .expect("Authentication-Info should parse");
  let _: rttp::AuthenticationInfoParseError = rttp_client::response::AuthenticationInfo::parse("")
    .expect_err("empty Authentication-Info should be rejected");
  let no_vary_search: rttp::NoVarySearch =
    rttp_client::response::NoVarySearch::parse(r#"params=("utm_source")"#)
      .expect("No-Vary-Search should parse");
  let _: rttp::AltSvcParseError =
    rttp_client::response::AltSvc::parse("h3=:443").expect_err("invalid Alt-Svc should fail");
  let nel: rttp::Nel =
    rttp_client::response::Nel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#)
      .expect("NEL should parse");
  let proxy_status =
    rttp_client::response::ProxyStatus::parse("ExampleCDN; error=connection_timeout")
      .expect("Proxy-Status should parse");
  let _: rttp_client::response::ProxyStatusParseError =
    rttp_client::response::ProxyStatus::parse("")
      .expect_err("empty Proxy-Status should be rejected");
  let embedder_policy: rttp::CrossOriginEmbedderPolicy =
    rttp_client::response::CrossOriginEmbedderPolicy::parse("require-corp; report-to=\"coep\"")
      .expect("Cross-Origin-Embedder-Policy should parse");
  let embedder_policy_report_only: rttp::CrossOriginEmbedderPolicyReportOnly =
    rttp_client::response::CrossOriginEmbedderPolicyReportOnly::parse(
      "require-corp; report-to=\"coep\"",
    )
    .expect("Cross-Origin-Embedder-Policy-Report-Only should parse");
  let opener_policy: rttp::CrossOriginOpenerPolicy =
    rttp_client::response::CrossOriginOpenerPolicy::parse(
      "noopener-allow-popups; report-to=\"coop\"",
    )
    .expect("Cross-Origin-Opener-Policy should parse");
  let opener_policy_report_only: rttp::CrossOriginOpenerPolicyReportOnly =
    rttp_client::response::CrossOriginOpenerPolicyReportOnly::parse(
      "same-origin; report-to=\"coop\"",
    )
    .expect("Cross-Origin-Opener-Policy-Report-Only should parse");
  let _: rttp::CrossOriginOpenerPolicyReportOnlyParseError =
    rttp_client::response::CrossOriginOpenerPolicyReportOnly::parse("same origin")
      .expect_err("malformed Cross-Origin-Opener-Policy-Report-Only should be rejected");
  let strict_transport_security: rttp::StrictTransportSecurity =
    rttp_client::response::StrictTransportSecurity::parse("max-age=31536000; includeSubDomains")
      .expect("Strict-Transport-Security should parse");
  let _: rttp::StrictTransportSecurityParseError =
    rttp_client::response::StrictTransportSecurity::parse("includeSubDomains")
      .expect_err("Strict-Transport-Security without max-age should be rejected");
  let www_authenticate: rttp::WwwAuthenticate =
    rttp_client::response::WwwAuthenticate::parse("Basic realm=\"users\"")
      .expect("WWW-Authenticate should parse");
  let _: rttp::WwwAuthenticateParseError =
    rttp_client::response::WwwAuthenticate::parse("Basic realm=\"")
      .expect_err("malformed WWW-Authenticate should be rejected");
  let upgrade: rttp::Upgrade =
    rttp_client::response::Upgrade::parse("websocket").expect("Upgrade should parse");
  let _: rttp::UpgradeParseError =
    rttp_client::response::Upgrade::parse("").expect_err("empty Upgrade should fail");
  let pragma: rttp::Pragma = rttp_client::response::Pragma::parse("no-cache, community=private")
    .expect("Pragma should parse");
  let _: rttp::PragmaParseError = rttp_client::response::Pragma::parse("no-cache, no-cache")
    .expect_err("duplicate Pragma directives should be rejected");
  let x_content_type_options: rttp::XContentTypeOptions =
    rttp_client::response::XContentTypeOptions::parse("NoSniff")
      .expect("X-Content-Type-Options should parse");
  let _: rttp::XContentTypeOptionsParseError =
    rttp_client::response::XContentTypeOptions::parse("unknown")
      .expect_err("unknown X-Content-Type-Options should be rejected");
  let x_frame_options: rttp::XFrameOptions =
    rttp_client::response::XFrameOptions::parse("deny").expect("X-Frame-Options should parse");
  let _: rttp::XFrameOptionsParseError =
    rttp_client::response::XFrameOptions::parse("ALLOW-FROM https://example.test")
      .expect_err("deprecated X-Frame-Options ALLOW-FROM should be rejected");
  let permissions_policy: rttp::PermissionsPolicy =
    rttp_client::response::PermissionsPolicy::parse(
      r#"geolocation=(self "https://maps.example.test"), camera=()"#,
    )
    .expect("Permissions-Policy should parse");
  let _: rttp::PermissionsPolicyParseError =
    rttp_client::response::PermissionsPolicy::parse("geolocation=src")
      .expect_err("src should be rejected");
  let document_policy: rttp::DocumentPolicy =
    rttp_client::response::DocumentPolicy::parse("oversized-images=2.0, unsized-media=?0")
      .expect("Document-Policy should parse");
  let _: rttp::DocumentPolicyParseError =
    rttp_client::response::DocumentPolicy::parse("unsized-media=src;foo=bar")
      .expect_err("unknown Document-Policy parameter should be rejected");
  let document_policy_report_only: rttp::DocumentPolicyReportOnly =
    rttp_client::response::DocumentPolicyReportOnly::parse(
      "oversized-images=2.0, unsized-media=?0",
    )
    .expect("Document-Policy-Report-Only should parse");
  let _: rttp::DocumentPolicyReportOnlyParseError =
    rttp_client::response::DocumentPolicyReportOnly::parse("unsized-media=src;foo=bar")
      .expect_err("unknown Document-Policy-Report-Only parameter should be rejected");
  let supports_loading_mode: rttp::SupportsLoadingMode =
    rttp_client::response::SupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
      .expect("Supports-Loading-Mode should parse");
  let _: rttp::SupportsLoadingModeParseError =
    rttp_client::response::SupportsLoadingMode::parse("?1")
      .expect_err("non-token should be rejected");
  let sec_websocket_version: rttp::SecWebSocketVersion =
    rttp_client::response::SecWebSocketVersion::parse("13")
      .expect("Sec-WebSocket-Version should parse");
  let _: rttp::SecWebSocketVersionParseError =
    rttp_client::response::SecWebSocketVersion::parse("8, 13")
      .expect_err("unordered Sec-WebSocket-Version should be rejected");
  let sec_websocket_protocol: rttp::SecWebSocketProtocol =
    rttp_client::response::SecWebSocketProtocol::parse("chat, superchat")
      .expect("Sec-WebSocket-Protocol offers should parse");
  let _: rttp::SecWebSocketProtocolParseError =
    rttp_client::response::SecWebSocketProtocol::parse_selection("chat, superchat")
      .expect_err("multi-token Sec-WebSocket-Protocol selection should be rejected");
  let sec_websocket_protocol_selection: rttp::SecWebSocketProtocol =
    rttp_client::response::SecWebSocketProtocol::from_selection("graphql-ws")
      .expect("Sec-WebSocket-Protocol should select");
  let fetch_site: rttp::SecFetchSite =
    rttp_client::SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let sec_purpose: rttp::SecPurpose =
    rttp_client::SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let a_im: rttp::AIm =
    rttp::AIm::parse("diffe, gzip;q=0.3;profile=compact").expect("A-IM should parse");
  let _: rttp::AImParseError =
    rttp::AIm::parse("diffe, DIFFE").expect_err("duplicate A-IM should be rejected");
  let _: &rttp::AImMember = &a_im.members()[0];
  let _: Option<&rttp::AImParameter> = a_im.members()[1].parameters().first();
  let negotiate: rttp::Negotiate =
    rttp::Negotiate::parse("trans, 1.0, feature-x=preview, *").expect("Negotiate should parse");
  let _: rttp::NegotiateParseError =
    rttp::Negotiate::parse("trans, TRANS").expect_err("duplicate Negotiate should be rejected");
  let _: &rttp::NegotiateDirective = &negotiate.members()[0];
  let tcn: rttp::Tcn = rttp::Tcn::parse("list, choice").expect("TCN should parse");
  let _: rttp::TcnParseError =
    rttp::Tcn::parse("list, LIST").expect_err("duplicate TCN should be rejected");
  let _: &rttp::TcnDirective = &tcn.members()[0];
  let set_cookie: rttp::HttpSetCookie =
    rttp::HttpSetCookie::parse(r#"session="abc def"; Path=/; SameSite=Lax; Foo=bar"#)
      .expect("Set-Cookie should parse");
  let _: rttp::HttpSameSite = set_cookie.same_site().expect("SameSite should parse");
  let _: rttp::HttpSetCookies =
    rttp::HttpSetCookies::parse_values([set_cookie.header_value().as_str()])
      .expect("Set-Cookie collection should parse");
  let _: rttp::HttpCookieParseError =
    rttp::HttpSetCookie::parse("session=abc; Path=/; path=/other")
      .expect_err("duplicate Set-Cookie attributes should be rejected");
  let baggage: rttp::Baggage =
    rttp_client::Baggage::parse("tenant=acme;source=gateway").expect("baggage should parse");
  let _: rttp::BaggageParseError = rttp_client::Baggage::parse("tenant=1,tenant=2")
    .expect_err("duplicate baggage should be rejected");
  let baggage_member: &rttp::BaggageMember = &baggage.members()[0];
  let baggage_property: &rttp::BaggageProperty = &baggage_member.properties()[0];
  let etag: rttp::EntityTag =
    rttp_client::response::EntityTag::parse("\"asset-v7\"").expect("ETag should parse");
  let schedule_tag: rttp::ScheduleTag =
    rttp_client::response::ScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse");
  let location: rttp::Location =
    rttp_client::response::Location::parse("/next").expect("Location should parse");
  let _: rttp::LocationParseError =
    rttp_client::response::Location::parse("").expect_err("empty Location should be rejected");
  let content_length = rttp::HttpContentLength::new(123);

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(
    client_sec_websocket_accept.as_str(),
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
  );
  assert_eq!(schedule_tag.header_value(), "\"sched-17\"");
  assert_eq!(
    client_sec_websocket_extensions.header_value(),
    r#"permessage-deflate; client_max_window_bits; mode="safe""#
  );
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(
    cache_status.members()[0].identifier().as_str(),
    "OriginCache"
  );
  assert_eq!(cache_status.members()[0].ttl(), Some(1100));
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
  assert_eq!(surrogate_control.directives()[1].value(), Some("ESI/1.0"));
  assert_eq!(accept_patch.media_types().len(), 1);
  assert_eq!(accept_post.media_types().len(), 1);
  assert_eq!(content_range_window.header_value(), "bytes 3-6/10");
  assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages");
  assert_eq!(
    accept_charset.header_value(),
    "utf-8, iso-8859-1;q=0.5, *;q=0"
  );
  assert_eq!(
    accept_encoding.header_value(),
    "gzip, br;q=0.8, identity;q=0"
  );
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3;profile=compact");
  assert_eq!(negotiate.members()[0], rttp::NegotiateDirective::Trans);
  assert_eq!(negotiate.members()[3], rttp::NegotiateDirective::Any);
  assert_eq!("trans, 1.0, feature-x=preview, *", negotiate.header_value());
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(service_worker_allowed.header_value(), "/");
  assert_eq!(service_worker_allowed.as_str(), "/");
  assert_eq!(content_dpr.ratio(), 1.5);
  assert_eq!(content_dpr.header_value(), "1.5");
  assert_eq!(deprecation, rttp::Deprecation::Boolean(true));
  assert_eq!(deprecation.header_value(), "?1");
  assert_eq!(
    destination.as_str(),
    "https://dav.example.test/archive/report.txt"
  );
  assert_eq!(
    destination.header_value(),
    "https://dav.example.test/archive/report.txt"
  );
  assert_eq!(rttp::Depth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.as_str()
  );
  assert!(!format!("{lock_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!(
    &[rttp::TimeoutType::Second(60), rttp::TimeoutType::Infinite],
    timeout.members()
  );
  assert_eq!("second-60, infinite", timeout.header_value());
  assert_eq!(rttp::Overwrite::F, overwrite);
  assert_eq!("F", overwrite.header_value());
  assert_eq!(
    if_schedule_tag_match.entity_tag().header_value(),
    "\"sched-17\""
  );
  assert_eq!(if_schedule_tag_match.opaque_tag(), "sched-17");
  assert!(!if_schedule_tag_match.is_weak());
  assert_eq!(if_schedule_tag_match.header_value(), "\"sched-17\"");
  assert_eq!(
    memento_datetime.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    content_security_policy.header_value(),
    "default-src 'self'; object-src 'none'"
  );
  assert_eq!(
    content_security_policy_report_only.header_value(),
    "default-src 'self'; report-to csp-endpoint"
  );
  assert_eq!("bytes", content_range.unit());
  assert_eq!(Some(0), content_range.start());
  assert_eq!(Some(4), content_range.end());
  assert_eq!(Some(10), content_range.complete_length());
  assert!(!content_range.is_unsatisfied());
  assert_eq!(alternates.variants()[0].uri(), "/resource.en.html");
  assert_eq!(alternates.variants()[0].quality(), "1.0");
  assert_eq!(
    alternates.variants()[0].attribute("type"),
    Some("text/html")
  );
  assert_eq!(alt_svc.alternatives()[0].protocol_id(), "h3");
  assert_eq!(alt_svc.alternatives()[0].max_age(), Some(60));
  assert_eq!(alt_used.host(), "alt.example");
  assert_eq!(alt_used.port(), Some("8443"));
  assert_eq!(origin_trials.tokens(), ["token-one", "token-two"]);
  assert!(!format!("{origin_trials:?}").contains("token-one"));
  assert_eq!(
    speculation_rules.header_value(),
    "https://example.test/speculation-rules.json"
  );
  assert!(!format!("{speculation_rules:?}").contains("speculation-rules.json"));
  assert_eq!(authentication_info.parameter("nextnonce"), Some("n-2"));
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(
    proxy_status.members()[0].identifier().as_str(),
    "ExampleCDN"
  );
  assert_eq!(
    no_vary_search.params(),
    Some(&rttp::NoVarySearchParams::Names(vec![
      "utm_source".to_owned()
    ]))
  );
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(embedder_policy_report_only.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(
    rttp::CrossOriginOpenerPolicy::SameOrigin,
    opener_policy_report_only.policy()
  );
  assert_eq!(Some("coop"), opener_policy_report_only.report_to());
  assert_eq!(
    opener_policy_report_only.header_value(),
    r#"same-origin; report-to="coop""#
  );
  assert_eq!(strict_transport_security.max_age(), 31_536_000);
  assert!(strict_transport_security.include_sub_domains());
  assert_eq!(
    www_authenticate.challenges()[0].parameter("realm"),
    Some("users")
  );
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert!(pragma.no_cache());
  assert_eq!("community", pragma.extensions()[0].name());
  assert_eq!(Some("private"), pragma.extensions()[0].value());
  assert_eq!("no-cache, community=private", pragma.header_value());
  assert_eq!(x_content_type_options, rttp::XContentTypeOptions::Nosniff);
  assert_eq!(x_content_type_options.header_value(), "nosniff");
  assert_eq!(x_frame_options, rttp::XFrameOptions::Deny);
  assert_eq!(x_frame_options.header_value(), "DENY");
  assert_eq!(
    permissions_policy.header_value(),
    r#"geolocation=(self "https://maps.example.test"), camera=()"#
  );
  assert_eq!(permissions_policy.directives().len(), 2);
  assert!(permissions_policy
    .directive("camera")
    .unwrap()
    .allowlist()
    .is_empty());
  assert_eq!(document_policy.directives().len(), 2);
  assert_eq!(
    document_policy.header_value(),
    "oversized-images=2.0, unsized-media=?0"
  );
  assert_eq!(
    document_policy
      .directive("oversized-images")
      .unwrap()
      .value(),
    &rttp::DocumentPolicyValue::Decimal("2.0".to_string())
  );
  assert_eq!(document_policy_report_only.directives().len(), 2);
  assert_eq!(
    document_policy_report_only
      .directive("oversized-images")
      .unwrap()
      .value(),
    &rttp::DocumentPolicyReportOnlyValue::Decimal("2.0".to_string())
  );
  assert_eq!(
    supports_loading_mode.tokens(),
    ["fenced-frame", "credentialed-prerender"]
  );
  assert!(supports_loading_mode.contains_fenced_frame());
  assert!(supports_loading_mode.contains_credentialed_prerender());
  assert_eq!(
    supports_loading_mode.header_value(),
    "fenced-frame, credentialed-prerender"
  );
  assert_eq!(sec_websocket_version.versions(), ["13"]);
  assert!(sec_websocket_version.contains("13"));
  assert_eq!(sec_websocket_version.header_value(), "13");
  assert_eq!(sec_websocket_protocol.protocols(), ["chat", "superchat"]);
  assert!(sec_websocket_protocol.contains("chat"));
  assert_eq!(sec_websocket_protocol.header_value(), "chat, superchat");
  assert_eq!(
    sec_websocket_protocol_selection.selected(),
    Some("graphql-ws")
  );
  assert_eq!("tenant", baggage_member.key());
  assert_eq!("source", baggage_property.key());
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(sec_purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(sec_purpose.contains_prefetch());
  assert_eq!(etag, rttp::EntityTag::strong("asset-v7"));
  assert_eq!(location.as_str(), "/next");
  assert_eq!(content_length.len(), 123);
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_exports_content_length_metadata_type() {
  let content_length: rttp::HttpContentLength = rttp::HttpContentLength::new(2);

  assert_eq!(2, content_length.len());
  assert_eq!("2", content_length.header_value());
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_representation_metadata_matrix() {
  let server_response = HttpResponse::ok(r#"{"ok":true}"#)
    .header("Content-Type", "text/plain")
    .header("Content-Encoding", "gzip")
    .header("Content-Digest", "sha-512=:b2xk:")
    .header("Repr-Digest", "sha-256=:b2xk:")
    .with_content_type("application/json; charset=utf-8")
    .expect("Content-Type should be accepted")
    .with_content_encoding(["identity"])
    .expect("Content-Encoding should be accepted")
    .with_content_language(["en-US"])
    .expect("Content-Language should be accepted")
    .with_content_location("/representations/asset.json")
    .expect("Content-Location should be accepted")
    .with_service_worker_allowed("/")
    .expect("Service-Worker-Allowed should be accepted")
    .with_digest("sha-256=:YWJj:")
    .expect("Content-Digest should be accepted")
    .with_repr_digest("sha-512=:ZGVm:")
    .expect("Repr-Digest should be accepted")
    .header("Content-Range", "bytes 0-10/11");

  assert_eq!(
    server_response
      .content_type()
      .expect("server Content-Type should parse")
      .expect("server Content-Type should be present")
      .header_value(),
    "application/json; charset=utf-8"
  );
  assert_eq!(
    server_response
      .content_encoding()
      .expect("server Content-Encoding should parse")
      .expect("server Content-Encoding should be present")
      .codings(),
    ["identity"]
  );
  assert_eq!(
    server_response
      .content_language()
      .expect("server Content-Language should parse")
      .expect("server Content-Language should be present")
      .tags(),
    ["en-US"]
  );
  assert_eq!(
    server_response
      .content_location()
      .expect("server Content-Location should parse")
      .expect("server Content-Location should be present")
      .header_value(),
    "/representations/asset.json"
  );
  assert_eq!(
    server_response
      .service_worker_allowed()
      .expect("server Service-Worker-Allowed should parse")
      .expect("server Service-Worker-Allowed should be present")
      .header_value(),
    "/"
  );
  assert_eq!(
    server_response
      .digest()
      .expect("server Content-Digest should parse")
      .expect("server Content-Digest should be present")
      .entry("sha-256")
      .map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    server_response
      .repr_digest()
      .expect("server Repr-Digest should parse")
      .expect("server Repr-Digest should be present")
      .entry("sha-512")
      .map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(
    server_response
      .content_range()
      .expect("server Content-Range should parse"),
    Some(HttpContentRange::Bytes {
      start: 0,
      end: 10,
      complete_length: Some(11),
    })
  );

  let mut serialized_response = Vec::new();
  server_response
    .write_to(&mut serialized_response)
    .expect("server response should serialize");
  let response_text =
    String::from_utf8(serialized_response.clone()).expect("server response should be utf-8");
  assert_eq!(
    Some("application/json; charset=utf-8"),
    header_value(&response_text, "Content-Type")
  );
  assert_eq!(
    Some("identity"),
    header_value(&response_text, "Content-Encoding")
  );
  assert_eq!(
    Some("en-US"),
    header_value(&response_text, "Content-Language")
  );
  assert_eq!(
    Some("/representations/asset.json"),
    header_value(&response_text, "Content-Location")
  );
  assert_eq!(
    Some("/"),
    header_value(&response_text, "Service-Worker-Allowed")
  );
  assert_eq!(
    Some("sha-256=:YWJj:"),
    header_value(&response_text, "Content-Digest")
  );
  assert_eq!(
    Some("sha-512=:ZGVm:"),
    header_value(&response_text, "Repr-Digest")
  );
  assert_eq!(1, response_text.matches("\r\nContent-Encoding: ").count());
  assert_eq!(1, response_text.matches("\r\nContent-Digest: ").count());
  assert_eq!(1, response_text.matches("\r\nRepr-Digest: ").count());

  let (addr, handle) = spawn_representation_metadata_response_server(serialized_response);
  let client_response = rttp::Http::client()
    .post()
    .url(format!("http://{addr}/asset"))
    .content_type("application/json; charset=utf-8")
    .header(("Content-Encoding", "identity"))
    .header(("Content-Language", "en-US"))
    .want_content_digest("sha-256")
    .expect("Want-Content-Digest algorithm should be accepted")
    .want_content_digest_with_q("sha-512", "8")
    .expect("Want-Content-Digest preference should be accepted")
    .want_repr_digest("sha-256")
    .expect("Want-Repr-Digest algorithm should be accepted")
    .want_repr_digest_with_q("sha-512", "0")
    .expect("Want-Repr-Digest preference should be accepted")
    .sec_gpc()
    .expect("Sec-GPC should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("representation metadata server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("sha-256=10, sha-512=8"),
    header_value(&captured_request_text, "Want-Content-Digest")
  );
  assert_eq!(
    Some("sha-256=10, sha-512=0"),
    header_value(&captured_request_text, "Want-Repr-Digest")
  );
  assert_eq!(
    Some("application/json; charset=utf-8"),
    header_value(&captured_request_text, "Content-Type")
  );
  assert_eq!(
    Some("identity"),
    header_value(&captured_request_text, "Content-Encoding")
  );
  assert_eq!(
    Some("en-US"),
    header_value(&captured_request_text, "Content-Language")
  );
  assert_eq!(Some("1"), header_value(&captured_request_text, "Sec-GPC"));

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let want_content_digest: rttp::WantContentDigest = server_request
    .want_content_digest()
    .expect("server Want-Content-Digest should parse")
    .expect("server Want-Content-Digest should be present");
  let want_repr_digest: rttp::WantReprDigest = server_request
    .want_repr_digest()
    .expect("server Want-Repr-Digest should parse")
    .expect("server Want-Repr-Digest should be present");
  assert_eq!(want_content_digest.header_value(), "sha-256=10, sha-512=8");
  assert_eq!(want_repr_digest.header_value(), "sha-256=10, sha-512=0");
  assert_eq!(
    server_request
      .sec_gpc()
      .expect("server Sec-GPC should parse")
      .expect("server Sec-GPC should be present")
      .header_value(),
    "1"
  );
  assert_eq!(
    server_request
      .content_type()
      .expect("request Content-Type should parse")
      .expect("request Content-Type should be present")
      .header_value(),
    "application/json; charset=utf-8"
  );
  assert_eq!(
    server_request
      .content_encoding()
      .expect("request Content-Encoding should parse")
      .expect("request Content-Encoding should be present")
      .codings(),
    ["identity"]
  );
  assert_eq!(
    server_request
      .content_language()
      .expect("request Content-Language should parse")
      .expect("request Content-Language should be present")
      .tags(),
    ["en-US"]
  );

  let content_type: rttp::ContentType = client_response
    .content_type()
    .expect("client Content-Type should parse")
    .expect("client Content-Type should be present");
  let content_encoding: rttp::ContentEncoding = client_response
    .content_encoding()
    .expect("client Content-Encoding should parse")
    .expect("client Content-Encoding should be present");
  let content_language: rttp::ContentLanguage = client_response
    .content_language()
    .expect("client Content-Language should parse")
    .expect("client Content-Language should be present");
  let content_digest: rttp::ContentDigest = client_response
    .content_digest()
    .expect("client Content-Digest should parse")
    .expect("client Content-Digest should be present");
  let repr_digest: rttp::ReprDigest = client_response
    .repr_digest()
    .expect("client Repr-Digest should parse")
    .expect("client Repr-Digest should be present");

  assert_eq!(content_type.essence(), "application/json");
  assert_eq!(content_type.parameter("charset"), Some("utf-8"));
  assert_eq!(content_encoding.codings(), ["identity"]);
  assert_eq!(content_language.tags(), ["en-US"]);
  assert_eq!(
    client_response
      .content_location()
      .expect("client Content-Location should parse")
      .expect("client Content-Location should be present")
      .header_value(),
    "/representations/asset.json"
  );
  assert_eq!(
    client_response
      .service_worker_allowed()
      .expect("client Service-Worker-Allowed should parse")
      .expect("client Service-Worker-Allowed should be present")
      .header_value(),
    "/"
  );
  assert_eq!(
    content_digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"abc"[..])
  );
  assert_eq!(
    repr_digest.entry("sha-512").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
  assert_eq!(
    client_response
      .content_range()
      .expect("client Content-Range should parse"),
    Some(rttp::ContentRange::Bytes {
      start: 0,
      end: 10,
      complete_length: Some(11),
    })
  );
}

#[test]
#[cfg(feature = "client")]
fn client_accept_charset_helpers_parse_through_shared_server_type() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset"))
    .accept_charset("utf-8")
    .expect("utf-8 should be accepted")
    .accept_charset_with_q("iso-8859-1", "0.5")
    .expect("iso-8859-1 quality should be accepted")
    .accept_charset_with_q("*", "0")
    .expect("wildcard quality should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("Accept-Charset capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("utf-8, iso-8859-1;q=0.5, *;q=0"),
    header_value(&captured_request_text, "Accept-Charset")
  );

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let charsets: rttp::AcceptCharset = server_request
    .accept_charset()
    .expect("server Accept-Charset should parse")
    .expect("server Accept-Charset should be present");

  assert_eq!(charsets.len(), 3);
  assert_eq!(charsets.charsets()[0].charset(), "utf-8");
  assert_eq!(charsets.charsets()[0].quality(), 1000);
  assert_eq!(charsets.charsets()[1].charset(), "iso-8859-1");
  assert_eq!(charsets.charsets()[1].quality(), 500);
  assert_eq!(charsets.charsets()[2].charset(), "*");
  assert_eq!(charsets.charsets()[2].quality(), 0);
  assert_eq!(charsets.header_value(), "utf-8, iso-8859-1;q=0.5, *;q=0");

  let malformed = rttp::server::HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Charset: utf-8, UTF-8\r\n\r\n",
  )
  .expect("malformed Accept-Charset request should still parse");
  assert_eq!(malformed.header("Accept-Charset"), Some("utf-8, UTF-8"));
  assert!(
    malformed.accept_charset().is_err(),
    "duplicate Accept-Charset members must fail closed"
  );

  assert!(
    rttp::AcceptCharset::parse("utf-8".repeat(64 * 1024 + 1)).is_err(),
    "oversized Accept-Charset values must fail closed"
  );
  let too_many = (0..33)
    .map(|index| format!("charset{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::server::HttpRequestAcceptCharsets::parse(too_many).is_err(),
    "more than 32 Accept-Charset members must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_rejects_invalid_sec_gpc_request_metadata() {
  let malformed = rttp::server::HttpRequest::parse(
    b"GET /privacy HTTP/1.1\r\nHost: example.test\r\nSec-GPC: 0\r\n\r\n",
  )
  .expect("malformed Sec-GPC request should still parse");
  assert!(
    malformed.sec_gpc().is_err(),
    "malformed Sec-GPC values must fail closed"
  );

  let duplicate = rttp::server::HttpRequest::parse(
    b"GET /privacy HTTP/1.1\r\nHost: example.test\r\nSec-GPC: 1\r\nsec-gpc: 1\r\n\r\n",
  )
  .expect("duplicate Sec-GPC request should still parse");
  assert!(
    duplicate.sec_gpc().is_err(),
    "duplicate Sec-GPC fields must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_depth_request_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 207 Multi-Status\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  let response = rttp::Http::client()
    .method("PROPFIND")
    .url(format!("http://{addr}/collection"))
    .depth("INFINITY")
    .expect("Depth should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle.join().expect("Depth capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("infinity"),
    header_value(&captured_request_text, "Depth")
  );
  assert_eq!(207, response.code());

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let depth: HttpDepth = server_request
    .depth()
    .expect("server Depth should parse")
    .expect("server Depth should be present");

  assert_eq!(HttpDepth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());

  let malformed = rttp::server::HttpRequest::parse(
    b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\nDepth: 2\r\n\r\n",
  )
  .expect("malformed Depth request should still parse");
  assert!(malformed.depth().is_err());
  assert_eq!(Some("2"), malformed.header("Depth"));

  let duplicate = rttp::server::HttpRequest::parse(
    b"PROPFIND /collection HTTP/1.1\r\nHost: example.test\r\nDepth: 0\r\ndepth: 1\r\n\r\n",
  )
  .expect("duplicate Depth request should still parse");
  assert!(duplicate.depth().is_err());
  assert_eq!(Some("0"), duplicate.header("Depth"));

  assert!(
    rttp::Depth::parse("0".repeat(64 * 1024 + 1)).is_err(),
    "oversized Depth values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_timeout_request_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  let response = rttp::Http::client()
    .method("LOCK")
    .url(format!("http://{addr}/collection"))
    .timeout("Second-60, Infinite")
    .expect("Timeout should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle.join().expect("Timeout capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("second-60, infinite"),
    header_value(&captured_request_text, "Timeout")
  );
  assert_eq!(200, response.code());

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let timeout: HttpTimeout = server_request
    .timeout()
    .expect("server Timeout should parse")
    .expect("server Timeout should be present");

  assert_eq!(
    &[HttpTimeoutType::Second(60), HttpTimeoutType::Infinite],
    timeout.members()
  );
  assert_eq!("second-60, infinite", timeout.header_value());

  let malformed = rttp::server::HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-\r\n\r\n",
  )
  .expect("malformed Timeout request should still parse");
  assert!(malformed.timeout().is_err());
  assert_eq!(Some("Second-"), malformed.header("Timeout"));

  let overflow = rttp::server::HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-18446744073709551616\r\n\r\n",
  )
  .expect("overflow Timeout request should still parse");
  assert!(overflow.timeout().is_err());

  let duplicate = rttp::server::HttpRequest::parse(
    b"LOCK /collection HTTP/1.1\r\nHost: example.test\r\nTimeout: Second-60\r\ntimeout: second-60\r\n\r\n",
  )
  .expect("duplicate Timeout request should still parse");
  assert!(duplicate.timeout().is_err());
  assert_eq!(Some("Second-60"), duplicate.header("Timeout"));

  assert!(
    rttp::Timeout::parse(format!("{}Second-1", " ".repeat(64 * 1024 + 1))).is_err(),
    "oversized Timeout values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_destination_request_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  let response = rttp::Http::client()
    .method("COPY")
    .url(format!("http://{addr}/documents/source.txt"))
    .destination(" https://dav.example.test/archive/source.txt ")
    .expect("Destination should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("Destination capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("https://dav.example.test/archive/source.txt"),
    header_value(&captured_request_text, "Destination")
  );
  assert_eq!(201, response.code());

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let destination: HttpDestination = server_request
    .destination()
    .expect("server Destination should parse")
    .expect("server Destination should be present");

  assert_eq!(
    "https://dav.example.test/archive/source.txt",
    destination.as_str()
  );
  assert_eq!(
    "https://dav.example.test/archive/source.txt",
    destination.header_value()
  );

  let malformed = rttp::server::HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nDestination: /relative\r\n\r\n",
  )
  .expect("malformed Destination request should still parse");
  assert!(malformed.destination().is_err());
  assert_eq!(Some("/relative"), malformed.header("Destination"));

  let duplicate = rttp::server::HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nDestination: https://dav.example.test/one\r\ndestination: https://dav.example.test/two\r\n\r\n",
  )
  .expect("duplicate Destination request should still parse");
  assert!(duplicate.destination().is_err());
  assert_eq!(
    Some("https://dav.example.test/one"),
    duplicate.header("Destination")
  );

  assert!(
    rttp::Destination::parse("a".repeat(64 * 1024 + 1)).is_err(),
    "oversized Destination values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_if_schedule_tag_match_request_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  let response = rttp::Http::client()
    .method("PUT")
    .url(format!("http://{addr}/calendars/alice/inbox/invite.ics"))
    .if_schedule_tag_match(" \"sched-17\" ")
    .expect("If-Schedule-Tag-Match should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("If-Schedule-Tag-Match capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("\"sched-17\""),
    header_value(&captured_request_text, "If-Schedule-Tag-Match")
  );
  assert_eq!(204, response.code());

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let validator: HttpIfScheduleTagMatch = server_request
    .if_schedule_tag_match()
    .expect("server If-Schedule-Tag-Match should parse")
    .expect("server If-Schedule-Tag-Match should be present");

  assert_eq!("\"sched-17\"", validator.header_value());
  assert_eq!("sched-17", validator.opaque_tag());
  assert!(!validator.is_weak());

  let weak = rttp::server::HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: W/\"sched-17\"\r\n\r\n",
  )
  .expect("weak If-Schedule-Tag-Match request should still parse");
  let weak_validator: HttpIfScheduleTagMatch = weak
    .if_schedule_tag_match()
    .expect("server weak If-Schedule-Tag-Match should parse")
    .expect("server weak If-Schedule-Tag-Match should be present");
  assert!(weak_validator.is_weak());
  assert_eq!("W/\"sched-17\"", weak_validator.header_value());

  let malformed = rttp::server::HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: *\r\n\r\n",
  )
  .expect("malformed If-Schedule-Tag-Match request should still parse");
  let malformed_result: Result<Option<HttpIfScheduleTagMatch>, HttpIfScheduleTagMatchParseError> =
    malformed.if_schedule_tag_match();
  assert!(malformed_result.is_err());
  assert_eq!(Some("*"), malformed.header("If-Schedule-Tag-Match"));

  let duplicate = rttp::server::HttpRequest::parse(
    b"PUT /calendars/alice/inbox/invite.ics HTTP/1.1\r\nHost: cal.example.test\r\nIf-Schedule-Tag-Match: \"sched-16\"\r\nif-schedule-tag-match: \"sched-17\"\r\n\r\n",
  )
  .expect("duplicate If-Schedule-Tag-Match request should still parse");
  assert!(duplicate.if_schedule_tag_match().is_err());
  assert_eq!(
    Some("\"sched-16\""),
    duplicate.header("If-Schedule-Tag-Match")
  );

  assert!(
    rttp::IfScheduleTagMatch::parse(format!("\"{}\"", "a".repeat(64 * 1024 - 1))).is_err(),
    "oversized If-Schedule-Tag-Match values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_lock_token_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    concat!(
      "HTTP/1.1 204 No Content\r\n",
      "Lock-Token: <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\n",
      "Content-Length: 0\r\n\r\n"
    )
    .as_bytes()
    .to_vec(),
  );
  let response = rttp::Http::client()
    .method("UNLOCK")
    .url(format!("http://{addr}/resource"))
    .lock_token("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
    .expect("Lock-Token should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("Lock-Token capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"),
    header_value(&captured_request_text, "Lock-Token")
  );
  let response_token = response
    .lock_token()
    .expect("client Lock-Token should parse")
    .expect("client Lock-Token should be present");
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    response_token.as_str()
  );
  assert!(!format!("{response_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let lock_token: HttpLockToken = server_request
    .lock_token()
    .expect("server Lock-Token should parse")
    .expect("server Lock-Token should be present");

  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.as_str()
  );
  assert_eq!(
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>",
    lock_token.header_value()
  );

  let malformed = rttp::server::HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nLock-Token: <relative>\r\n\r\n",
  )
  .expect("malformed Lock-Token request should still parse");
  assert!(malformed.lock_token().is_err());
  assert_eq!(Some("<relative>"), malformed.header("Lock-Token"));

  let duplicate = rttp::server::HttpRequest::parse(
    b"UNLOCK /resource HTTP/1.1\r\nHost: example.test\r\nLock-Token: <opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>\r\nlock-token: <http://example.test/locks/2>\r\n\r\n",
  )
  .expect("duplicate Lock-Token request should still parse");
  assert!(duplicate.lock_token().is_err());
  assert_eq!(
    Some("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"),
    duplicate.header("Lock-Token")
  );

  assert!(
    rttp::LockToken::parse("x".repeat(64 * 1024 + 1)).is_err(),
    "oversized Lock-Token values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_roundtrips_overwrite_request_metadata_without_policy() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  let response = rttp::Http::client()
    .method("COPY")
    .url(format!("http://{addr}/documents/source.txt"))
    .overwrite(" F ")
    .expect("Overwrite should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle.join().expect("Overwrite capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(Some("F"), header_value(&captured_request_text, "Overwrite"));
  assert_eq!(204, response.code());

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let overwrite: HttpOverwrite = server_request
    .overwrite()
    .expect("server Overwrite should parse")
    .expect("server Overwrite should be present");

  assert_eq!(HttpOverwrite::F, overwrite);
  assert_eq!("F", overwrite.header_value());

  let malformed = rttp::server::HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nOverwrite: true\r\n\r\n",
  )
  .expect("malformed Overwrite request should still parse");
  assert!(malformed.overwrite().is_err());
  assert_eq!(Some("true"), malformed.header("Overwrite"));

  let duplicate = rttp::server::HttpRequest::parse(
    b"COPY /documents/source.txt HTTP/1.1\r\nHost: example.test\r\nOverwrite: T\r\noverwrite: F\r\n\r\n",
  )
  .expect("duplicate Overwrite request should still parse");
  assert!(duplicate.overwrite().is_err());
  assert_eq!(Some("T"), duplicate.header("Overwrite"));

  assert!(
    rttp::Overwrite::parse("T".repeat(64 * 1024 + 1)).is_err(),
    "oversized Overwrite values must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn client_a_im_helpers_parse_through_shared_server_type() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset"))
    .a_im("diffe")
    .expect("diffe should be accepted")
    .a_im_with_q("gzip", "0.3")
    .expect("gzip quality should be accepted")
    .a_im_value("identity;q=0;profile=compact")
    .expect("parameterized A-IM should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle.join().expect("A-IM capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("diffe, gzip;q=0.3, identity;q=0;profile=compact"),
    header_value(&captured_request_text, "A-IM")
  );

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let a_im: rttp::AIm = server_request
    .a_im()
    .expect("server A-IM should parse")
    .expect("server A-IM should be present");

  assert_eq!(a_im.len(), 3);
  assert_eq!(a_im.members()[0].token(), "diffe");
  assert_eq!(a_im.members()[0].quality(), 1000);
  assert_eq!(a_im.members()[1].token(), "gzip");
  assert_eq!(a_im.members()[1].quality(), 300);
  assert_eq!(a_im.members()[2].token(), "identity");
  assert_eq!(a_im.members()[2].quality(), 0);
  assert_eq!(
    a_im.header_value(),
    "diffe, gzip;q=0.3, identity;q=0;profile=compact"
  );

  let malformed = rttp::server::HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nA-IM: diffe, DIFFE\r\n\r\n",
  )
  .expect("malformed A-IM request should still parse");
  assert!(
    malformed.a_im().is_err(),
    "duplicate A-IM members must fail closed"
  );
  assert_eq!(malformed.header("A-IM"), Some("diffe, DIFFE"));

  assert!(
    rttp::AIm::parse("x".repeat(64 * 1024 + 1)).is_err(),
    "oversized A-IM values must fail closed"
  );
  let too_many = (0..33)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::server::HttpAIm::parse(too_many).is_err(),
    "more than 32 A-IM members must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn client_negotiate_helpers_parse_through_shared_server_type() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset"))
    .negotiate("Trans, 1.0, feature-x=preview, *")
    .expect("Negotiate directives should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle.join().expect("Negotiate capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("trans, 1.0, feature-x=preview, *"),
    header_value(&captured_request_text, "Negotiate")
  );

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let negotiate: rttp::Negotiate = server_request
    .negotiate()
    .expect("server Negotiate should parse")
    .expect("server Negotiate should be present");

  assert_eq!(negotiate.len(), 4);
  assert_eq!(negotiate.members()[0], rttp::NegotiateDirective::Trans);
  assert_eq!(
    negotiate.members()[1],
    rttp::NegotiateDirective::RvsaVersion { major: 1, minor: 0 }
  );
  assert_eq!(
    negotiate.members()[2],
    rttp::NegotiateDirective::Extension {
      name: "feature-x".to_owned(),
      value: Some("preview".to_owned()),
    }
  );
  assert_eq!(negotiate.members()[3], rttp::NegotiateDirective::Any);
  assert_eq!(negotiate.header_value(), "trans, 1.0, feature-x=preview, *");

  let malformed = rttp::server::HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nNegotiate: trans, TRANS\r\n\r\n",
  )
  .expect("malformed Negotiate request should still parse");
  assert!(
    malformed.negotiate().is_err(),
    "duplicate Negotiate directives must fail closed"
  );
  assert_eq!(malformed.header("Negotiate"), Some("trans, TRANS"));

  assert!(
    rttp::Negotiate::parse(format!("feature-x={}", "a".repeat(64 * 1024 + 1))).is_err(),
    "oversized Negotiate values must fail closed"
  );
  let too_many = (0..33)
    .map(|index| format!("feature-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::server::HttpNegotiate::parse(too_many).is_err(),
    "more than 32 Negotiate members must fail closed"
  );
}

#[test]
#[cfg(feature = "client")]
fn client_accept_encoding_helpers_parse_through_shared_server_type() {
  let (addr, handle) = spawn_representation_metadata_response_server(
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  );
  rttp::Http::client()
    .get()
    .url(format!("http://{addr}/asset"))
    .accept_gzip()
    .expect("gzip should be accepted")
    .accept_br_with_q("0.8")
    .expect("br quality should be accepted")
    .accept_identity_with_q("0")
    .expect("identity quality should be accepted")
    .emit()
    .expect("client request should complete");
  let captured_request = handle
    .join()
    .expect("Accept-Encoding capture server should join");
  let captured_request_text =
    String::from_utf8(captured_request.clone()).expect("request should be utf-8");

  assert_eq!(
    Some("gzip, br;q=0.8, identity;q=0"),
    header_value(&captured_request_text, "Accept-Encoding")
  );

  let server_request =
    rttp::server::HttpRequest::parse(&captured_request).expect("server request should parse");
  let encodings: rttp::AcceptEncoding = server_request
    .accept_encoding()
    .expect("server Accept-Encoding should parse")
    .expect("server Accept-Encoding should be present");

  assert_eq!(encodings.len(), 3);
  assert_eq!(encodings.codings()[0].coding(), "gzip");
  assert_eq!(encodings.codings()[0].quality(), 1000);
  assert_eq!(encodings.codings()[1].coding(), "br");
  assert_eq!(encodings.codings()[1].quality(), 800);
  assert_eq!(encodings.codings()[2].coding(), "identity");
  assert_eq!(encodings.codings()[2].quality(), 0);
  assert_eq!(encodings.header_value(), "gzip, br;q=0.8, identity;q=0");

  let malformed = rttp::server::HttpRequest::parse(
    b"GET /asset HTTP/1.1\r\nHost: example.test\r\nAccept-Encoding: gzip, GZIP\r\n\r\n",
  )
  .expect("malformed Accept-Encoding request should still parse");
  assert!(
    malformed.accept_encoding().is_err(),
    "duplicate Accept-Encoding members must fail closed"
  );

  assert!(
    rttp::AcceptEncoding::parse("gzip".repeat(64 * 1024 + 1)).is_err(),
    "oversized Accept-Encoding values must fail closed"
  );
  let too_many = (0..33)
    .map(|index| format!("coding{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    rttp::server::HttpRequestAcceptEncodings::parse(too_many).is_err(),
    "more than 32 Accept-Encoding members must fail closed"
  );
}

#[test]
fn compatibility_facade_keeps_server_metadata_in_the_server_module() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let a_im: HttpAIm =
    HttpAIm::parse("diffe, gzip;q=0.3;profile=compact").expect("A-IM should parse");
  let _: HttpAImParseError =
    HttpAIm::parse("diffe, DIFFE").expect_err("duplicate A-IM should be rejected");
  let negotiate: HttpNegotiate =
    HttpNegotiate::parse("trans, 1.0, feature-x=preview, *").expect("Negotiate should parse");
  let _: HttpNegotiateParseError =
    HttpNegotiate::parse("trans, TRANS").expect_err("duplicate Negotiate should be rejected");
  let _: HttpNegotiateDirective = negotiate.members()[0].clone();
  let tcn: HttpTcn = HttpTcn::parse("list, choice").expect("TCN should parse");
  let _: HttpTcnParseError =
    HttpTcn::parse("list, LIST").expect_err("duplicate TCN should be rejected");
  let _: HttpTcnDirective = tcn.members()[0].clone();
  let set_cookie: HttpSetCookie =
    HttpSetCookie::parse(r#"session="abc def"; Path=/; SameSite=Lax; Foo=bar"#)
      .expect("Set-Cookie should parse");
  let _: HttpSameSite = set_cookie.same_site().expect("SameSite should parse");
  let _: HttpSetCookies = HttpSetCookies::parse_values([set_cookie.header_value().as_str()])
    .expect("Set-Cookie collection should parse");
  let _: HttpCookieParseError = HttpSetCookie::parse("session=abc; Path=/; path=/other")
    .expect_err("duplicate Set-Cookie attributes should be rejected");
  let accept_charsets: HttpRequestAcceptCharsets =
    HttpRequestAcceptCharsets::parse("utf-8, iso-8859-1;q=0.5")
      .expect("Accept-Charset should parse");
  let _: HttpAcceptCharsetParseError = HttpRequestAcceptCharsets::parse("utf-8, UTF-8")
    .expect_err("malformed Accept-Charset should be rejected");
  let accept_languages: HttpAcceptLanguages =
    HttpAcceptLanguages::parse("en-US, fr-CA; q=0.8").expect("Accept-Language should parse");
  let _: HttpAcceptLanguageParseError = HttpAcceptLanguages::parse("en; q=1.001")
    .expect_err("malformed Accept-Language should be rejected");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let response = HttpResponse::ok("")
    .with_etag(HttpEntityTag::weak("revision-42"))
    .with_schedule_tag(HttpScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse"));
  let request_method: HttpAccessControlRequestMethod =
    HttpAccessControlRequestMethod::parse("patch")
      .expect("Access-Control-Request-Method should parse");
  let private_network: HttpAccessControlRequestPrivateNetwork =
    HttpAccessControlRequestPrivateNetwork::parse("true")
      .expect("Access-Control-Request-Private-Network should parse");
  let save_data: HttpSaveData = HttpSaveData::parse("on").expect("Save-Data should parse");
  let sec_gpc: HttpSecGpc = HttpSecGpc::parse("1").expect("Sec-GPC should parse");
  let _: HttpSecGpcParseError =
    HttpSecGpc::parse("0").expect_err("invalid Sec-GPC should be rejected");
  let upgrade_insecure_requests: HttpUpgradeInsecureRequests =
    HttpUpgradeInsecureRequests::parse("1").expect("Upgrade-Insecure-Requests should parse");
  let _: Result<HttpUpgradeInsecureRequests, HttpUpgradeInsecureRequestsParseError> =
    HttpUpgradeInsecureRequests::parse("0");
  let authorization: HttpAuthorization =
    HttpAuthorization::parse("Bearer origin-token").expect("Authorization should parse");
  let proxy_authorization: HttpProxyAuthorization =
    HttpProxyAuthorization::parse("Basic cHJveHk6c2VjcmV0")
      .expect("Proxy-Authorization should parse");
  let max_forwards: HttpMaxForwards =
    HttpMaxForwards::parse("0").expect("Max-Forwards should parse");
  let destination: HttpDestination =
    HttpDestination::parse("https://dav.example.test/archive/report.txt")
      .expect("Destination should parse");
  let destination_error: Result<HttpDestination, HttpDestinationParseError> =
    HttpDestination::parse("/relative");
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
  let expectations: HttpExpectations =
    HttpExpectations::parse("100-continue, preview").expect("Expect should parse");
  let idempotency_key: HttpIdempotencyKey =
    HttpIdempotencyKey::parse("charge-2026-08-19-9f3c").expect("Idempotency-Key should parse");
  let sec_websocket_key: HttpSecWebSocketKey =
    HttpSecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");
  let sec_websocket_version: HttpSecWebSocketVersion =
    HttpSecWebSocketVersion::parse("13").expect("Sec-WebSocket-Version should parse");
  let sec_websocket_protocol: HttpSecWebSocketProtocol =
    HttpSecWebSocketProtocol::parse("chat, superchat")
      .expect("Sec-WebSocket-Protocol offers should parse");
  let sec_websocket_extensions: HttpSecWebSocketExtensions =
    HttpSecWebSocketExtensions::parse(r#"permessage-deflate; client_max_window_bits; mode="safe""#)
      .expect("Sec-WebSocket-Extensions offers should parse");
  let sec_websocket_accept = HttpSecWebSocketAccept::derive_from_key(&sec_websocket_key);
  let baggage: HttpBaggage =
    HttpBaggage::parse("tenant=acme;source=gateway").expect("baggage should parse");
  let _: HttpBaggageParseError =
    HttpBaggage::parse("tenant=1,tenant=2").expect_err("duplicate baggage should be rejected");
  let baggage_member: &HttpBaggageMember = &baggage.members()[0];
  let baggage_property: &HttpBaggageProperty = &baggage_member.properties()[0];
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
  let _: Result<HttpIdempotencyKey, HttpIdempotencyKeyParseError> =
    HttpIdempotencyKey::parse("key with space");
  let _: Result<HttpSecWebSocketKey, HttpSecWebSocketKeyParseError> =
    HttpSecWebSocketKey::parse("the sample nonce");
  let _: Result<HttpSecWebSocketVersion, HttpSecWebSocketVersionParseError> =
    HttpSecWebSocketVersion::parse("8, 13");
  let _: Result<HttpSecWebSocketProtocol, HttpSecWebSocketProtocolParseError> =
    HttpSecWebSocketProtocol::parse_selection("chat, superchat");
  let _: Result<HttpSecWebSocketExtensions, HttpSecWebSocketExtensionsParseError> =
    HttpSecWebSocketExtensions::parse_selection("permessage-deflate, x-test");
  let _: Result<HttpSecWebSocketAccept, HttpSecWebSocketAcceptParseError> =
    HttpSecWebSocketAccept::parse("the accept value");
  let if_modified_since: HttpIfModifiedSince =
    HttpIfModifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("If-Modified-Since should parse");
  let if_unmodified_since: HttpIfUnmodifiedSince =
    HttpIfUnmodifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
      .expect("If-Unmodified-Since should parse");
  let policy: HttpCrossOriginResourcePolicy = HttpCrossOriginResourcePolicy::parse("same-origin")
    .expect("Cross-Origin-Resource-Policy should parse");
  let embedder_policy: HttpCrossOriginEmbedderPolicy =
    HttpCrossOriginEmbedderPolicy::parse("require-corp; report-to=\"coep\"")
      .expect("Cross-Origin-Embedder-Policy should parse");
  let embedder_policy_report_only: HttpCrossOriginEmbedderPolicyReportOnly =
    HttpCrossOriginEmbedderPolicyReportOnly::parse("require-corp; report-to=\"coep\"")
      .expect("Cross-Origin-Embedder-Policy-Report-Only should parse");
  let opener_policy: HttpCrossOriginOpenerPolicy =
    HttpCrossOriginOpenerPolicy::parse("noopener-allow-popups; report-to=\"coop\"")
      .expect("Cross-Origin-Opener-Policy should parse");
  let opener_policy_report_only: HttpCrossOriginOpenerPolicyReportOnly =
    HttpCrossOriginOpenerPolicyReportOnly::parse("same-origin; report-to=\"coop\"")
      .expect("Cross-Origin-Opener-Policy-Report-Only should parse");
  let upgrade: HttpUpgrade = HttpUpgrade::parse("websocket").expect("Upgrade should parse");
  let _: HttpUpgradeParseError = HttpUpgrade::parse("").expect_err("empty Upgrade should fail");
  let pragma: HttpPragma =
    HttpPragma::parse("no-cache, community=private").expect("Pragma should parse");
  let _: HttpPragmaParseError = HttpPragma::parse("no-cache, no-cache")
    .expect_err("duplicate Pragma directives should be rejected");
  let nel: HttpNel = HttpNel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#)
    .expect("NEL should parse");
  let proxy_status: HttpProxyStatus =
    HttpProxyStatus::parse("ExampleCDN; error=connection_timeout")
      .expect("Proxy-Status should parse");
  let _: HttpProxyStatusParseError =
    HttpProxyStatus::parse("").expect_err("empty Proxy-Status should be rejected");
  let alternates: HttpAlternates = HttpAlternates::parse(
    r#"{ "/resource.en.html" 1.0 {type text/html} {language en} {length 1234} }"#,
  )
  .expect("Alternates should parse");
  let _: HttpAlternatesParseError =
    HttpAlternates::parse(r#"{ "/broken" 1.001 }"#).expect_err("invalid Alternates should fail");
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
  let permissions_policy: HttpPermissionsPolicy =
    HttpPermissionsPolicy::parse(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
      .expect("Permissions-Policy should parse");
  let _: HttpPermissionsPolicyParseError =
    HttpPermissionsPolicy::parse("geolocation=src").expect_err("src should be rejected");
  let supports_loading_mode: HttpSupportsLoadingMode =
    HttpSupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
      .expect("Supports-Loading-Mode should parse");
  let _: HttpSupportsLoadingModeParseError =
    HttpSupportsLoadingMode::parse("?1").expect_err("non-token should be rejected");
  let content_location: HttpContentLocation =
    HttpContentLocation::parse("../representations/current.json")
      .expect("Content-Location should parse");
  let _: HttpContentLocationParseError = HttpContentLocation::parse("not valid")
    .expect_err("invalid Content-Location should be rejected");
  let service_worker_allowed: HttpServiceWorkerAllowed =
    HttpServiceWorkerAllowed::parse("/").expect("Service-Worker-Allowed should parse");
  let _: HttpServiceWorkerAllowedParseError =
    HttpServiceWorkerAllowed::parse("http://example.test/scope")
      .expect_err("absolute URI Service-Worker-Allowed should be rejected");
  let content_dpr: HttpContentDpr = HttpContentDpr::parse("2.0").expect("Content-DPR should parse");
  let _: HttpContentDprParseError =
    HttpContentDpr::parse("0").expect_err("zero Content-DPR should be rejected");
  let deprecation: HttpDeprecation =
    HttpDeprecation::parse("?1").expect("Deprecation should parse");
  let _: HttpDeprecationParseError =
    HttpDeprecation::parse("true").expect_err("historical Deprecation token should be rejected");
  let content_range: HttpContentRange =
    HttpContentRange::parse("bytes */10").expect("Content-Range should parse");
  let _: HttpContentRangeParseError =
    HttpContentRange::parse("bytes */*").expect_err("invalid Content-Range should be rejected");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(a_im.header_value(), "diffe, gzip;q=0.3;profile=compact");
  assert_eq!(negotiate.members()[0], HttpNegotiateDirective::Trans);
  assert_eq!(negotiate.members()[3], HttpNegotiateDirective::Any);
  assert_eq!("trans, 1.0, feature-x=preview, *", negotiate.header_value());
  assert_eq!(accept_charsets.charsets()[0].charset(), "utf-8");
  assert_eq!(accept_charsets.charsets()[1].quality(), 500);
  assert_eq!(accept_charsets.header_value(), "utf-8, iso-8859-1;q=0.5");
  assert_eq!(accept_languages.ranges(), ["en-US", "fr-CA"]);
  assert_eq!(accept_languages.qualities(), [None, Some("0.8")]);
  assert_eq!(accept_languages.header_value(), "en-US, fr-CA; q=0.8");
  assert_eq!(request_method.method(), "PATCH");
  assert_eq!(request_method.header_value(), "PATCH");
  assert_eq!(private_network.header_value(), "true");
  assert_eq!(save_data.header_value(), "on");
  assert_eq!(sec_gpc.header_value(), "1");
  assert_eq!(upgrade_insecure_requests.header_value(), "1");
  assert_eq!(authorization.header_value(), "Bearer origin-token");
  assert_eq!(proxy_authorization.header_value(), "Basic cHJveHk6c2VjcmV0");
  assert_eq!(max_forwards.value(), 0);
  assert_eq!(max_forwards.header_value(), "0");
  assert_eq!(
    destination.as_str(),
    "https://dav.example.test/archive/report.txt"
  );
  assert_eq!(
    destination.header_value(),
    "https://dav.example.test/archive/report.txt"
  );
  assert!(destination_error.is_err());
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
  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
  assert_eq!(expectations.header_value(), "100-continue, preview");
  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.as_str());
  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.header_value());
  assert!(!format!("{idempotency_key:?}").contains("charge-2026-08-19-9f3c"));
  assert_eq!("dGhlIHNhbXBsZSBub25jZQ==", sec_websocket_key.as_str());
  assert_eq!("dGhlIHNhbXBsZSBub25jZQ==", sec_websocket_key.header_value());
  assert!(!format!("{sec_websocket_key:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert_eq!(sec_websocket_version.versions(), ["13"]);
  assert!(sec_websocket_version.contains("13"));
  assert_eq!(sec_websocket_version.header_value(), "13");
  assert_eq!(sec_websocket_protocol.protocols(), ["chat", "superchat"]);
  assert!(sec_websocket_protocol.contains("chat"));
  assert_eq!(sec_websocket_protocol.header_value(), "chat, superchat");
  assert_eq!(sec_websocket_protocol.selected(), None);
  assert_eq!(
    sec_websocket_extensions.header_value(),
    r#"permessage-deflate; client_max_window_bits; mode="safe""#
  );
  assert_eq!(
    sec_websocket_extensions.extensions()[0].token(),
    "permessage-deflate"
  );
  assert_eq!(
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
    sec_websocket_accept.as_str()
  );
  assert!(sec_websocket_accept.verify_key(&sec_websocket_key));
  assert!(!format!("{sec_websocket_accept:?}").contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
  assert_eq!("tenant", baggage_member.key());
  assert_eq!("source", baggage_property.key());
  assert!(!format!("{baggage:?}").contains("acme"));
  assert_eq!(cdn_loop.members()[0].identifier(), "foo123.foocdn.example");
  assert_eq!(cdn_loop.members()[1].parameter("trace"), Some("abcdef"));
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!(
    if_modified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    if_unmodified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(service_worker_allowed.header_value(), "/");
  assert_eq!(service_worker_allowed.as_str(), "/");
  assert_eq!(content_dpr.ratio(), 2.0);
  assert_eq!(content_dpr.header_value(), "2.0");
  assert_eq!(deprecation, HttpDeprecation::Boolean(true));
  assert_eq!(deprecation.header_value(), "?1");
  assert_eq!(content_range.header_value(), "bytes */10");
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(embedder_policy_report_only.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(
    HttpCrossOriginOpenerPolicy::SameOrigin,
    opener_policy_report_only.policy()
  );
  assert_eq!(Some("coop"), opener_policy_report_only.report_to());
  assert_eq!(
    opener_policy_report_only.header_value(),
    r#"same-origin; report-to="coop""#
  );
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert!(pragma.no_cache());
  assert_eq!("no-cache, community=private", pragma.header_value());
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(
    proxy_status.members()[0].identifier().as_str(),
    "ExampleCDN"
  );
  assert_eq!(alternates.variants()[0].uri(), "/resource.en.html");
  assert_eq!(
    alternates.variants()[0].attribute("type"),
    Some("text/html")
  );
  assert_eq!(alt_used.host(), "[2001:db8::1]");
  assert_eq!(alt_used.port(), Some("8443"));
  assert_eq!(origin_trials.tokens(), ["token-one", "token-two"]);
  assert!(!format!("{origin_trials:?}").contains("token-one"));
  assert_eq!(
    speculation_rules.header_value(),
    "https://example.test/speculation-rules.json"
  );
  assert!(!format!("{speculation_rules:?}").contains("speculation-rules.json"));
  assert_eq!(
    permissions_policy.header_value(),
    r#"geolocation=(self "https://maps.example.test"), camera=()"#
  );
  assert!(permissions_policy
    .directive("camera")
    .unwrap()
    .allowlist()
    .is_empty());
  assert_eq!(
    supports_loading_mode.tokens(),
    ["fenced-frame", "credentialed-prerender"]
  );
  assert!(supports_loading_mode.contains_fenced_frame());
  assert!(supports_loading_mode.contains_credentialed_prerender());
  assert_eq!(
    supports_loading_mode.header_value(),
    "fenced-frame, credentialed-prerender"
  );
  assert_eq!(
    metadata
      .entity_tag_value()
      .expect("entity tag should be retained")
      .header_value(),
    "\"revision-42\""
  );
  assert_eq!(
    response.etag().expect("ETag should parse"),
    Some(HttpEntityTag::weak("revision-42"))
  );
  assert_eq!(
    response.schedule_tag().expect("Schedule-Tag should parse"),
    Some(HttpScheduleTag::parse("\"sched-17\"").expect("Schedule-Tag should parse"))
  );
}

#[test]
fn compatibility_facade_exposes_memento_datetime_response_metadata() {
  let datetime = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let response = HttpResponse::ok("").with_memento_datetime(datetime);
  let _: Result<Option<HttpMementoDatetime>, HttpMementoDatetimeParseError> =
    response.memento_datetime();

  assert_eq!(
    Some(HttpMementoDatetime::new(datetime)),
    response
      .memento_datetime()
      .expect("Memento-Datetime should parse")
  );
}

#[test]
fn compatibility_facade_exposes_deprecation_response_metadata() {
  let response = HttpResponse::ok("").with_deprecation(HttpDeprecation::Boolean(true));
  let _: Result<Option<HttpDeprecation>, HttpDeprecationParseError> = response.deprecation();

  assert_eq!(
    Some(HttpDeprecation::Boolean(true)),
    response.deprecation().expect("Deprecation should parse")
  );
}

#[test]
fn compatibility_facade_exposes_sunset_response_metadata() {
  let sunset = UNIX_EPOCH + Duration::from_secs(784_111_777);
  let response = HttpResponse::ok("").with_sunset(sunset);
  let _: Result<Option<std::time::SystemTime>, HttpSunsetParseError> = response.sunset();

  assert_eq!(
    Some(sunset),
    response.sunset().expect("Sunset should parse")
  );
}

#[test]
fn compatibility_facade_keeps_signature_metadata_in_the_server_module() {
  let signature: HttpSignature =
    HttpSignature::parse("sig1=:YWJj:").expect("Signature should parse");
  let signature_input: HttpSignatureInput =
    HttpSignatureInput::parse(r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#)
      .expect("Signature-Input should parse");
  let _: HttpSignatureParseError =
    HttpSignature::parse("").expect_err("empty Signature should be rejected");
  let _: HttpSignatureInputParseError =
    HttpSignatureInput::parse("").expect_err("empty Signature-Input should be rejected");
  let response = HttpResponse::ok("")
    .with_signature("sig1=:YWJj:")
    .expect("Signature should be accepted")
    .with_signature_input(r#"sig1=("@method")"#)
    .expect("Signature-Input should be accepted");

  let entry: &HttpSignatureInputEntry = &signature_input.entries()[0];
  let _: &[HttpSignatureInputComponent] = entry.components();
  let _: &[HttpSignatureInputParameter] = entry.parameters();

  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#
  );
  assert!(matches!(
    entry
      .parameter("created")
      .map(HttpSignatureInputParameter::value),
    Some(HttpSignatureInputBareItem::Integer(1_618_884_473))
  ));
  assert_eq!(
    response
      .signature()
      .expect("Signature should parse")
      .expect("Signature should be present")
      .header_value(),
    "sig1=:YWJj:"
  );
}

#[test]
fn compatibility_facade_exposes_content_dpr_response_metadata() {
  let response = HttpResponse::ok("")
    .header("Content-DPR", "3")
    .with_content_dpr("1.5")
    .expect("valid Content-DPR should be accepted");

  assert_eq!(
    "1.5",
    response
      .content_dpr()
      .expect("Content-DPR should parse")
      .expect("Content-DPR should be present")
      .header_value()
  );
  assert!(HttpResponse::ok("").with_content_dpr("0").is_err());
  assert!(HttpResponse::ok("")
    .header("Content-DPR", "1")
    .header("Content-DPR", "2")
    .content_dpr()
    .is_err());
}

#[test]
fn via_facade_exports_shared_request_and_response_type() {
  let via: HttpVia =
    HttpVia::parse("1.1 edge-a (TLS terminator), HTTP/2 upstream").expect("Via should parse");
  let _: HttpViaParseError =
    HttpVia::parse("1.1").expect_err("incomplete Via hop should be rejected");
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
}

#[cfg(feature = "client")]
#[test]
fn via_compatibility_facade_exports_client_type() {
  let via: rttp::Via =
    rttp::Via::parse("1.1 edge-a (TLS terminator), HTTP/2 upstream").expect("Via should parse");
  let _: rttp::ViaParseError =
    rttp::Via::parse("1.1").expect_err("incomplete Via hop should be rejected");
  assert_eq!("edge-a", via.members()[0].received_by());
  let member: rttp::ViaMember = via.members()[1].clone();
  assert_eq!(Some("HTTP"), member.protocol_name());
}

#[test]
fn set_cookie_facade_reuses_protocol_type_across_server_and_client() {
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
  let response = HttpResponse::ok("ok")
    .with_set_cookie(session)
    .with_set_cookie(csrf);
  let server_cookies = response
    .set_cookies()
    .expect("server Set-Cookie should parse")
    .expect("server Set-Cookie should be present");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(2, serialized.matches("\r\nSet-Cookie: ").count());
  assert_eq!(
    server_cookies,
    HttpSetCookies::parse_values([
      r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned"#,
      "csrf=token; Path=/form; Max-Age=60; Foo=bar",
    ])
    .expect("protocol collection should parse")
  );
  assert!(!format!("{server_cookies:?}").contains("abc def"));
  assert!(!format!("{server_cookies:?}").contains("token"));

  let malformed =
    HttpResponse::ok("ok").header("Set-Cookie", "session=super-secret; Path=/; path=/other");
  assert!(malformed.set_cookies().is_err());
  assert!(String::from_utf8(malformed.to_bytes())
    .expect("response should serialize")
    .contains("\r\nSet-Cookie: session=super-secret; Path=/; path=/other\r\n"));

  #[cfg(feature = "client")]
  {
    let client_response = rttp_client::response::Response::new(
      rttp_client::types::RoUrl::with("http://example.test/"),
      serialized.into_bytes(),
    )
    .expect("client should parse the server response");
    let client_cookies = client_response
      .set_cookies()
      .expect("client Set-Cookie should parse")
      .expect("client Set-Cookie should be present");
    assert_eq!(server_cookies, client_cookies);
    assert_eq!(
      vec![
        r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned"#,
        "csrf=token; Path=/form; Max-Age=60; Foo=bar"
      ],
      client_response
        .header_values("set-cookie")
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>()
    );
    assert!(!format!("{client_cookies:?}").contains("abc def"));
  }
}
