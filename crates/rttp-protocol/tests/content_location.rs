use rttp_protocol::content_location::{ContentLocation, MAX_CONTENT_LOCATION_VALUE_BYTES};

#[test]
fn content_location_accepts_absolute_and_relative_uri_references() {
  for (value, expected) in [
    (
      "https://example.test/representations/current.json",
      "https://example.test/representations/current.json",
    ),
    (
      "/representations/current.json",
      "/representations/current.json",
    ),
    (
      "../current?variant=full#metadata",
      "../current?variant=full#metadata",
    ),
    (
      "\t../representations/current.json\t",
      "../representations/current.json",
    ),
    ("//example.test/current", "//example.test/current"),
    (
      "//[2001:db8::1]/representation",
      "//[2001:db8::1]/representation",
    ),
  ] {
    let content_location = ContentLocation::parse(value).expect("Content-Location should parse");

    assert_eq!(expected, content_location.as_str());
    assert_eq!(expected, content_location.header_value());
    assert_eq!(expected, content_location.as_ref());
  }
}

#[test]
fn content_location_parse_values_enforces_singleton_fields() {
  let content_location = ContentLocation::parse_values([" /representations/current.json "])
    .expect("single field should parse");

  assert_eq!("/representations/current.json", content_location.as_str());
  assert!(
    ContentLocation::parse_values(["/one", "/two"]).is_err(),
    "duplicate fields must be rejected"
  );
  assert!(
    ContentLocation::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn content_location_rejects_invalid_uri_reference_values() {
  for value in [
    "",
    " ",
    "/safe\u{7f}",
    "/safe\u{1f}",
    "http://[::1",
    "not valid",
    "/bad path",
    "/bad%zz",
    "/bad<path>",
    "/bad\\path",
    "/bad\"path",
    "/bad#one#two",
  ] {
    assert!(
      ContentLocation::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_location_enforces_value_bounds() {
  let at_limit = format!("/{}", "a".repeat(MAX_CONTENT_LOCATION_VALUE_BYTES - 1));
  let parsed = ContentLocation::parse(&at_limit).expect("value at limit should parse");
  assert_eq!(at_limit, parsed.as_str());

  assert!(
    ContentLocation::parse(format!("/{}", "a".repeat(MAX_CONTENT_LOCATION_VALUE_BYTES))).is_err(),
    "oversized values must be rejected"
  );

  let oversized_duplicate = format!("/{}", "a".repeat(MAX_CONTENT_LOCATION_VALUE_BYTES));
  assert!(
    ContentLocation::parse_values(["/valid", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
