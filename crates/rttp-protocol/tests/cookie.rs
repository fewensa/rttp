use rttp_protocol::cookie::{
  HttpSameSite, HttpSetCookie, HttpSetCookieAttributeKind, HttpSetCookies, MAX_COOKIE_COUNT,
  MAX_COOKIE_VALUE_BYTES, MAX_SET_COOKIE_ATTRIBUTES,
};

#[test]
fn cookie_parses_standard_and_extension_attributes_in_wire_order() {
  let cookie = HttpSetCookie::parse(
    r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned; Foo=bar"#,
  )
  .expect("Set-Cookie should parse");

  assert_eq!("session", cookie.name());
  assert_eq!("abc def", cookie.value());
  assert!(cookie.is_value_quoted());
  assert_eq!(Some("/"), cookie.path());
  assert!(cookie.http_only());
  assert_eq!(Some(HttpSameSite::Lax), cookie.same_site());
  assert_eq!(Some("High"), cookie.priority());
  assert!(cookie.partitioned());
  assert_eq!(
    vec![("Foo", Some("bar"))],
    cookie
      .extension_attributes()
      .map(|attribute| (attribute.name(), attribute.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    HttpSetCookieAttributeKind::Path,
    cookie.attributes()[0].kind()
  );
  assert_eq!(
    r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned; Foo=bar"#,
    cookie.header_value()
  );
}

#[test]
fn cookie_builder_emits_quoted_values_and_standard_attributes() {
  let cookie = HttpSetCookie::new("csrf", "token")
    .expect("cookie name and value should be valid")
    .with_path("/form")
    .expect("path should be accepted")
    .with_max_age(60)
    .expect("max-age should be accepted")
    .with_extension("Foo", Some("bar"))
    .expect("extension should be accepted");

  assert_eq!("token", cookie.value());
  assert!(!cookie.is_value_quoted());
  assert_eq!(Some("/form"), cookie.path());
  assert_eq!(Some(60), cookie.max_age());
  assert_eq!(
    "csrf=token; Path=/form; Max-Age=60; Foo=bar",
    cookie.header_value()
  );

  let quoted = HttpSetCookie::new("session", "abc def")
    .expect("quoted cookie should be accepted")
    .with_http_only()
    .expect("HttpOnly should be accepted")
    .with_same_site(HttpSameSite::Lax)
    .expect("SameSite should be accepted")
    .with_priority("High")
    .expect("Priority should be accepted")
    .with_partitioned()
    .expect("Partitioned should be accepted");
  assert!(quoted.is_value_quoted());
  assert_eq!(
    r#"session="abc def"; HttpOnly; SameSite=Lax; Priority=High; Partitioned"#,
    quoted.header_value()
  );
}

#[test]
fn cookie_parses_multiple_set_cookie_fields_in_wire_order() {
  let cookies = HttpSetCookies::parse_values([
    r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned"#,
    "csrf=token; Path=/form; Max-Age=60; Foo=bar",
  ])
  .expect("Set-Cookie fields should parse");

  assert_eq!(2, cookies.len());
  assert_eq!("session", cookies.cookies()[0].name());
  assert_eq!("csrf", cookies.cookies()[1].name());
  assert_eq!(
    vec![
      r#"session="abc def"; Path=/; HttpOnly; SameSite=Lax; Priority=High; Partitioned"#,
      "csrf=token; Path=/form; Max-Age=60; Foo=bar",
    ],
    cookies.header_values()
  );
}

#[test]
fn cookie_rejects_duplicate_attributes_case_insensitively() {
  assert!(HttpSetCookie::parse("session=abc; Path=/; path=/form").is_err());
  assert!(HttpSetCookie::parse("session=abc; HttpOnly; httponly").is_err());
  assert!(HttpSetCookie::parse("session=abc; SameSite=Lax; SameSite=Strict").is_err());
  let error = HttpSetCookie::parse("session=secret; Path=/; PATH=/other")
    .expect_err("duplicate attributes should be rejected");
  assert!(!error.to_string().contains("secret"));
}

#[test]
fn cookie_rejects_malformed_quoted_and_standard_attribute_values() {
  for value in [
    r#"session="abc"#,
    r#"session=abc"def"#,
    "session=abc; Secure=true",
    "session=abc; SameSite=whatever",
    "session=abc; Max-Age=-1",
    "session=abc; Max-Age=",
    "session=abc; Path",
  ] {
    assert!(
      HttpSetCookie::parse(value).is_err(),
      "Set-Cookie should reject {value:?}"
    );
  }
}

#[test]
fn cookie_rejects_oversized_values_attribute_counts_and_aggregate_fields() {
  let oversized_value = format!("session={}", "a".repeat(MAX_COOKIE_VALUE_BYTES + 1));
  assert!(HttpSetCookie::parse(&oversized_value).is_err());

  let too_many_attributes = format!(
    "session=abc{}",
    (0..MAX_SET_COOKIE_ATTRIBUTES + 1)
      .map(|index| format!("; Ext{index}=1"))
      .collect::<String>()
  );
  assert!(HttpSetCookie::parse(&too_many_attributes).is_err());

  let fields = std::iter::repeat_n("name=value", MAX_COOKIE_COUNT + 1);
  assert!(HttpSetCookies::parse_values(fields).is_err());

  let value = "a".repeat(MAX_COOKIE_VALUE_BYTES);
  let fields = (0..17)
    .map(|index| format!("n{index}={value}"))
    .collect::<Vec<_>>();
  assert!(HttpSetCookies::parse_values(fields.iter().map(String::as_str)).is_err());
  assert!(HttpSetCookies::parse_values(fields[..10].iter().map(String::as_str)).is_ok());
}

#[test]
fn cookie_debug_and_errors_redact_values() {
  let cookie = HttpSetCookie::parse(r#"session="abc def"; Path=/secret; Foo=hidden"#)
    .expect("Set-Cookie should parse");
  let cookies = HttpSetCookies::parse_values([cookie.header_value().as_str()])
    .expect("collection should parse");
  let cookie_debug = format!("{cookie:?}");
  let cookies_debug = format!("{cookies:?}");
  let attribute_debug = format!("{:?}", cookie.attributes()[0]);

  assert!(cookie_debug.contains("[REDACTED]"));
  assert!(cookie_debug.contains("session"));
  assert!(!cookie_debug.contains("abc def"));
  assert!(!cookie_debug.contains("/secret"));
  assert!(!cookies_debug.contains("abc def"));
  assert!(attribute_debug.contains("[REDACTED]"));
  assert!(!attribute_debug.contains("/secret"));

  let error = HttpSetCookie::parse("session=super-secret; Path=/; path=/other")
    .expect_err("duplicate attributes should fail");
  assert!(!error.to_string().contains("super-secret"));
  assert!(!format!("{error:?}").contains("super-secret"));
}
