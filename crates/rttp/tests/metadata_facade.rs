use rttp::server::{
  HttpAcceptCh, HttpAccessControlRequestMethod, HttpAccessControlRequestPrivateNetwork,
  HttpConditionalMetadata, HttpContentLocation, HttpContentLocationParseError, HttpContentRange,
  HttpContentRangeParseError, HttpCrossOriginEmbedderPolicy,
  HttpCrossOriginEmbedderPolicyReportOnly, HttpCrossOriginOpenerPolicy,
  HttpCrossOriginResourcePolicy, HttpEntityTag, HttpNel, HttpResponse, HttpSaveData, HttpSignature,
  HttpSignatureInput, HttpSignatureInputBareItem, HttpSignatureInputComponent,
  HttpSignatureInputEntry, HttpSignatureInputParameter, HttpSignatureInputParseError,
  HttpSignatureParseError, HttpSunsetParseError, HttpUpgrade, HttpUpgradeParseError,
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
  let content_location: rttp::ContentLocation =
    rttp_client::response::ContentLocation::parse("../representations/current.json")
      .expect("Content-Location should parse");
  let _: rttp::ContentLocationParseError =
    rttp_client::response::ContentLocation::parse("not valid")
      .expect_err("invalid Content-Location should be rejected");
  let content_security_policy: rttp::ContentSecurityPolicy =
    rttp_client::response::ContentSecurityPolicy::parse("default-src 'self'; object-src 'none'")
      .expect("Content-Security-Policy should parse");
  let _: rttp::ContentSecurityPolicyParseError =
    rttp_client::response::ContentSecurityPolicy::parse("")
      .expect_err("empty Content-Security-Policy should be rejected");
  let content_range: rttp::ContentRange =
    rttp_client::response::ContentRange::parse("bytes 0-4/10").expect("Content-Range should parse");
  let alt_svc: rttp::AltSvc =
    rttp_client::response::AltSvc::parse("h3=\":443\"; ma=60").expect("Alt-Svc should parse");
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
  let fetch_site: rttp::SecFetchSite =
    rttp_client::SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let sec_purpose: rttp::SecPurpose =
    rttp_client::SecPurpose::parse("prefetch, vendor-ext").expect("Sec-Purpose should parse");
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
  assert_eq!(cdn_cache_control.directives()[1].value(), Some("a, b"));
  assert_eq!(accept_patch.media_types().len(), 1);
  assert_eq!(accept_post.media_types().len(), 1);
  assert_eq!(content_range_window.header_value(), "bytes 3-6/10");
  assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages");
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(
    content_security_policy.header_value(),
    "default-src 'self'; object-src 'none'"
  );
  assert_eq!("bytes", content_range.unit());
  assert_eq!(Some(0), content_range.start());
  assert_eq!(Some(4), content_range.end());
  assert_eq!(Some(10), content_range.complete_length());
  assert!(!content_range.is_unsatisfied());
  assert_eq!(alt_svc.alternatives()[0].protocol_id(), "h3");
  assert_eq!(alt_svc.alternatives()[0].max_age(), Some(60));
  assert_eq!(authentication_info.parameter("nextnonce"), Some("n-2"));
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(
    no_vary_search.params(),
    Some(&rttp::NoVarySearchParams::Names(vec![
      "utm_source".to_owned()
    ]))
  );
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(embedder_policy_report_only.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(strict_transport_security.max_age(), 31_536_000);
  assert!(strict_transport_security.include_sub_domains());
  assert_eq!(
    www_authenticate.challenges()[0].parameter("realm"),
    Some("users")
  );
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert_eq!(x_content_type_options, rttp::XContentTypeOptions::Nosniff);
  assert_eq!(x_content_type_options.header_value(), "nosniff");
  assert_eq!(x_frame_options, rttp::XFrameOptions::Deny);
  assert_eq!(x_frame_options.header_value(), "DENY");
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
fn compatibility_facade_keeps_server_metadata_in_the_server_module() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let response = HttpResponse::ok("").with_etag(HttpEntityTag::weak("revision-42"));
  let request_method: HttpAccessControlRequestMethod =
    HttpAccessControlRequestMethod::parse("patch")
      .expect("Access-Control-Request-Method should parse");
  let private_network: HttpAccessControlRequestPrivateNetwork =
    HttpAccessControlRequestPrivateNetwork::parse("true")
      .expect("Access-Control-Request-Private-Network should parse");
  let save_data: HttpSaveData = HttpSaveData::parse("on").expect("Save-Data should parse");
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
  let upgrade: HttpUpgrade = HttpUpgrade::parse("websocket").expect("Upgrade should parse");
  let _: HttpUpgradeParseError = HttpUpgrade::parse("").expect_err("empty Upgrade should fail");
  let nel: HttpNel = HttpNel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#)
    .expect("NEL should parse");
  let content_location: HttpContentLocation =
    HttpContentLocation::parse("../representations/current.json")
      .expect("Content-Location should parse");
  let _: HttpContentLocationParseError = HttpContentLocation::parse("not valid")
    .expect_err("invalid Content-Location should be rejected");
  let content_range: HttpContentRange =
    HttpContentRange::parse("bytes */10").expect("Content-Range should parse");
  let _: HttpContentRangeParseError =
    HttpContentRange::parse("bytes */*").expect_err("invalid Content-Range should be rejected");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(request_method.method(), "PATCH");
  assert_eq!(request_method.header_value(), "PATCH");
  assert_eq!(private_network.header_value(), "true");
  assert_eq!(save_data.header_value(), "on");
  assert_eq!(
    content_location.header_value(),
    "../representations/current.json"
  );
  assert_eq!(content_range.header_value(), "bytes */10");
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(embedder_policy_report_only.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(upgrade.protocols(), ["websocket"]);
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
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
