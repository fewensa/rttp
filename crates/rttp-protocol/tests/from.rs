use rttp_protocol::from::{From, MAX_FROM_VALUE_BYTES};

#[test]
fn from_parses_bare_addr_spec() {
  let mailbox = From::parse("ops+alerts@example.test").expect("bare From mailbox must parse");

  assert_eq!("ops+alerts@example.test", mailbox.header_value());
  assert_eq!("ops+alerts@example.test", mailbox.address());
  assert_eq!("ops+alerts", mailbox.local_part());
  assert_eq!("example.test", mailbox.domain());
  assert_eq!(None, mailbox.display_name());
}

#[test]
fn from_parses_single_name_addr() {
  let mailbox =
    From::parse("Ops Team <ops@example.test>").expect("name-addr From mailbox must parse");

  assert_eq!("Ops Team <ops@example.test>", mailbox.header_value());
  assert_eq!("ops@example.test", mailbox.address());
  assert_eq!("ops", mailbox.local_part());
  assert_eq!("example.test", mailbox.domain());
  assert_eq!(Some("Ops Team"), mailbox.display_name());
}

#[test]
fn from_trims_http_optional_whitespace() {
  let mailbox = From::parse("\tops@example.test ").expect("OWS-padded From mailbox must parse");

  assert_eq!("ops@example.test", mailbox.header_value());
}

#[test]
fn from_normalizes_display_name_whitespace() {
  let mailbox =
    From::parse("Ops\t Team  <ops@example.test>").expect("display-name OWS must normalize");

  assert_eq!("Ops Team <ops@example.test>", mailbox.header_value());
  assert_eq!(Some("Ops Team"), mailbox.display_name());
}

#[test]
fn from_rejects_malformed_mailboxes() {
  for value in [
    "",
    "   ",
    "\t",
    "ops",
    "@example.test",
    "ops@",
    "ops@@example.test",
    ".ops@example.test",
    "ops.@example.test",
    "ops..team@example.test",
    "ops@example..test",
    "ops@example-.test",
    "ops@-example.test",
    "ops@example_test",
    "\"ops\"@example.test",
    "ops@[127.0.0.1]",
    "Ops Team<ops@example.test>",
    "Ops Team < ops@example.test>",
    ". Ops <ops@example.test>",
    ". <ops@example.test>",
    "Ops Team <ops@example.test",
    "Ops Team ops@example.test>",
    "<ops@example.test>",
    "Ops (Team) <ops@example.test>",
    "Ops Team <ops@example.test>\r\nX-Injected: true",
    "=?utf-8?q?Ops?= <ops@example.test>",
    "ops@exämple.test",
  ] {
    assert!(From::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    From::parse_values([]).is_err(),
    "empty field set must be rejected"
  );
}

#[test]
fn from_rejects_ambiguous_mailboxes() {
  for value in [
    "ops@example.test, security@example.test",
    "Ops Team: ops@example.test;",
    "route:<ops@example.test>",
    "Ops, Team <ops@example.test>",
    "Ops Team <ops@example.test>, Other <other@example.test>",
  ] {
    assert!(From::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn from_rejects_duplicate_singleton_fields() {
  assert!(From::parse_values(["ops@example.test", "security@example.test"]).is_err());
  assert!(From::parse_values(["ops@example.test", "ops@example.test"]).is_err());
}

#[test]
fn from_enforces_value_bounds_without_panicking() {
  assert!(From::parse("a".repeat(MAX_FROM_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_FROM_VALUE_BYTES + 1);
  assert!(From::parse_values(["ops@example.test", oversized_duplicate.as_str()]).is_err());
}
