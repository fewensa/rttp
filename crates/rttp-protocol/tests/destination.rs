use rttp_protocol::destination::{Destination, MAX_DESTINATION_VALUE_BYTES};

#[test]
fn parses_valid_absolute_destination_uris() {
  for value in [
    "https://dav.example.test/archive/report.txt",
    "http://example.test/collection/%E2%82%AC?copy=1#frag",
  ] {
    let destination = Destination::parse(value).expect("Destination should parse");

    assert_eq!(value, destination.as_str());
    assert_eq!(value, destination.header_value());
    assert_eq!(value, destination.as_ref());
  }
}

#[test]
fn trims_outer_optional_whitespace_and_preserves_the_trimmed_uri() {
  let destination = Destination::parse(" \thttps://dav.example.test/archive/report.txt\t ")
    .expect("Destination should parse");

  assert_eq!(
    "https://dav.example.test/archive/report.txt",
    destination.as_str()
  );
  assert_eq!(
    "https://dav.example.test/archive/report.txt",
    destination.header_value()
  );
}

#[test]
fn rejects_relative_and_malformed_destination_values() {
  for value in [
    "",
    " \t ",
    "/relative",
    "../path",
    "//example.test/path",
    "https://example.test/a b",
    "https://example.test/%zz",
  ] {
    assert!(
      Destination::parse(value).is_err(),
      "Destination should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_destination_fields() {
  assert!(Destination::parse_values([
    "https://dav.example.test/one",
    "https://dav.example.test/two",
  ])
  .is_err());
}

#[test]
fn rejects_oversized_destination_values() {
  let oversized = "a".repeat(MAX_DESTINATION_VALUE_BYTES + 1);
  assert!(Destination::parse(oversized).is_err());
}

#[test]
fn rejects_destination_control_byte_injection() {
  assert!(Destination::parse("https://example.test/a\r\nX: y").is_err());
  assert!(Destination::parse("https://example.test/a\n").is_err());
  assert!(Destination::parse("https://example.test/a\0").is_err());
  assert!(Destination::parse("https://example.test/\u{7f}").is_err());
  assert!(Destination::parse("https://example.test/a\tinner").is_err());
}
