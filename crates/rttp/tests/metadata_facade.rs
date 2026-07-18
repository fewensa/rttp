use rttp::server::{
  HttpAcceptCh, HttpConditionalMetadata, HttpEntityTag, HttpResponse, HttpSunsetParseError,
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
  let fetch_site: rttp::SecFetchSite =
    rttp_client::SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(accept_patch.media_types().len(), 1);
  assert_eq!(accept_post.media_types().len(), 1);
  assert_eq!(fetch_site.header_value(), "same-origin");
}

#[test]
fn compatibility_facade_keeps_server_metadata_in_the_server_module() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
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
