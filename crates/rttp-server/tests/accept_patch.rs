use rttp_protocol::accept_patch::MAX_ACCEPT_PATCH_MEDIA_TYPES;
use rttp_server::server::{
  HttpAcceptPatch, HttpAcceptPatchParseError, HttpMediaType, HttpMediaTypeParameter, HttpResponse,
};

fn header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
  response.lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    header_name
      .eq_ignore_ascii_case(name)
      .then_some(value.trim())
  })
}

#[test]
fn response_accept_patch_builder_uses_protocol_media_types_and_replaces_fields() {
  let response = HttpResponse::ok("body")
    .header("Accept-Patch", "application/old")
    .header("accept-patch", "application/stale")
    .with_accept_patch([r#"Text/Plain; title="a,b\"c""#, "application/json"])
    .expect("Accept-Patch declaration should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(
    1,
    serialized
      .lines()
      .filter(|line| line.to_ascii_lowercase().starts_with("accept-patch:"))
      .count()
  );
  assert_eq!(
    Some(r#"Text/Plain; title="a,b\"c", application/json"#),
    header_value(&serialized, "Accept-Patch")
  );

  let metadata = response
    .accept_patch()
    .expect("attached Accept-Patch should parse")
    .expect("Accept-Patch should be present");
  let _: &[HttpMediaType] = metadata.media_types();
  let _: &[HttpMediaTypeParameter] = metadata.media_types()[0].parameters();
  assert_eq!(metadata.len(), 2);
  assert_eq!(metadata.media_types()[0].type_(), "Text");
  assert_eq!(metadata.media_types()[0].subtype(), "Plain");
  assert_eq!(metadata.media_types()[0].parameters()[0].value(), "a,b\"c");
}

#[test]
fn response_accept_patch_accessor_preserves_order_and_duplicates() {
  let response = HttpResponse::ok("")
    .header("Accept-Patch", "application/json, text/plain")
    .header("accept-patch", "application/json");
  let metadata = response
    .accept_patch()
    .expect("Accept-Patch should parse")
    .expect("Accept-Patch should be present");

  assert_eq!(
    vec![
      ("application", "json"),
      ("text", "plain"),
      ("application", "json"),
    ],
    metadata
      .media_types()
      .iter()
      .map(|media_type| (media_type.type_(), media_type.subtype()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn response_accept_patch_builder_uses_256_member_bound_and_is_atomic_on_failure() {
  let accepted = HttpResponse::ok("")
    .with_accept_patch(vec!["application/json"; 256])
    .expect("256 Accept-Patch members should parse");
  assert_eq!(256, accepted.accept_patch().unwrap().unwrap().len());
  assert!(HttpResponse::ok("")
    .with_accept_patch(vec!["application/json"; 257])
    .is_err());

  let original = HttpResponse::ok("").header("Accept-Patch", "application/json");
  assert!(original
    .clone()
    .with_accept_patch(["not-a-media-type"])
    .is_err());
  let serialized = String::from_utf8(original.to_bytes()).expect("response should serialize");
  assert_eq!(
    Some("application/json"),
    header_value(&serialized, "Accept-Patch")
  );
}

#[test]
fn response_accept_patch_builder_does_not_materialize_unbounded_iterators() {
  let mut yielded = 0;
  let result = HttpResponse::ok("").with_accept_patch(std::iter::from_fn(|| {
    yielded += 1;
    Some("application/json")
  }));

  assert!(result.is_err());
  assert_eq!(MAX_ACCEPT_PATCH_MEDIA_TYPES + 1, yielded);
}

#[test]
fn response_accept_patch_error_is_the_shared_protocol_error() {
  let _: HttpAcceptPatchParseError =
    HttpAcceptPatch::parse("application/json,").expect_err("malformed metadata should fail");
}
