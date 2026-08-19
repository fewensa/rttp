use rttp::server::{
  HttpAcceptCh, HttpConditionalMetadata, HttpCrossOriginEmbedderPolicy,
  HttpCrossOriginOpenerPolicy, HttpCrossOriginResourcePolicy, HttpEntityTag, HttpNel, HttpResponse,
  HttpSunsetParseError,
};
use std::time::{Duration, UNIX_EPOCH};

#[test]
#[cfg(feature = "client")]
fn compatibility_facade_exports_client_metadata_types() {
  let accept_ch: rttp::AcceptCh =
    rttp_client::response::AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let critical_ch: rttp::CriticalCh =
    rttp_client::response::CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let accept_patch: rttp::AcceptPatch =
    rttp_client::response::AcceptPatch::parse("application/json")
      .expect("Accept-Patch should parse");
  let accept_post: rttp::AcceptPost =
    rttp_client::response::AcceptPost::parse("application/json").expect("Accept-Post should parse");
  let alt_svc: rttp::AltSvc =
    rttp_client::response::AltSvc::parse("h3=\":443\"; ma=60").expect("Alt-Svc should parse");
  let _: rttp::AltSvcParseError =
    rttp_client::response::AltSvc::parse("h3=:443").expect_err("invalid Alt-Svc should fail");
  let nel: rttp::Nel =
    rttp_client::response::Nel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#)
      .expect("NEL should parse");
  let embedder_policy: rttp::CrossOriginEmbedderPolicy =
    rttp_client::response::CrossOriginEmbedderPolicy::parse("require-corp; report-to=\"coep\"")
      .expect("Cross-Origin-Embedder-Policy should parse");
  let opener_policy: rttp::CrossOriginOpenerPolicy =
    rttp_client::response::CrossOriginOpenerPolicy::parse(
      "noopener-allow-popups; report-to=\"coop\"",
    )
    .expect("Cross-Origin-Opener-Policy should parse");
  let fetch_site: rttp::SecFetchSite =
    rttp_client::SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(accept_patch.media_types().len(), 1);
  assert_eq!(accept_post.media_types().len(), 1);
  assert_eq!(alt_svc.alternatives()[0].protocol_id(), "h3");
  assert_eq!(alt_svc.alternatives()[0].max_age(), Some(60));
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(fetch_site.header_value(), "same-origin");
}

#[test]
fn compatibility_facade_keeps_server_metadata_in_the_server_module() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let policy: HttpCrossOriginResourcePolicy = HttpCrossOriginResourcePolicy::parse("same-origin")
    .expect("Cross-Origin-Resource-Policy should parse");
  let embedder_policy: HttpCrossOriginEmbedderPolicy =
    HttpCrossOriginEmbedderPolicy::parse("require-corp; report-to=\"coep\"")
      .expect("Cross-Origin-Embedder-Policy should parse");
  let opener_policy: HttpCrossOriginOpenerPolicy =
    HttpCrossOriginOpenerPolicy::parse("noopener-allow-popups; report-to=\"coop\"")
      .expect("Cross-Origin-Opener-Policy should parse");
  let nel: HttpNel = HttpNel::parse(r#"{"report_to":"network-errors","max_age":2592000}"#)
    .expect("NEL should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(policy.header_value(), "same-origin");
  assert_eq!(embedder_policy.header_value(), "require-corp");
  assert_eq!(opener_policy.header_value(), "noopener-allow-popups");
  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(
    metadata
      .entity_tag_value()
      .expect("entity tag should be retained")
      .header_value(),
    "\"revision-42\""
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
