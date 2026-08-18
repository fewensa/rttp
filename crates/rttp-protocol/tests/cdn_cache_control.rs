use rttp_protocol::cdn_cache_control::{
  CdnCacheControl, MAX_CDN_CACHE_CONTROL_DIRECTIVES, MAX_CDN_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES,
  MAX_CDN_CACHE_CONTROL_VALUE_BYTES,
};

#[test]
fn parses_repeated_fields_and_preserves_cdn_extension_directives() {
  let metadata = CdnCacheControl::parse_values([
    "max-age=600, stale-while-revalidate=30, cdn-example=\"a, b\"",
    "immutable, escaped=\"quoted\\\\value\"",
  ])
  .expect("CDN-Cache-Control should parse");

  assert_eq!(metadata.len(), 5);
  assert_eq!(metadata.directives()[0].name(), "max-age");
  assert_eq!(metadata.directives()[0].value(), Some("600"));
  assert_eq!(metadata.directives()[2].name(), "cdn-example");
  assert_eq!(metadata.directives()[2].value(), Some("a, b"));
  assert_eq!(metadata.directives()[3].name(), "immutable");
  assert_eq!(metadata.directives()[3].value(), None);
  assert_eq!(metadata.directives()[4].value(), Some("quoted\\value"));
  assert_eq!(
    metadata.header_value(),
    "max-age=600, stale-while-revalidate=30, cdn-example=\"a, b\", immutable, escaped=\"quoted\\\\value\""
  );
}

#[test]
fn rejects_malformed_or_empty_cdn_cache_control_values() {
  for value in [
    "",
    "max-age=",
    "max-age=not a token",
    "custom=\"unterminated",
    "max-age=60\r\nno-store",
  ] {
    assert!(
      CdnCacheControl::parse(value).is_err(),
      "{value:?} should fail"
    );
  }
}

#[test]
fn enforces_cdn_cache_control_bounds() {
  assert!(CdnCacheControl::parse("x".repeat(MAX_CDN_CACHE_CONTROL_VALUE_BYTES + 1)).is_err());
  assert!(CdnCacheControl::parse(format!(
    "x={}",
    "x".repeat(MAX_CDN_CACHE_CONTROL_DIRECTIVE_VALUE_BYTES + 1)
  ))
  .is_err());
  assert!(CdnCacheControl::parse(
    std::iter::repeat_n("x", MAX_CDN_CACHE_CONTROL_DIRECTIVES + 1)
      .collect::<Vec<_>>()
      .join(","),
  )
  .is_err());
}
