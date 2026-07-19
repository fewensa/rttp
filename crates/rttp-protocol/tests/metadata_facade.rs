use rttp_protocol::client_hints::{AcceptCh, CriticalCh};
use rttp_protocol::entity_tag::{EntityTag, IfMatch};
use rttp_protocol::fetch_metadata::{SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser};
use rttp_protocol::origin::Origin;
use rttp_protocol::prefer::{Prefer, PreferenceApplied, PreferenceKind};
use rttp_protocol::referrer_policy::{ReferrerPolicy, ReferrerPolicyToken};
use rttp_protocol::timing_allow_origin::TimingAllowOrigin;

#[test]
fn protocol_exports_representative_bounded_metadata_types() {
  let accept_ch = AcceptCh::parse("Sec-CH-UA, DPR").expect("Accept-CH should parse");
  let critical_ch = CriticalCh::parse("Sec-CH-UA").expect("Critical-CH should parse");
  let entity_tag = EntityTag::parse("\"revision-42\"").expect("entity tag should parse");
  let if_match = IfMatch::parse("\"revision-42\"").expect("If-Match should parse");
  let fetch_site = SecFetchSite::parse("same-origin").expect("Sec-Fetch-Site should parse");
  let fetch_mode = SecFetchMode::parse("navigate").expect("Sec-Fetch-Mode should parse");
  let fetch_dest = SecFetchDest::parse("document").expect("Sec-Fetch-Dest should parse");
  let fetch_user = SecFetchUser::parse("?1").expect("Sec-Fetch-User should parse");
  let origin = Origin::parse("https://example.test").expect("Origin should parse");
  let timing_allow_origin =
    TimingAllowOrigin::parse("https://example.test").expect("Timing-Allow-Origin should parse");

  assert_eq!(accept_ch.client_hints(), ["Sec-CH-UA", "DPR"]);
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
  assert_eq!(origin.header_value(), "https://example.test");
  assert_eq!(timing_allow_origin.origins(), ["https://example.test"]);
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
