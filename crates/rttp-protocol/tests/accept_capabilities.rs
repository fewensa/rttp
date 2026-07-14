use rttp_protocol::accept_patch::{
  AcceptPatch, MAX_ACCEPT_PATCH_MEDIA_TYPES, MAX_ACCEPT_PATCH_VALUE_BYTES,
};
use rttp_protocol::accept_post::{
  AcceptPost, MAX_ACCEPT_POST_MEDIA_TYPES, MAX_ACCEPT_POST_VALUE_BYTES,
};

#[test]
fn accept_patch_parses_media_types_with_parameters_and_preserves_order() {
  let accept_patch = AcceptPatch::parse_values([
    "application/merge-patch+json; charset=utf-8",
    "application/json; profile=\"https://example.test/profile, v1\"",
  ])
  .expect("Accept-Patch should parse");

  assert_eq!(accept_patch.len(), 2);
  assert_eq!(accept_patch.media_types()[0].type_(), "application");
  assert_eq!(accept_patch.media_types()[0].subtype(), "merge-patch+json");
  assert_eq!(
    accept_patch.media_types()[0].parameters()[0].name(),
    "charset"
  );
  assert_eq!(
    accept_patch.media_types()[0].parameters()[0].value(),
    "utf-8"
  );
  assert_eq!(
    accept_patch.media_types()[1].parameters()[0].value(),
    "https://example.test/profile, v1"
  );
  assert_eq!(
    accept_patch.header_value(),
    "application/merge-patch+json; charset=utf-8, application/json; profile=\"https://example.test/profile, v1\""
  );
}

#[test]
fn accept_post_parses_media_types_with_parameters_and_preserves_order() {
  let accept_post = AcceptPost::parse("application/json; charset=utf-8, text/plain")
    .expect("Accept-Post should parse");

  assert_eq!(accept_post.len(), 2);
  assert_eq!(accept_post.media_types()[0].subtype(), "json");
  assert_eq!(accept_post.media_types()[1].type_(), "text");
  assert_eq!(
    accept_post.header_value(),
    "application/json; charset=utf-8, text/plain"
  );
}

#[test]
fn accept_capability_headers_reject_invalid_media_types_and_empty_members() {
  for value in [
    "application",
    "/json",
    "application/",
    "application/json; charset",
    "application/json,, text/plain",
    ",application/json",
    "application/json,",
  ] {
    assert!(AcceptPatch::parse(value).is_err(), "{value:?} should fail");
    assert!(AcceptPost::parse(value).is_err(), "{value:?} should fail");
  }
}

#[test]
fn accept_capability_headers_enforce_value_and_list_limits() {
  assert!(AcceptPatch::parse("x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES + 1)).is_err());
  assert!(AcceptPost::parse("x".repeat(MAX_ACCEPT_POST_VALUE_BYTES + 1)).is_err());

  assert!(AcceptPatch::parse(
    std::iter::repeat_n("application/json", MAX_ACCEPT_PATCH_MEDIA_TYPES + 1)
      .collect::<Vec<_>>()
      .join(","),
  )
  .is_err());
  assert!(AcceptPost::parse(
    std::iter::repeat_n("application/json", MAX_ACCEPT_POST_MEDIA_TYPES + 1)
      .collect::<Vec<_>>()
      .join(","),
  )
  .is_err());
}
