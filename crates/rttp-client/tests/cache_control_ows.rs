use rttp_client::response::{CacheControl, Response};
use rttp_client::types::RoUrl;

fn response_with_cache_control(value: &str) -> Response {
  let raw = format!(
    "HTTP/1.1 200 OK\r\nCache-Control: {value}\r\nContent-Length: 2\r\n\r\nOK"
  );
  Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("raw response remains usable")
}

fn response_with_cache_control_bytes(value: &[u8]) -> Response {
  let mut raw = b"HTTP/1.1 200 OK\r\nCache-Control: ".to_vec();
  raw.extend_from_slice(value);
  raw.extend_from_slice(b"\r\nContent-Length: 2\r\n\r\nOK");
  Response::new(RoUrl::with("https://example.test"), raw)
    .expect("raw response remains usable")
}

#[test]
fn cache_control_accepts_sp_and_htab_ows_around_directives() {
  let value = "max-age \t= \t60 \t,\t no-store \t,\t community \t= \t\"a, b\"";
  let response = response_with_cache_control(value);

  let cache_control = response
    .cache_control()
    .expect("SP/HTAB OWS should parse")
    .expect("Cache-Control should be present");

  assert_eq!(Some(60), cache_control.max_age());
  assert!(cache_control.no_store());
  assert_eq!(1, cache_control.extensions().len());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(Some("a, b"), cache_control.extensions()[0].value());
  assert_eq!(
    Some(&value.to_string()),
    response.header_value("Cache-Control")
  );
  assert_eq!("OK", response.body().string().unwrap());
}

#[test]
fn cache_control_rejects_non_ows_obs_text_without_dropping_header() {
  let invalid_values: &[&[u8]] = &[
    b"\x85max-age=60",
    b"max-age\x85=60",
    b"max-age=\x8560",
    b"max-age=60\x85",
    b"max-age=60,\x85no-store",
    b"community=\"x\"\x85",
    b"\xa0max-age=60",
    b"max-age\xa0=60",
    b"max-age=\xa060",
    b"max-age=60\xa0",
    b"max-age=60,\xa0no-store",
    b"community=\"x\"\xa0",
  ];

  for value in invalid_values {
    let response = response_with_cache_control_bytes(value);
    let expected = value.iter().copied().map(char::from).collect::<String>();
    assert!(
      response.cache_control().is_err(),
      "cache_control helper should reject {expected:?}"
    );
    assert_eq!(
      Some(&expected),
      response.header_value("Cache-Control")
    );
    assert_eq!("OK", response.body().string().unwrap());
  }
}

#[test]
fn cache_control_rejects_vt_and_cr_lf_padding_via_typed_parser() {
  for padding in ["\u{000b}", "\r", "\n", "\r\n"] {
    let between = format!("max-age=60,{padding}no-store");
    assert!(
      CacheControl::parse(&between).is_err(),
      "non-OWS padding should be rejected: {between:?}"
    );

    let after_quote = format!("community=\"x\"{padding}");
    assert!(
      CacheControl::parse(&after_quote).is_err(),
      "non-OWS after quoted-string should be rejected: {after_quote:?}"
    );

    let before_name = format!("{padding}max-age=60");
    assert!(
      CacheControl::parse(&before_name).is_err(),
      "non-OWS before directive should be rejected: {before_name:?}"
    );
  }
}

#[test]
fn cache_control_preserves_quoted_commas_and_duplicate_fields() {
  let raw = concat!(
    "HTTP/1.1 200 OK\r\n",
    "Cache-Control: no-cache=\"Set-Cookie, Authorization\", max-age=60\r\n",
    "Cache-Control: community=\"u=1, tier=gold\", no-store\r\n",
    "Content-Length: 2\r\n",
    "\r\n",
    "OK"
  );
  let response = Response::new(RoUrl::with("https://example.test"), raw.as_bytes().to_vec())
    .expect("duplicate Cache-Control fields remain usable");

  let cache_control = response
    .cache_control()
    .expect("quoted commas and duplicates should parse")
    .expect("Cache-Control should be present");

  assert!(cache_control.no_cache());
  assert_eq!(
    vec!["Set-Cookie", "Authorization"],
    cache_control.no_cache_fields()
  );
  assert_eq!(Some(60), cache_control.max_age());
  assert!(cache_control.no_store());
  assert_eq!(1, cache_control.extensions().len());
  assert_eq!("community", cache_control.extensions()[0].name());
  assert_eq!(
    Some("u=1, tier=gold"),
    cache_control.extensions()[0].value()
  );
  assert_eq!("OK", response.body().string().unwrap());
}
