use rttp_protocol::proxy_authentication_info::{
  ProxyAuthenticationInfo, MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS,
  MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES,
};

#[test]
fn proxy_authentication_info_parses_ordered_parameters_with_accessors() {
  let info = ProxyAuthenticationInfo::parse(
    "nextnonce=\"xyz789\", qop=auth, rspauth=\"...\", cnonce=\"c\", nc=00000001",
  )
  .expect("valid Proxy-Authentication-Info");

  assert_eq!(
    info
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>(),
    ["nextnonce", "qop", "rspauth", "cnonce", "nc"]
  );
  assert_eq!(info.parameters()[0].value(), "xyz789");
  assert_eq!(info.parameter("nextnonce"), Some("xyz789"));
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(info.parameter("rspauth"), Some("..."));
  assert_eq!(info.parameter("cnonce"), Some("c"));
  assert_eq!(info.parameter("nc"), Some("00000001"));
  assert_eq!(info.parameter("NEXTNONCE"), Some("xyz789"));
  assert_eq!(info.parameter("unknown"), None);
  assert_eq!(info.len(), 5);
  assert!(!info.is_empty());
}

#[test]
fn proxy_authentication_info_unescapes_quoted_values_and_round_trips() {
  let info = ProxyAuthenticationInfo::parse("nextnonce=\"a\\\"b\\\\c\", qop=auth")
    .expect("valid Proxy-Authentication-Info");

  assert_eq!(info.parameter("nextnonce"), Some("a\"b\\c"));
  assert_eq!(info.header_value(), "nextnonce=\"a\\\"b\\\\c\", qop=auth");
}

#[test]
fn proxy_authentication_info_renders_token_values_bare_and_quotes_others() {
  let info = ProxyAuthenticationInfo::parse("qop=auth, nextnonce=\"x y\"")
    .expect("valid Proxy-Authentication-Info");

  assert_eq!(info.header_value(), "qop=auth, nextnonce=\"x y\"");
}

#[test]
fn proxy_authentication_info_tolerates_optional_whitespace() {
  let info = ProxyAuthenticationInfo::parse("  nextnonce \t = \t \"xyz789\" ,\t qop = auth ")
    .expect("OWS-padded Proxy-Authentication-Info should parse");

  assert_eq!(info.parameter("nextnonce"), Some("xyz789"));
  assert_eq!(info.parameter("qop"), Some("auth"));
  assert_eq!(info.len(), 2);
}

#[test]
fn proxy_authentication_info_combines_multiple_header_fields_in_wire_order() {
  let info = ProxyAuthenticationInfo::parse_values([
    "nextnonce=\"xyz789\", qop=auth",
    "rspauth=\"...\", nc=00000001",
  ])
  .expect("multiple Proxy-Authentication-Info fields");

  assert_eq!(
    info
      .parameters()
      .iter()
      .map(|parameter| parameter.name())
      .collect::<Vec<_>>(),
    ["nextnonce", "qop", "rspauth", "nc"]
  );
  assert_eq!(info.parameter("nc"), Some("00000001"));
}

#[test]
fn proxy_authentication_info_rejects_malformed_values() {
  for value in [
    "",
    "   ",
    "nextnonce",
    "=auth",
    "nextnonce=",
    "nextnonce= ",
    "nextnonce=\"unterminated",
    "nextnonce=\"bad\nescaped\"",
    "nextnonce=\"bad\\\rescape\"",
    "nextnonce=\"a\",",
    ", qop=auth",
    "nextnonce=\"a\",,qop=auth",
    "nextnonce=\"a\" qop=auth",
    "nextnonce=auth extra",
    "nextnonce=\"a\"\r\nX: y",
  ] {
    assert!(
      ProxyAuthenticationInfo::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn proxy_authentication_info_rejects_case_insensitive_duplicates() {
  for value in [
    "qop=auth, QOP=auth",
    "nextnonce=\"a\", NextNonce=\"b\"",
    "nc=00000001, NC=00000002",
  ] {
    assert!(
      ProxyAuthenticationInfo::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn proxy_authentication_info_rejects_duplicates_across_fields() {
  assert!(
    ProxyAuthenticationInfo::parse_values(["qop=auth", "qop=auth"]).is_err(),
    "duplicate parameters across fields must be rejected"
  );
}

#[test]
fn proxy_authentication_info_enforces_value_and_parameter_bounds() {
  assert!(ProxyAuthenticationInfo::parse(
    "x".repeat(MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES + 1)
  )
  .is_err());

  let at_field_bound = format!(
    "a={}",
    "a".repeat(MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES - 2)
  );
  assert!(ProxyAuthenticationInfo::parse(at_field_bound).is_ok());

  let at_limit = (0..MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS)
    .map(|index| format!("x{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(ProxyAuthenticationInfo::parse(at_limit).is_ok());

  let too_many = (0..=MAX_PROXY_AUTHENTICATION_INFO_PARAMETERS)
    .map(|index| format!("x{index}=v"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(ProxyAuthenticationInfo::parse(too_many).is_err());
}

#[test]
fn proxy_authentication_info_checks_each_field_value_against_its_bound() {
  let oversized = "x".repeat(MAX_PROXY_AUTHENTICATION_INFO_VALUE_BYTES + 1);

  assert!(
    ProxyAuthenticationInfo::parse_values(["nextnonce=\"a\"", oversized.as_str()]).is_err(),
    "oversized fields must not bypass validation"
  );
}

#[test]
fn proxy_authentication_info_rejects_empty_field_sets() {
  assert!(
    ProxyAuthenticationInfo::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}
