use rttp_protocol::cache_status::{
  CacheStatus, CacheStatusIdentifier, MAX_CACHE_STATUS_MEMBERS, MAX_CACHE_STATUS_PARAMETERS,
  MAX_CACHE_STATUS_PARAMETER_VALUE_BYTES, MAX_CACHE_STATUS_VALUE_BYTES,
};

#[test]
fn parses_rfc_9211_origin_and_cdn_members() {
  let metadata = CacheStatus::parse_values([
    "OriginCache; hit; ttl=1100",
    r#""CDN Company Here"; hit; ttl=545"#,
  ])
  .expect("Cache-Status should parse");

  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.members()[0].identifier().as_str(), "OriginCache");
  assert!(metadata.members()[0].identifier().is_token());
  assert_eq!(metadata.members()[0].hit(), Some(true));
  assert_eq!(metadata.members()[0].ttl(), Some(1100));
  assert_eq!(
    metadata.members()[1].identifier(),
    &CacheStatusIdentifier::String("CDN Company Here".to_owned())
  );
  assert!(metadata.members()[1].identifier().is_string());
  assert_eq!(metadata.members()[1].hit(), Some(true));
  assert_eq!(metadata.members()[1].ttl(), Some(545));
  assert_eq!(
    metadata.header_value(),
    r#"OriginCache; hit; ttl=1100, "CDN Company Here"; hit; ttl=545"#
  );
}

#[test]
fn parses_forwarding_and_known_parameters_including_negative_ttl() {
  let metadata = CacheStatus::parse(
    r#"Edge; fwd=uri-miss; fwd-status=200; ttl=-15; stored=?0; collapsed; key="ABC890"; detail=stale"#,
  )
  .expect("Cache-Status should parse");

  let member = &metadata.members()[0];
  assert_eq!(member.identifier().as_str(), "Edge");
  assert_eq!(member.fwd(), Some("uri-miss"));
  assert_eq!(member.fwd_status(), Some(200));
  assert_eq!(member.ttl(), Some(-15));
  assert_eq!(member.stored(), Some(false));
  assert_eq!(member.collapsed(), Some(true));
  assert_eq!(member.key(), Some("ABC890"));
  assert_eq!(
    member.detail(),
    Some(&CacheStatusIdentifier::Token("stale".to_owned()))
  );
}

#[test]
fn parses_bare_identifier_and_both_hit_and_fwd() {
  let bare = CacheStatus::parse("OriginCache").expect("bare identifier should parse");
  assert_eq!(bare.members()[0].hit(), None);
  assert_eq!(bare.members()[0].fwd(), None);

  let both = CacheStatus::parse("Edge; hit; fwd=miss").expect("hit and fwd should parse");
  assert_eq!(both.members()[0].hit(), Some(true));
  assert_eq!(both.members()[0].fwd(), Some("miss"));
}

#[test]
fn preserves_unknown_extension_parameters() {
  let metadata = CacheStatus::parse(r#"Edge; hit; vendor=alpha; note="a, b"; flag"#)
    .expect("extension parameters should parse");

  let extensions = metadata.members()[0].extensions();
  assert_eq!(extensions.len(), 3);
  assert_eq!(extensions[0].name(), "vendor");
  assert_eq!(extensions[0].value(), Some("alpha"));
  assert_eq!(extensions[1].name(), "note");
  assert_eq!(extensions[1].value(), Some(r#""a, b""#));
  assert_eq!(extensions[2].name(), "flag");
  assert_eq!(extensions[2].value(), None);
}

#[test]
fn rejects_control_bytes_empty_members_inner_lists_and_trailing_commas() {
  for value in [
    "OriginCache\r\nhit",
    "OriginCache\n; hit",
    "OriginCache\0; hit",
    "OriginCache;\x7f hit",
    "",
    "OriginCache,",
    ",OriginCache",
    "OriginCache,,Edge",
    "OriginCache, ",
    "(OriginCache Edge)",
    "OriginCache, (Edge)",
  ] {
    assert!(CacheStatus::parse(value).is_err(), "{value:?} should fail");
  }
}

#[test]
fn rejects_invalid_booleans_integers_and_typed_parameters() {
  for value in [
    "OriginCache; hit=yes",
    "OriginCache; hit=1",
    "OriginCache; stored=true",
    "OriginCache; ttl=1.5",
    "OriginCache; ttl=\"1100\"",
    "OriginCache; fwd-status=ok",
    "OriginCache; fwd=\"miss\"",
    "OriginCache; key=ABC890",
    "OriginCache; detail=?",
  ] {
    assert!(CacheStatus::parse(value).is_err(), "{value:?} should fail");
  }
}

#[test]
fn rejects_duplicate_parameters_but_combines_repeated_fields() {
  assert!(CacheStatus::parse("Edge; hit; hit").is_err());
  assert!(CacheStatus::parse("Edge; ttl=1; ttl=2").is_err());
  assert!(CacheStatus::parse("Edge; vendor=1; vendor=2").is_err());

  let combined = CacheStatus::parse_values(["OriginCache; hit", "Edge; fwd=miss"])
    .expect("repeated Cache-Status fields should combine");
  assert_eq!(combined.len(), 2);
  assert_eq!(combined.members()[0].identifier().as_str(), "OriginCache");
  assert_eq!(combined.members()[1].identifier().as_str(), "Edge");
}

#[test]
fn enforces_cache_status_bounds() {
  assert!(CacheStatus::parse("x".repeat(MAX_CACHE_STATUS_VALUE_BYTES + 1)).is_err());
  assert!(CacheStatus::parse(format!(
    "a; k=\"{}\"",
    "x".repeat(MAX_CACHE_STATUS_PARAMETER_VALUE_BYTES + 1)
  ))
  .is_err());
  assert!(CacheStatus::parse(
    std::iter::repeat_n("a", MAX_CACHE_STATUS_MEMBERS + 1)
      .collect::<Vec<_>>()
      .join(",")
  )
  .is_err());
  assert!(CacheStatus::parse(format!(
    "a{}",
    (0..=MAX_CACHE_STATUS_PARAMETERS)
      .map(|index| format!("; p{index}"))
      .collect::<String>()
  ))
  .is_err());
}
