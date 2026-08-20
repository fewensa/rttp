use rttp_protocol::sec_websocket_version::{
  SecWebSocketVersion, MAX_SEC_WEBSOCKET_VERSION_MEMBERS, MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES,
};

#[test]
fn sec_websocket_version_accepts_version_13_and_normalizes_ows() {
  for (value, expected) in [
    ("13", "13"),
    ("0", "0"),
    ("8", "8"),
    ("299", "299"),
    (" \t13\t ", "13"),
    ("13, 8, 7", "13, 8, 7"),
    (" \t13\t ,\t8 , 7\t ", "13, 8, 7"),
  ] {
    let versions = SecWebSocketVersion::parse(value).expect("version should parse");
    assert_eq!(versions.header_value(), expected);
    assert!(versions.contains("13") == expected.contains("13"));
  }

  let versions = SecWebSocketVersion::parse("13").expect("version 13 should parse");
  assert_eq!(versions.versions(), ["13"]);
  assert!(versions.contains("13"));
  assert!(!versions.contains("12"));
  assert_eq!(versions.header_value(), "13");
}

#[test]
fn sec_websocket_version_combines_fields_in_wire_order() {
  let versions = SecWebSocketVersion::parse_values(["13", "8, 7"])
    .expect("combined Sec-WebSocket-Version fields should parse");
  assert_eq!(versions.versions(), ["13", "8", "7"]);
  assert_eq!(versions.header_value(), "13, 8, 7");
  assert!(versions.contains("8"));
}

#[test]
fn sec_websocket_version_from_versions_validates_declared_tokens() {
  let versions =
    SecWebSocketVersion::from_versions(["13", "8", "7"]).expect("declared versions should parse");
  assert_eq!(versions.versions(), ["13", "8", "7"]);
  assert_eq!(versions.header_value(), "13, 8, 7");
  assert_eq!(
    SecWebSocketVersion::parse(versions.header_value()).expect("canonical header must round-trip"),
    versions
  );
  assert!(
    SecWebSocketVersion::from_versions(["8", "13"]).is_err(),
    "ascending order must be rejected"
  );
  assert!(
    SecWebSocketVersion::from_versions(["13", "13"]).is_err(),
    "duplicates must be rejected"
  );
  assert!(
    SecWebSocketVersion::from_versions(["013"]).is_err(),
    "leading-zero tokens must be rejected"
  );
  assert!(
    SecWebSocketVersion::from_versions(std::iter::empty::<&str>()).is_err(),
    "empty version sets must be rejected"
  );
}

#[test]
fn sec_websocket_version_rejects_malformed_tokens() {
  for value in [
    "", " ", "\t", ",", "13,", ",13", "13,,8", "13 8", "13;8", "v13", "13.0", "13a", "01", "08",
    "00", "013", "300", "2560", "1000",
  ] {
    assert!(
      SecWebSocketVersion::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_version_rejects_duplicates_and_empty_field_sets() {
  assert!(SecWebSocketVersion::parse("13, 13").is_err());
  assert!(SecWebSocketVersion::parse_values(["13", "13"]).is_err());
  assert!(SecWebSocketVersion::parse_values(["13, 8", "8"]).is_err());
  assert!(SecWebSocketVersion::parse_values([]).is_err());
}

#[test]
fn sec_websocket_version_rejects_non_descending_order() {
  for value in ["8, 13", "13, 7, 8", "13, 13, 8"] {
    let error = SecWebSocketVersion::parse(value).expect_err("unordered versions must be rejected");
    let message = error.to_string();
    assert!(
      message.contains("order") || message.contains("duplicate") || message.contains("invalid"),
      "{message}"
    );
  }
  assert!(SecWebSocketVersion::parse_values(["8", "13"]).is_err());
}

#[test]
fn sec_websocket_version_rejects_injected_obs_text_and_control_bytes() {
  for value in [
    "13\r\nX-Injected: 1",
    "13\rX: y",
    "13\nX: y",
    "13\0value",
    "13\u{1}value",
    "13\u{7f}value",
    "13\u{80}value",
  ] {
    assert!(
      SecWebSocketVersion::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn sec_websocket_version_enforces_member_count_bounds() {
  let at_limit = (0..MAX_SEC_WEBSOCKET_VERSION_MEMBERS)
    .rev()
    .map(|index| (200 + index).to_string())
    .collect::<Vec<_>>();
  let parsed =
    SecWebSocketVersion::parse(at_limit.join(", ")).expect("32 descending versions should parse");
  assert_eq!(parsed.versions().len(), MAX_SEC_WEBSOCKET_VERSION_MEMBERS);

  let mut too_many = at_limit;
  too_many.push("167".to_string());
  assert!(
    SecWebSocketVersion::parse(too_many.join(", ")).is_err(),
    "more than 32 versions must be rejected"
  );
  assert!(
    SecWebSocketVersion::from_versions(too_many).is_err(),
    "from_versions must reject more than 32 versions"
  );
}

#[test]
fn sec_websocket_version_enforces_value_bounds() {
  let oversized = "1".repeat(MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES + 1);
  assert!(
    SecWebSocketVersion::parse(&oversized).is_err(),
    "a value over the 64 KiB bound should be rejected"
  );

  let padded = format!("{}13", " ".repeat(MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES));
  assert!(
    SecWebSocketVersion::parse(&padded).is_err(),
    "an OWS-padded field over 64 KiB must be rejected"
  );

  let half = " ".repeat(MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES / 2);
  let first = format!("13{half}");
  let second = format!("{half}8");
  assert!(
    first.len() + second.len() > MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES,
    "fixture must exceed the combined bound"
  );
  assert!(
    SecWebSocketVersion::parse_values([first.as_str(), second.as_str()]).is_err(),
    "combined values over 64 KiB must be rejected"
  );

  let oversized_duplicate = "1".repeat(MAX_SEC_WEBSOCKET_VERSION_VALUE_BYTES + 1);
  assert!(
    SecWebSocketVersion::parse_values(["13", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );
}
