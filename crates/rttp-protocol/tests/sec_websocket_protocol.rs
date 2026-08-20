use rttp_protocol::sec_websocket_protocol::{
  SecWebSocketProtocol, MAX_SEC_WEBSOCKET_PROTOCOL_MEMBERS, MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES,
};

#[test]
fn sec_websocket_protocol_accepts_ordered_offers_and_normalizes_ows() {
  for (value, expected) in [
    ("chat", "chat"),
    (" \tchat\t ", "chat"),
    ("chat, superchat", "chat, superchat"),
    (
      "graphql-ws, graphql-transport-ws",
      "graphql-ws, graphql-transport-ws",
    ),
    (
      " \tchat\t ,\tsuperchat , graphql-ws\t ",
      "chat, superchat, graphql-ws",
    ),
  ] {
    let protocols = SecWebSocketProtocol::parse(value).expect("offer should parse");
    assert_eq!(protocols.header_value(), expected);
  }

  let protocols = SecWebSocketProtocol::parse("chat, superchat").expect("offer should parse");
  assert_eq!(protocols.protocols(), ["chat", "superchat"]);
  assert!(protocols.contains("chat"));
  assert!(protocols.contains("superchat"));
  assert!(!protocols.contains("graphql-ws"));
  assert_eq!(protocols.header_value(), "chat, superchat");
  assert_eq!(
    protocols.selected(),
    None,
    "a multi-token offer is not a selection"
  );
}

#[test]
fn sec_websocket_protocol_combines_fields_in_wire_order() {
  let protocols = SecWebSocketProtocol::parse_values(["chat", "superchat, graphql-ws"])
    .expect("offers should parse");
  assert_eq!(protocols.protocols(), ["chat", "superchat", "graphql-ws"]);
  assert_eq!(protocols.header_value(), "chat, superchat, graphql-ws");
  assert!(protocols.contains("superchat"));
}

#[test]
fn sec_websocket_protocol_from_protocols_validates_declared_tokens() {
  let protocols = SecWebSocketProtocol::from_protocols(["chat", "superchat", "graphql-ws"])
    .expect("declared offers should parse");
  assert_eq!(protocols.protocols(), ["chat", "superchat", "graphql-ws"]);
  assert_eq!(protocols.header_value(), "chat, superchat, graphql-ws");
  assert_eq!(
    SecWebSocketProtocol::parse(protocols.header_value())
      .expect("canonical header must round-trip"),
    protocols
  );
  assert!(
    SecWebSocketProtocol::from_protocols(["chat", "chat"]).is_err(),
    "duplicates must be rejected"
  );
  assert!(
    SecWebSocketProtocol::from_protocols(["not a token"]).is_err(),
    "malformed tokens must be rejected"
  );
  assert!(
    SecWebSocketProtocol::from_protocols(std::iter::empty::<&str>()).is_err(),
    "empty offer sets must be rejected"
  );
}

#[test]
fn sec_websocket_protocol_selection_is_a_singleton() {
  let selection = SecWebSocketProtocol::from_selection("graphql-transport-ws")
    .expect("a single token should select");
  assert_eq!(selection.protocols(), ["graphql-transport-ws"]);
  assert_eq!(selection.selected(), Some("graphql-transport-ws"));
  assert_eq!(selection.header_value(), "graphql-transport-ws");
  assert!(selection.contains("graphql-transport-ws"));

  let parsed = SecWebSocketProtocol::parse_selection(" \tchat\t ").expect("selection should parse");
  assert_eq!(parsed.selected(), Some("chat"));
  assert_eq!(
    selection,
    SecWebSocketProtocol::from_protocols(["graphql-transport-ws"])
      .expect("a one-token offer should equal a selection"),
    "a one-token offer equals a selection of that token"
  );
}

#[test]
fn sec_websocket_protocol_selection_rejects_lists() {
  for value in ["chat, superchat", " \tchat\t ,\tsuperchat ", "chat,"] {
    assert!(
      SecWebSocketProtocol::parse_selection(value).is_err(),
      "{value:?} must be rejected as a selection"
    );
    assert!(
      SecWebSocketProtocol::from_selection(value).is_err(),
      "{value:?} must be rejected as a selection"
    );
  }
  assert!(
    SecWebSocketProtocol::parse_selection_values(["chat", "superchat"]).is_err(),
    "combined selection fields must still be a singleton"
  );
}

#[test]
fn sec_websocket_protocol_rejects_malformed_tokens() {
  for value in [
    "",
    " ",
    "\t",
    ",",
    "chat,",
    ",chat",
    "chat,,superchat",
    "chat;foo",
    "chat/1",
    "chat/",
    "not a token",
    "\"chat\"",
    "chat superchat",
    "chat\u{80}",
  ] {
    assert!(
      SecWebSocketProtocol::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_protocol_rejects_duplicates_and_empty_field_sets() {
  assert!(SecWebSocketProtocol::parse("chat, chat").is_err());
  assert!(SecWebSocketProtocol::parse_values(["chat", "chat"]).is_err());
  assert!(SecWebSocketProtocol::parse_values(["chat, superchat", "superchat"]).is_err());
  assert!(SecWebSocketProtocol::parse_values([]).is_err());
}

#[test]
fn sec_websocket_protocol_preserves_case_sensitive_distinct_tokens() {
  let protocols =
    SecWebSocketProtocol::parse("chat, Chat").expect("case-distinct tokens should be distinct");
  assert_eq!(protocols.protocols(), ["chat", "Chat"]);
  assert_eq!(protocols.header_value(), "chat, Chat");
  assert!(protocols.contains("chat"));
  assert!(protocols.contains("Chat"));
  assert!(!protocols.contains("CHAT"));
}

#[test]
fn sec_websocket_protocol_rejects_injected_obs_text_and_control_bytes() {
  for value in [
    "chat\r\nX-Injected: 1",
    "chat\rX: y",
    "chat\nX: y",
    "chat\0value",
    "chat\u{1}value",
    "chat\u{7f}value",
    "chat\u{80}value",
  ] {
    let error = SecWebSocketProtocol::parse(value).expect_err("injected offers must be rejected");
    let message = error.to_string();
    assert!(
      message.contains("Sec-WebSocket-Protocol"),
      "errors must name the header: {message}"
    );
    assert!(
      !message.contains("X-Injected") && !message.contains("chat"),
      "errors must not echo injected text: {message}"
    );
  }
}

#[test]
fn sec_websocket_protocol_enforces_member_count_bounds() {
  let at_limit = (0..MAX_SEC_WEBSOCKET_PROTOCOL_MEMBERS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>();
  let parsed = SecWebSocketProtocol::parse(at_limit.join(", ")).expect("32 offers should parse");
  assert_eq!(parsed.protocols().len(), MAX_SEC_WEBSOCKET_PROTOCOL_MEMBERS);

  let mut too_many = at_limit;
  too_many.push("overflow".to_string());
  assert!(
    SecWebSocketProtocol::parse(too_many.join(", ")).is_err(),
    "more than 32 protocols must be rejected"
  );
  assert!(
    SecWebSocketProtocol::from_protocols(too_many).is_err(),
    "from_protocols must reject more than 32 protocols"
  );
}

#[test]
fn sec_websocket_protocol_enforces_value_bounds() {
  let oversized = "a".repeat(MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES + 1);
  assert!(
    SecWebSocketProtocol::parse(&oversized).is_err(),
    "a value over the 64 KiB bound should be rejected"
  );
  assert!(
    SecWebSocketProtocol::from_selection(&oversized).is_err(),
    "from_selection must reject oversized tokens"
  );

  let padded = format!("{}chat", " ".repeat(MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES));
  assert!(
    SecWebSocketProtocol::parse(&padded).is_err(),
    "an OWS-padded field over 64 KiB must be rejected"
  );

  let half = " ".repeat(MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES / 2);
  let first = format!("chat{half}");
  let second = format!("{half}superchat");
  assert!(
    first.len() + second.len() > MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES,
    "fixture must exceed the combined bound"
  );
  assert!(
    SecWebSocketProtocol::parse_values([first.as_str(), second.as_str()]).is_err(),
    "combined values over 64 KiB must be rejected"
  );

  let oversized_duplicate = "a".repeat(MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES + 1);
  assert!(
    SecWebSocketProtocol::parse_values(["chat", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let mut huge_from_protocols = vec!["a".repeat(MAX_SEC_WEBSOCKET_PROTOCOL_VALUE_BYTES)];
  huge_from_protocols.push("b".to_string());
  assert!(
    SecWebSocketProtocol::from_protocols(huge_from_protocols).is_err(),
    "from_protocols must reject combined values over 64 KiB"
  );
}
