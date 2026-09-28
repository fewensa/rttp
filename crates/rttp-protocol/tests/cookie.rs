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
fn cookie_builder_rejects_control_bytes_in_values_and_attributes() {
  for value in ["b\r\nX-Injected: 1", "b\u{1}c", "b\u{7f}c"] {
    let error = HttpSetCookie::new("session", value)
      .expect_err("builder should reject control bytes in cookie values");
    assert_eq!("cookie header contains a control byte", error.to_string());
    assert!(!format!("{error:?}").contains(value));
  }

  for builder in [
    HttpSetCookie::new("a", "b")
      .expect("base cookie should be valid")
      .with_expires("Wed, 21 Oct 2015 07:28:00 GMT\r\nX-Expires: 1"),
    HttpSetCookie::new("a", "b")
      .expect("base cookie should be valid")
      .with_domain("example.com\r\nX-Domain: 1"),
    HttpSetCookie::new("a", "b")
      .expect("base cookie should be valid")
      .with_path("/x\r\nX-Path: 1"),
    HttpSetCookie::new("a", "b")
      .expect("base cookie should be valid")
      .with_priority("High\r\nX-Priority: 1"),
    HttpSetCookie::new("a", "b")
      .expect("base cookie should be valid")
      .with_extension("Ext", Some("v\r\nX-Ext: 1")),
  ] {
    let error = builder.expect_err("builder should reject control bytes in attributes");
    assert_eq!("cookie header contains a control byte", error.to_string());
    assert!(!format!("{error:?}").contains("X-"));
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
fn cookie_max_age_takes_precedence_over_expires_and_round_trips() {
  let expires_first =
    HttpSetCookie::parse("session=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=60; Path=/")
      .expect("Max-Age after Expires should parse");
  assert_eq!(Some(60), expires_first.max_age());
  assert_eq!(
    Some("Wed, 21 Oct 2015 07:28:00 GMT"),
    expires_first.expires()
  );
  assert_eq!(Some("/"), expires_first.path());
  assert_eq!(
    "session=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=60; Path=/",
    expires_first.header_value()
  );
  assert_eq!(
    expires_first,
    HttpSetCookie::parse(expires_first.header_value())
      .expect("canonical Max-Age form should reparse")
  );

  let max_age_first =
    HttpSetCookie::parse("session=abc; Max-Age=120; Expires=Wed, 21 Oct 2015 07:28:00 GMT")
      .expect("Max-Age before Expires should parse");
  assert_eq!(Some(120), max_age_first.max_age());
  assert_eq!(
    Some("Wed, 21 Oct 2015 07:28:00 GMT"),
    max_age_first.expires()
  );
  assert_eq!(
    "session=abc; Max-Age=120; Expires=Wed, 21 Oct 2015 07:28:00 GMT",
    max_age_first.header_value()
  );

  let built = HttpSetCookie::new("session", "abc")
    .expect("cookie should be valid")
    .with_expires("Wed, 21 Oct 2015 07:28:00 GMT")
    .expect("Expires should be accepted")
    .with_max_age(60)
    .expect("Max-Age should be accepted");
  assert_eq!(Some(60), built.max_age());
  assert_eq!(Some("Wed, 21 Oct 2015 07:28:00 GMT"), built.expires());
  assert_eq!(
    "session=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=60",
    built.header_value()
  );
}

#[test]
fn cookie_accepts_zero_and_u64_max_age_and_rejects_signed_or_overflowing_values() {
  let zero = HttpSetCookie::parse("session=abc; Max-Age=0").expect("Max-Age=0 should parse");
  assert_eq!(Some(0), zero.max_age());
  assert_eq!("session=abc; Max-Age=0", zero.header_value());

  let maximum = HttpSetCookie::parse(format!("session=abc; Max-Age={}", u64::MAX))
    .expect("maximum u64 Max-Age should parse");
  assert_eq!(Some(u64::MAX), maximum.max_age());
  assert_eq!(
    format!("session=abc; Max-Age={}", u64::MAX),
    maximum.header_value()
  );
  assert_eq!(
    maximum,
    HttpSetCookie::parse(maximum.header_value()).expect("maximum Max-Age should round-trip")
  );

  let leading_zeros =
    HttpSetCookie::parse("session=abc; Max-Age=00060").expect("leading zeros should parse");
  assert_eq!(Some(60), leading_zeros.max_age());
  assert_eq!("session=abc; Max-Age=00060", leading_zeros.header_value());

  for value in [
    "session=abc; Max-Age=-1",
    "session=abc; Max-Age=-0",
    "session=abc; Max-Age=+60",
    "session=abc; Max-Age=",
    "session=abc; Max-Age=60.0",
    "session=abc; Max-Age=1e2",
    "session=abc; Max-Age=18446744073709551616",
    "session=abc; Max-Age=18446744073709551617",
  ] {
    assert!(
      HttpSetCookie::parse(value).is_err(),
      "Set-Cookie should reject {value:?}"
    );
  }
}

#[test]
fn cookie_rejects_duplicate_lifetime_and_samesite_attributes() {
  for value in [
    "session=abc; Max-Age=10; Max-Age=20",
    "session=abc; max-age=10; MAX-AGE=20",
    "session=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT; expires=Thu, 22 Oct 2015 07:28:00 GMT",
    "session=abc; SameSite=Lax; SameSite=None",
    "session=abc; SameSite=None; samesite=Strict; Secure",
    "session=abc; Secure; secure",
  ] {
    assert!(
      HttpSetCookie::parse(value).is_err(),
      "Set-Cookie should reject {value:?}"
    );
  }
}

#[test]
fn cookie_parses_quoted_values_and_samesite_none_with_secure() {
  let cookie =
    HttpSetCookie::parse(r#"session="abc def"; Path="/app"; Max-Age="60"; SameSite=None; Secure"#)
      .expect("quoted values and SameSite=None with Secure should parse");

  assert_eq!("session", cookie.name());
  assert_eq!("abc def", cookie.value());
  assert!(cookie.is_value_quoted());
  assert_eq!(Some("/app"), cookie.path());
  assert!(cookie.attributes()[0].is_quoted());
  assert_eq!(Some(60), cookie.max_age());
  assert!(cookie.attributes()[1].is_quoted());
  assert_eq!(Some(HttpSameSite::None), cookie.same_site());
  assert!(cookie.secure());
  assert_eq!(
    r#"session="abc def"; Path="/app"; Max-Age="60"; SameSite=None; Secure"#,
    cookie.header_value()
  );
  assert_eq!(
    cookie,
    HttpSetCookie::parse(cookie.header_value()).expect("quoted SameSite=None form should reparse")
  );

  let built = HttpSetCookie::new("session", "abc def")
    .expect("quoted cookie value should be accepted")
    .with_same_site(HttpSameSite::None)
    .expect("SameSite=None should be accepted")
    .with_secure()
    .expect("Secure should be accepted");
  assert_eq!(Some(HttpSameSite::None), built.same_site());
  assert!(built.secure());
  assert_eq!(
    r#"session="abc def"; SameSite=None; Secure"#,
    built.header_value()
  );
  assert_eq!(
    built,
    HttpSetCookie::parse(built.header_value()).expect("builder SameSite=None form should reparse")
  );
}

#[test]
fn cookie_serializes_attributes_in_insertion_order_across_round_trips() {
  let cookie = HttpSetCookie::parse(
    r#"session="abc def"; HttpOnly; Path=/; SameSite=Lax; Max-Age=60; Secure; Foo=bar"#,
  )
  .expect("insertion-order attributes should parse");

  assert_eq!(
    vec![
      HttpSetCookieAttributeKind::HttpOnly,
      HttpSetCookieAttributeKind::Path,
      HttpSetCookieAttributeKind::SameSite,
      HttpSetCookieAttributeKind::MaxAge,
      HttpSetCookieAttributeKind::Secure,
      HttpSetCookieAttributeKind::Extension,
    ],
    cookie
      .attributes()
      .iter()
      .map(|attribute| attribute.kind())
      .collect::<Vec<_>>()
  );
  assert_eq!(
    r#"session="abc def"; HttpOnly; Path=/; SameSite=Lax; Max-Age=60; Secure; Foo=bar"#,
    cookie.header_value()
  );
  assert_eq!(
    cookie,
    HttpSetCookie::parse(cookie.header_value())
      .expect("insertion-order serialization should reparse")
  );

  let cookies = HttpSetCookies::parse_values([
    r#"session="abc def"; SameSite=None; Secure"#,
    "csrf=token; Max-Age=60; Expires=Wed, 21 Oct 2015 07:28:00 GMT",
  ])
  .expect("repeated Set-Cookie fields should remain separate");
  assert_eq!(2, cookies.len());
  assert_eq!(Some(HttpSameSite::None), cookies.cookies()[0].same_site());
  assert!(cookies.cookies()[0].secure());
  assert_eq!(Some(60), cookies.cookies()[1].max_age());
  assert_eq!(
    vec![
      r#"session="abc def"; SameSite=None; Secure"#,
      "csrf=token; Max-Age=60; Expires=Wed, 21 Oct 2015 07:28:00 GMT",
    ],
    cookies.header_values()
  );
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
