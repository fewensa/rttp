use rttp_protocol::client_hints::{AcceptCh, CriticalCh};
use rttp_protocol::entity_tag::{EntityTag, IfMatch};

#[test]
fn protocol_exports_representative_bounded_metadata_types() {
  let accept_ch = AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let critical_ch = CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let entity_tag = EntityTag::parse("\"revision-42\"").expect("entity tag should parse");
  let if_match = IfMatch::parse("\"revision-42\"").expect("If-Match should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
  assert_eq!(critical_ch.client_hints(), ["Sec-CH-UA"]);
  assert_eq!(entity_tag.opaque_tag(), "revision-42");
  assert_eq!(
    if_match.entity_tags()[0].header_value(),
    entity_tag.header_value()
  );
}
