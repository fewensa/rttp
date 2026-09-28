use rttp_protocol::destination::{Destination, MAX_DESTINATION_VALUE_BYTES};

#[test]
fn parses_valid_absolute_destination_uris() {
  for value in [
    "https://dav.example.test/archive/report.txt",
    "http://example.test/collection/%E2%82%AC?copy=1",
    "webdav+ssh://user@example.test:8443/a;b/\u{27}c\u{27}?x=1&y=2",
    "urn:example:animal:ferret:nose",
  ] {
    let destination = Destination::parse(value).expect("Destination should parse");

    assert_eq!(value, destination.as_str());
    assert_eq!(value, destination.header_value());
    assert_eq!(value, destination.as_ref());
    assert_eq!(
      destination.as_str(),
      Destination::parse(destination.header_value())
        .unwrap()
        .as_str()
    );
  }
}

#[test]
fn preserves_percent_encoding_and_rejects_invalid_percent_encoding() {
  for value in [
    "https://example.test/%00/%2f/%E2%82%AC?literal=%25",
    "mailto:user%40example.test",
  ] {
    assert!(Destination::parse(value).is_ok(), "should accept {value:?}");
  }
  for value in [
    "https://example.test/%",
    "https://example.test/%0",
    "https://example.test/%gg",
  ] {
    assert!(
      Destination::parse(value).is_err(),
      "should reject {value:?}"
    );
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
    "1https://example.test/path",
    "https://example.test/a b",
    "https://example.test/%zz",
    "https://example.test/collection#frag",
    "https://example.test/path?query#fragment",
    "https://example.test/path?bad query",
    "https://example.test/path\\value",
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
fn enforces_exact_raw_byte_limit_including_ows() {
  let prefix = "https://e/";
  let path = "a".repeat(MAX_DESTINATION_VALUE_BYTES - prefix.len());
  let exact = format!("{prefix}{path}");
  assert_eq!(MAX_DESTINATION_VALUE_BYTES, exact.len());
  assert!(Destination::parse(&exact).is_ok());
  assert!(Destination::parse(format!("{exact}a")).is_err());

  let ows_exact = format!(" {exact}");
  assert!(Destination::parse(&ows_exact).is_err());
}

#[test]
fn counts_utf8_bytes_before_uri_validation() {
  let prefix = "https://e/";
  let exact = format!(
    "{prefix}{}",
    "é".repeat((MAX_DESTINATION_VALUE_BYTES - prefix.len()) / 2)
  );
  assert!(exact.len() <= MAX_DESTINATION_VALUE_BYTES);
  assert!(Destination::parse(&exact).is_err());

  let oversized = format!(
    "{}é",
    "a".repeat(MAX_DESTINATION_VALUE_BYTES - prefix.len())
  );
  assert!(Destination::parse(format!("{prefix}{oversized}")).is_err());
}

#[test]
fn rejects_destination_control_byte_injection() {
  for control in 0u8..=31 {
    let value = format!("https://example.test/a{}b", char::from(control));
    assert!(
      Destination::parse(value).is_err(),
      "control byte {control} accepted"
    );
  }
  for control in [127u8] {
    let value = format!("https://example.test/a{}b", char::from(control));
    assert!(
      Destination::parse(value).is_err(),
      "control byte {control} accepted"
    );
  }
  assert!(Destination::parse("https://example.test/a\r\nX: y").is_err());
  assert!(Destination::parse("https://example.test/a\tinner").is_err());
}
