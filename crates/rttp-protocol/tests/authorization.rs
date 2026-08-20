use rttp_protocol::authorization::{
  Authorization, AuthorizationParseError, ProxyAuthorization, MAX_AUTHORIZATION_VALUE_BYTES,
};

#[test]
fn authorization_parses_basic_bearer_custom_and_proxy_metadata() {
  let basic = Authorization::parse("Basic dXNlcjpzZWNyZXQ=").expect("Basic auth should parse");
  assert_eq!("Basic", basic.scheme());
  assert_eq!("dXNlcjpzZWNyZXQ=", basic.credentials());
  assert_eq!("Basic dXNlcjpzZWNyZXQ=", basic.header_value());

  let bearer = Authorization::new(" Bearer ", "token-123").expect("Bearer auth should build");
  assert_eq!("Bearer token-123", bearer.header_value());

  let custom =
    Authorization::parse("ApiKey v1:client-42").expect("custom scheme auth should parse");
  assert_eq!("ApiKey", custom.scheme());
  assert_eq!("v1:client-42", custom.credentials());

  let proxy = ProxyAuthorization::parse("Basic cHJveHk6c2VjcmV0").expect("proxy auth should parse");
  assert_eq!("Basic", proxy.scheme());
  assert_eq!("cHJveHk6c2VjcmV0", proxy.credentials());
  assert_eq!("Basic cHJveHk6c2VjcmV0", proxy.header_value());
}

#[test]
fn authorization_rejects_malformed_or_injected_metadata() {
  for value in [
    "Bearer",
    "bad(scheme token",
    "Bearer \t ",
    "Bearer token\rnext",
    "Bearer token\nnext",
    "Bearer token\0next",
    "Bearer token\u{1f}next",
  ] {
    assert!(
      Authorization::parse(value).is_err(),
      "Authorization should reject {value:?}"
    );
  }

  assert!(Authorization::new("bad scheme", "token").is_err());
  assert!(Authorization::new("Bearer", "").is_err());
  assert!(Authorization::new("Bearer", " \t ").is_err());
  assert!(ProxyAuthorization::parse("Basic").is_err());
  assert!(ProxyAuthorization::new("Basic", "proxy\rsecret").is_err());
}

#[test]
fn authorization_enforces_explicit_size_bounds() {
  let credentials = "x".repeat(MAX_AUTHORIZATION_VALUE_BYTES - "Bearer ".len());
  let authorization = Authorization::new("Bearer", &credentials)
    .expect("exactly bounded Authorization should be accepted");
  assert_eq!(
    MAX_AUTHORIZATION_VALUE_BYTES,
    authorization.header_value().len()
  );

  let too_large = format!("Bearer {}x", credentials);
  assert!(Authorization::parse(&too_large).is_err());
  assert!(Authorization::new("Bearer", format!("{credentials}x")).is_err());

  let proxy_credentials = "x".repeat(MAX_AUTHORIZATION_VALUE_BYTES - "Basic ".len());
  assert!(
    ProxyAuthorization::new("Basic", &proxy_credentials).is_ok(),
    "exactly bounded Proxy-Authorization should be accepted"
  );
  assert!(
    ProxyAuthorization::new("Basic", format!("{proxy_credentials}x")).is_err(),
    "oversized Proxy-Authorization should be rejected"
  );
}

#[test]
fn authorization_parse_values_rejects_duplicates() {
  let duplicate = ["Bearer first", "Bearer second"];
  assert!(Authorization::parse_values(duplicate).is_err());
  assert!(ProxyAuthorization::parse_values(duplicate).is_err());
}

#[test]
fn authorization_debug_and_errors_redact_credentials() {
  let secret = "super-secret-token";
  let authorization = Authorization::parse(format!("Bearer {secret}")).expect("auth should parse");
  let proxy =
    ProxyAuthorization::parse(format!("Basic {secret}")).expect("proxy auth should parse");

  assert!(!format!("{authorization:?}").contains(secret));
  assert!(!format!("{proxy:?}").contains(secret));

  let error = Authorization::parse(format!("Bearer {secret}\r"))
    .expect_err("invalid credential should fail")
    .to_string();
  assert!(!error.contains(secret));
  assert!(error.contains("Authorization"));

  let proxy_error = ProxyAuthorization::parse(format!("Basic {secret}\n"))
    .expect_err("invalid proxy credential should fail")
    .to_string();
  assert!(!proxy_error.contains(secret));
  assert!(proxy_error.contains("Proxy-Authorization"));

  let _: AuthorizationParseError =
    Authorization::parse("bad(scheme secret").expect_err("bad scheme should fail");
}
