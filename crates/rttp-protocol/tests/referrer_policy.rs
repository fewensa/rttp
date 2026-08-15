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
fn referrer_policy_accepts_valid_unknown_tokens_and_rejects_malformed_unknown_tokens() {
  let policy =
    ReferrerPolicy::parse_values(["origin, future-policy", "\tno-referrer \t, future-policy-2"])
      .expect("valid unknown tokens around recognized tokens should parse");

  assert_eq!(
    policy.policies(),
    &[ReferrerPolicyToken::Origin, ReferrerPolicyToken::NoReferrer]
  );

  for value in [
    "origin, bad\tinside",
    "origin, bad\u{7f}",
    "origin, bad\u{0b}",
    "origin, bad\u{1f}",
    "origin, bad, control\u{01}inside",
  ] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn referrer_policy_accepts_exactly_max_mixed_tokens_across_fields() {
  let unknowns = std::iter::repeat_n("future-policy", MAX_REFERRER_POLICY_TOKENS - 2)
    .collect::<Vec<_>>()
    .join(", ");

  let policy = ReferrerPolicy::parse_values([unknowns.as_str(), "origin, no-referrer"])
    .expect("exactly MAX_REFERRER_POLICY_TOKENS members should parse");

  assert_eq!(
    policy.policies(),
    &[ReferrerPolicyToken::Origin, ReferrerPolicyToken::NoReferrer]
  );
}

#[test]
fn referrer_policy_rejects_cumulative_multi_field_token_overflow() {
  let first_field = std::iter::repeat_n("future-policy", MAX_REFERRER_POLICY_TOKENS)
    .collect::<Vec<_>>()
    .join(", ");
  let second_field = std::iter::repeat_n("origin", MAX_REFERRER_POLICY_TOKENS)
    .collect::<Vec<_>>()
    .join(", ");

  assert!(
    ReferrerPolicy::parse_values([first_field.as_str(), second_field.as_str()]).is_err(),
    "more than MAX_REFERRER_POLICY_TOKENS members across fields must be rejected"
  );
}
