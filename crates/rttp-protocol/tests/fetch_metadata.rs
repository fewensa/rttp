use rttp_protocol::fetch_metadata::{
  SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, MAX_SEC_FETCH_DEST_VALUE_BYTES,
  MAX_SEC_FETCH_MODE_VALUE_BYTES, MAX_SEC_FETCH_SITE_VALUE_BYTES, MAX_SEC_FETCH_USER_VALUE_BYTES,
};

#[test]
fn fetch_metadata_parses_standard_browser_values() {
  assert_eq!(
    SecFetchSite::SameOrigin,
    SecFetchSite::parse("same-origin").expect("valid Sec-Fetch-Site")
  );
  assert_eq!(
    SecFetchMode::Navigate,
    SecFetchMode::parse("navigate").expect("valid Sec-Fetch-Mode")
  );
  assert_eq!(
    SecFetchDest::Image,
    SecFetchDest::parse("image").expect("valid Sec-Fetch-Dest")
  );
  assert_eq!(
    SecFetchUser::Activated,
    SecFetchUser::parse("?1").expect("valid Sec-Fetch-User")
  );
}

#[test]
fn fetch_metadata_serializes_standard_browser_values() {
  assert_eq!("cross-site", SecFetchSite::CrossSite.header_value());
  assert_eq!("no-cors", SecFetchMode::NoCors.header_value());
  assert_eq!("empty", SecFetchDest::Empty.header_value());
  assert_eq!("?1", SecFetchUser::Activated.header_value());
}

#[test]
fn fetch_metadata_rejects_empty_unknown_and_list_like_values() {
  for value in [
    "",
    "unknown",
    "same-origin, same-site",
    "same origin",
    "same-origin\r\nX: y",
  ] {
    assert!(
      SecFetchSite::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  for value in ["", "websocket, cors", "CORS", "no cors"] {
    assert!(
      SecFetchMode::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  for value in ["", "image, script", "IMAGE", "not-a-destination"] {
    assert!(
      SecFetchDest::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  for value in ["", "?0", "?1, ?1", "true"] {
    assert!(
      SecFetchUser::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn fetch_metadata_enforces_value_bounds() {
  assert!(SecFetchSite::parse("a".repeat(MAX_SEC_FETCH_SITE_VALUE_BYTES + 1)).is_err());
  assert!(SecFetchMode::parse("a".repeat(MAX_SEC_FETCH_MODE_VALUE_BYTES + 1)).is_err());
  assert!(SecFetchDest::parse("a".repeat(MAX_SEC_FETCH_DEST_VALUE_BYTES + 1)).is_err());
  assert!(SecFetchUser::parse("a".repeat(MAX_SEC_FETCH_USER_VALUE_BYTES + 1)).is_err());
}

#[test]
fn fetch_metadata_rejects_duplicate_singleton_fields() {
  assert!(SecFetchSite::parse_values(["same-origin", "same-site"]).is_err());
  assert!(SecFetchMode::parse_values(["cors", "navigate"]).is_err());
  assert!(SecFetchDest::parse_values(["image", "script"]).is_err());
  assert!(SecFetchUser::parse_values(["?1", "?1"]).is_err());
}

#[test]
fn fetch_metadata_checks_every_singleton_field_against_its_bound() {
  let error = SecFetchSite::parse_values([
    "same-origin",
    &"a".repeat(MAX_SEC_FETCH_SITE_VALUE_BYTES + 1),
  ])
  .expect_err("an oversized duplicate field must not bypass the value bound");

  assert!(error.to_string().contains("too large"));
}
