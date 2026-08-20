use rttp_protocol::if_header::{
  If, IfCondition, IfList, IfParseError, IfPredicate, IfStateToken, MAX_IF_CONDITIONS,
  MAX_IF_LISTS, MAX_IF_TOTAL_BYTES, MAX_IF_VALUE_BYTES,
};

const OPAQUE_LOCK_TOKEN: &str = "<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>";
const HTTP_LOCK_TOKEN: &str = "<http://example.test/locks/1>";

fn state_token(predicate: &IfPredicate) -> &IfStateToken {
  match predicate {
    IfPredicate::StateToken(token) => token,
    IfPredicate::EntityTag(_) => panic!("expected a state token"),
  }
}

fn entity_tag(predicate: &IfPredicate) -> &rttp_protocol::entity_tag::EntityTag {
  match predicate {
    IfPredicate::EntityTag(tag) => tag,
    IfPredicate::StateToken(_) => panic!("expected an entity tag"),
  }
}

#[test]
fn if_header_parses_untagged_lists_and_preserves_order() {
  let parsed = If::parse(format!(
    "({OPAQUE_LOCK_TOKEN}) ({HTTP_LOCK_TOKEN}) (Not <DAV:no-lock>)"
  ))
  .expect("untagged If should parse");

  assert!(!parsed.is_tagged());
  assert_eq!(3, parsed.lists().len());
  assert_eq!(parsed.lists()[0].resource_tag(), None);
  assert_eq!(1, parsed.lists()[0].conditions().len());
  assert!(!parsed.lists()[0].conditions()[0].is_negated());
  assert!(parsed.lists()[0].conditions()[0]
    .predicate()
    .is_state_token());
  assert_eq!(
    OPAQUE_LOCK_TOKEN,
    state_token(parsed.lists()[0].conditions()[0].predicate()).as_str()
  );
  assert!(parsed.lists()[2].conditions()[0].is_negated());
  assert_eq!(
    "<DAV:no-lock>",
    state_token(parsed.lists()[2].conditions()[0].predicate()).as_str()
  );
  assert_eq!(
    parsed.header_value(),
    format!("({OPAQUE_LOCK_TOKEN}) ({HTTP_LOCK_TOKEN}) (Not <DAV:no-lock>)")
  );
}

#[test]
fn if_header_parses_tagged_lists_with_resource_tags() {
  let parsed = If::parse(format!(
    "<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) </dst> (Not <DAV:no-lock>)"
  ))
  .expect("tagged If should parse");

  assert!(parsed.is_tagged());
  assert_eq!(2, parsed.lists().len());

  let source = parsed.lists()[0]
    .resource_tag()
    .expect("tagged list needs a tag");
  assert_eq!("<http://example.test/src>", source.as_str());
  assert!(parsed.lists()[0].conditions()[0]
    .predicate()
    .is_state_token());

  let destination = parsed.lists()[1]
    .resource_tag()
    .expect("tagged list needs a tag");
  assert_eq!("</dst>", destination.as_str());
  assert!(parsed.lists()[1].conditions()[0].is_negated());
  assert_eq!(
    parsed.header_value(),
    format!("<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) </dst> (Not <DAV:no-lock>)")
  );
}

#[test]
fn if_header_parses_entity_tag_conditions() {
  let parsed = If::parse(r#"(["etag-one"]) (Not [W/"weak-two"])"#)
    .expect("entity-tag conditions should parse");

  assert_eq!(2, parsed.lists().len());
  let strong = parsed.lists()[0].conditions()[0].predicate();
  assert!(strong.is_entity_tag());
  assert!(!parsed.lists()[0].conditions()[0].is_negated());
  assert_eq!("\"etag-one\"", entity_tag(strong).header_value());

  let weak = parsed.lists()[1].conditions()[0].predicate();
  assert!(weak.is_entity_tag());
  assert!(parsed.lists()[1].conditions()[0].is_negated());
  assert_eq!("W/\"weak-two\"", entity_tag(weak).header_value());

  assert_eq!(
    parsed.header_value(),
    r#"(["etag-one"]) (Not [W/"weak-two"])"#
  );
}

#[test]
fn if_header_entity_tag_condition_preserves_quoted_brackets() {
  let parsed = If::parse(r#"(["a]b"])"#).expect("entity tag containing ] should parse");
  assert_eq!(
    "a]b",
    entity_tag(parsed.lists()[0].conditions()[0].predicate()).opaque_tag()
  );
  assert_eq!(parsed.header_value(), r#"(["a]b"])"#);
}

#[test]
fn if_header_repeats_a_resource_tag_across_following_lists() {
  let parsed = If::parse(format!(
    "<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) ({HTTP_LOCK_TOKEN})"
  ))
  .expect("one tag with several lists should parse");

  assert!(parsed.is_tagged());
  assert_eq!(2, parsed.lists().len());
  assert_eq!(
    "<http://example.test/src>",
    parsed.lists()[0]
      .resource_tag()
      .expect("tagged list needs a tag")
      .as_str()
  );
  assert_eq!(
    "<http://example.test/src>",
    parsed.lists()[1]
      .resource_tag()
      .expect("tagged list needs a tag")
      .as_str()
  );
  assert_eq!(
    parsed.header_value(),
    format!(
      "<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) <http://example.test/src> ({HTTP_LOCK_TOKEN})"
    )
  );
}

#[test]
fn if_header_normalizes_ows_in_canonical_output() {
  let parsed = If::parse(format!(
    " \t({OPAQUE_LOCK_TOKEN})  \t( Not  {HTTP_LOCK_TOKEN} )\t "
  ))
  .expect("surrounding OWS should be trimmed");

  assert_eq!(
    parsed.header_value(),
    format!("({OPAQUE_LOCK_TOKEN}) (Not {HTTP_LOCK_TOKEN})")
  );
}

#[test]
fn if_header_rejects_empty_and_unterminated_values() {
  for value in [
    "",
    " ",
    "\t",
    "()",
    "( )",
    "<http://example.test/src>",
    "<http://example.test/src> ()",
    "(",
    "(<a:b>",
    "(<a:b>) (",
    "<http://example.test/src> (",
  ] {
    let error = If::parse(value).expect_err(&format!("{value:?} must be rejected"));
    assert!(
      !error.to_string().contains('('),
      "{value:?} error must not echo input: {error}"
    );
  }
}

#[test]
fn if_header_rejects_mixed_tagged_and_untagged_productions() {
  for value in [
    &format!("({OPAQUE_LOCK_TOKEN}) <http://example.test/src> ({HTTP_LOCK_TOKEN})"),
    "<http://example.test/src> junk",
    "junk",
  ] {
    assert!(If::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn if_header_rejects_invalid_conditions() {
  for value in [
    "(junk)",
    "(<a:b> junk)",
    "(Not)",
    "(NotNot <a:b>)",
    "(not <a:b>)",
    "(NOT <a:b>)",
    "(<a:b>])",
    "([] )",
    "( [unquoted] )",
    "( [\"unterminated )",
    "( [\"a\"x] )",
    "( Not )",
    "( NotNot )",
  ] {
    let error = If::parse(value).expect_err(&format!("{value:?} must be rejected"));
    let message = error.to_string();
    assert!(
      message.contains("If"),
      "{value:?} error must name the header: {message}"
    );
    assert!(
      !message.contains('(') && !message.contains('<'),
      "{value:?} error must not echo input: {message}"
    );
  }
}

#[test]
fn if_header_rejects_invalid_state_tokens() {
  for value in [
    "(</relative>)",
    "(<locks/1>)",
    "(<>)",
    "(< >)",
    "(<a b>)",
    "(<http://example.test/locks/1#fragment>)",
    "(<<opaquelocktoken:550e8400-e29b-41d4-a716-446655440000>>)",
  ] {
    assert!(
      If::parse(value).is_err(),
      "{value:?} must be rejected as a state token"
    );
  }
}

#[test]
fn if_header_rejects_invalid_resource_tags() {
  for value in [
    "<relative> (<a:b>)",
    "<> (<a:b>)",
    "< > (<a:b>)",
    "<//scheme-relative> (<a:b>)",
    "<http://example.test/#fragment> (<a:b>)",
    "<http://example.test/a b> (<a:b>)",
    "<http://example.test/a%zz> (<a:b>)",
    "</a b> (<a:b>)",
  ] {
    assert!(
      If::parse(value).is_err(),
      "{value:?} must be rejected as a resource tag"
    );
  }
}

#[test]
fn if_header_accepts_path_absolute_and_absolute_uri_resource_tags() {
  for value in [
    "</> (<a:b>)",
    "</collection/item> (<a:b>)",
    "</a/b?query=1> (<a:b>)",
    "<http://example.test/collection> (<a:b>)",
    "<urn:uuid:6e7bc004-2445-45a3-8d16-392b33764f00> (<a:b>)",
    "<a:b> (<a:b>)",
  ] {
    assert!(
      If::parse(value).is_ok(),
      "{value:?} should parse as a resource tag"
    );
  }
}

#[test]
fn if_header_rejects_duplicate_fields_after_binding_each() {
  assert!(If::parse_values([OPAQUE_LOCK_TOKEN, HTTP_LOCK_TOKEN]).is_err());
  assert!(If::parse_values([OPAQUE_LOCK_TOKEN, OPAQUE_LOCK_TOKEN]).is_err());
  assert!(If::parse_values([]).is_err());

  let oversized = "x".repeat(MAX_IF_VALUE_BYTES + 1);
  assert!(
    If::parse_values([OPAQUE_LOCK_TOKEN, oversized.as_str()]).is_err(),
    "an oversized duplicate field must not bypass validation"
  );
  assert!(
    If::parse_values([oversized.as_str(), OPAQUE_LOCK_TOKEN]).is_err(),
    "an oversized first field must not bypass validation"
  );
}

#[test]
fn if_header_enforces_value_and_total_bounds() {
  let at_bound = format!("(<a:{}>)", "b".repeat(MAX_IF_TOTAL_BYTES - 6));
  assert!(
    If::parse(&at_bound).is_ok(),
    "a value at the 64 KiB total bound should parse"
  );

  let oversized = "x".repeat(MAX_IF_VALUE_BYTES + 1);
  assert!(If::parse(oversized).is_err());

  let first = format!("(<a:{}>)", "b".repeat(MAX_IF_VALUE_BYTES - 6));
  let second = format!("(<a:{}>)", "c".repeat(MAX_IF_VALUE_BYTES - 6));
  let error = If::parse_values([first.as_str(), second.as_str()])
    .expect_err("two values each under the per-field bound must not exceed the total");
  assert!(error.to_string().contains("too large"));
}

#[test]
fn if_header_enforces_list_and_condition_caps() {
  let list = "(<a:b>)";
  let at_list_cap = std::iter::repeat_n(list, MAX_IF_LISTS)
    .collect::<Vec<_>>()
    .join(" ");
  assert!(
    If::parse(&at_list_cap).is_ok(),
    "{MAX_IF_LISTS} lists should parse"
  );
  let over_list_cap = format!("{at_list_cap} ({HTTP_LOCK_TOKEN})");
  let error = If::parse(&over_list_cap).expect_err("33 lists must be rejected");
  assert!(error.to_string().contains("too many If lists"));

  let conditions = " <a:b>".repeat(MAX_IF_CONDITIONS);
  let at_condition_cap = format!("({conditions})");
  assert!(
    If::parse(&at_condition_cap).is_ok(),
    "{MAX_IF_CONDITIONS} conditions should parse"
  );
  let over_condition_cap = format!("({conditions} <a:b>)");
  let error = If::parse(&over_condition_cap).expect_err("257 conditions must be rejected");
  assert!(error.to_string().contains("too many If conditions"));
}

#[test]
fn if_header_debug_redacts_state_tokens_but_keeps_structure() {
  let parsed = If::parse(format!(
    "<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) (Not [\"etag-one\"])"
  ))
  .expect("If should parse");

  let debug = format!("{parsed:?}");
  assert!(debug.contains("[REDACTED]"));
  assert!(debug.contains("<http://example.test/src>"));
  assert!(debug.contains("\"etag-one\""));
  assert!(!debug.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!debug.contains("opaquelocktoken"));

  let list_debug = format!("{:?}", parsed.lists()[0]);
  assert!(list_debug.contains("[REDACTED]"));
  assert!(!list_debug.contains("550e8400-e29b-41d4-a716-446655440000"));

  let condition_debug = format!("{:?}", parsed.lists()[1].conditions()[0]);
  assert!(condition_debug.contains("negated: true"));
  assert!(condition_debug.contains("\"etag-one\""));

  let token = state_token(parsed.lists()[0].conditions()[0].predicate());
  let token_debug = format!("{token:?}");
  assert!(token_debug.contains("[REDACTED]"));
  assert!(!token_debug.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!token_debug.contains("opaquelocktoken"));
}

#[test]
fn if_header_errors_never_expose_value_material() {
  let error = If::parse(format!(
    "<http://example.test/src> ({OPAQUE_LOCK_TOKEN}) junk"
  ))
  .expect_err("junk after a list should be rejected");
  let message = error.to_string();
  assert!(message.contains("If"));
  assert!(!message.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!message.contains("opaquelocktoken"));
  assert!(!message.contains("example.test"));
  assert!(!message.contains("junk"));

  let duplicate = If::parse_values([OPAQUE_LOCK_TOKEN, HTTP_LOCK_TOKEN])
    .expect_err("duplicate fields should be rejected");
  let duplicate_message = duplicate.to_string();
  assert!(duplicate_message.contains("duplicate"));
  assert!(duplicate_message.contains("If"));
  assert!(!duplicate_message.contains("550e8400-e29b-41d4-a716-446655440000"));
  assert!(!duplicate_message.contains("example.test"));

  let oversized =
    If::parse("x".repeat(MAX_IF_VALUE_BYTES + 1)).expect_err("oversized value should be rejected");
  assert!(oversized.to_string().contains("too large"));

  let _: IfParseError = If::parse("").expect_err("empty If should be rejected");
}

#[test]
fn if_header_typed_accessors_share_the_protocol_types() {
  let parsed = If::parse(format!("({OPAQUE_LOCK_TOKEN})")).expect("If should parse");
  let list: &IfList = &parsed.lists()[0];
  let condition: &IfCondition = &list.conditions()[0];
  let predicate: &IfPredicate = condition.predicate();
  let token: &IfStateToken = state_token(predicate);
  assert_eq!(OPAQUE_LOCK_TOKEN, token.as_str());
  assert!(parsed.lists()[0].resource_tag().is_none());

  let tagged = If::parse("</dst> (<a:b>)").expect("tagged If should parse");
  let tag = tagged.lists()[0]
    .resource_tag()
    .expect("tagged list needs a tag");
  assert_eq!("</dst>", tag.as_str());
  let entity_tag = rttp_protocol::entity_tag::EntityTag::strong("etag-one");
  assert!(IfPredicate::EntityTag(entity_tag).is_entity_tag());
}
