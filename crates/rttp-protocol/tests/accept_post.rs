use rttp_protocol::accept_post::{AcceptPost, MAX_ACCEPT_POST_MEDIA_TYPES, MAX_ACCEPT_POST_VALUE_BYTES};

#[test]
fn parses_repeated_fields_and_quoted_separators_in_order() {
  let parsed = AcceptPost::parse_values([
    " application/json\t",
    r#"text/plain; title="a,b\"c" ; charset=utf-8"#,
  ])
  .expect("valid repeated Accept-Post fields");

  assert_eq!(parsed.len(), 2);
  assert_eq!(parsed.media_types()[0].subtype(), "json");
  assert_eq!(parsed.media_types()[1].parameters()[0].value(), "a,b\"c");
  assert_eq!(
    parsed.header_value(),
    r##"application/json, text/plain; title="a,b\"c"; charset=utf-8"##
  );
}

#[test]
fn accepts_only_optional_whitespace_at_list_boundaries() {
  for value in [
    "\t application/json \t,\t text/plain\t",
    "application/json;\tcharset =\tutf-8",
  ] {
    assert!(AcceptPost::parse(value).is_ok(), "{value:?} should parse");
  }

  for value in [
    "application/json , , text/plain",
    "application/json,",
    ",application/json",
    "application/json text/plain",
    "application/json; charset",
    "application/json;=utf-8",
    "application/json; charset=",
  ] {
    assert!(AcceptPost::parse(value).is_err(), "{value:?} should fail");
  }
}

#[test]
fn rejects_controls_but_allows_tab_ows() {
  for value in [
    "application/json\0",
    "application/json\r",
    "application/json\n",
    "application/json\u{7f}",
  ] {
    assert!(AcceptPost::parse(value).is_err(), "{value:?} should fail");
  }
  assert!(AcceptPost::parse("\tapplication/json\t").is_ok());
}

#[test]
fn enforces_media_type_count_and_value_byte_limits() {
  let members = std::iter::repeat_n("application/json", MAX_ACCEPT_POST_MEDIA_TYPES)
    .collect::<Vec<_>>();
  assert_eq!(AcceptPost::parse(members.join(",")).unwrap().len(), MAX_ACCEPT_POST_MEDIA_TYPES);
  assert!(AcceptPost::parse(
    std::iter::repeat_n("application/json", MAX_ACCEPT_POST_MEDIA_TYPES + 1)
      .collect::<Vec<_>>()
      .join(",")
  )
  .is_err());

  let exact = format!("a/{}", "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - 2));
  assert_eq!(exact.len(), MAX_ACCEPT_POST_VALUE_BYTES);
  assert!(AcceptPost::parse(&exact).is_ok());
  assert!(AcceptPost::parse(format!("{exact}x")).is_err());
}

#[test]
fn from_media_types_validates_and_canonicalizes_builder_input() {
  let built = AcceptPost::from_media_types([
    "Text/Plain; title=hello",
    "application/json; profile=\"a,b\"",
  ])
  .expect("builder input should parse");
  assert_eq!(
    built.header_value(),
    "Text/Plain; title=hello, application/json; profile=\"a,b\""
  );

  for values in [
    vec![""],
    vec!["application/json\n"],
    vec!["application"],
    vec!["application/json", ""],
  ] {
    assert!(AcceptPost::from_media_types(values).is_err());
  }
}

#[test]
fn canonical_round_trip_is_stable() {
  let original = r#"Text/Plain; title="a,b\"c", application/json; charset=utf-8"#;
  let first = AcceptPost::parse(original).expect("valid Accept-Post");
  let canonical = first.header_value();
  let second = AcceptPost::parse(&canonical).expect("canonical Accept-Post");
  assert_eq!(second, first);
  assert_eq!(second.header_value(), canonical);
}

#[test]
fn builder_enforces_count_and_canonical_byte_bounds() {
  let too_many = std::iter::repeat_n("application/json", MAX_ACCEPT_POST_MEDIA_TYPES + 1);
  assert!(AcceptPost::from_media_types(too_many).is_err());

  let raw = format!("a/b;foo={}", "x".repeat(MAX_ACCEPT_POST_VALUE_BYTES - 8));
  assert!(AcceptPost::from_media_types([raw]).is_err());
}
