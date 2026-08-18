use rttp_protocol::allow::{Allow, MAX_ALLOW_METHODS, MAX_ALLOW_VALUE_BYTES};

#[test]
fn parses_single_allow_field() {
  let allow = Allow::parse("GET, HEAD, POST").expect("valid Allow");

  assert_eq!(allow.methods(), ["GET", "HEAD", "POST"]);
  assert!(allow.contains_method("HEAD"));
  assert!(!allow.contains_method("PATCH"));
  assert_eq!(allow.header_value(), "GET, HEAD, POST");
}

#[test]
fn combines_multiple_allow_fields_in_order() {
  let allow = Allow::parse_values(["GET, HEAD", "POST", "PATCH"]).expect("multiple Allow fields");

  assert_eq!(allow.methods(), ["GET", "HEAD", "POST", "PATCH"]);
  assert_eq!(allow.header_value(), "GET, HEAD, POST, PATCH");
}

#[test]
fn trims_optional_whitespace_only() {
  let allow = Allow::parse("GET,\tHEAD, POST").expect("optional whitespace");

  assert_eq!(allow.methods(), ["GET", "HEAD", "POST"]);
}

#[test]
fn preserves_extension_method_order_and_case() {
  let allow = Allow::parse("GET, com.example.sync, PATCH").expect("extension methods");

  assert_eq!(allow.methods(), ["GET", "com.example.sync", "PATCH"]);
  assert!(allow.contains_method("com.example.sync"));
  assert!(!allow.contains_method("COM.EXAMPLE.SYNC"));
}

#[test]
fn rejects_malformed_allow_values() {
  for value in ["", "GET,,POST", "GET POST", "GET\r", "GET, POST\n"] {
    assert!(
      Allow::parse(value).is_err(),
      "Allow parser should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_methods_in_one_or_multiple_fields() {
  assert!(Allow::parse("GET, HEAD, GET").is_err());
  assert!(Allow::parse_values(["GET, HEAD", "POST, GET"]).is_err());
}

#[test]
fn rejects_oversized_field_value() {
  assert!(Allow::parse("x".repeat(MAX_ALLOW_VALUE_BYTES + 1)).is_err());
}

#[test]
fn enforces_method_count_bound() {
  let at_limit = (0..MAX_ALLOW_METHODS)
    .map(|index| format!("M{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Allow::parse(&at_limit).is_ok());

  let too_many = (0..=MAX_ALLOW_METHODS)
    .map(|index| format!("M{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Allow::parse(&too_many).is_err());
}

#[test]
fn validates_from_methods_with_same_parser() {
  let allow = Allow::from_methods(["GET", "HEAD"]).expect("valid method iterator");

  assert_eq!(allow.methods(), ["GET", "HEAD"]);
  assert_eq!(allow.header_value(), "GET, HEAD");
  assert!(Allow::from_methods(["GET", "GET"]).is_err());
}
