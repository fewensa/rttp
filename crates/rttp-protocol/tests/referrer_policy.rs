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
  for value in ["", "origin,", "origin\n", "origin\r\nX: y"] {
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
    "origin, future\npolicy",
    "origin, future-policy\n",
    "future\tpolicy, origin",
    "origin, future\r\npolicy",
    "origin, future-policy\u{7f}",
  ] {
    assert!(
      ReferrerPolicy::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn referrer_policy_permits_htab_only_around_comma_delimited_tokens() {
  for value in [
    "\tfuture-policy\t, ORIGIN",
    " \t future-policy \t , ORIGIN \t ",
  ] {
    let policy =
      ReferrerPolicy::parse(value).expect("HTAB around comma-delimited tokens should be permitted");
    assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);
    assert_eq!(policy.header_value(), "origin");
  }
}

#[test]
fn referrer_policy_accepts_exact_token_boundary_with_unknown_tokens() {
  const UNKNOWN_IN_FIELD_1: usize = 200;

  let field_1 = unknown_tokens(UNKNOWN_IN_FIELD_1);
  let field_2 = format!(
    "{}, ORIGIN",
    unknown_tokens(MAX_REFERRER_POLICY_TOKENS - 1 - UNKNOWN_IN_FIELD_1)
  );

  let policy = ReferrerPolicy::parse_values([field_1.as_str(), field_2.as_str()])
    .expect("exactly MAX_REFERRER_POLICY_TOKENS tokens should parse");
  assert_eq!(policy.policies(), &[ReferrerPolicyToken::Origin]);
  assert_eq!(policy.header_value(), "origin");
}

#[test]
fn referrer_policy_rejects_one_beyond_token_boundary_across_fields() {
  let field_1 = unknown_tokens(MAX_REFERRER_POLICY_TOKENS);

  assert!(
    ReferrerPolicy::parse_values([field_1.as_str(), "ORIGIN"]).is_err(),
    "the token beyond MAX_REFERRER_POLICY_TOKENS must be rejected"
  );
}

fn unknown_tokens(count: usize) -> String {
  vec!["future-policy"; count].join(",")
}
