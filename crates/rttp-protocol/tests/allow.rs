use rttp_protocol::allow::{Allow, MAX_ALLOW_METHODS, MAX_ALLOW_VALUE_BYTES};

#[test]
fn parses_multi_field_allow_values_in_order() {
  let allow =
    Allow::parse_values(["GET, HEAD", "POST,\tOPTIONS"]).expect("valid Allow fields should parse");

  assert_eq!(vec!["GET", "HEAD", "POST", "OPTIONS"], allow.methods());
  assert_eq!("GET, HEAD, POST, OPTIONS", allow.header_value());
  assert!(allow.contains_method("POST"));
  assert!(!allow.contains_method("PATCH"));
}

#[test]
fn preserves_exact_method_tokens_without_policy() {
  let allow = Allow::parse("get, REPORT, X-Custom").expect("valid tokens should parse");

  assert_eq!(vec!["get", "REPORT", "X-Custom"], allow.methods());
  assert!(allow.contains_method("get"));
  assert!(!allow.contains_method("GET"));
}

#[test]
fn rejects_malformed_members_and_control_bytes() {
  for value in [
    "",
    "GET,",
    ",GET",
    "GET,,POST",
    "GET, ,POST",
    "GET POST",
    "GET@POST",
    "GE\tT",
    "GET\r",
    "GET\u{7f}",
  ] {
    assert!(
      Allow::parse(value).is_err(),
      "Allow parser should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_tokens_across_fields() {
  assert!(
    Allow::parse_values(["GET, HEAD", "POST, GET"]).is_err(),
    "duplicate Allow tokens across fields should be rejected"
  );
}

#[test]
fn enforces_exact_method_bound() {
  let max_methods = (0..MAX_ALLOW_METHODS)
    .map(|index| format!("M{index}"))
    .collect::<Vec<_>>();
  let allow =
    Allow::from_methods(max_methods.iter().map(String::as_str)).expect("max methods should parse");

  assert_eq!(MAX_ALLOW_METHODS, allow.methods().len());

  let too_many = (0..=MAX_ALLOW_METHODS)
    .map(|index| format!("M{index}"))
    .collect::<Vec<_>>();

  assert!(
    Allow::from_methods(too_many.iter().map(String::as_str)).is_err(),
    "more than the method bound should be rejected"
  );
}

#[test]
fn enforces_per_field_value_bound() {
  let exact = format!("M{}", "A".repeat(MAX_ALLOW_VALUE_BYTES - 1));
  let allow = Allow::parse(&exact).expect("exact value byte bound should parse");

  assert_eq!(vec![exact.as_str()], allow.methods());

  let oversized = format!("M{}", "A".repeat(MAX_ALLOW_VALUE_BYTES));
  assert!(
    Allow::parse(&oversized).is_err(),
    "oversized field values should be rejected"
  );
}
