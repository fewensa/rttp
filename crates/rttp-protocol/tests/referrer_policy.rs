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
fn referrer_policy_rejects_forbidden_controls_inside_unknown_tokens() {
  for value in [
    "origin, future-policy\rbad",
    "origin, future-policy\nbad",
    "origin, future-policy\x00bad",
    "origin, future-policy\x7fbad",
    "origin, future\tpolicy",
  ] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn referrer_policy_accepts_htab_only_as_member_whitespace() {
  let policy = ReferrerPolicy::parse("origin,\tfuture-policy\t")
    .expect("HTAB around comma-delimited members should be trimmed");

  assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);
}

#[test]
fn referrer_policy_accepts_exactly_max_tokens_across_recognized_and_unknown_members() {
  let recognized = (0..(MAX_REFERRER_POLICY_TOKENS / 2))
    .map(|_| "origin")
    .collect::<Vec<_>>()
    .join(", ");
  let unknown = (0..(MAX_REFERRER_POLICY_TOKENS / 2))
    .map(|_| "future-policy")
    .collect::<Vec<_>>()
    .join(", ");

  let policy = ReferrerPolicy::parse_values([recognized.as_str(), unknown.as_str()])
    .expect("exactly MAX_REFERRER_POLICY_TOKENS members should parse");

  assert_eq!(
    policy.policies(),
    &[ReferrerPolicyToken::Origin; MAX_REFERRER_POLICY_TOKENS / 2]
  );
}

#[test]
fn referrer_policy_rejects_cumulative_multi_field_token_overflow() {
  let recognized = (0..MAX_REFERRER_POLICY_TOKENS)
    .map(|_| "origin")
    .collect::<Vec<_>>()
    .join(", ");

  assert!(
    ReferrerPolicy::parse_values([recognized.as_str(), "future-policy"]).is_err(),
    "a 257th unknown member across fields must be rejected"
  );
}
