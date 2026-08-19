use rttp_client::response::{
  AcceptCh, AccessControlAllowHeaders, AccessControlAllowHeadersParseError,
  AccessControlAllowMethods, AccessControlAllowMethodsParseError, AccessControlExposeHeaders,
  AccessControlMaxAge, AccessControlMaxAgeParseError, Age, AgeParseError, AltSvc, Connection,
  ConnectionParseError, CrossOriginEmbedderPolicy, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginOpenerPolicy, CrossOriginResourcePolicy, Digest, HttpClearSiteData, KeepAlive,
  LinkValues, Location, LocationParseError, NoVarySearch, NoVarySearchParams,
  NoVarySearchParseError, PreferenceApplied, Priority, ProxyAuthenticate,
  ProxyAuthenticateParseError, ProxyAuthenticationInfo, ProxyAuthenticationInfoParseError,
  ReferrerPolicy, ReferrerPolicyToken, ServerTiming, Signature, SignatureInput,
  SignatureInputParseError, SignatureParseError, StrictTransportSecurity,
  StrictTransportSecurityParseError, Trailer, TransferEncoding, TransferEncodingParseError, Vary,
  VaryParseError, WantContentDigest, WantReprDigest, Warning, XContentTypeOptions,
  XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
};
use rttp_client::response::{
  ContentDigest, ContentLocation, ContentLocationParseError, HttpContentLength, ReprDigest,
};
use rttp_client::{HttpClient, SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser};
use rttp_test_support as support;

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
  let age = Age::parse("60").expect("Age should parse");
  let _: AgeParseError = Age::parse("").expect_err("empty Age should be rejected");
  let expose_headers = AccessControlExposeHeaders::parse("X-Request-Id")
    .expect("Access-Control-Expose-Headers should parse");
  let clear_site_data =
    HttpClearSiteData::parse("\"cache\"").expect("Clear-Site-Data should parse");
  let content_location = ContentLocation::parse("../representations/current.json")
    .expect("Content-Location should parse");
  let _: ContentLocationParseError =
    ContentLocation::parse("not valid").expect_err("invalid Content-Location should be rejected");
  let digest = Digest::parse("sha-256=:YWJj:").expect("Digest should parse");
  let location = Location::parse("/next").expect("Location should parse");
  let _: LocationParseError = Location::parse("").expect_err("empty Location should be rejected");
  let no_vary_search =
    NoVarySearch::parse(r#"params=("utm_source")"#).expect("No-Vary-Search should parse");
  let _: NoVarySearchParseError =
    NoVarySearch::parse("params=utm").expect_err("invalid No-Vary-Search should be rejected");
  let want_content_digest =
    WantContentDigest::parse("sha-256=10").expect("Want-Content-Digest should parse");
  let want_repr_digest =
    WantReprDigest::parse("sha-256=10").expect("Want-Repr-Digest should parse");
  let priority = Priority::parse("u=1, i").expect("Priority should parse");
  let server_timing = ServerTiming::parse("db;dur=53").expect("Server-Timing should parse");
  let keep_alive = KeepAlive::parse("timeout=5, max=100").expect("Keep-Alive should parse");
  let strict_transport_security =
    StrictTransportSecurity::parse("max-age=31536000; includeSubDomains")
      .expect("Strict-Transport-Security should parse");
  let _: StrictTransportSecurityParseError = StrictTransportSecurity::parse("includeSubDomains")
    .expect_err("Strict-Transport-Security without max-age should be rejected");
  let x_content_type_options =
    XContentTypeOptions::parse("NoSniff").expect("X-Content-Type-Options should parse");
  let _: XContentTypeOptionsParseError = XContentTypeOptions::parse("unknown")
    .expect_err("unknown X-Content-Type-Options should be rejected");
  let x_frame_options = XFrameOptions::parse("deny").expect("X-Frame-Options should parse");
  let _: XFrameOptionsParseError = XFrameOptions::parse("ALLOW-FROM https://example.test")
    .expect_err("deprecated X-Frame-Options ALLOW-FROM should be rejected");
  let warning = Warning::parse(r#"110 - "Response is Stale""#).expect("Warning should parse");
  let trailer = Trailer::parse("X-Trace").expect("Trailer should parse");
  let connection = Connection::parse("close").expect("Connection should parse");
  let _: ConnectionParseError =
    Connection::parse("close; foo").expect_err("parameterized Connection should be rejected");
  let transfer_encoding =
    TransferEncoding::parse("chunked").expect("Transfer-Encoding should parse");
  let _: TransferEncodingParseError = TransferEncoding::parse("gzip, chunked")
    .expect_err("non-sole chunked Transfer-Encoding should be rejected");
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
  let proxy_authenticate =
    ProxyAuthenticate::parse(r#"Basic realm="corp""#).expect("Proxy-Authenticate should parse");
  let _: ProxyAuthenticateParseError =
    ProxyAuthenticate::parse("").expect_err("empty Proxy-Authenticate should be rejected");
  let vary = Vary::parse("Accept-Encoding, User-Agent").expect("Vary should parse");
  let _: VaryParseError = Vary::parse("").expect_err("empty Vary should be rejected");
  let signature = Signature::parse("sig1=:YWJj:").expect("Signature should parse");
  let _: SignatureParseError =
    Signature::parse("").expect_err("empty Signature should be rejected");
  let signature_input = SignatureInput::parse(
    r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#,
  )
  .expect("Signature-Input should parse");
  let _: SignatureInputParseError =
    SignatureInput::parse("").expect_err("empty Signature-Input should be rejected");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(allow_methods.methods(), ["GET", "POST"]);
  assert_eq!(allow_headers.field_names(), ["x-request-id", "etag"]);
  assert_eq!(max_age.seconds(), 60);
  assert_eq!(age.seconds(), 60);
  assert_eq!(expose_headers.field_names(), ["x-request-id"]);
  assert_eq!(clear_site_data.directives().len(), 1);
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(digest.entries().len(), 1);
  assert_eq!(location.as_str(), "/next");
  assert_eq!(
    no_vary_search.params(),
    Some(&NoVarySearchParams::Names(vec!["utm_source".to_owned()]))
  );
  assert_eq!(want_content_digest.entries()[0].preference(), 10);
  assert_eq!(want_repr_digest.entries()[0].preference(), 10);
  assert_eq!(priority.urgency(), Some(1));
  assert_eq!(server_timing.metrics().len(), 1);
  assert_eq!(strict_transport_security.max_age(), 31_536_000);
  assert!(strict_transport_security.include_sub_domains());
  assert_eq!(x_content_type_options, XContentTypeOptions::Nosniff);
  assert_eq!(x_content_type_options.header_value(), "nosniff");
  assert_eq!(x_frame_options, XFrameOptions::Deny);
  assert_eq!(x_frame_options.header_value(), "DENY");
  assert_eq!(warning.items()[0].code(), 110);
  assert_eq!(keep_alive.timeout(), Some(5));
  assert_eq!(keep_alive.max(), Some(100));
  assert_eq!(trailer.field_names(), ["x-trace"]);
  assert_eq!(connection.tokens(), ["close"]);
  assert_eq!(transfer_encoding.codings(), ["chunked"]);
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
  assert_eq!(
    proxy_authenticate.challenges()[0].parameter("realm"),
    Some("corp")
  );
  assert_eq!(vary.field_names(), ["accept-encoding", "user-agent"]);
  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@authority" "@path");created=1618884473;keyid="test-key""#
  );
}

#[test]
fn response_facade_parses_signature_metadata_pair() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      r#"Signature-Input: sig1=("@method" "@path");created=1618884473;keyid="test-key""#,
      "\r\n",
      "Signature: sig1=:YWJj:\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let signature = response
    .signature()
    .expect("Signature should parse")
    .expect("Signature should be present");
  let signature_input = response
    .signature_input()
    .expect("Signature-Input should parse")
    .expect("Signature-Input should be present");

  assert_eq!(signature.header_value(), "sig1=:YWJj:");
  assert_eq!(
    signature_input.header_value(),
    r#"sig1=("@method" "@path");created=1618884473;keyid="test-key""#
  );
}

#[test]
fn response_facade_exports_repr_digest_metadata() {
  let repr_digest = ReprDigest::parse("sha-512=:ZGVm:").expect("Repr-Digest should parse");

  assert_eq!(
    repr_digest.entry("sha-512").map(|entry| entry.value()),
    Some(&b"def"[..])
  );
}

#[test]
fn response_facade_exports_content_digest_metadata() {
  let content_digest = ContentDigest::parse("sha-256=:YWJj:").expect("Content-Digest should parse");

  assert_eq!(
    content_digest.entry("sha-256").map(|entry| entry.value()),
    Some(&b"abc"[..])
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
fn client_response_exposes_validated_content_length_metadata() {
  let (addr, _handle) = support::spawn_chunked_response_server(
    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
  );

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/"))
    .emit()
    .expect("response should parse");
  let content_length = response
    .content_length()
    .expect("validated fixed length should be retained");

  assert_eq!(2, content_length.len());
  assert_eq!("2", content_length.header_value());
}

#[test]
fn client_response_omits_content_length_metadata_for_chunked_framing() {
  let (addr, _handle) = support::spawn_chunked_response_server(concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "Connection: close\r\n",
    "\r\n",
    "2\r\nOK\r\n",
    "0\r\n\r\n"
  ));

  let response = HttpClient::new()
    .get()
    .url(format!("http://{addr}/"))
    .emit()
    .expect("response should parse");

  assert_eq!("OK", response.body().string().expect("body should decode"));
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
fn response_facade_parses_link_metadata() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    concat!(
      "HTTP/1.1 200 OK\r\n",
      "Link: </style.css>; rel=preload; as=style\r\n",
      "Link: <https://cdn.example.test/app.js>; rel=modulepreload\r\n",
      "\r\n"
    )
    .as_bytes()
    .to_vec(),
  )
  .expect("response should parse");

  let links: LinkValues = response
    .links()
    .expect("Link should parse")
    .expect("Link should be present");

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

#[test]
fn response_facade_parses_connection_from_http1_head() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  let connection: Connection = response
    .connection()
    .expect("Connection should parse")
    .expect("Connection should be present");

  assert_eq!(connection.tokens(), ["close"]);
  assert_eq!(connection.header_value(), "close");
  assert_eq!(
    response.header_value("Connection").map(String::as_str),
    Some("close")
  );
}

#[test]
fn response_facade_returns_none_when_connection_is_absent() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert!(response
    .connection()
    .expect("missing Connection should be accepted")
    .is_none());
}

#[test]
fn response_facade_parses_transfer_encoding_from_validated_chunked_framing() {
  use std::io::Cursor;

  use rttp_client::ConnectionReader;

  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Transfer-Encoding: chunked\r\n",
    "\r\n",
    "5\r\nhello\r\n",
    "0\r\n\r\n"
  );
  let url = url::Url::parse("http://example.test/").expect("url should parse");
  let mut cursor = Cursor::new(raw.as_bytes());
  let mut reader = ConnectionReader::new(&url, &mut cursor, false);
  let response = reader
    .response()
    .expect("chunked response framing should parse");

  let transfer_encoding: TransferEncoding = response
    .transfer_encoding()
    .expect("Transfer-Encoding should parse")
    .expect("Transfer-Encoding should be present");

  assert_eq!(transfer_encoding.codings(), ["chunked"]);
  assert_eq!(transfer_encoding.header_value(), "chunked");
  assert_eq!(
    response
      .header_value("Transfer-Encoding")
      .map(String::as_str),
    Some("chunked")
  );
}

#[test]
fn response_facade_returns_none_when_transfer_encoding_is_absent() {
  let response = rttp_client::response::Response::new(
    rttp_client::types::RoUrl::with("http://example.test/"),
    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response should parse");

  assert!(response
    .transfer_encoding()
    .expect("missing Transfer-Encoding should be accepted")
    .is_none());
}
