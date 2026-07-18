use rttp_client::response::{
  AcceptCh, AltSvc, Digest, HttpClearSiteData, PreferenceApplied, Priority, ServerTiming, Trailer,
};
use rttp_client::{SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser};

#[test]
fn response_facade_exports_representative_bounded_metadata_types() {
  let accept_ch = AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let clear_site_data =
    HttpClearSiteData::parse("\"cache\"").expect("Clear-Site-Data should parse");
  let digest = Digest::parse("sha-256=:YWJj:").expect("Digest should parse");
  let priority = Priority::parse("u=1, i").expect("Priority should parse");
  let server_timing = ServerTiming::parse("db;dur=53").expect("Server-Timing should parse");
  let trailer = Trailer::parse("X-Trace").expect("Trailer should parse");
  let alt_svc = AltSvc::parse("h3=\":443\"").expect("Alt-Svc should parse");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(clear_site_data.directives().len(), 1);
  assert_eq!(digest.entries().len(), 1);
  assert_eq!(priority.urgency(), Some(1));
  assert_eq!(server_timing.metrics().len(), 1);
  assert_eq!(trailer.field_names(), ["x-trace"]);
  assert_eq!(alt_svc.alternatives().len(), 1);
  assert_eq!(fetch_site.header_value(), "same-origin");
  assert_eq!(fetch_mode.header_value(), "navigate");
  assert_eq!(fetch_dest.header_value(), "document");
  assert_eq!(fetch_user.header_value(), "?1");
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
