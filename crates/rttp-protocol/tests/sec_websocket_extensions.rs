use rttp_protocol::sec_websocket_extensions::{
  SecWebSocketExtensionParameterValue, SecWebSocketExtensions,
  MAX_SEC_WEBSOCKET_EXTENSIONS_MEMBERS, MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES,
};

#[test]
fn sec_websocket_extensions_accepts_ordered_offers_and_parameters() {
  let extensions = SecWebSocketExtensions::parse(
    r#"permessage-deflate; client_max_window_bits; server_max_window_bits=15, x-test; quoted="a,b;c"; token=value"#,
  )
  .expect("extensions should parse");

  assert_eq!(
    extensions.header_value(),
    r#"permessage-deflate; client_max_window_bits; server_max_window_bits=15, x-test; quoted="a,b;c"; token=value"#
  );
  assert_eq!(extensions.extensions().len(), 2);
  assert!(extensions.contains("permessage-deflate"));
  assert_eq!(extensions.selected(), None);

  let first = &extensions.extensions()[0];
  assert_eq!(first.token(), "permessage-deflate");
  assert_eq!(first.parameters()[0].name(), "client_max_window_bits");
  assert_eq!(first.parameters()[0].value(), None);
  assert_eq!(
    first.parameters()[1].value(),
    Some(&SecWebSocketExtensionParameterValue::Token(
      "15".to_string()
    ))
  );

  let second = &extensions.extensions()[1];
  assert_eq!(second.token(), "x-test");
  let quoted = second.parameter("quoted").expect("quoted parameter");
  assert_eq!(quoted.value().expect("quoted value").as_str(), "a,b;c");
  assert!(quoted.value().expect("quoted value").is_quoted());
}

#[test]
fn sec_websocket_extensions_normalizes_ows_and_quoted_pairs() {
  let extensions = SecWebSocketExtensions::parse(
    " \tpermessage-deflate\t ;\tclient_max_window_bits = 15 ; token = \"a\\\"b\\\\c\"\t ",
  )
  .expect("extensions should parse");

  assert_eq!(
    extensions.header_value(),
    r#"permessage-deflate; client_max_window_bits=15; token="a\"b\\c""#
  );
  let token = extensions.extensions()[0]
    .parameter("token")
    .expect("token parameter");
  assert_eq!(token.value().expect("token value").as_str(), "a\"b\\c");
}

#[test]
fn sec_websocket_extensions_combines_fields_in_wire_order() {
  let extensions = SecWebSocketExtensions::parse_values([
    "permessage-deflate; client_no_context_takeover",
    r#"x-test; mode="safe""#,
  ])
  .expect("combined extensions should parse");

  assert_eq!(
    extensions.header_value(),
    r#"permessage-deflate; client_no_context_takeover, x-test; mode="safe""#
  );
  assert_eq!(extensions.extensions()[1].token(), "x-test");
}

#[test]
fn sec_websocket_extensions_selection_is_a_singleton_extension() {
  let selection = SecWebSocketExtensions::parse_selection(
    "permessage-deflate; client_no_context_takeover; server_max_window_bits=15",
  )
  .expect("selection should parse");

  let selected = selection.selected().expect("selected extension");
  assert_eq!(selected.token(), "permessage-deflate");
  assert_eq!(selected.parameters().len(), 2);
  assert_eq!(
    selection.header_value(),
    "permessage-deflate; client_no_context_takeover; server_max_window_bits=15"
  );
}

#[test]
fn sec_websocket_extensions_selection_rejects_multiple_extensions() {
  assert!(
    SecWebSocketExtensions::parse_selection("permessage-deflate, x-test").is_err(),
    "selection must contain exactly one extension"
  );
  assert!(
    SecWebSocketExtensions::parse_selection_values(["permessage-deflate", "x-test"]).is_err(),
    "combined selection fields must still be a singleton"
  );
}

#[test]
fn sec_websocket_extensions_rejects_malformed_forms() {
  for value in [
    "",
    " ",
    ",",
    "permessage deflate",
    "permessage-deflate;",
    "permessage-deflate; =15",
    "permessage-deflate; bad param",
    "permessage-deflate; p=",
    "permessage-deflate; p=\"unterminated",
    "permessage-deflate; p=\"bad\"tail",
    "permessage-deflate; p=bad/value",
    "permessage-deflate\r\nX-Injected: 1",
    "permessage-deflate\0",
    "permessage-deflate\u{7f}",
    "permessage-deflate\u{80}",
  ] {
    assert!(
      SecWebSocketExtensions::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_extensions_rejects_duplicate_extensions_and_parameters() {
  assert!(SecWebSocketExtensions::parse("permessage-deflate, permessage-deflate").is_err());
  assert!(SecWebSocketExtensions::parse_values(["x-test", "x-test; p=1"]).is_err());
  assert!(SecWebSocketExtensions::parse("x-test; p=1; p=2").is_err());
}

#[test]
fn sec_websocket_extensions_preserves_case_sensitive_distinct_tokens() {
  let extensions =
    SecWebSocketExtensions::parse("x-test, X-Test; P=1; p=2").expect("case-distinct names parse");

  assert_eq!(extensions.extensions()[0].token(), "x-test");
  assert_eq!(extensions.extensions()[1].token(), "X-Test");
  assert!(extensions.extensions()[1].parameter("P").is_some());
  assert!(extensions.extensions()[1].parameter("p").is_some());
}

#[test]
fn sec_websocket_extensions_rejects_member_and_size_bounds() {
  let too_many = (0..=MAX_SEC_WEBSOCKET_EXTENSIONS_MEMBERS)
    .map(|index| format!("x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(SecWebSocketExtensions::parse(too_many).is_err());

  let oversized = "a".repeat(MAX_SEC_WEBSOCKET_EXTENSIONS_VALUE_BYTES + 1);
  assert!(SecWebSocketExtensions::parse(oversized).is_err());
}
