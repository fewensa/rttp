use rttp_protocol::user_agent::{
  UserAgent, MAX_USER_AGENT_COMMENT_DEPTH, MAX_USER_AGENT_MEMBERS, MAX_USER_AGENT_VALUE_BYTES,
};

#[test]
fn user_agent_parses_ordered_products_and_comments() {
  let user_agent = UserAgent::parse("  Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko) ")
    .expect("valid User-Agent should parse");

  assert_eq!(3, user_agent.len());
  assert!(!user_agent.is_empty());
  assert_eq!(Some("Mozilla"), user_agent.members()[0].product());
  assert_eq!(Some("5.0"), user_agent.members()[0].version());
  assert_eq!(Some("AppleWebKit"), user_agent.members()[1].product());
  assert_eq!(Some("537.36"), user_agent.members()[1].version());
  assert_eq!(Some("KHTML, like Gecko"), user_agent.members()[2].comment());
  assert_eq!(None, user_agent.members()[2].product());
  assert_eq!(None, user_agent.members()[2].version());
  assert_eq!(
    "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko)",
    user_agent.header_value()
  );
}

#[test]
fn user_agent_preserves_product_version_and_comment_spelling() {
  let user_agent = UserAgent::parse(r#"Acme/01.0 (outer \(literal\) (inner) "quoted")"#)
    .expect("valid spelling should parse");

  assert_eq!(Some("Acme"), user_agent.members()[0].product());
  assert_eq!(Some("01.0"), user_agent.members()[0].version());
  assert_eq!(
    Some(r#"outer \(literal\) (inner) "quoted""#),
    user_agent.members()[1].comment()
  );
  assert_eq!(
    r#"Acme/01.0 (outer \(literal\) (inner) "quoted")"#,
    user_agent.header_value()
  );
  assert_eq!(
    user_agent,
    UserAgent::parse(user_agent.header_value()).expect("serialized value should round-trip")
  );
}

#[test]
fn user_agent_accepts_empty_comments_and_required_rws() {
  for value in ["product ()", "product\t(comment)", "product (one) next/2"] {
    let parsed = UserAgent::parse(value).expect("valid RWS-separated members should parse");
    assert_eq!(value.replace('\t', " "), parsed.header_value());
  }
}

#[test]
fn user_agent_rejects_malformed_products_comments_and_member_order() {
  for value in [
    "",
    " ",
    "\t",
    "(comment) product",
    "product(comment)",
    "product/",
    "/version",
    "product//version",
    "product/version/extra",
    "product;parameter",
    "product, other",
    "product (unterminated",
    "product (bad\\)",
    "product (bad\\\r)",
    "product (bad\u{7f})",
    "product (nested (comment)",
    "product )",
  ] {
    assert!(
      UserAgent::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn user_agent_rejects_duplicate_fields_and_empty_field_sets() {
  assert!(UserAgent::parse_values(["client/1", "client/2"]).is_err());
  assert!(UserAgent::parse_values(["client/1", ""]).is_err());
  assert!(UserAgent::parse_values([]).is_err());
}

#[test]
fn user_agent_rejects_forbidden_controls_and_accepts_valid_comment_obs_text() {
  for value in [
    "client/1\r\nX-Injected: secret",
    "client/1\nX: value",
    "client/1\0",
    "client/1\u{1f}",
    "client/1\u{7f}",
    "client/1 (bad\\\n)",
  ] {
    let error = UserAgent::parse(value).expect_err("forbidden control must be rejected");
    assert!(!error.to_string().contains("secret"));
  }

  let parsed = UserAgent::parse("client/1 (café)").expect("obs-text in comments is valid");
  assert_eq!(Some("café"), parsed.members()[1].comment());
}

#[test]
fn user_agent_enforces_member_and_comment_depth_bounds() {
  let at_limit = (0..MAX_USER_AGENT_MEMBERS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(" ");
  let parsed = UserAgent::parse(&at_limit).expect("member bound should be inclusive");
  assert_eq!(MAX_USER_AGENT_MEMBERS, parsed.len());

  let too_many = format!("{at_limit} overflow");
  assert!(UserAgent::parse(too_many).is_err());

  let at_depth = format!(
    "client {}{}",
    "(".repeat(MAX_USER_AGENT_COMMENT_DEPTH),
    ")".repeat(MAX_USER_AGENT_COMMENT_DEPTH)
  );
  assert!(UserAgent::parse(&at_depth).is_ok());

  let too_deep = format!(
    "client {}{}",
    "(".repeat(MAX_USER_AGENT_COMMENT_DEPTH + 1),
    ")".repeat(MAX_USER_AGENT_COMMENT_DEPTH + 1)
  );
  assert!(UserAgent::parse(&too_deep).is_err());
}

#[test]
fn user_agent_enforces_value_size_and_checks_duplicate_values() {
  assert!(UserAgent::parse("p".repeat(MAX_USER_AGENT_VALUE_BYTES + 1)).is_err());

  let at_limit = format!("p{}", " ".repeat(MAX_USER_AGENT_VALUE_BYTES - 1));
  assert_eq!(MAX_USER_AGENT_VALUE_BYTES, at_limit.len());
  assert!(UserAgent::parse(&at_limit).is_ok());

  let oversized = "p".repeat(MAX_USER_AGENT_VALUE_BYTES + 1);
  assert!(UserAgent::parse_values(["client/1", oversized.as_str()]).is_err());
}

#[test]
fn user_agent_debug_and_errors_do_not_echo_field_contents() {
  let value = "SensitiveBrowser/9.9 (private fingerprint)";
  let user_agent = UserAgent::parse(value).expect("valid User-Agent should parse");
  let debug = format!("{user_agent:?} {:?}", user_agent.members()[0]);
  assert!(!debug.contains("SensitiveBrowser"));
  assert!(!debug.contains("private fingerprint"));

  let error = UserAgent::parse("SensitiveBrowser/invalid=version secret")
    .expect_err("malformed User-Agent should fail");
  assert!(!error.to_string().contains("SensitiveBrowser"));
  assert!(!format!("{error:?}").contains("secret"));
}
