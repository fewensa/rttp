use rttp_protocol::accept_patch::{
  AcceptPatch, MAX_ACCEPT_PATCH_MEDIA_TYPES, MAX_ACCEPT_PATCH_PARAMETERS,
  MAX_ACCEPT_PATCH_VALUE_BYTES,
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

#[test]
fn accept_patch_preserves_duplicates_order_and_quoted_parameters() {
  let parsed =
    AcceptPatch::parse_values([r#"Text/Plain; title="a,b\"c""#, "text/plain; charset=utf-8"])
      .expect("Accept-Patch should parse quoted commas and escapes");

  assert_eq!(parsed.len(), 2);
  assert_eq!(parsed.media_types()[0].type_(), "Text");
  assert_eq!(parsed.media_types()[0].subtype(), "Plain");
  assert_eq!(parsed.media_types()[0].parameters()[0].name(), "title");
  assert_eq!(parsed.media_types()[0].parameters()[0].value(), "a,b\"c");
  assert_eq!(parsed.media_types()[1].type_(), "text");
  assert_eq!(parsed.media_types()[1].subtype(), "plain");
  assert_eq!(
    parsed.header_value(),
    r#"Text/Plain; title="a,b\"c", text/plain; charset=utf-8"#
  );
}

#[test]
fn accept_patch_rejects_control_bytes_and_accepts_exact_value_bound() {
  for value in [
    "application/json\0",
    "application/json\r",
    "application/json\n",
    "application/json\u{7f}",
  ] {
    assert!(AcceptPatch::parse(value).is_err(), "{value:?} should fail");
  }

  let prefix = "application/";
  let exact = format!(
    "{prefix}{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - prefix.len())
  );
  assert_eq!(exact.len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(AcceptPatch::parse(&exact).is_ok());
  assert!(AcceptPatch::parse(format!("{exact}x")).is_err());
}

#[test]
fn accept_patch_bounds_canonical_serialization_at_exact_value_limit() {
  let raw_prefix = "a/b;foo=";
  let raw_at_limit = format!(
    "{raw_prefix}{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - raw_prefix.len())
  );
  assert_eq!(raw_at_limit.len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(AcceptPatch::parse(&raw_at_limit).is_err());
  assert!(AcceptPatch::from_media_types([raw_at_limit.as_str()]).is_err());

  let canonical_prefix = "a/b; foo=";
  let canonical_at_limit = format!(
    "{canonical_prefix}{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - canonical_prefix.len())
  );
  let parsed = AcceptPatch::parse(&canonical_at_limit).expect("canonical value should parse");
  assert_eq!(parsed.header_value(), canonical_at_limit);
  assert_eq!(parsed.header_value().len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
}

#[test]
fn accept_post_preserves_duplicates_order_and_quoted_parameters() {
  let parsed =
    AcceptPost::parse_values([r#"Text/Plain; title="a,b\"c""#, "text/plain; charset=utf-8"])
      .expect("Accept-Post should parse quoted commas and escapes");

  assert_eq!(parsed.len(), 2);
  assert_eq!(parsed.media_types()[0].type_(), "Text");
  assert_eq!(parsed.media_types()[0].subtype(), "Plain");
  assert_eq!(parsed.media_types()[0].parameters()[0].name(), "title");
  assert_eq!(parsed.media_types()[0].parameters()[0].value(), "a,b\"c");
  assert_eq!(parsed.media_types()[1].type_(), "text");
  assert_eq!(parsed.media_types()[1].subtype(), "plain");
  assert_eq!(
    parsed.header_value(),
    r#"Text/Plain; title="a,b\"c", text/plain; charset=utf-8"#
  );
}

#[test]
fn accept_post_rejects_control_bytes_and_accepts_exact_value_bound() {
  for value in [
    "application/json\0",
    "application/json\r",
    "application/json\n",
    "application/json\u{7f}",
  ] {
    assert!(AcceptPost::parse(value).is_err(), "{value:?} should fail");
  }

  let prefix = "application/";
  let exact = format!(
    "{prefix}{}",
    "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - prefix.len())
  );
  assert_eq!(exact.len(), MAX_ACCEPT_POST_VALUE_BYTES);
  assert!(AcceptPost::parse(&exact).is_ok());
  assert!(AcceptPost::parse(format!("{exact}x")).is_err());
}

#[test]
fn accept_post_bounds_canonical_serialization_at_exact_value_limit() {
  let raw_prefix = "a/b;foo=";
  let raw_at_limit = format!(
    "{raw_prefix}{}",
    "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - raw_prefix.len())
  );
  assert_eq!(raw_at_limit.len(), MAX_ACCEPT_POST_VALUE_BYTES);
  assert!(AcceptPost::parse(&raw_at_limit).is_err());
  assert!(AcceptPost::from_media_types([raw_at_limit.as_str()]).is_err());

  let canonical_prefix = "a/b; foo=";
  let canonical_at_limit = format!(
    "{canonical_prefix}{}",
    "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - canonical_prefix.len())
  );
  let parsed = AcceptPost::parse(&canonical_at_limit).expect("canonical value should parse");
  assert_eq!(parsed.header_value(), canonical_at_limit);
  assert_eq!(parsed.header_value().len(), MAX_ACCEPT_POST_VALUE_BYTES);
}

#[test]
fn accept_post_parse_values_bounds_canonical_size_per_input_field() {
  let field = format!("a/{}", "x".repeat(40_000));
  assert!(field.len() < MAX_ACCEPT_POST_VALUE_BYTES);
  let parsed = AcceptPost::parse_values([field.as_str(), field.as_str()])
    .expect("repeated fields should use a per-field canonical bound");
  assert_eq!(parsed.len(), 2);
  assert!(parsed.header_value().len() > MAX_ACCEPT_POST_VALUE_BYTES);
  assert!(AcceptPost::from_media_types([field.as_str(), field.as_str()]).is_err());

  let raw_prefix = "a/b;foo=";
  let raw_at_limit = format!(
    "{raw_prefix}{}",
    "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - raw_prefix.len())
  );
  assert!(AcceptPost::parse_values([raw_at_limit.as_str(), "text/plain"]).is_err());
}

#[test]
fn accept_patch_normalizes_ows_quoted_escapes_and_round_trips() {
  let parsed = AcceptPatch::parse_values([
    "\tapplication/merge-patch+json\t;\tcharset\t=\tutf-8\t",
    r#" text/plain ; title = "a,b\"c" , application/json "#,
  ])
  .expect("OWS, quoted commas, and escapes should parse across fields");

  assert_eq!(parsed.len(), 3);
  assert_eq!(parsed.media_types()[0].type_(), "application");
  assert_eq!(parsed.media_types()[0].subtype(), "merge-patch+json");
  assert_eq!(parsed.media_types()[0].parameters()[0].value(), "utf-8");
  assert_eq!(parsed.media_types()[1].parameters()[0].value(), "a,b\"c");
  assert_eq!(
    parsed.header_value(),
    r#"application/merge-patch+json; charset=utf-8, text/plain; title="a,b\"c", application/json"#
  );

  let round_trip = AcceptPatch::parse(parsed.header_value()).expect("canonical value should parse");
  assert_eq!(round_trip, parsed);
  assert_eq!(round_trip.header_value(), parsed.header_value());
}

#[test]
fn accept_patch_rejects_malformed_members_and_control_bytes() {
  for value in [
    "",
    "application",
    "/json",
    "application/",
    "application/json; charset",
    "application/json,, text/plain",
    ",application/json",
    "application/json,",
    "application/json\0",
    "application/json\r",
    "application/json\n",
    "application/json\u{7f}",
    "application/json;\x0bcharset=utf-8",
  ] {
    assert!(
      AcceptPatch::parse(value).is_err(),
      "{value:?} should fail Accept-Patch parsing"
    );
  }

  assert!(
    AcceptPatch::parse_values(["application/json", ""]).is_err(),
    "empty repeated fields must be rejected"
  );
  assert!(
    AcceptPatch::parse_values([] as [&str; 0]).is_err(),
    "empty Accept-Patch field sets must be rejected"
  );
}

#[test]
fn accept_patch_enforces_exact_media_type_and_parameter_limits() {
  let at_media_type_limit = std::iter::repeat_n("application/json", MAX_ACCEPT_PATCH_MEDIA_TYPES)
    .collect::<Vec<_>>()
    .join(",");
  let parsed =
    AcceptPatch::parse(&at_media_type_limit).expect("exactly 256 media types should parse");
  assert_eq!(parsed.len(), MAX_ACCEPT_PATCH_MEDIA_TYPES);
  assert!(
    AcceptPatch::parse(
      std::iter::repeat_n("application/json", MAX_ACCEPT_PATCH_MEDIA_TYPES + 1)
        .collect::<Vec<_>>()
        .join(",")
    )
    .is_err(),
    "257 media types must be rejected"
  );
  assert!(
    AcceptPatch::parse_values([at_media_type_limit.as_str(), "text/plain",]).is_err(),
    "media types across fields must stay within the published limit"
  );

  let at_parameter_limit = format!(
    "application/json{}",
    (0..MAX_ACCEPT_PATCH_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let with_parameters =
    AcceptPatch::parse(&at_parameter_limit).expect("exactly 256 parameters should parse");
  assert_eq!(
    with_parameters.media_types()[0].parameters().len(),
    MAX_ACCEPT_PATCH_PARAMETERS
  );

  let too_many_parameters = format!(
    "application/json{}",
    (0..=MAX_ACCEPT_PATCH_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(
    AcceptPatch::parse(&too_many_parameters).is_err(),
    "257 parameters must be rejected"
  );
}

#[test]
fn accept_patch_enforces_total_canonical_serialized_size_including_ows_expansion() {
  let raw_prefix = "a/b;foo=";
  let raw_at_limit = format!(
    "{raw_prefix}{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - raw_prefix.len())
  );
  assert_eq!(raw_at_limit.len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(
    AcceptPatch::parse(&raw_at_limit).is_err(),
    "missing canonical parameter spacing must expand past the limit"
  );
  assert!(AcceptPatch::from_media_types([raw_at_limit.as_str()]).is_err());

  let canonical_prefix = "a/b; foo=";
  let canonical_at_limit = format!(
    "{canonical_prefix}{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - canonical_prefix.len())
  );
  let parsed = AcceptPatch::parse(&canonical_at_limit).expect("canonical value should parse");
  assert_eq!(parsed.header_value(), canonical_at_limit);
  assert_eq!(parsed.header_value().len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(
    AcceptPatch::parse(format!("{canonical_at_limit}x")).is_err(),
    "one byte over the canonical limit must be rejected"
  );

  let half = format!("a/{}", "x".repeat(40_000));
  assert!(half.len() < MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(half.len() * 2 + 2 > MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(
    AcceptPatch::parse_values([half.as_str(), half.as_str()]).is_err(),
    "repeated fields must enforce a cumulative canonical size bound"
  );
  assert!(AcceptPatch::from_media_types([half.as_str(), half.as_str()]).is_err());

  let near_limit_member = format!(
    "a/{}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - "a/".len() - ", text/plain".len())
  );
  let at_cumulative_limit = AcceptPatch::parse_values([near_limit_member.as_str(), "text/plain"])
    .expect("exact cumulative canonical size should parse");
  assert_eq!(
    at_cumulative_limit.header_value().len(),
    MAX_ACCEPT_PATCH_VALUE_BYTES
  );
  assert!(
    AcceptPatch::parse_values([near_limit_member.as_str(), "text/plainx"]).is_err(),
    "one byte over the cumulative canonical bound must be rejected"
  );

  let ows_compact = format!(
    "a/b;foo={}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - "a/b; foo=".len())
  );
  assert!(ows_compact.len() < MAX_ACCEPT_PATCH_VALUE_BYTES);
  let ows_parsed =
    AcceptPatch::parse(&ows_compact).expect("compact form under the limit should parse");
  assert_eq!(
    ows_parsed.header_value().len(),
    MAX_ACCEPT_PATCH_VALUE_BYTES
  );

  let ows_over = format!(
    "a/b;foo={}",
    "x".repeat(MAX_ACCEPT_PATCH_VALUE_BYTES - "a/b; foo=".len() + 1)
  );
  assert!(ows_over.len() <= MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(
    AcceptPatch::parse(&ows_over).is_err(),
    "parameter OWS expansion past the canonical limit must be rejected"
  );

  // Compact commas expand to ", " in the canonical serialization.
  let member_len = (MAX_ACCEPT_PATCH_VALUE_BYTES - 2) / 2;
  let member = format!("a/{}", "x".repeat(member_len - 2));
  assert_eq!(member.len(), member_len);
  let compact_pair = format!("{member},{member}");
  let canonical_pair = format!("{member}, {member}");
  assert_eq!(compact_pair.len(), MAX_ACCEPT_PATCH_VALUE_BYTES - 1);
  assert_eq!(canonical_pair.len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  let comma_parsed =
    AcceptPatch::parse(&compact_pair).expect("comma OWS expansion at the limit should parse");
  assert_eq!(comma_parsed.header_value(), canonical_pair);

  let longer = format!("a/{}", "x".repeat(member_len - 1));
  assert_eq!(longer.len(), member_len + 1);
  let over_compact = format!("{member},{longer}");
  assert_eq!(over_compact.len(), MAX_ACCEPT_PATCH_VALUE_BYTES);
  assert!(
    AcceptPatch::parse(&over_compact).is_err(),
    "comma OWS expansion past the canonical limit must be rejected"
  );
}
