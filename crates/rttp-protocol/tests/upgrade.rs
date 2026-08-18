use rttp_protocol::upgrade::{Upgrade, MAX_UPGRADE_PROTOCOLS, MAX_UPGRADE_VALUE_BYTES};

#[test]
fn upgrade_preserves_protocol_spelling_and_order() {
  let upgrade = Upgrade::parse("websocket, h2c").expect("Upgrade should parse");

  assert_eq!(upgrade.protocols(), ["websocket", "h2c"]);
  assert_eq!(upgrade.header_value(), "websocket, h2c");
  assert_eq!(upgrade.len(), 2);
}

#[test]
fn upgrade_accepts_multiple_fields_in_wire_order() {
  let upgrade =
    Upgrade::parse_values(["websocket", "h2c, custom"]).expect("Upgrade fields should parse");

  assert_eq!(upgrade.protocols(), ["websocket", "h2c", "custom"]);
  assert_eq!(upgrade.header_value(), "websocket, h2c, custom");
}

#[test]
fn upgrade_accepts_versioned_protocol_identifiers() {
  let upgrade =
    Upgrade::parse("HTTP/2.0, TLS/1.3").expect("versioned Upgrade protocols should parse");

  assert_eq!(upgrade.protocols(), ["HTTP/2.0", "TLS/1.3"]);
  assert_eq!(upgrade.header_value(), "HTTP/2.0, TLS/1.3");
}

#[test]
fn upgrade_accepts_http_optional_whitespace_padding() {
  for value in ["\twebsocket\t", " websocket "] {
    let upgrade = Upgrade::parse(value).expect("OWS-padded Upgrade should parse");
    assert_eq!(upgrade.protocols(), ["websocket"]);
  }

  for value in [" websocket ,\th2c ", "websocket,h2c"] {
    let upgrade = Upgrade::parse(value).expect("OWS-padded Upgrade list should parse");
    assert_eq!(upgrade.protocols(), ["websocket", "h2c"]);
  }
}

#[test]
fn upgrade_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",websocket",
    "websocket,",
    "websocket,,h2c",
    "web socket",
    "websocket; q=1",
    "websocket: h2c",
    "websocket/",
    "/2.0",
    "HTTP/2.0/extra",
    "\u{0d}websocket",
    "websocket\r\nX: y",
    "websocket\u{7f}",
  ] {
    assert!(Upgrade::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn upgrade_rejects_empty_field_sets() {
  assert!(
    Upgrade::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn upgrade_enforces_value_and_protocol_bounds() {
  assert!(
    Upgrade::parse("x".repeat(MAX_UPGRADE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_UPGRADE_VALUE_BYTES);
  assert!(
    Upgrade::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_UPGRADE_VALUE_BYTES + 1);
  assert!(
    Upgrade::parse_values(["websocket", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_UPGRADE_PROTOCOLS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Upgrade::parse(&at_limit).expect("32 protocols should parse");
  assert_eq!(parsed.len(), MAX_UPGRADE_PROTOCOLS);

  let too_many = (0..=MAX_UPGRADE_PROTOCOLS)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Upgrade::parse(&too_many).is_err(),
    "more than 32 protocols must be rejected"
  );
}
