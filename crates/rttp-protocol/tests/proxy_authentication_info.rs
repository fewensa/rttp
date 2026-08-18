use rttp_protocol::proxy_authentication_info::{
  ProxyAuthenticationInfo, MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS,
  MAX_PROXY_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES, MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES,
};

#[test]
fn proxy_authentication_info_parses_digest_style_token_and_quoted_string_list() {
  let info = ProxyAuthenticationInfo::parse(
    r#"nextnonce="6629fae49393a05397450978507c4ef1", qop=auth, rspauth="6629fae49393a05397450978507c4ef1", cnonce="0a4f113b", nc=00000001"#,
  )
  .expect("valid Proxy-Authentication-Info");

  assert_eq!(info.len(), 5);
  assert!(!info.is_empty());
  assert_eq!(
    info.parameter("nextnonce"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(
    info.parameter("rspauth"),
    Some("6629fae49393a05397450978507c4ef1")
  );
  assert_eq!(info.parameter("cnonce"), Some("0a4f113b"));
  assert_eq!(info.parameter("nc"), Some("00000001"));
  assert_eq!(
    info
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>(),
    [
      ("nextnonce", "6629fae49393a05397450978507c4ef1"),
      ("qop", "auth"),
      ("rspauth", "6629fae49393a05397450978507c4ef1"),
      ("cnonce", "0a4f113b"),
      ("nc", "00000001"),
    ]
  );
}

#[test]
fn proxy_authentication_info_unescapes_quoted_string_values() {
  let info = ProxyAuthenticationInfo::parse(r#"msg="say \"hi\" and \\""#)
    .expect("quoted-string escapes should parse");

  assert_eq!(info.parameter("msg"), Some(r#"say "hi" and \"#));
}

#[test]
fn proxy_authentication_info_accepts_obs_text_in_quoted_pair_escapes() {
  let info = ProxyAuthenticationInfo::parse(r#"note="\é""#).expect("escaped obs-text should parse");

  assert_eq!(info.parameter("note"), Some("é"));
}

#[test]
fn proxy_authentication_info_accepts_bws_around_equals_and_ows_around_commas() {
  let info = ProxyAuthenticationInfo::parse("nextnonce\t=\t\"abc\" , qop = auth")
    .expect("BWS and OWS should parse");

  assert_eq!(info.parameter("nextnonce"), Some("abc"));
  assert_eq!(info.parameter("qop"), Some("auth"));
}

#[test]
fn proxy_authentication_info_parse_values_combines_and_inspects_every_field() {
  let mut values = ["nextnonce=abc", "qop=auth"].into_iter();
  let mut calls = 0;

  let info = ProxyAuthenticationInfo::parse_values(std::iter::from_fn(|| {
    calls += 1;
    assert!(calls <= 3, "parser must inspect every list field");
    values.next()
  }))
  .expect("multiple fields form one auth-param list");

  assert_eq!(info.parameter("nextnonce"), Some("abc"));
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(info.len(), 2);
}

#[test]
fn proxy_authentication_info_looks_up_parameters_case_insensitively_and_stores_lowercase_names() {
  let info = ProxyAuthenticationInfo::parse("NextNonce=abc, QOP=auth")
    .expect("case-insensitive parameter names should parse");

  assert_eq!(info.parameter("nextnonce"), Some("abc"));
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(info.parameter("QOP"), Some("auth"));
  assert_eq!(info.parameters()[0].name(), "nextnonce");
  assert_eq!(info.parameters()[1].name(), "qop");
}

#[test]
fn proxy_authentication_info_formats_token_and_quoted_parameter_values() {
  let info = ProxyAuthenticationInfo::parse(r#"qop=auth, note="hello world", msg="say \"hi\"""#)
    .expect("mixed token and quoted values should parse");

  assert_eq!(
    info.header_value(),
    r#"qop=auth, note="hello world", msg="say \"hi\"""#
  );
}

#[test]
fn proxy_authentication_info_rejects_empty_malformed_and_token68_values() {
  for value in [
    "",
    " ",
    "\t",
    ",nextnonce=abc",
    "nextnonce=abc,",
    "nextnonce=abc,,qop=auth",
    "nextnonce",
    "nextnonce=",
    r#"nextnonce="abc"#,
    "na me=value",
    "name=val ue",
    "(name)=value",
    "Bearer mF_9.B5f-4.1JqM",
    "mF_9.B5f-4.1JqM",
  ] {
    assert!(
      ProxyAuthenticationInfo::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    ProxyAuthenticationInfo::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn proxy_authentication_info_rejects_case_insensitive_duplicates_in_and_across_fields() {
  assert!(ProxyAuthenticationInfo::parse("qop=auth, QOP=auth").is_err());
  assert!(ProxyAuthenticationInfo::parse_values(["qop=auth", "QOP=auth"]).is_err());
}

#[test]
fn proxy_authentication_info_enforces_value_parameter_and_count_bounds() {
  assert!(ProxyAuthenticationInfo::parse(
    "x".repeat(MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES + 1)
  )
  .is_err());
  assert!(ProxyAuthenticationInfo::parse(format!(
    "name={}",
    "x".repeat(MAX_PROXY_AUTHENTICATION_INFO_PARAMETER_VALUE_BYTES + 1)
  ))
  .is_err());
  assert!(
    ProxyAuthenticationInfo::parse_values([
      "qop=auth",
      "x"
        .repeat(MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES + 1)
        .as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );

  let too_many = (0..=MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS)
    .map(|index| format!("p{index}=v"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(ProxyAuthenticationInfo::parse(too_many).is_err());
}
