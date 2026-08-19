use rttp_protocol::location::{Location, MAX_LOCATION_VALUE_BYTES};

#[test]
fn location_accepts_absolute_and_relative_uri_references() {
  for value in [
    "https://example.test/path?q=1#section",
    "/next",
    "../login?next=%2Fdashboard",
    "?page=2",
    "//cdn.example.test/asset.js",
  ] {
    let location = Location::parse(value).expect("Location should parse");

    assert_eq!(value, location.as_str());
    assert_eq!(value, location.header_value());
  }
}

#[test]
fn location_trims_outer_optional_whitespace() {
  let location = Location::parse(" \t/next\t ").expect("Location should parse");

  assert_eq!("/next", location.as_str());
}

#[test]
fn location_rejects_absent_empty_duplicate_and_oversized_values() {
  assert!(Location::parse("").is_err());
  assert!(Location::parse(" \t ").is_err());
  assert!(Location::parse_values(["/one", "/two"]).is_err());

  let oversized = format!("/{}", "a".repeat(MAX_LOCATION_VALUE_BYTES));
  assert!(Location::parse(oversized).is_err());
}

#[test]
fn location_rejects_malformed_uri_references() {
  for value in [
    "http://[::1",
    "/bad path",
    "/bad%zz",
    "/ok\r",
    "/ok\n",
    "/ok\u{7f}",
    "/ok\tinner",
  ] {
    assert!(
      Location::parse(value).is_err(),
      "Location parser should reject {value:?}"
    );
  }
}
