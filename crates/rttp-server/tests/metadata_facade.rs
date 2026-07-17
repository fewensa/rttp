use rttp_server::server::{HttpAcceptCh, HttpConditionalMetadata, HttpEntityTag, HttpResponse};

#[test]
fn server_facade_exports_representative_bounded_metadata_types() {
  let accept_ch: HttpAcceptCh = HttpAcceptCh::parse("Sec-CH-UA").expect("Accept-CH should parse");
  let metadata = HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("revision-42"));
  let response = HttpResponse::ok("")
    .with_accept_ch(["Sec-CH-UA"])
    .expect("Accept-CH should be accepted");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(
    metadata
      .entity_tag_value()
      .expect("entity tag should be retained")
      .opaque_tag(),
    "revision-42"
  );
  assert_eq!(
    response
      .accept_ch()
      .expect("Accept-CH should parse")
      .expect("Accept-CH should be present")
      .client_hints(),
    ["Sec-CH-UA"]
  );
}
