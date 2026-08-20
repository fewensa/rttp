use rttp::server::{
  HttpAcceptCh, HttpAcceptCharsetParseError, HttpAcceptLanguageParseError, HttpAcceptLanguages,
  HttpAccessControlRequestMethod, HttpAccessControlRequestPrivateNetwork, HttpAltUsed,
  HttpAltUsedParseError, HttpAuthorization, HttpBaggage, HttpBaggageMember, HttpBaggageParseError,
  HttpBaggageProperty, HttpCdnLoop, HttpCdnLoopParseError, HttpConditionalMetadata, HttpContentDpr,
  HttpContentDprParseError, HttpContentLocation, HttpContentLocationParseError, HttpContentRange,
  HttpContentRangeParseError, HttpCrossOriginEmbedderPolicy,
  HttpCrossOriginEmbedderPolicyReportOnly, HttpCrossOriginOpenerPolicy,
  HttpCrossOriginOpenerPolicyReportOnly, HttpCrossOriginResourcePolicy, HttpDeprecation,
  HttpDeprecationParseError, HttpDepth, HttpDepthParseError, HttpEntityTag, HttpExpectations,
  HttpIdempotencyKey, HttpIdempotencyKeyParseError, HttpIfModifiedSince, HttpIfUnmodifiedSince,
  HttpMaxForwards, HttpMementoDatetime, HttpMementoDatetimeParseError, HttpNel,
  HttpOriginTrialParseError, HttpOriginTrials, HttpPermissionsPolicy,
  HttpPermissionsPolicyParseError, HttpPragma, HttpPragmaParseError, HttpProxyAuthorization,
  HttpProxyStatus, HttpProxyStatusParseError, HttpRequestAcceptCharsets, HttpResponse,
  HttpSaveData, HttpSecGpc, HttpSecGpcParseError, HttpSecWebSocketKey,
  HttpSecWebSocketKeyParseError, HttpServiceWorkerAllowed, HttpServiceWorkerAllowedParseError,
  HttpSignature, HttpSignatureInput, HttpSignatureInputBareItem, HttpSignatureInputComponent,
  HttpSignatureInputEntry, HttpSignatureInputParameter, HttpSignatureInputParseError,
  HttpSignatureParseError, HttpSpeculationRules, HttpSpeculationRulesParseError,
  HttpSunsetParseError, HttpSupportsLoadingMode, HttpSupportsLoadingModeParseError, HttpUpgrade,
  HttpUpgradeInsecureRequests, HttpUpgradeInsecureRequestsParseError, HttpUpgradeParseError,
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
  let accept_ch: rttp::AcceptCh =
    rttp_client::response::AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let allow_credentials: rttp::AccessControlAllowCredentials =
    rttp_client::response::AccessControlAllowCredentials::parse("true")
      .expect("Access-Control-Allow-Credentials should parse");
  let _: rttp::AccessControlAllowCredentialsParseError =
    rttp_client::response::AccessControlAllowCredentials::parse("false")
      .expect_err("invalid Access-Control-Allow-Credentials should fail");
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
  let depth: rttp::Depth = rttp::Depth::parse("infinity").expect("Depth should parse");
  let _: rttp::DepthParseError =
    rttp::Depth::parse("2").expect_err("malformed Depth should be rejected");
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
  let supports_loading_mode: rttp::SupportsLoadingMode =
    rttp_client::response::SupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
      .expect("Supports-Loading-Mode should parse");
  let _: rttp::SupportsLoadingModeParseError =
    rttp_client::response::SupportsLoadingMode::parse("?1")
      .expect_err("non-token should be rejected");
  let fetch_site: rttp::SecFetchSite =
    rttp_client::SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let sec_purpose: rttp::SecPurpose =
    rttp_client::SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let baggage: rttp::Baggage =
    rttp_client::Baggage::parse("tenant=acme;source=gateway").expect("baggage should parse");
  let _: rttp::BaggageParseError = rttp_client::Baggage::parse("tenant=1,tenant=2")
    .expect_err("duplicate baggage should be rejected");
  let baggage_member: &rttp::BaggageMember = &baggage.members()[0];
  let baggage_property: &rttp::BaggageProperty = &baggage_member.properties()[0];
  let etag: rttp::EntityTag =
    rttp_client::response::EntityTag::parse("\"asset-v7\"").expect("ETag should parse");
  let location: rttp::Location =
    rttp_client::response::Location::parse("/next").expect("Location should parse");
  let _: rttp::LocationParseError =
    rttp_client::response::Location::parse("").expect_err("empty Location should be rejected");
  let content_length = rttp::HttpContentLength::new(123);

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(
    cache_status.members()[0].identifier().as_str(),
    "OriginCache"
  );
  assert_eq!(cache_status.members()[0].ttl(), Some(1100));
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
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
  assert_eq!(rttp::Depth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
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
  let response = HttpResponse::ok("").with_etag(HttpEntityTag::weak("revision-42"));
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
  let depth: HttpDepth = HttpDepth::parse("infinity").expect("Depth should parse");
  let depth_error: Result<HttpDepth, HttpDepthParseError> = HttpDepth::parse("2");
  let expectations: HttpExpectations =
    HttpExpectations::parse("100-continue, preview").expect("Expect should parse");
  let idempotency_key: HttpIdempotencyKey =
    HttpIdempotencyKey::parse("charge-2026-08-19-9f3c").expect("Idempotency-Key should parse");
  let sec_websocket_key: HttpSecWebSocketKey =
    HttpSecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==").expect("Sec-WebSocket-Key should parse");
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
  let _: Result<HttpIdempotencyKey, HttpIdempotencyKeyParseError> =
    HttpIdempotencyKey::parse("key with space");
  let _: Result<HttpSecWebSocketKey, HttpSecWebSocketKeyParseError> =
    HttpSecWebSocketKey::parse("the sample nonce");
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
  assert_eq!(HttpDepth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert!(depth_error.is_err());
  assert!(expectations.expects_continue());
  assert_eq!(["preview"], expectations.unsupported());
  assert_eq!(expectations.header_value(), "100-continue, preview");
  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.as_str());
  assert_eq!("charge-2026-08-19-9f3c", idempotency_key.header_value());
  assert!(!format!("{idempotency_key:?}").contains("charge-2026-08-19-9f3c"));
  assert_eq!("dGhlIHNhbXBsZSBub25jZQ==", sec_websocket_key.as_str());
  assert_eq!("dGhlIHNhbXBsZSBub25jZQ==", sec_websocket_key.header_value());
  assert!(!format!("{sec_websocket_key:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert_eq!("tenant", baggage_member.key());
  assert_eq!("source", baggage_property.key());
  assert!(!format!("{baggage:?}").contains("acme"));
  assert_eq!(cdn_loop.members()[0].identifier(), "foo123.foocdn.example");
  assert_eq!(cdn_loop.members()[1].parameter("trace"), Some("abcdef"));
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
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
