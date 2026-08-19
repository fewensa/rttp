use rttp_protocol::accept_ranges::{
  AcceptRanges, MAX_ACCEPT_RANGES_UNITS, MAX_ACCEPT_RANGES_VALUE_BYTES,
};

#[test]
fn accept_ranges_preserves_unit_spelling_and_order() {
  let accept_ranges = AcceptRanges::parse("bytes, Pages").expect("Accept-Ranges should parse");

  assert_eq!(accept_ranges.units(), ["bytes", "Pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, Pages");
  assert!(!accept_ranges.is_none());
}

#[test]
fn accept_ranges_accepts_multiple_fields_in_wire_order() {
  let accept_ranges = AcceptRanges::parse_values(["bytes, pages", "records"])
    .expect("multiple Accept-Ranges fields should parse");

  assert_eq!(accept_ranges.units(), ["bytes", "pages", "records"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages, records");
  assert!(accept_ranges.accepts_bytes());
}

#[test]
fn accept_ranges_accepts_http_optional_whitespace_padding() {
  for value in ["\tbytes\t", " bytes "] {
    let accept_ranges = AcceptRanges::parse(value).expect("OWS-padded Accept-Ranges should parse");
    assert_eq!(accept_ranges.units(), ["bytes"]);
  }

  for value in [" bytes ,\tpages ", "bytes,pages"] {
    let accept_ranges = AcceptRanges::parse(value).expect("OWS-padded Accept-Ranges should parse");
    assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  }
}

#[test]
fn accept_ranges_none_is_an_empty_unit_list() {
  let accept_ranges = AcceptRanges::parse("none").expect("Accept-Ranges none should parse");

  assert!(accept_ranges.is_none());
  assert!(accept_ranges.units().is_empty());
  assert!(!accept_ranges.accepts_bytes());
  assert_eq!(accept_ranges.header_value(), "none");

  assert_eq!(AcceptRanges::none(), accept_ranges);
}

#[test]
fn accept_ranges_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",bytes",
    "bytes,",
    "bytes,,pages",
    "bytes, ,pages",
    "byte ranges",
    "bytes:pages",
    "bytes@pages",
    "none, bytes",
    "bytes, none",
    "none, none",
    "\u{0d}bytes",
    "bytes\r\nX: y",
    "bytes\u{7f}",
  ] {
    assert!(
      AcceptRanges::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_ranges_rejects_case_insensitive_duplicates_but_keeps_first_spelling() {
  for value in ["bytes, BYTES", "bytes, pages, PAGES", "BYTES, bytes"] {
    assert!(
      AcceptRanges::parse(value).is_err(),
      "{value:?} duplicate units must be rejected"
    );
  }

  let cross_field = AcceptRanges::parse_values(["bytes, pages", "BYTES"]);
  assert!(
    cross_field.is_err(),
    "duplicate units across fields must be rejected"
  );

  let preserved = AcceptRanges::parse("bytes, Pages").expect("distinct units should parse");
  assert_eq!(preserved.units(), ["bytes", "Pages"]);
}

#[test]
fn accept_ranges_rejects_empty_field_sets() {
  assert!(
    AcceptRanges::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn accept_ranges_from_units_validates_and_joins() {
  let accept_ranges =
    AcceptRanges::from_units(["bytes", "pages"]).expect("valid units should build");
  assert_eq!(accept_ranges.units(), ["bytes", "pages"]);
  assert_eq!(accept_ranges.header_value(), "bytes, pages");

  assert!(
    AcceptRanges::from_units(std::iter::empty::<&str>()).is_err(),
    "empty unit lists must be rejected"
  );
  assert!(
    AcceptRanges::from_units(["none"]).is_err(),
    "the none sentinel must use the none helper"
  );
  assert!(
    AcceptRanges::from_units(["bytes", "BYTES"]).is_err(),
    "duplicate units must be rejected"
  );
}

#[test]
fn accept_ranges_enforces_value_and_unit_bounds() {
  assert!(
    AcceptRanges::parse("x".repeat(MAX_ACCEPT_RANGES_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_ACCEPT_RANGES_VALUE_BYTES);
  assert!(
    AcceptRanges::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_ACCEPT_RANGES_VALUE_BYTES + 1);
  assert!(
    AcceptRanges::parse_values(["bytes", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_ACCEPT_RANGES_UNITS)
    .map(|index| format!("unit{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = AcceptRanges::parse(&at_limit).expect("256 units should parse");
  assert_eq!(parsed.units().len(), MAX_ACCEPT_RANGES_UNITS);

  let too_many = (0..=MAX_ACCEPT_RANGES_UNITS)
    .map(|index| format!("unit{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    AcceptRanges::parse(&too_many).is_err(),
    "more than 256 units must be rejected"
  );

  let too_many_from_units = (0..=MAX_ACCEPT_RANGES_UNITS)
    .map(|index| format!("unit{index}"))
    .collect::<Vec<_>>();
  assert!(
    AcceptRanges::from_units(too_many_from_units).is_err(),
    "more than 256 units from the builder must be rejected"
  );
}

#[test]
fn accept_ranges_parse_error_implements_display_and_error() {
  use std::error::Error;

  let error = AcceptRanges::parse("bytes,").expect_err("trailing comma must be rejected");
  let _: &dyn Error = &error;
  assert!(!error.to_string().is_empty());
}
