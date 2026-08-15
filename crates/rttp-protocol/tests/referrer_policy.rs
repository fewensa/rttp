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
fn referrer_policy_rejects_forbidden_controls_in_unknown_tokens() {
  for value in [
    "origin, future\rpolicy",
    "origin, future\npolicy",
    "origin, future\u{7f}policy",
    "origin, future\tpolicy",
  ] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn referrer_policy_allows_htab_only_as_optional_whitespace_around_members() {
  let policy = ReferrerPolicy::parse("origin,\tfuture-policy\t, no-referrer")
    .expect("HTAB optional whitespace around members should parse");

  assert_eq!(
    policy.policies(),
    &[ReferrerPolicyToken::Origin, ReferrerPolicyToken::NoReferrer,]
  );
}

#[test]
fn referrer_policy_accepts_exact_token_boundary_with_recognized_and_unknown_tokens() {
  let mut value = String::from("origin");
  for index in 0..MAX_REFERRER_POLICY_TOKENS - 1 {
    value.push_str(&format!(", future-{index}"));
  }
  let policy = ReferrerPolicy::parse(&value).expect("exact token boundary should parse");

  assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);
}

#[test]
fn referrer_policy_enforces_token_boundary_cumulatively_across_fields() {
  let mut members = vec!["origin".to_string()];
  members.extend((1..MAX_REFERRER_POLICY_TOKENS).map(|index| format!("future-{index}")));
  assert_eq!(members.len(), MAX_REFERRER_POLICY_TOKENS);

  let exact_fields = vec![members[..128].join(","), members[128..].join(",")];
  let policy = ReferrerPolicy::parse_values(exact_fields.iter().map(String::as_str))
    .expect("exact cumulative token boundary should parse across fields");
  assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);

  let mut overflow_fields = exact_fields.clone();
  overflow_fields.push("future-extra".to_string());
  assert!(
    ReferrerPolicy::parse_values(overflow_fields.iter().map(String::as_str)).is_err(),
    "one member beyond the cumulative boundary must be rejected"
  );
}
