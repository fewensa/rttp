use rttp_client::response::{CacheControl, Response};
use rttp_client::types::RoUrl;

fn response_with_cache_control(value: &str) -> Response {
  let raw = format!("HTTP/1.1 200 OK\r\nCache-Control: {value}\r\nContent-Length: 2\r\n\r\nOK");
  Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response remains usable")
}

fn response_with_cache_control_bytes(value: &[u8]) -> Response {
  let mut raw = b"HTTP/1.1 200 OK\r\nCache-Control: ".to_vec();
  raw.extend_from_slice(value);
  raw.extend_from_slice(b"\r\nContent-Length: 2\r\n\r\nOK");
  Response::new(RoUrl::with("https://example.test"), raw).expect("raw response remains usable")
}

fn response_with_cache_control_fields(values: &[&str]) -> Response {
  let mut raw = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    raw.push_str("Cache-Control: ");
    raw.push_str(value);
    raw.push_str("\r\n");
  }
  raw.push_str("Content-Length: 2\r\n\r\nOK");
  Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response remains usable")
}

#[test]
fn cache_control_accepts_sp_htab_ows_around_directives_names_equals_and_values() {
  let value = " \tno-store\t ,\t max-age \t=\t 60\t ,\t community \t=\t private\t ";
  let cache_control = CacheControl::parse(value).expect("SP/HTAB OWS should parse");

  assert!(cache_control.no_store());
  assert_eq!(Some(60), cache_control.max_age());
  assert_eq!(1, cache_control.extensions().len());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(Some("private"), cache_control.extensions()[0].value());

  let response = response_with_cache_control(value);
  let parsed = response
    .cache_control()
    .expect("valid cache-control should parse")
    .expect("cache-control header should be present");
  assert_eq!(cache_control, parsed);
  // Response framing trims outer SP/HTAB from stored header values.
  assert_eq!(
    Some(&"no-store\t ,\t max-age \t=\t 60\t ,\t community \t=\t private".to_string()),
    response.header_value("Cache-Control")
  );
}

#[test]
fn cache_control_preserves_quoted_strings_duplicates_bounds_and_accessors() {
  let response = response_with_cache_control_fields(&[
    "no-cache=\"Set-Cookie, Authorization\", max-age=60",
    "private=\"X-User\", community=\"u=1, tier=gold\"",
  ]);
  let cache_control = response
    .cache_control()
    .expect("duplicate Cache-Control fields should flatten")
    .expect("cache-control header should be present");

  assert!(cache_control.no_cache());
  assert_eq!(
    vec!["Set-Cookie", "Authorization"],
    cache_control.no_cache_fields()
  );
  assert_eq!(Some(60), cache_control.max_age());
  assert!(cache_control.private());
  assert_eq!(vec!["X-User"], cache_control.private_fields());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(
    Some("u=1, tier=gold"),
    cache_control.extensions()[0].value()
  );

  let quoted_ows = CacheControl::parse(r#"community= "say \"hi\"" "#)
    .expect("quoted-string with OWS padding should parse");
  assert_eq!(Some("say \"hi\""), quoted_ows.extensions()[0].value());

  let exact_directives = (0..256)
    .map(|index| format!("ext-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    CacheControl::parse(&exact_directives).is_ok(),
    "256 directives should parse"
  );

  let overflow_directives = (0..=256)
    .map(|index| format!("ext-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    CacheControl::parse(&overflow_directives).is_err(),
    "257 directives should be rejected"
  );

  let oversized = "x".repeat(64 * 1024 + 1);
  assert!(
    CacheControl::parse(&oversized).is_err(),
    "values over 64KiB should be rejected"
  );
}

#[test]
fn cache_control_parse_rejects_vt_ff_cr_lf_and_unicode_padding() {
  let invalid_values = [
    "\u{000b}no-store",
    "no-store\u{000c}",
    "max-age\u{000b}=60",
    "max-age=\u{000c}60",
    "community\u{00a0}=private",
    "community=\u{00a0}private",
    "community=private\u{2003}",
    "no-store,\u{000b}immutable",
    "community=\"ok\"\u{000b}",
    "community=\"ok\"\u{000c}",
    "community=\"ok\"\u{00a0}",
    "\rno-store",
    "no-store\n",
  ];

  for value in invalid_values {
    assert!(
      CacheControl::parse(value).is_err(),
      "CacheControl::parse should reject {value:?}"
    );
  }
}

#[test]
fn cache_control_rejects_non_ows_obs_text_without_dropping_response_or_header() {
  let invalid_values: &[&[u8]] = &[
    b"\xa0no-store",
    b"no-store\xa0",
    b"max-age\xa0=60",
    b"max-age=\xa060",
    b"community\xa0=private",
    b"community=\xa0private",
    b"community=private\xa0",
    b"no-store,\xa0immutable",
    b"community=\"ok\"\xa0",
  ];

  for value in invalid_values {
    let expected = value.iter().copied().map(char::from).collect::<String>();
    assert!(
      CacheControl::parse(&expected).is_err(),
      "CacheControl::parse should reject {expected:?}"
    );

    let response = response_with_cache_control_bytes(value);
    assert!(
      response.cache_control().is_err(),
      "cache_control helper should reject {expected:?}"
    );
    assert_eq!(
      Some(&expected),
      response.header_value("Cache-Control"),
      "original Cache-Control header must remain available for {expected:?}"
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}
