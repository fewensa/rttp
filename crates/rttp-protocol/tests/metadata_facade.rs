use rttp_protocol::accept_ranges::AcceptRanges;
use rttp_protocol::access_control_allow_credentials::AccessControlAllowCredentials;
use rttp_protocol::access_control_expose_headers::AccessControlExposeHeaders;
use rttp_protocol::access_control_request_method::AccessControlRequestMethod;
use rttp_protocol::access_control_request_private_network::AccessControlRequestPrivateNetwork;
use rttp_protocol::age::Age;
use rttp_protocol::cache_status::CacheStatus;
use rttp_protocol::cdn_cache_control::CdnCacheControl;
use rttp_protocol::client_hints::{AcceptCh, CriticalCh};
use rttp_protocol::connection::Connection;
use rttp_protocol::content_disposition::ContentDisposition;
use rttp_protocol::content_dpr::ContentDpr;
use rttp_protocol::content_encoding::ContentEncoding;
use rttp_protocol::content_language::ContentLanguage;
use rttp_protocol::content_location::ContentLocation;
use rttp_protocol::content_security_policy::ContentSecurityPolicy;
use rttp_protocol::content_type::ContentType;
use rttp_protocol::cross_origin_embedder_policy::CrossOriginEmbedderPolicy;
use rttp_protocol::cross_origin_embedder_policy_report_only::CrossOriginEmbedderPolicyReportOnly;
use rttp_protocol::cross_origin_opener_policy::CrossOriginOpenerPolicy;
use rttp_protocol::deprecation::Deprecation;
use rttp_protocol::entity_tag::{EntityTag, IfMatch};
use rttp_protocol::fetch_metadata::{
  SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose,
};
use rttp_protocol::from::From;
use rttp_protocol::host::Host;
use rttp_protocol::http_date::{ResponseDate, ResponseExpires, ResponseLastModified};
use rttp_protocol::keep_alive::KeepAlive;
use rttp_protocol::link::LinkValues;
use rttp_protocol::location::Location;
use rttp_protocol::max_forwards::MaxForwards;
use rttp_protocol::memento_datetime::MementoDatetime;
use rttp_protocol::nel::Nel;
use rttp_protocol::no_vary_search::{NoVarySearch, NoVarySearchParams};
use rttp_protocol::origin::Origin;
use rttp_protocol::prefer::{Prefer, PreferenceApplied, PreferenceKind};
use rttp_protocol::proxy_authentication_info::ProxyAuthenticationInfo;
use rttp_protocol::proxy_status::{ProxyStatus, ProxyStatusParseError};
use rttp_protocol::referer::Referer;
use rttp_protocol::referrer_policy::{ReferrerPolicy, ReferrerPolicyToken};
use rttp_protocol::save_data::SaveData;
use rttp_protocol::signature::{Signature, SignatureParseError};
use rttp_protocol::signature_input::{SignatureInput, SignatureInputParseError};
use rttp_protocol::strict_transport_security::StrictTransportSecurity;
use rttp_protocol::timing_allow_origin::TimingAllowOrigin;
use rttp_protocol::transfer_encoding::TransferEncoding;
use rttp_protocol::upgrade::{Upgrade, UpgradeParseError};
use rttp_protocol::want_content_digest::WantContentDigest;
use rttp_protocol::want_repr_digest::WantReprDigest;
use rttp_protocol::warning::Warning;
use rttp_protocol::x_content_type_options::XContentTypeOptions;
use rttp_protocol::x_frame_options::XFrameOptions;

#[test]
fn protocol_exports_representative_bounded_metadata_types() {
  let age = Age::parse("60").expect("Age should parse");
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
  let critical_ch = CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let entity_tag = EntityTag::parse("\"revision-42\"").expect("entity tag should parse");
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
  let memento_datetime =
    MementoDatetime::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Memento-Datetime should parse");
  let response_date =
    ResponseDate::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Date should parse");
  let response_expires =
    ResponseExpires::parse("Sun, 06 Nov 1994 08:49:37 GMT").expect("Expires should parse");
  let response_last_modified = ResponseLastModified::parse("Sun, 06 Nov 1994 08:49:37 GMT")
    .expect("Last-Modified should parse");
  let host = Host::parse("example.test:8443").expect("Host should parse");
  let origin = Origin::parse("https://example.test").expect("Origin should parse");
  let no_vary_search =
    NoVarySearch::parse(r#"params=("utm_source")"#).expect("No-Vary-Search should parse");
  let proxy_authentication_info = ProxyAuthenticationInfo::parse(
    "nextnonce=\"xyz789\", qop=auth, rspauth=\"...\", cnonce=\"c\", nc=00000001",
  )
  .expect("Proxy-Authentication-Info should parse");
  let proxy_status =
    ProxyStatus::parse("ExampleCDN; error=connection_timeout").expect("Proxy-Status should parse");
  let _: ProxyStatusParseError =
    ProxyStatus::parse("").expect_err("empty Proxy-Status should be rejected");
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
  let deprecation = Deprecation::parse("?1").expect("Deprecation should parse");
  let connection = Connection::parse("keep-alive, TE").expect("Connection should parse");
  let content_encoding = ContentEncoding::parse("gzip, br").expect("Content-Encoding should parse");
  let content_security_policy =
    ContentSecurityPolicy::parse("default-src 'self'; object-src 'none'")
      .expect("Content-Security-Policy should parse");
  let content_language =
    ContentLanguage::parse("fr-CA, es-419").expect("Content-Language should parse");
  let cache_status =
    CacheStatus::parse("OriginCache; hit; ttl=1100").expect("Cache-Status should parse");
  let cdn_cache_control =
    CdnCacheControl::parse("max-age=600, cdn-example=\"a, b\"").expect("CDN metadata should parse");
  let accept_ranges = AcceptRanges::parse("bytes, pages").expect("Accept-Ranges should parse");
  let transfer_encoding =
    TransferEncoding::parse("chunked").expect("Transfer-Encoding should parse");
  let upgrade = Upgrade::parse("websocket").expect("Upgrade should parse");
  let _: UpgradeParseError = Upgrade::parse("").expect_err("empty Upgrade should be rejected");
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
  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_credentials.header_value(), "true");
  assert_eq!(expose_headers.field_names(), ["x-request-id"]);
  assert_eq!(request_method.method(), "PATCH");
  assert_eq!(request_method.header_value(), "PATCH");
  assert_eq!(request_private_network.header_value(), "true");
  assert_eq!(save_data.header_value(), "on");
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(entity_tag.opaque_tag(), "revision-42");
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
  assert_eq!(deprecation, Deprecation::Boolean(true));
  assert_eq!(deprecation.header_value(), "?1");
  assert_eq!(location.as_str(), "../login?next=%2Fdashboard");
  assert_eq!(max_forwards.value(), 0);
  assert_eq!(max_forwards.header_value(), "0");
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
  assert_eq!(host.host(), "example.test");
  assert_eq!(host.port(), Some("8443"));
  assert_eq!(origin.header_value(), "https://example.test");
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
  assert_eq!(connection.tokens(), ["keep-alive", "TE"]);
  assert_eq!(connection.header_value(), "keep-alive, TE");
  assert_eq!(content_encoding.codings(), ["gzip", "br"]);
  assert_eq!(content_encoding.header_value(), "gzip, br");
  assert_eq!(
    content_security_policy.header_value(),
    "default-src 'self'; object-src 'none'"
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
  assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages");
  assert_eq!(transfer_encoding.codings(), ["chunked"]);
  assert_eq!(transfer_encoding.header_value(), "chunked");
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert_eq!(upgrade.header_value(), "websocket");
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

  assert_eq!(
    cross_origin_opener_policy.header_value(),
    "noopener-allow-popups"
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
