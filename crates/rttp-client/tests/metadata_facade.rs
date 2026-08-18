use rttp_client::response::{
  AcceptCh, AccessControlAllowHeaders, AccessControlAllowHeadersParseError,
  AccessControlAllowMethods, AccessControlAllowMethodsParseError, AccessControlExposeHeaders,
  AccessControlMaxAge, AccessControlMaxAgeParseError, AltSvc, CrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyReportOnly, CrossOriginOpenerPolicy, CrossOriginResourcePolicy, Digest,
  HttpClearSiteData, HttpContentLength, PreferenceApplied, Priority, ProxyAuthenticationInfo,
  ProxyAuthenticationInfoParseError, ReferrerPolicy, ReferrerPolicyToken, ServerTiming,
  StrictTransportSecurity, StrictTransportSecurityParseError, Trailer, WantReprDigest, Warning,
};
use rttp_client::{SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser};

#[test]
fn response_facade_exports_representative_bounded_metadata_types() {
  let accept_ch = AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let allow_methods = AccessControlAllowMethods::parse("GET, POST")
    .expect("Access-Control-Allow-Methods should parse");
  let _: AccessControlAllowMethodsParseError = AccessControlAllowMethods::parse("")
    .expect_err("empty Access-Control-Allow-Methods should be rejected");
  let allow_headers = AccessControlAllowHeaders::parse("X-Request-Id, ETag")
    .expect("Access-Control-Allow-Headers should parse");
  let _: AccessControlAllowHeadersParseError = AccessControlAllowHeaders::parse("")
    .expect_err("empty Access-Control-Allow-Headers should be rejected");
  let max_age = AccessControlMaxAge::parse("60").expect("Access-Control-Max-Age should parse");
  let _: AccessControlMaxAgeParseError =
    AccessControlMaxAge::parse("").expect_err("empty Access-Control-Max-Age should be rejected");
  let expose_headers = AccessControlExposeHeaders::parse("X-Request-Id")
    .expect("Access-Control-Expose-Headers should parse");
  let clear_site_data =
    HttpClearSiteData::parse("\"cache\"").expect("Clear-Site-Data should parse");
  let digest = Digest::parse("sha-256=:YWJj:").expect("Digest should parse");
  let want_repr_digest =
    WantReprDigest::parse("sha-256=10").expect("Want-Repr-Digest should parse");
  let priority = Priority::parse("u=1, i").expect("Priority should parse");
  let server_timing = ServerTiming::parse("db;dur=53").expect("Server-Timing should parse");
  let strict_transport_security =
    StrictTransportSecurity::parse("max-age=31536000; includeSubDomains")
      .expect("Strict-Transport-Security should parse");
  let _: StrictTransportSecurityParseError = StrictTransportSecurity::parse("includeSubDomains")
    .expect_err("Strict-Transport-Security without max-age should be rejected");
  let warning = Warning::parse(r#"110 - "Response is Stale""#).expect("Warning should parse");
  let trailer = Trailer::parse("X-Trace").expect("Trailer should parse");
  let alt_svc = AltSvc::parse("h3=\":443\"").expect("Alt-Svc should parse");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let cross_origin_resource_policy =
    CrossOriginResourcePolicy::parse("same-origin").expect("CORP should parse");
  let cross_origin_embedder_policy =
    CrossOriginEmbedderPolicy::parse("require-corp").expect("COEP should parse");
  let cross_origin_embedder_policy_report_only =
    CrossOriginEmbedderPolicyReportOnly::parse("require-corp")
      .expect("COEP-Report-Only should parse");
  let cross_origin_opener_policy =
    CrossOriginOpenerPolicy::parse("noopener-allow-popups").expect("COOP should parse");
  let proxy_authentication_info =
    ProxyAuthenticationInfo::parse(r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth"#)
      .expect("Proxy-Authentication-Info should parse");
  let _: ProxyAuthenticationInfoParseError = ProxyAuthenticationInfo::parse("")
    .expect_err("empty Proxy-Authentication-Info should be rejected");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_methods.methods(), ["GET", "POST"]);
  assert_eq!(allow_headers.field_names(), ["x-request-id", "etag"]);
  assert_eq!(max_age.seconds(), 60);
  assert_eq!(expose_headers.field_names(), ["x-request-id"]);
  assert_eq!(clear_site_data.directives().len(), 1);
  assert_eq!(digest.entries().len(), 1);
  assert_eq!(want_repr_digest.entries()[0].preference(), 10);
  assert_eq!(priority.urgency(), Some(1));
  assert_eq!(server_timing.metrics().len(), 1);
  assert_eq!(strict_transport_security.max_age(), 31_536_000);
  assert!(strict_transport_security.include_sub_domains());
  assert_eq!(warning.items()[0].code(), 110);
  assert_eq!(trailer.field_names(), ["x-trace"]);
  assert_eq!(alt_svc.alternatives().len(), 1);
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
  assert_eq!(cross_origin_resource_policy.header_value(), "same-origin");
  assert_eq!(cross_origin_embedder_policy.header_value(), "require-corp");
  assert_eq!(
    cross_origin_embedder_policy_report_only.header_value(),
    "require-corp"
  );
  assert_eq!(
    cross_origin_opener_policy.header_value(),
    "noopener-allow-popups"
  );
  assert_eq!(
    proxy_authentication_info.parameter("nextnonce"),
    Some("6629fae49393a05397450978507c4ef1")
  );
}

#[test]
fn response_facade_exports_content_length_metadata_type() {
  let content_length = HttpContentLength::new(2);

  assert_eq!(2, content_length.len());
  assert!(!content_length.is_zero());
  assert_eq!("2", content_length.header_value());
}

#[test]
fn direct_response_new_does_not_infer_content_length_metadata() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK".to_vec(),
  )
  .expect("response should parse");

  assert_eq!(None, response.content_length());
}

#[test]
fn response_facade_parses_preference_applied_metadata() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nPreference-Applied: return=minimal; source=cache\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  let applied: PreferenceApplied = response
    .preference_applied()
    .expect("Preference-Applied should parse")
    .expect("Preference-Applied should be present");

  assert_eq!(applied.preferences()[0].name(), "return");
  assert_eq!(applied.preferences()[0].value(), Some("minimal"));
  assert_eq!(applied.preferences()[0].parameters()[0].name(), "source");
}

#[test]
fn response_facade_parses_referrer_policy_metadata() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Referrer-Policy: strict-origin\r\n",
      "Referrer-Policy: no-referrer, origin\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let policy: ReferrerPolicy = response
    .referrer_policy()
    .expect("Referrer-Policy should parse")
    .expect("Referrer-Policy should be present");

  assert_eq!(
    policy.policies(),
    &[
      ReferrerPolicyToken::StrictOrigin,
      ReferrerPolicyToken::NoReferrer,
      ReferrerPolicyToken::Origin,
    ]
  );
}
