use rttp_protocol::te::{Te, MAX_TE_CODINGS, MAX_TE_VALUE_BYTES};

#[test]
fn te_parses_codings_in_wire_order_with_quality_mapping() {
  let te = Te::parse("gzip, deflate;q=0.5, trailers").expect("TE should parse");

  assert_eq!(3, te.len());
  assert!(!te.is_empty());
  assert_eq!("gzip", te.codings()[0].coding());
  assert_eq!(Some(1000), te.codings()[0].quality());
  assert!(!te.codings()[0].is_trailers());
  assert_eq!("deflate", te.codings()[1].coding());
  assert_eq!(Some(500), te.codings()[1].quality());
  assert_eq!("trailers", te.codings()[2].coding());
  assert_eq!(None, te.codings()[2].quality());
  assert!(te.codings()[2].is_trailers());
}

#[test]
fn te_maps_qvalue_thousandths_across_representations() {
  for (value, expected) in [
    ("0", 0),
    ("1", 1000),
    ("0.0", 0),
    ("1.000", 1000),
    ("0.5", 500),
    ("0.123", 123),
    ("0.999", 999),
  ] {
    let te = Te::parse(format!("gzip;q={value}")).expect("TE q-value should parse");
    assert_eq!(
      Some(expected),
      te.codings()[0].quality(),
      "{value:?} should map to {expected}"
    );
  }
}

#[test]
fn te_accepts_http_optional_whitespace_padding() {
  for (value, first) in [
    (" trailers ", "trailers"),
    (" gzip ,\tdeflate;q=0.5 ", "gzip"),
    ("gzip; q=0.5", "gzip"),
  ] {
    let te = Te::parse(value).expect("OWS-padded TE should parse");
    assert_eq!(first, te.codings()[0].coding(), "{value:?} should parse");
  }
}

#[test]
fn te_combines_multiple_fields_in_wire_order() {
  let te =
    Te::parse_values(["gzip", "deflate;q=0.5, trailers"]).expect("multiple TE fields should parse");

  assert_eq!(3, te.len());
  assert_eq!("gzip", te.codings()[0].coding());
  assert_eq!("deflate", te.codings()[1].coding());
  assert_eq!("trailers", te.codings()[2].coding());
}

#[test]
fn te_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",",
    "gzip,",
    ",gzip",
    "gzip,,deflate",
    "chunked",
    "Chunked",
    "trailers;q=0.5",
    "gzip;q=1.1",
    "gzip;q=0.1234",
    "gzip;q=0.12a",
    "gzip;q=",
    "gzip;q",
    "gzip;",
    "gzip;level=1",
    "gzip;q=0.5;level=1",
    "gzip;foo=q=1",
    "gzip; q=0.5 ; level=1",
    "bad coding",
    "gzip\u{7f}",
    "gzip\r\nX: y",
  ] {
    assert!(Te::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn te_rejects_duplicate_codings_case_insensitively() {
  for value in ["gzip, GZIP", "gzip, gzip;q=0.5"] {
    assert!(Te::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    Te::parse_values(["gzip", "GZIP"]).is_err(),
    "duplicate codings across fields must be rejected"
  );
}

#[test]
fn te_rejects_empty_field_sets() {
  assert!(
    Te::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn te_enforces_coding_count_and_value_bounds() {
  assert!(
    Te::parse("x".repeat(MAX_TE_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_TE_VALUE_BYTES);
  assert!(
    Te::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_TE_VALUE_BYTES + 1);
  assert!(
    Te::parse_values(["gzip", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let at_limit = (0..MAX_TE_CODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Te::parse(&at_limit).expect("32 codings should parse");
  assert_eq!(parsed.len(), MAX_TE_CODINGS);

  let too_many = (0..=MAX_TE_CODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Te::parse(&too_many).is_err(),
    "more than 32 codings must be rejected"
  );
}
