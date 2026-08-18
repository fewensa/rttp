use rttp_protocol::proxy_authenticate::{
  ProxyAuthenticate, MAX_PROXY_AUTHENTICATE_CHALLENGES, MAX_PROXY_AUTHENTICATE_PARAMETERS,
  MAX_PROXY_AUTHENTICATE_PARAMETER_VALUE_BYTES, MAX_PROXY_AUTHENTICATE_VALUE_BYTES,
};

#[test]
fn proxy_authenticate_parses_multiple_challenges_across_fields() {
  let challenges = ProxyAuthenticate::parse_values([
    r#"Basic realm="corp""#,
    r#"Bearer mF_9.B5f-4.1JqM, Digest realm="apps", nonce="n-1""#,
  ])
  .expect("valid Proxy-Authenticate challenges should parse");

  assert_eq!(challenges.len(), 3);
  assert!(!challenges.is_empty());
  assert_eq!(challenges.challenges()[0].scheme(), "Basic");
  assert_eq!(challenges.challenges()[0].parameter("realm"), Some("corp"));
  assert_eq!(challenges.challenges()[1].scheme(), "Bearer");
  assert_eq!(
    challenges.challenges()[1].token68(),
    Some("mF_9.B5f-4.1JqM")
  );
  assert_eq!(challenges.challenges()[2].scheme(), "Digest");
  assert_eq!(challenges.challenges()[2].parameter("realm"), Some("apps"));
  assert_eq!(challenges.challenges()[2].parameter("nonce"), Some("n-1"));
  assert_eq!(
    challenges.header_value(),
    r#"Basic realm="corp", Bearer mF_9.B5f-4.1JqM, Digest realm="apps", nonce=n-1"#
  );
}

#[test]
fn proxy_authenticate_combines_repeated_fields_before_parsing() {
  let challenges = ProxyAuthenticate::parse_values(["Digest realm=corp", "nonce=abc"])
    .expect("repeated Proxy-Authenticate fields should combine before parsing");

  assert_eq!(challenges.len(), 1);
  let digest = &challenges.challenges()[0];
  assert_eq!(digest.scheme(), "Digest");
  assert_eq!(digest.parameter("realm"), Some("corp"));
  assert_eq!(digest.parameter("nonce"), Some("abc"));
}

#[test]
fn proxy_authenticate_parses_padded_token68_values() {
  let challenges =
    ProxyAuthenticate::parse_values(["Bearer abc=", "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="])
      .expect("padded token68 Proxy-Authenticate challenges should parse");

  assert_eq!(challenges.len(), 2);
  assert_eq!(challenges.challenges()[0].scheme(), "Bearer");
  assert_eq!(challenges.challenges()[0].token68(), Some("abc="));
  assert_eq!(challenges.challenges()[1].scheme(), "Basic");
  assert_eq!(
    challenges.challenges()[1].token68(),
    Some("QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
  );
}

#[test]
fn proxy_authenticate_unescapes_quoted_parameter_values() {
  let challenges = ProxyAuthenticate::parse(r#"Digest realm="say \"hi\" and \\""#)
    .expect("quoted-string escapes should parse");

  assert_eq!(
    challenges.challenges()[0].parameter("realm"),
    Some(r#"say "hi" and \"#)
  );
}

#[test]
fn proxy_authenticate_accepts_bws_and_case_insensitive_parameter_lookup() {
  let challenges = ProxyAuthenticate::parse(r#"Digest Realm = "apps" , NONCE = abc"#)
    .expect("BWS around equals and OWS around commas should parse");
  let digest = &challenges.challenges()[0];

  assert_eq!(digest.parameter("realm"), Some("apps"));
  assert_eq!(digest.parameter("REALM"), Some("apps"));
  assert_eq!(digest.parameter("nonce"), Some("abc"));
  assert_eq!(digest.parameters()[0].name(), "realm");
  assert_eq!(digest.parameters()[1].name(), "nonce");
  assert_eq!(digest.parameters()[1].value(), "abc");
}

#[test]
fn proxy_authenticate_rejects_empty_malformed_and_duplicates() {
  for value in [
    "",
    " ",
    "\t",
    ",Basic",
    "Basic,",
    "Basic,,Digest",
    "Basic @",
    r#"Basic realm="unterminated"#,
    "Basic realm=one, REALM=two",
    "Basic token===more",
    "Basic token next",
  ] {
    assert!(
      ProxyAuthenticate::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    ProxyAuthenticate::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    ProxyAuthenticate::parse_values(["", "Basic"]).is_err(),
    "empty repeated fields must not disappear during list combination"
  );
}

#[test]
fn proxy_authenticate_enforces_value_parameter_and_count_bounds() {
  assert!(ProxyAuthenticate::parse("x".repeat(MAX_PROXY_AUTHENTICATE_VALUE_BYTES + 1)).is_err());
  assert!(
    ProxyAuthenticate::parse_values([
      "Basic realm=corp",
      "x".repeat(MAX_PROXY_AUTHENTICATE_VALUE_BYTES + 1).as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );
  assert!(ProxyAuthenticate::parse(format!(
    "Basic realm={}",
    "x".repeat(MAX_PROXY_AUTHENTICATE_PARAMETER_VALUE_BYTES + 1)
  ))
  .is_err());

  let too_many_parameters = format!(
    "Digest {}",
    (0..=MAX_PROXY_AUTHENTICATE_PARAMETERS)
      .map(|index| format!("p{index}=v"))
      .collect::<Vec<_>>()
      .join(", ")
  );
  assert!(ProxyAuthenticate::parse(too_many_parameters).is_err());

  let too_many_challenges = (0..=MAX_PROXY_AUTHENTICATE_CHALLENGES)
    .map(|index| format!("Scheme{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(ProxyAuthenticate::parse(too_many_challenges).is_err());
}
