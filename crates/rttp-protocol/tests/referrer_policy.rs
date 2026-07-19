use rttp_protocol::referrer_policy::{
  ReferrerPolicy, ReferrerPolicyToken, MAX_REFERRER_POLICY_VALUE_BYTES,
};

#[test]
fn referrer_policy_parses_ordered_tokens_across_fields() {
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

#[test]
fn referrer_policy_rejects_invalid_empty_duplicate_and_oversized_fields() {
  for value in ["", "origin,", "invalid", "origin\r\nX: y"] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  assert!(ReferrerPolicy::parse_values(["origin", "origin"]).is_err());

  let oversized = "x".repeat(MAX_REFERRER_POLICY_VALUE_BYTES + 1);
  assert!(ReferrerPolicy::parse(&oversized).is_err());
}
