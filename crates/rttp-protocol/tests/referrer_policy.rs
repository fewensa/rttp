use rttp_protocol::referrer_policy::{
  ReferrerPolicy, ReferrerPolicyToken, MAX_REFERRER_POLICY_TOKENS, MAX_REFERRER_POLICY_VALUE_BYTES,
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
fn referrer_policy_ignores_unknown_tokens_accepts_repeated_tokens_and_normalizes_case() {
  let policy = ReferrerPolicy::parse_values([
    "future-policy, ORIGIN",
    "origin, no-referrer, experimental-policy",
  ])
  .expect("recognized Referrer-Policy tokens should parse");

  assert_eq!(
    policy.policies(),
    &[
      ReferrerPolicyToken::Origin,
      ReferrerPolicyToken::Origin,
      ReferrerPolicyToken::NoReferrer,
    ]
  );
  assert_eq!(policy.header_value(), "origin, origin, no-referrer");
}

#[test]
fn referrer_policy_rejects_invalid_empty_duplicate_and_oversized_fields() {
  for value in ["", "origin,", "origin\r\nX: y"] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "x".repeat(MAX_REFERRER_POLICY_VALUE_BYTES + 1);
  assert!(ReferrerPolicy::parse(&oversized).is_err());
}

#[test]
fn referrer_policy_rejects_control_characters_inside_unknown_tokens() {
  for value in ["origin, future\rpolicy", "origin, future\tpolicy"] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    ReferrerPolicy::parse_values(["origin, future-policy", "future\rInjected: x"]).is_err(),
    "CR inside an unknown token in a later field must reject"
  );
}

#[test]
fn referrer_policy_allows_htab_only_as_edge_optional_whitespace() {
  let policy = ReferrerPolicy::parse("\tORIGIN\t, \tfuture-policy\t")
    .expect("SP/HTAB around comma-delimited tokens should be trimmed");

  assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);
  assert_eq!(policy.header_value(), "origin");
}

#[test]
fn referrer_policy_accepts_exactly_max_tokens_mixed_recognized_and_unknown() {
  let mixed = (0..MAX_REFERRER_POLICY_TOKENS)
    .map(|index| {
      if index % 2 == 0 {
        "origin"
      } else {
        "future-policy"
      }
    })
    .collect::<Vec<_>>()
    .join(", ");

  let policy =
    ReferrerPolicy::parse(&mixed).expect("256 mixed tokens should parse at the exact bound");

  assert_eq!(policy.policies().len(), MAX_REFERRER_POLICY_TOKENS / 2);
  assert!(policy
    .policies()
    .iter()
    .all(|&p| p == ReferrerPolicyToken::Origin));
}

#[test]
fn referrer_policy_rejects_one_beyond_max_tokens_cumulatively_across_fields() {
  let first_field = (0..MAX_REFERRER_POLICY_TOKENS)
    .map(|_| "origin")
    .collect::<Vec<_>>()
    .join(", ");

  assert!(
    ReferrerPolicy::parse_values([&first_field, "future-policy"]).is_err(),
    "a 257th unknown token in another field must reject"
  );
}
