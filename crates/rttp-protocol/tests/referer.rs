use rttp_protocol::referer::{Referer, MAX_REFERER_VALUE_BYTES};

#[test]
fn referer_preserves_absolute_and_relative_references() {
  let absolute =
    Referer::parse("https://shop.example/checkout?step=pay").expect("absolute Referer must parse");
  let relative = Referer::parse("/checkout?step=pay").expect("relative Referer must parse");
  let scheme_relative =
    Referer::parse("//cdn.example/lib.js").expect("scheme-relative Referer must parse");
  let mixed_case = Referer::parse("https://EXAMPLE.test:443/A%2fB")
    .expect("non-canonical absolute Referer must parse");
  let opaque_relative = Referer::parse("null").expect("relative path null must parse");

  assert_eq!(
    "https://shop.example/checkout?step=pay",
    absolute.header_value()
  );
  assert_eq!("/checkout?step=pay", relative.header_value());
  assert_eq!("//cdn.example/lib.js", scheme_relative.header_value());
  assert_eq!("https://EXAMPLE.test:443/A%2fB", mixed_case.header_value());
  assert_eq!("null", opaque_relative.header_value());
}

#[test]
fn referer_trims_http_optional_whitespace() {
  let referer =
    Referer::parse("\thttps://example.test/path?q=1\t").expect("OWS-padded Referer must parse");

  assert_eq!("https://example.test/path?q=1", referer.header_value());
}

#[test]
fn referer_preserves_query_userinfo_and_comma() {
  let query = Referer::parse("/items?ids=1,2").expect("comma in query must parse");
  let userinfo =
    Referer::parse("https://user:pass@shop.example/checkout").expect("userinfo Referer must parse");

  assert_eq!("/items?ids=1,2", query.header_value());
  assert_eq!(
    "https://user:pass@shop.example/checkout",
    userinfo.header_value()
  );
}

#[test]
fn referer_rejects_controls_fragments_and_malformed_values() {
  for value in [
    "",
    "   ",
    "\t",
    "https://shop.example/checkout\r\nX-Injected: true",
    "https://example.test/path#frag",
    "#frag",
    "https://example.test/path with space",
    "https://example.test/foo\\bar",
    "https://example.test/a<b>",
    "https://exämple.test/",
    "https://example.test/%ZZ",
    "https://example.test/%2",
    "https://example.test/%",
    "https://",
  ] {
    assert!(Referer::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    Referer::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn referer_rejects_duplicate_singleton_fields() {
  assert!(Referer::parse_values(["https://example.test/a", "https://example.test/b"]).is_err());
  assert!(Referer::parse_values(["/same", "/same"]).is_err());
}

#[test]
fn referer_enforces_value_bounds_without_panicking() {
  assert!(Referer::parse("a".repeat(MAX_REFERER_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_REFERER_VALUE_BYTES + 1);
  assert!(Referer::parse_values(["https://example.test/", oversized_duplicate.as_str()]).is_err());
}
