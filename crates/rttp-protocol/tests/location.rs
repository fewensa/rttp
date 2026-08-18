use rttp_protocol::location::{Location, MAX_LOCATION_VALUE_BYTES};

#[test]
fn location_preserves_absolute_and_relative_references() {
  let absolute =
    Location::parse("https://shop.example/orders/123").expect("absolute Location must parse");
  let absolute_path = Location::parse("/orders/123").expect("absolute path Location must parse");
  let relative = Location::parse("../orders/123").expect("relative Location must parse");
  let query_only = Location::parse("?next=1").expect("query-only Location must parse");
  let scheme_relative =
    Location::parse("//cdn.example/new").expect("scheme-relative Location must parse");
  let fragment = Location::parse("/orders/123#receipt").expect("fragment Location must parse");

  assert_eq!("https://shop.example/orders/123", absolute.as_str());
  assert_eq!("/orders/123", absolute_path.header_value());
  assert_eq!("../orders/123", relative.as_str());
  assert_eq!("?next=1", query_only.as_str());
  assert_eq!("//cdn.example/new", scheme_relative.as_str());
  assert_eq!("/orders/123#receipt", fragment.as_str());
}

#[test]
fn location_trims_http_optional_whitespace() {
  let location = Location::parse("\t/orders/123\t").expect("OWS-padded Location must parse");

  assert_eq!("/orders/123", location.as_str());
}

#[test]
fn location_rejects_controls_and_malformed_values() {
  for value in [
    "",
    "   ",
    "\t",
    "https://shop.example/checkout\r\nX-Injected: true",
    "https://example.test/path with space",
    "https://example.test/foo\\bar",
    "https://example.test/a<b>",
    "https://exämple.test/",
    "https://example.test/%ZZ",
    "https://example.test/%2",
    "https://example.test/%",
    "https://",
  ] {
    assert!(
      Location::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    Location::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn location_rejects_duplicate_singleton_fields() {
  assert!(Location::parse_values(["https://example.test/a", "https://example.test/b"]).is_err());
  assert!(Location::parse_values(["/same", "/same"]).is_err());
}

#[test]
fn location_enforces_value_bounds_without_panicking() {
  assert!(Location::parse("a".repeat(MAX_LOCATION_VALUE_BYTES + 1)).is_err());

  let oversized_duplicate = "a".repeat(MAX_LOCATION_VALUE_BYTES + 1);
  assert!(Location::parse_values(["https://example.test/", oversized_duplicate.as_str()]).is_err());
}
