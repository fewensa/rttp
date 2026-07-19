use rttp_protocol::vary::{Vary, MAX_VARY_FIELD_NAMES};

#[test]
fn vary_parses_wildcard() {
  let vary = Vary::parse("*").expect("valid wildcard Vary");

  assert!(vary.is_any());
  assert_eq!(Vec::<&str>::new(), vary.field_names());
  assert_eq!("*", vary.header_value());
}

#[test]
fn vary_deduplicates_wildcards_across_values() {
  let vary = Vary::parse_values(["*", "*"]).expect("duplicate wildcard Vary");

  assert!(vary.is_any());
  assert_eq!(Vec::<&str>::new(), vary.field_names());
  assert_eq!("*", vary.header_value());
}

#[test]
fn vary_deduplicates_wildcards_in_a_comma_list() {
  let vary = Vary::parse("*, *").expect("duplicate wildcard Vary");

  assert!(vary.is_any());
  assert_eq!(Vec::<&str>::new(), vary.field_names());
  assert_eq!("*", vary.header_value());
}

#[test]
fn vary_normalizes_singleton_field_names_case_insensitively() {
  let vary = Vary::parse("Accept-Language").expect("valid Vary field name");

  assert!(!vary.is_any());
  assert_eq!(vec!["accept-language"], vary.field_names());
  assert_eq!("accept-language", vary.header_value());
}

#[test]
fn vary_parses_comma_lists_and_optional_whitespace() {
  let vary = Vary::parse_values([" Accept-Encoding , Accept-Language ", "User-Agent"])
    .expect("valid Vary field names");

  assert_eq!(
    vec!["accept-encoding", "accept-language", "user-agent"],
    vary.field_names()
  );
  assert_eq!(
    "accept-encoding, accept-language, user-agent",
    vary.header_value()
  );
}

#[test]
fn vary_rejects_empty_members() {
  for value in [
    "",
    "Accept-Encoding,",
    ",Accept-Encoding",
    "Accept-Encoding,,User-Agent",
  ] {
    assert!(Vary::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn vary_rejects_control_bytes() {
  assert!(Vary::parse("\rAccept-Encoding").is_err());
}

#[test]
fn vary_deduplicates_field_names_case_insensitively() {
  let vary = Vary::parse("Accept-Encoding, accept-encoding, ACCEPT-ENCODING")
    .expect("duplicate Vary field names are valid");

  assert_eq!(vec!["accept-encoding"], vary.field_names());
  assert_eq!("accept-encoding", vary.header_value());
}

#[test]
fn vary_rejects_field_name_list_overflow() {
  let too_many = std::iter::repeat_n("Accept-Encoding", MAX_VARY_FIELD_NAMES + 1)
    .collect::<Vec<_>>()
    .join(",");

  assert!(Vary::parse(too_many).is_err());
}
