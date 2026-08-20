use rttp_protocol::origin_trial::{
  OriginTrials, MAX_ORIGIN_TRIAL_TOKENS, MAX_ORIGIN_TRIAL_TOTAL_BYTES, MAX_ORIGIN_TRIAL_VALUE_BYTES,
};

#[test]
fn origin_trial_preserves_multiple_tokens_and_normalizes_ows() {
  let trials = OriginTrials::parse_values([" token-one\t", "token-two"])
    .expect("valid Origin-Trial tokens should parse");

  assert_eq!(trials.tokens(), ["token-one", "token-two"]);
  assert_eq!(trials.header_values(), ["token-one", "token-two"]);
  assert_eq!(2, trials.len());
  assert!(!trials.is_empty());

  let singleton = OriginTrials::parse(" \tsingle-token ").expect("singleton token should parse");
  assert_eq!(singleton.tokens(), ["single-token"]);
  assert_eq!(singleton.header_values(), ["single-token"]);
}

#[test]
fn origin_trial_preserves_duplicate_token_strings() {
  let trials = OriginTrials::parse_values(["same-token", "same-token"])
    .expect("duplicate Origin-Trial tokens should be preserved");

  assert_eq!(trials.tokens(), ["same-token", "same-token"]);
  assert_eq!(2, trials.len());
}

#[test]
fn origin_trial_rejects_empty_injected_control_and_obs_text_values() {
  for value in [
    "",
    " ",
    "\t",
    "token\r\nX-Injected: 1",
    "token\rX: y",
    "token\nX: y",
    "token\0value",
    "token\u{1}value",
    "token\u{7f}value",
    "token\u{80}value",
    "token\twith-tab",
  ] {
    assert!(
      OriginTrials::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(OriginTrials::parse_values([]).is_err());
}

#[test]
fn origin_trial_enforces_per_value_count_and_aggregate_bounds() {
  assert!(
    OriginTrials::parse("x".repeat(MAX_ORIGIN_TRIAL_VALUE_BYTES)).is_ok(),
    "a token at the 8 KiB bound should parse"
  );
  assert!(
    OriginTrials::parse("x".repeat(MAX_ORIGIN_TRIAL_VALUE_BYTES + 1)).is_err(),
    "a token over the 8 KiB bound should be rejected"
  );

  let at_count: Vec<String> = (0..MAX_ORIGIN_TRIAL_TOKENS)
    .map(|index| format!("token-{index}"))
    .collect();
  let at_count_refs: Vec<&str> = at_count.iter().map(String::as_str).collect();
  assert!(
    OriginTrials::parse_values(at_count_refs).is_ok(),
    "64 tokens should parse"
  );

  let over_count: Vec<String> = (0..=MAX_ORIGIN_TRIAL_TOKENS)
    .map(|index| format!("token-{index}"))
    .collect();
  let over_count_refs: Vec<&str> = over_count.iter().map(String::as_str).collect();
  let too_many =
    OriginTrials::parse_values(over_count_refs).expect_err("65 tokens should be rejected");
  assert_eq!(too_many.to_string(), "too many Origin-Trial header values");

  let at_total = vec!["x".repeat(MAX_ORIGIN_TRIAL_VALUE_BYTES); 8];
  let at_total_refs: Vec<&str> = at_total.iter().map(String::as_str).collect();
  assert_eq!(
    8 * MAX_ORIGIN_TRIAL_VALUE_BYTES,
    MAX_ORIGIN_TRIAL_TOTAL_BYTES
  );
  assert!(
    OriginTrials::parse_values(at_total_refs).is_ok(),
    "an 64 KiB aggregate should parse"
  );

  let mut over_total = vec!["x".repeat(MAX_ORIGIN_TRIAL_VALUE_BYTES); 8];
  over_total.push("y".to_string());
  let over_total_refs: Vec<&str> = over_total.iter().map(String::as_str).collect();
  let oversized = OriginTrials::parse_values(over_total_refs)
    .expect_err("an aggregate over 64 KiB should be rejected");
  assert_eq!(
    oversized.to_string(),
    "Origin-Trial header values are too large"
  );
}

#[test]
fn origin_trial_debug_and_errors_redact_token_material() {
  let trials = OriginTrials::parse_values(["secret-token-one", "secret-token-two"])
    .expect("valid tokens should parse");
  let debug = format!("{trials:?}");
  assert!(debug.contains("OriginTrials"));
  assert!(debug.contains("token_count"));
  assert!(debug.contains('2'));
  assert!(!debug.contains("secret-token-one"));
  assert!(!debug.contains("secret-token-two"));

  let injected = OriginTrials::parse("secret-token\r\nX-Injected: 1")
    .expect_err("injected token should be rejected");
  let message = injected.to_string();
  assert_eq!(message, "invalid Origin-Trial header value");
  assert!(!message.contains("secret-token"));
  assert!(!message.contains("X-Injected"));
  assert!(!format!("{injected:?}").contains("secret-token"));

  let oversized = OriginTrials::parse("x".repeat(MAX_ORIGIN_TRIAL_VALUE_BYTES + 1))
    .expect_err("oversized token should be rejected");
  assert_eq!(
    oversized.to_string(),
    "Origin-Trial header values are too large"
  );
  assert!(!oversized.to_string().contains('x'));
}
