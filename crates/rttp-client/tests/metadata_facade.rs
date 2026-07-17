use rttp_client::response::{
  AcceptCh, AltSvc, Digest, HttpClearSiteData, Priority, ServerTiming, Trailer,
};

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

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(clear_site_data.directives().len(), 1);
  assert_eq!(digest.entries().len(), 1);
  assert_eq!(priority.urgency(), Some(1));
  assert_eq!(server_timing.metrics().len(), 1);
  assert_eq!(trailer.field_names(), ["x-trace"]);
  assert_eq!(alt_svc.alternatives().len(), 1);
}
