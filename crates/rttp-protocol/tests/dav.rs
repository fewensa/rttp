use rttp_protocol::dav::{Dav, DavClass, MAX_DAV_AGGREGATE_VALUE_BYTES, MAX_DAV_CLASSES};

#[test]
fn parses_ordered_compliance_classes_tokens_and_coded_urls() {
  let dav = Dav::parse_values([
    "1, 2",
    "extended-mkcol, <https://dav.example.test/ns,with-comma>",
  ])
  .expect("valid DAV fields should parse");

  assert_eq!(
    &[
      DavClass::One,
      DavClass::Two,
      DavClass::ExtensionToken("extended-mkcol".to_string()),
      DavClass::CodedUrl("https://dav.example.test/ns,with-comma".to_string()),
    ],
    dav.classes()
  );
  assert_eq!(
    "1, 2, extended-mkcol, <https://dav.example.test/ns,with-comma>",
    dav.header_value()
  );
}

#[test]
fn rejects_malformed_dav_members() {
  for value in [
    "",
    "1,",
    ",1",
    "1,,2",
    "1, ,2",
    "not valid",
    "<relative/path>",
    "<https://example.test/a b>",
    "<https://example.test/a",
    "https://example.test/a>",
    "<<https://example.test/a>>",
    "1\r",
    "1\u{7f}",
  ] {
    assert!(
      Dav::parse(value).is_err(),
      "DAV parser should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_classes_across_fields() {
  assert!(Dav::parse_values(["1, 2", "3, 1"]).is_err());
  assert!(Dav::parse_values(["extended-mkcol", "extended-mkcol"]).is_err());
  assert!(Dav::parse_values([
    "<https://dav.example.test/ns>",
    "<https://dav.example.test/ns>"
  ])
  .is_err());
}

#[test]
fn enforces_member_count_bound() {
  let exact = (0..MAX_DAV_CLASSES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  let dav = Dav::parse(&exact).expect("max DAV class count should parse");
  assert_eq!(MAX_DAV_CLASSES, dav.classes().len());

  let too_many = (0..=MAX_DAV_CLASSES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Dav::parse(&too_many).is_err());
}

#[test]
fn enforces_per_field_and_aggregate_value_bounds() {
  let exact = format!("x{}", "a".repeat(MAX_DAV_AGGREGATE_VALUE_BYTES - 1));
  assert!(Dav::parse(&exact).is_ok());

  let oversized = format!("x{}", "a".repeat(MAX_DAV_AGGREGATE_VALUE_BYTES));
  assert!(Dav::parse(&oversized).is_err());

  let first_half = format!("x{}", "a".repeat(MAX_DAV_AGGREGATE_VALUE_BYTES / 2));
  let second_half = format!("y{}", "b".repeat(MAX_DAV_AGGREGATE_VALUE_BYTES / 2));
  assert!(Dav::parse_values([first_half.as_str(), second_half.as_str()]).is_err());
}
