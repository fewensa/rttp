use rttp_protocol::connection::{Connection, MAX_CONNECTION_TOKENS, MAX_CONNECTION_VALUE_BYTES};

#[test]
fn connection_parses_keep_alive_and_te_in_wire_order() {
  let connection = Connection::parse("keep-alive, TE").expect("Connection should parse");

  assert_eq!(connection.tokens(), ["keep-alive", "TE"]);
  assert_eq!(connection.header_value(), "keep-alive, TE");
  assert_eq!(connection.len(), 2);
  assert!(!connection.is_empty());
}

#[test]
fn connection_accepts_http_optional_whitespace_padding() {
  let connection = Connection::parse(" close \t").expect("OWS-padded Connection should parse");
  assert_eq!(connection.tokens(), ["close"]);

  for value in [" keep-alive ,\tTE ", "keep-alive,TE"] {
    let connection = Connection::parse(value).expect("OWS-padded Connection should parse");
    assert_eq!(connection.tokens(), ["keep-alive", "TE"]);
  }
}

#[test]
fn connection_combines_multiple_fields_in_wire_order() {
  let connection =
    Connection::parse_values(["close", "TE"]).expect("multiple Connection fields should parse");

  assert_eq!(connection.tokens(), ["close", "TE"]);
  assert_eq!(connection.header_value(), "close, TE");
}

#[test]
fn connection_preserves_token_spelling_and_contains_is_case_insensitive() {
  let connection = Connection::parse("CLOSE").expect("Connection should parse");

  assert_eq!(connection.tokens(), ["CLOSE"]);
  assert_eq!(connection.header_value(), "CLOSE");
  assert!(connection.contains("close"));
  assert!(connection.contains("CLOSE"));
  assert!(!connection.contains("keep-alive"));
}

#[test]
fn connection_retains_repeated_tokens_in_wire_order() {
  let in_field = Connection::parse("close, close").expect("repeated tokens should parse");
  assert_eq!(in_field.tokens(), ["close", "close"]);
  assert_eq!(in_field.header_value(), "close, close");

  let cross_field = Connection::parse_values(["keep-alive, TE", "keep-alive"])
    .expect("repeated tokens across fields should parse");
  assert_eq!(cross_field.tokens(), ["keep-alive", "TE", "keep-alive"]);
  assert_eq!(cross_field.header_value(), "keep-alive, TE, keep-alive");
}

#[test]
fn connection_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",",
    "close,",
    ",close",
    "close,,TE",
    "clo se",
    "close; foo",
    "close: TE",
    "\u{0d}close",
    "close\r\nX: y",
    "close\u{7f}",
  ] {
    assert!(
      Connection::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn connection_rejects_empty_field_sets() {
  assert!(
    Connection::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn connection_enforces_value_and_token_bounds() {
  assert!(
    Connection::parse("x".repeat(MAX_CONNECTION_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_CONNECTION_VALUE_BYTES);
  assert!(
    Connection::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_CONNECTION_VALUE_BYTES + 1);
  assert!(
    Connection::parse_values(["close", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_CONNECTION_TOKENS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Connection::parse(&at_limit).expect("256 tokens should parse");
  assert_eq!(parsed.len(), MAX_CONNECTION_TOKENS);

  let too_many = (0..=MAX_CONNECTION_TOKENS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Connection::parse(&too_many).is_err(),
    "more than 256 tokens must be rejected"
  );
}
