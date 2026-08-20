use rttp_protocol::accept_charset::AcceptCharset;
use rttp_protocol::accept_encoding::AcceptEncoding;
use rttp_protocol::accept_language::AcceptLanguage;
use rttp_protocol::accept_ranges::AcceptRanges;
use rttp_protocol::access_control_allow_credentials::AccessControlAllowCredentials;
use rttp_protocol::access_control_expose_headers::AccessControlExposeHeaders;
use rttp_protocol::access_control_request_method::AccessControlRequestMethod;
use rttp_protocol::access_control_request_private_network::AccessControlRequestPrivateNetwork;
use rttp_protocol::age::Age;
use rttp_protocol::alt_used::AltUsed;
use rttp_protocol::authorization::{Authorization, ProxyAuthorization};
use rttp_protocol::baggage::Baggage;
use rttp_protocol::cache_status::CacheStatus;
use rttp_protocol::cdn_cache_control::CdnCacheControl;
use rttp_protocol::cdn_loop::{CdnLoop, CdnLoopParseError};
use rttp_protocol::client_hints::{AcceptCh, CriticalCh};
use rttp_protocol::connection::Connection;
use rttp_protocol::content_disposition::ContentDisposition;
use rttp_protocol::content_dpr::ContentDpr;
use rttp_protocol::content_encoding::ContentEncoding;
use rttp_protocol::content_language::ContentLanguage;
use rttp_protocol::content_location::ContentLocation;
use rttp_protocol::content_security_policy::ContentSecurityPolicy;
use rttp_protocol::content_security_policy_report_only::ContentSecurityPolicyReportOnly;
use rttp_protocol::content_type::ContentType;
use rttp_protocol::cross_origin_embedder_policy::CrossOriginEmbedderPolicy;
use rttp_protocol::cross_origin_embedder_policy_report_only::CrossOriginEmbedderPolicyReportOnly;
use rttp_protocol::cross_origin_opener_policy::CrossOriginOpenerPolicy;
use rttp_protocol::cross_origin_opener_policy_report_only::CrossOriginOpenerPolicyReportOnly;
use rttp_protocol::dav::{Dav, DavClass, DavParseError};
use rttp_protocol::deprecation::Deprecation;
use rttp_protocol::depth::Depth;
use rttp_protocol::destination::Destination;
use rttp_protocol::document_policy::{DocumentPolicy, DocumentPolicyParseError};
use rttp_protocol::document_policy_report_only::{
  DocumentPolicyReportOnly, DocumentPolicyReportOnlyParseError,
};
use rttp_protocol::entity_tag::{EntityTag, IfMatch};
use rttp_protocol::expect::Expect;
use rttp_protocol::fetch_metadata::{
  SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};
use rttp_protocol::from::From;
use rttp_protocol::host::Host;
use rttp_protocol::idempotency_key::IdempotencyKey;
use rttp_protocol::if_header::{If, IfParseError, IfPredicate};
use rttp_protocol::if_modified_since::IfModifiedSince;
use rttp_protocol::if_schedule_tag_match::IfScheduleTagMatch;
use rttp_protocol::if_unmodified_since::IfUnmodifiedSince;
use rttp_protocol::keep_alive::KeepAlive;
use rttp_protocol::link::LinkValues;
use rttp_protocol::location::Location;
use rttp_protocol::lock_token::LockToken;
use rttp_protocol::max_forwards::MaxForwards;
use rttp_protocol::memento_datetime::MementoDatetime;
use rttp_protocol::nel::Nel;
use rttp_protocol::no_vary_search::{NoVarySearch, NoVarySearchParams};
use rttp_protocol::origin::Origin;
use rttp_protocol::origin_trial::OriginTrials;
use rttp_protocol::overwrite::{Overwrite, OverwriteParseError};
use rttp_protocol::permissions_policy::PermissionsPolicy;
use rttp_protocol::pragma::{Pragma, PragmaParseError};
use rttp_protocol::prefer::{Prefer, PreferenceApplied, PreferenceKind};
use rttp_protocol::proxy_authentication_info::ProxyAuthenticationInfo;
use rttp_protocol::proxy_status::{ProxyStatus, ProxyStatusParseError};
use rttp_protocol::referer::Referer;
use rttp_protocol::referrer_policy::{ReferrerPolicy, ReferrerPolicyToken};
use rttp_protocol::reporting_endpoints::ReportingEndpoints;
use rttp_protocol::save_data::SaveData;
use rttp_protocol::sec_gpc::SecGpc;
use rttp_protocol::sec_websocket_accept::SecWebSocketAccept;
use rttp_protocol::sec_websocket_key::SecWebSocketKey;
use rttp_protocol::sec_websocket_protocol::{SecWebSocketProtocol, SecWebSocketProtocolParseError};
use rttp_protocol::sec_websocket_version::SecWebSocketVersion;
use rttp_protocol::service_worker_allowed::ServiceWorkerAllowed;
use rttp_protocol::signature::{Signature, SignatureParseError};
use rttp_protocol::signature_input::{SignatureInput, SignatureInputParseError};
use rttp_protocol::strict_transport_security::StrictTransportSecurity;
use rttp_protocol::supports_loading_mode::SupportsLoadingMode;
use rttp_protocol::te::Te;
use rttp_protocol::timeout::{Timeout, TimeoutType};
use rttp_protocol::timing_allow_origin::TimingAllowOrigin;
use rttp_protocol::trace_context::{TraceParent, TraceState};
use rttp_protocol::transfer_encoding::TransferEncoding;
use rttp_protocol::upgrade::{Upgrade, UpgradeParseError};
use rttp_protocol::upgrade_insecure_requests::UpgradeInsecureRequests;
use rttp_protocol::via::{Via, ViaParseError};
use rttp_protocol::want_content_digest::WantContentDigest;
use rttp_protocol::want_repr_digest::WantReprDigest;
use rttp_protocol::warning::Warning;
use rttp_protocol::x_content_type_options::XContentTypeOptions;
use rttp_protocol::x_forwarded_for::{XForwardedFor, XForwardedForParseError};
use rttp_protocol::x_forwarded_host::{XForwardedHost, XForwardedHostParseError};
use rttp_protocol::x_forwarded_proto::{XForwardedProto, XForwardedProtoParseError};
use rttp_protocol::x_frame_options::XFrameOptions;

#[test]
fn protocol_exports_dav_response_metadata() {
  let dav =
    Dav::parse("1, 2, extended-mkcol, <https://dav.example.test/ns>").expect("DAV should parse");
  assert_eq!(
    &[
      DavClass::One,
      DavClass::Two,
      DavClass::ExtensionToken("extended-mkcol".to_string()),
      DavClass::CodedUrl("https://dav.example.test/ns".to_string()),
    ],
    dav.classes()
  );
  let _: DavParseError = Dav::parse("1, 1").expect_err("duplicate DAV class should be rejected");
}

#[test]
fn protocol_exports_representative_bounded_metadata_types() {
  let age = Age::parse("60").expect("Age should parse");
  let accept_language =
    AcceptLanguage::parse("en-US, fr-CA; q=0.8, *;q=0").expect("Accept-Language should parse");
  let accept_ch = AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let allow_credentials = AccessControlAllowCredentials::parse("true")
    .expect("Access-Control-Allow-Credentials should parse");
  let expose_headers = AccessControlExposeHeaders::parse("X-Request-Id")
    .expect("Access-Control-Expose-Headers should parse");
  let request_method =
    AccessControlRequestMethod::parse("patch").expect("Access-Control-Request-Method should parse");
  let request_private_network = AccessControlRequestPrivateNetwork::parse("true")
    .expect("Access-Control-Request-Private-Network should parse");
  let save_data = SaveData::parse("on").expect("Save-Data should parse");
  let sec_gpc = SecGpc::parse("1").expect("Sec-GPC should parse");
  let upgrade_insecure_requests =
    UpgradeInsecureRequests::parse("1").expect("Upgrade-Insecure-Requests should parse");
  let critical_ch = CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let entity_tag = EntityTag::parse("\"revision-42\"").expect("entity tag should parse");
  let expect = Expect::parse("100-continue, preview").expect("Expect should parse");
  let if_match = IfMatch::parse("\"revision-42\"").expect("If-Match should parse");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let sec_purpose = SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
  let from = From::parse("Ops Team <ops@example.test>").expect("From should parse");
  let nel =
    Nel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#).expect("NEL should parse");
  let keep_alive = KeepAlive::parse("timeout=5, max=100").expect("Keep-Alive should parse");
  let location = Location::parse("../login?next=%2Fdashboard").expect("Location should parse");
  let max_forwards = MaxForwards::parse("0").expect("Max-Forwards should parse");
  let destination = Destination::parse("https://dav.example.test/archive/report.txt")
    .expect("Destination should parse");
  let depth = Depth::parse("infinity").expect("Depth should parse");
  let lock_token = LockToken::parse("<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>")
    .expect("Lock-Token metadata should parse");
  let timeout = Timeout::parse("Second-60, Infinite").expect("Timeout should parse");
  let overwrite = Overwrite::parse("F").expect("Overwrite should parse");
  let _: OverwriteParseError =
    Overwrite::parse("t").expect_err("lowercase Overwrite should be rejected");
  let idempotency_key = IdempotencyKey::parse("charge-2026-08-19-9f3c")
    .expect("Idempotency-Key request metadata should parse");
  let sec_websocket_key = SecWebSocketKey::parse("dGhlIHNhbXBsZSBub25jZQ==")
    .expect("Sec-WebSocket-Key request metadata should parse");
  let sec_websocket_version =
    SecWebSocketVersion::parse("13").expect("Sec-WebSocket-Version metadata should parse");
  let sec_websocket_accept = SecWebSocketAccept::derive_from_key(&sec_websocket_key);
  let sec_websocket_protocol = SecWebSocketProtocol::parse("chat, superchat")
    .expect("Sec-WebSocket-Protocol offers should parse");
  let sec_websocket_protocol_selection =
    SecWebSocketProtocol::from_selection("chat").expect("Sec-WebSocket-Protocol should select");
  let _: SecWebSocketProtocolParseError = SecWebSocketProtocol::parse_selection("chat, superchat")
    .expect_err("multi-token Sec-WebSocket-Protocol selection should be rejected");
  let if_modified_since = IfModifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("If-Modified-Since should parse");
  let if_schedule_tag_match =
    IfScheduleTagMatch::parse("\"sched-17\"").expect("If-Schedule-Tag-Match should parse");
  let if_schedule_tag_match_weak =
    IfScheduleTagMatch::parse("W/\"sched-17\"").expect("weak If-Schedule-Tag-Match should parse");
  let if_unmodified_since = IfUnmodifiedSince::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("If-Unmodified-Since should parse");
  let if_header = If::parse(
    "<http://example.test/src> (<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) (Not [\"etag-one\"])",
  )
  .expect("WebDAV If request metadata should parse");
  let memento_datetime =
    MementoDatetime::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Memento-Datetime should parse");
  let host = Host::parse("example.test:8443").expect("Host should parse");
  let alt_used = AltUsed::parse("[2001:db8::1]:8443").expect("Alt-Used should parse");
  let origin = Origin::parse("https://example.test").expect("Origin should parse");
  let origin_trials = OriginTrials::parse_values(["token-one", "token-two"])
    .expect("Origin-Trial response metadata should parse");
  let no_vary_search =
    NoVarySearch::parse(r#"params=("utm_source")"#).expect("No-Vary-Search should parse");
  let permissions_policy =
    PermissionsPolicy::parse(r#"geolocation=(self "https://maps.example.test"), camera=()"#)
      .expect("Permissions-Policy should parse");
  let supports_loading_mode = SupportsLoadingMode::parse("fenced-frame, credentialed-prerender")
    .expect("Supports-Loading-Mode should parse");
  let proxy_authentication_info = ProxyAuthenticationInfo::parse(
    "nextnonce=\"xyz789\", qop=auth, rspauth=\"...\", cnonce=\"c\", nc=00000001",
  )
  .expect("Proxy-Authentication-Info should parse");
  let authorization = Authorization::parse("Bearer origin-token")
    .expect("Authorization request metadata should parse");
  let proxy_authorization = ProxyAuthorization::parse("Basic cHJveHk6c2VjcmV0")
    .expect("Proxy-Authorization request metadata should parse");
  let proxy_status =
    ProxyStatus::parse("ExampleCDN; error=connection_timeout").expect("Proxy-Status should parse");
  let _: ProxyStatusParseError =
    ProxyStatus::parse("").expect_err("empty Proxy-Status should be rejected");
  let reporting_endpoints = ReportingEndpoints::parse(
    r#"default="https://reports.example/default", csp="https://reports.example/csp""#,
  )
  .expect("Reporting-Endpoints should parse");
  let referer = Referer::parse("https://example.test/path?q=1").expect("Referer should parse");
  let timing_allow_origin =
    TimingAllowOrigin::parse("https://example.test").expect("Timing-Allow-Origin should parse");
  let warning = Warning::parse(r#"110 - "Response is Stale""#).expect("Warning should parse");
  let _signature_input = SignatureInput::parse(r#"sig1=("@method" "@path");created=1700000000"#)
    .expect("Signature-Input should parse");
  let x_content_type_options =
    XContentTypeOptions::parse("nosniff").expect("X-Content-Type-Options should parse");
  let x_frame_options = XFrameOptions::parse("SAMEORIGIN").expect("X-Frame-Options should parse");
  let cross_origin_embedder_policy =
    CrossOriginEmbedderPolicy::parse(r#"require-corp; report-to="coep""#)
      .expect("Cross-Origin-Embedder-Policy should parse");
  let cross_origin_embedder_policy_report_only =
    CrossOriginEmbedderPolicyReportOnly::parse(r#"require-corp; report-to="coep""#)
      .expect("Cross-Origin-Embedder-Policy-Report-Only should parse");
  let strict_transport_security =
    StrictTransportSecurity::parse("max-age=31536000; includeSubDomains; preload")
      .expect("Strict-Transport-Security should parse");
  let content_type =
    ContentType::parse("text/plain; charset=utf-8").expect("Content-Type should parse");
  let content_dpr = ContentDpr::parse("1.5").expect("Content-DPR should parse");
  let content_disposition =
    ContentDisposition::parse("attachment; filename=\"report.txt\"; filename*=UTF-8''report.txt")
      .expect("Content-Disposition should parse");
  let content_location = ContentLocation::parse("../representations/current.json")
    .expect("Content-Location should parse");
  let service_worker_allowed =
    ServiceWorkerAllowed::parse("/").expect("Service-Worker-Allowed should parse");
  let deprecation = Deprecation::parse("?1").expect("Deprecation should parse");
  let document_policy =
    DocumentPolicy::parse("oversized-images=2.0, unsized-media=?0, *;report-to=default")
      .expect("Document-Policy should parse");
  let _: DocumentPolicyParseError = DocumentPolicy::parse("unsized-media=src;foo=bar")
    .expect_err("Document-Policy with an unknown parameter should be rejected");
  let document_policy_report_only =
    DocumentPolicyReportOnly::parse("oversized-images=2.0, unsized-media=?0, *;report-to=default")
      .expect("Document-Policy-Report-Only should parse");
  let _: DocumentPolicyReportOnlyParseError =
    DocumentPolicyReportOnly::parse("unsized-media=src;foo=bar")
      .expect_err("Document-Policy-Report-Only with an unknown parameter should be rejected");
  let connection = Connection::parse("keep-alive, TE").expect("Connection should parse");
  let content_encoding = ContentEncoding::parse("gzip, br").expect("Content-Encoding should parse");
  let content_security_policy =
    ContentSecurityPolicy::parse("default-src 'self'; object-src 'none'")
      .expect("Content-Security-Policy should parse");
  let content_security_policy_report_only =
    ContentSecurityPolicyReportOnly::parse("default-src 'self'; report-to csp-endpoint")
      .expect("Content-Security-Policy-Report-Only should parse");
  let content_language =
    ContentLanguage::parse("fr-CA, es-419").expect("Content-Language should parse");
  let cache_status =
    CacheStatus::parse("OriginCache; hit; ttl=1100").expect("Cache-Status should parse");
  let cdn_cache_control =
    CdnCacheControl::parse("max-age=600, cdn-example=\"a, b\"").expect("CDN metadata should parse");
  let cdn_loop = CdnLoop::parse(r#"foo123.foocdn.example, barcdn.example; trace="abcdef""#)
    .expect("CDN-Loop request metadata should parse");
  let _: CdnLoopParseError =
    CdnLoop::parse("cdn; trace").expect_err("valueless CDN-Loop parameter should be rejected");
  let x_forwarded_for =
    XForwardedFor::parse("192.0.2.60, unknown").expect("X-Forwarded-For should parse");
  let _: XForwardedForParseError =
    XForwardedFor::parse("client.example").expect_err("invalid X-Forwarded-For should fail");
  let x_forwarded_host =
    XForwardedHost::parse("example.test:443").expect("X-Forwarded-Host should parse");
  let _: XForwardedHostParseError = XForwardedHost::parse("https://example.test")
    .expect_err("invalid X-Forwarded-Host should fail");
  let x_forwarded_proto = XForwardedProto::parse("https").expect("X-Forwarded-Proto should parse");
  let _: XForwardedProtoParseError =
    XForwardedProto::parse("https://").expect_err("invalid X-Forwarded-Proto should fail");
  let via = Via::parse("1.1 edge-a (TLS terminator), HTTP/2 upstream")
    .expect("Via request metadata should parse");
  let _: ViaParseError = Via::parse("1.1").expect_err("incomplete Via hop should be rejected");
  let accept_charset =
    AcceptCharset::parse("utf-8, iso-8859-1;q=0.5, *;q=0").expect("Accept-Charset should parse");
  let accept_encoding =
    AcceptEncoding::parse("gzip, br;q=0.8, identity;q=0").expect("Accept-Encoding should parse");
  let accept_ranges = AcceptRanges::parse("bytes, pages").expect("Accept-Ranges should parse");
  let transfer_encoding =
    TransferEncoding::parse("chunked").expect("Transfer-Encoding should parse");
  let te = Te::parse("gzip;q=0.5, trailers").expect("TE should parse");
  let baggage =
    Baggage::parse("tenant=acme;source=gateway,release=2026-08-19").expect("baggage should parse");
  let traceparent = TraceParent::parse("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
    .expect("traceparent should parse");
  let tracestate =
    TraceState::parse("rojo=00f067aa0ba902b7,congo=t61rcWkgMzE").expect("tracestate should parse");
  let upgrade = Upgrade::parse("websocket").expect("Upgrade should parse");
  let _: UpgradeParseError = Upgrade::parse("").expect_err("empty Upgrade should be rejected");
  let pragma = Pragma::parse("no-cache, community=private").expect("Pragma should parse");
  let _: PragmaParseError =
    Pragma::parse("no-cache, no-cache").expect_err("duplicate Pragma should be rejected");
  let want_content_digest =
    WantContentDigest::parse("sha-256=10, sha-512=0").expect("Want-Content-Digest should parse");
  let want_repr_digest =
    WantReprDigest::parse("sha-256=10, sha-512=0").expect("Want-Repr-Digest should parse");
  let signature = Signature::parse("sig1=:YWJj:").expect("Signature should parse");
  let _: SignatureParseError =
    Signature::parse("").expect_err("empty Signature should be rejected");
  let signature_input = SignatureInput::parse(r#"sig1=("@method" "@path");created=1618884473"#)
    .expect("Signature-Input should parse");
  let _: SignatureInputParseError =
    SignatureInput::parse("").expect_err("empty Signature-Input should be rejected");

  assert_eq!(age.seconds(), 60);
  assert_eq!(accept_language.ranges(), ["en-US", "fr-CA", "*"]);
  assert_eq!(accept_language.qualities(), [None, Some("0.8"), Some("0")]);
  assert_eq!(
    accept_language.header_value(),
    "en-US, fr-CA; q=0.8, *; q=0"
  );
  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(expose_headers.field_names(), ["x-request-id"]);
  assert_eq!(request_method.method(), "PATCH");
  assert_eq!(request_method.header_value(), "PATCH");
  assert_eq!(request_private_network.header_value(), "true");
  assert_eq!(save_data.header_value(), "on");
  assert_eq!(sec_gpc.header_value(), "1");
  assert_eq!(upgrade_insecure_requests.header_value(), "1");
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(entity_tag.opaque_tag(), "revision-42");
  assert!(expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "100-continue, preview");
  assert_eq!(Expect::expect_continue().header_value(), "100-continue");
  assert_eq!(
    if_match.entity_tags()[0].header_value(),
    entity_tag.header_value()
  );
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
  assert_eq!(sec_purpose.tokens(), ["prefetch", "vendor-ext"]);
  assert!(sec_purpose.contains_prefetch());
  assert_eq!(from.header_value(), "Ops Team <ops@example.test>");
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(keep_alive.timeout(), Some(5));
  assert_eq!(keep_alive.max(), Some(100));
  assert_eq!(keep_alive.header_value(), "timeout=5, max=100");
  assert_eq!(
    reporting_endpoints.endpoints(),
    [
      ("default", "https://reports.example/default"),
      ("csp", "https://reports.example/csp"),
    ]
  );
  assert_eq!(deprecation, Deprecation::Boolean(true));
  assert_eq!(deprecation.header_value(), "?1");
  assert_eq!(document_policy.directives().len(), 3);
  assert_eq!(
    document_policy.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
  assert_eq!(
    document_policy.directive("*").unwrap().report_to(),
    Some("default")
  );
  assert_eq!(document_policy_report_only.directives().len(), 3);
  assert_eq!(
    document_policy_report_only.header_value(),
    "oversized-images=2.0, unsized-media=?0, *;report-to=default"
  );
  assert_eq!(
    document_policy_report_only
      .directive("*")
      .unwrap()
      .report_to(),
    Some("default")
  );
  assert_eq!(location.as_str(), "../login?next=%2Fdashboard");
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
  assert_eq!(Depth::Infinity, depth);
  assert_eq!("infinity", depth.header_value());
  assert_eq!(
    lock_token.as_str(),
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"
  );
  assert_eq!(
    lock_token.header_value(),
    "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>"
  );
  assert!(!format!("{lock_token:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert_eq!(
    &[TimeoutType::Second(60), TimeoutType::Infinite],
    timeout.members()
  );
  assert_eq!("second-60, infinite", timeout.header_value());
  assert_eq!(Overwrite::F, overwrite);
  assert_eq!("F", overwrite.header_value());
  assert_eq!(idempotency_key.as_str(), "charge-2026-08-19-9f3c");
  assert_eq!(idempotency_key.header_value(), "charge-2026-08-19-9f3c");
  assert!(!format!("{idempotency_key:?}").contains("charge-2026-08-19-9f3c"));
  assert_eq!(sec_websocket_key.as_str(), "dGhlIHNhbXBsZSBub25jZQ==");
  assert_eq!(sec_websocket_key.header_value(), "dGhlIHNhbXBsZSBub25jZQ==");
  assert!(!format!("{sec_websocket_key:?}").contains("dGhlIHNhbXBsZSBub25jZQ=="));
  assert_eq!(sec_websocket_version.versions(), ["13"]);
  assert!(sec_websocket_version.contains("13"));
  assert_eq!(sec_websocket_version.header_value(), "13");
  assert_eq!(sec_websocket_protocol.protocols(), ["chat", "superchat"]);
  assert!(sec_websocket_protocol.contains("chat"));
  assert_eq!(sec_websocket_protocol.header_value(), "chat, superchat");
  assert_eq!(sec_websocket_protocol_selection.selected(), Some("chat"));
  assert_eq!(sec_websocket_protocol_selection.header_value(), "chat");
  assert_eq!(
    sec_websocket_accept.as_str(),
    "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
  );
  assert!(sec_websocket_accept.verify_key(&sec_websocket_key));
  assert!(!format!("{sec_websocket_accept:?}").contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
  assert_eq!(
    if_modified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(
    if_schedule_tag_match.entity_tag().header_value(),
    "\"sched-17\""
  );
  assert_eq!(if_schedule_tag_match.opaque_tag(), "sched-17");
  assert!(!if_schedule_tag_match.is_weak());
  assert_eq!(if_schedule_tag_match.header_value(), "\"sched-17\"");
  assert_eq!(if_schedule_tag_match_weak.opaque_tag(), "sched-17");
  assert!(if_schedule_tag_match_weak.is_weak());
  assert_eq!(if_schedule_tag_match_weak.header_value(), "W/\"sched-17\"");
  assert_eq!(
    if_unmodified_since.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert!(if_header.is_tagged());
  assert_eq!(2, if_header.lists().len());
  assert_eq!(
    if_header.header_value(),
    "<http://example.test/src> (<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) \
     <http://example.test/src> (Not [\"etag-one\"])"
  );
  assert!(if_header.lists()[1].conditions()[0].is_negated());
  assert!(if_header.lists()[1].conditions()[0]
    .predicate()
    .is_entity_tag());
  let _request_if_predicate: IfPredicate = if_header.lists()[0].conditions()[0].predicate().clone();
  let _: IfParseError = If::parse(
    "(<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>) <http://example.test/src> (Not <DAV:no-lock>)",
  )
  .expect_err("mixed tagged and untagged If should be rejected");
  let _: IfParseError = If::parse("(Not<DAV:no-lock>)")
    .expect_err("Not without required whitespace should be rejected");
  let _: IfParseError =
    If::parse(r#"(Not["etag"])"#).expect_err("Not without required whitespace should be rejected");
  assert!(!format!("{if_header:?}").contains("550e8400-e29b-41d4-a716-446655440000"));
  assert_eq!(
    memento_datetime.header_value(),
    "Sun, 06 Nov 1994 08:49:37 GMT"
  );
  assert_eq!(host.host(), "example.test");
  assert_eq!(host.port(), Some("8443"));
  assert_eq!(alt_used.host(), "[2001:db8::1]");
  assert_eq!(alt_used.port(), Some("8443"));
  assert_eq!(alt_used.header_value(), "[2001:db8::1]:8443");
  assert_eq!(origin_trials.tokens(), ["token-one", "token-two"]);
  assert!(!format!("{origin_trials:?}").contains("token-one"));
  assert_eq!(origin.header_value(), "https://example.test");
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
    no_vary_search.params(),
    Some(&NoVarySearchParams::Names(vec!["utm_source".to_owned()]))
  );
  assert_eq!(
    proxy_status.members()[0].identifier().as_str(),
    "ExampleCDN"
  );
  assert_eq!(
    proxy_authentication_info.parameter("nextnonce"),
    Some("xyz789")
  );
  assert_eq!(authorization.scheme(), "Bearer");
  assert_eq!(authorization.header_value(), "Bearer origin-token");
  assert_eq!(proxy_authorization.scheme(), "Basic");
  assert_eq!(proxy_authorization.header_value(), "Basic cHJveHk6c2VjcmV0");
  assert_eq!(referer.header_value(), "https://example.test/path?q=1");
  assert_eq!(timing_allow_origin.origins(), ["https://example.test"]);
  assert_eq!(warning.items()[0].code(), 110);
  assert_eq!(warning.items()[0].text(), "Response is Stale");
  assert_eq!(signature_input.members()[0].label(), "sig1");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@path");created=1618884473"#
  );
  assert_eq!(x_content_type_options.header_value(), "nosniff");
  assert_eq!(x_frame_options.header_value(), "SAMEORIGIN");
  assert_eq!(cross_origin_embedder_policy.header_value(), "require-corp");
  assert_eq!(
    cross_origin_embedder_policy_report_only.header_value(),
    "require-corp"
  );
  assert_eq!(
    strict_transport_security.header_value(),
    "max-age=31536000; includeSubDomains; preload"
  );
  assert_eq!(content_type.header_value(), "text/plain; charset=utf-8");
  assert_eq!(content_dpr.ratio(), 1.5);
  assert_eq!(content_dpr.header_value(), "1.5");
  assert_eq!(content_disposition.disposition_type(), "attachment");
  assert_eq!(content_disposition.filename(), Some("report.txt"));
  assert_eq!(
    content_disposition.filename_ext(),
    Some("UTF-8''report.txt")
  );
  assert_eq!(
    content_disposition.header_value(),
    "attachment; filename=report.txt; filename*=UTF-8''report.txt"
  );
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(service_worker_allowed.header_value(), "/");
  assert_eq!(service_worker_allowed.as_str(), "/");
  assert_eq!(connection.tokens(), ["keep-alive", "TE"]);
  assert_eq!(connection.header_value(), "keep-alive, TE");
  assert_eq!(content_encoding.codings(), ["gzip", "br"]);
  assert_eq!(content_encoding.header_value(), "gzip, br");
  assert_eq!(
    content_security_policy.header_value(),
    "default-src 'self'; object-src 'none'"
  );
  assert_eq!(
    content_security_policy_report_only.header_value(),
    "default-src 'self'; report-to csp-endpoint"
  );
  assert_eq!(content_language.tags(), ["fr-CA", "es-419"]);
  assert_eq!(content_language.header_value(), "fr-CA, es-419");
  assert_eq!(
    cache_status.members()[0].identifier().as_str(),
    "OriginCache"
  );
  assert_eq!(cache_status.members()[0].ttl(), Some(1100));
  assert_eq!(cdn_cache_control.directives()[1].name(), "cdn-example");
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
  assert_eq!(cdn_loop.members()[0].identifier(), "foo123.foocdn.example");
  assert_eq!(cdn_loop.members()[1].parameter("trace"), Some("abcdef"));
  assert_eq!("192.0.2.60", x_forwarded_for.nodes()[0].value());
  assert!(x_forwarded_for.nodes()[1].is_unknown());
  assert_eq!("example.test", x_forwarded_host.hosts()[0].host());
  assert_eq!(Some("443"), x_forwarded_host.hosts()[0].port());
  assert_eq!(["https".to_string()], x_forwarded_proto.schemes());
  assert_eq!("edge-a", via.members()[0].received_by());
  assert_eq!(Some("HTTP"), via.members()[1].protocol_name());
  assert_eq!(accept_charset.charsets()[0].charset(), "utf-8");
  assert_eq!(accept_charset.charsets()[0].quality(), 1000);
  assert_eq!(accept_charset.charsets()[1].charset(), "iso-8859-1");
  assert_eq!(accept_charset.charsets()[1].quality(), 500);
  assert_eq!(accept_charset.charsets()[2].charset(), "*");
  assert_eq!(accept_charset.charsets()[2].quality(), 0);
  assert!(accept_charset.charsets()[2].is_wildcard());
  assert_eq!(
    accept_charset.header_value(),
    "utf-8, iso-8859-1;q=0.5, *;q=0"
  );
  assert_eq!(accept_encoding.codings()[0].coding(), "gzip");
  assert_eq!(accept_encoding.codings()[0].quality(), 1000);
  assert_eq!(accept_encoding.codings()[1].coding(), "br");
  assert_eq!(accept_encoding.codings()[1].quality(), 800);
  assert_eq!(accept_encoding.codings()[2].coding(), "identity");
  assert_eq!(accept_encoding.codings()[2].quality(), 0);
  assert_eq!(
    accept_encoding.header_value(),
    "gzip, br;q=0.8, identity;q=0"
  );
  assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages");
  assert_eq!(transfer_encoding.codings(), ["chunked"]);
  assert_eq!(transfer_encoding.header_value(), "chunked");
  assert_eq!(te.codings()[0].coding(), "gzip");
  assert_eq!(te.codings()[0].quality(), Some(500));
  assert_eq!(te.codings()[1].coding(), "trailers");
  assert!(te.codings()[1].is_trailers());
  assert_eq!(2, baggage.members().len());
  assert_eq!("tenant", baggage.members()[0].key());
  assert_eq!("acme", baggage.members()[0].value());
  assert_eq!("source", baggage.members()[0].properties()[0].key());
  assert_eq!("00", traceparent.version());
  assert_eq!("4bf92f3577b34da6a3ce929d0e0e4736", traceparent.trace_id());
  assert_eq!(2, tracestate.members().len());
  assert_eq!("rojo", tracestate.members()[0].key());
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert_eq!(upgrade.header_value(), "websocket");
  assert!(pragma.no_cache());
  assert_eq!(pragma.directives()[1].name(), "community");
  assert_eq!(pragma.directives()[1].value(), Some("private"));
  assert_eq!(pragma.header_value(), "no-cache, community=private");
  assert_eq!(want_content_digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(want_content_digest.entries()[0].preference(), 10);
  assert_eq!(want_content_digest.header_value(), "sha-256=10, sha-512=0");
  assert_eq!(want_repr_digest.entries()[0].algorithm(), "sha-256");
  assert_eq!(want_repr_digest.entries()[0].preference(), 10);
  assert_eq!(want_repr_digest.header_value(), "sha-256=10, sha-512=0");
  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@path");created=1618884473"#
  );
}

#[test]
fn protocol_exports_structured_preference_metadata() {
  let prefer = Prefer::parse(
    "return=representation, wait=10; priority=high, example=\"quoted value\"; mode=fast",
  )
  .expect("Prefer should parse");
  let applied = PreferenceApplied::parse("respond-async; accepted=true")
    .expect("Preference-Applied should parse");

  assert_eq!(prefer.preferences().len(), 3);
  assert_eq!(prefer.preferences()[0].kind(), PreferenceKind::Return);
  assert_eq!(prefer.preferences()[1].parameters()[0].name(), "priority");
  assert_eq!(
    prefer.header_value(),
    "return=representation, wait=10; priority=high, example=\"quoted value\"; mode=fast"
  );
  assert_eq!(
    applied.preferences()[0].kind(),
    PreferenceKind::RespondAsync
  );
  assert_eq!(
    applied.preferences()[0].parameters()[0].value(),
    Some("true")
  );
}

#[test]
fn protocol_exports_bounded_referrer_policy_metadata() {
  let policy =
    ReferrerPolicy::parse_values(["strict-origin-when-cross-origin, origin", "no-referrer"])
      .expect("Referrer-Policy fields should parse");

  assert_eq!(
    policy.policies(),
    &[
      ReferrerPolicyToken::StrictOriginWhenCrossOrigin,
      ReferrerPolicyToken::Origin,
      ReferrerPolicyToken::NoReferrer,
    ]
  );
  assert_eq!(
    policy.header_value(),
    "strict-origin-when-cross-origin, origin, no-referrer"
  );
}

#[test]
fn protocol_exports_bounded_cross_origin_opener_policy_metadata() {
  let cross_origin_opener_policy =
    CrossOriginOpenerPolicy::parse(r#"noopener-allow-popups; report-to="coop""#)
      .expect("Cross-Origin-Opener-Policy should parse");
  let cross_origin_opener_policy_report_only =
    CrossOriginOpenerPolicyReportOnly::parse(r#"same-origin; report-to="coop"; endpoint="canary""#)
      .expect("Cross-Origin-Opener-Policy-Report-Only should parse");

  assert_eq!(
    cross_origin_opener_policy.header_value(),
    "noopener-allow-popups"
  );
  assert_eq!(
    CrossOriginOpenerPolicy::SameOrigin,
    cross_origin_opener_policy_report_only.policy()
  );
  assert_eq!(
    Some("coop"),
    cross_origin_opener_policy_report_only.report_to()
  );
  assert_eq!(
    cross_origin_opener_policy_report_only.header_value(),
    r#"same-origin; report-to="coop"; endpoint="canary""#
  );
}

#[test]
fn protocol_exports_bounded_link_metadata() {
  let links = LinkValues::parse(
    "</style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload",
  )
  .expect("Link should parse");

  assert_eq!(2, links.len());
  assert_eq!("/style.css", links.values()[0].target());
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
  assert_eq!(Some("style"), links.values()[0].parameter("as"));
  assert_eq!(
    "https://cdn.example.test/app.js",
    links.values()[1].target()
  );
  assert_eq!(Some("modulepreload"), links.values()[1].parameter("rel"));
}
