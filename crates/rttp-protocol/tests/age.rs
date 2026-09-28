use rttp_protocol::age::{Age, MAX_AGE_VALUE_BYTES};

#[test]
fn age_parses_zero_ordinary_and_unsigned_u64_boundaries() {
  let zero = Age::parse("0").expect("zero delta-seconds");
  assert_eq!(zero.seconds(), 0);
  assert_eq!(zero.header_value(), "0");

  let ordinary = Age::parse("60").expect("ordinary delta-seconds");
  assert_eq!(ordinary.seconds(), 60);
  assert_eq!(ordinary.header_value(), "60");

  let maximum = Age::parse(u64::MAX.to_string()).expect("maximum u64 delta-seconds");
  assert_eq!(maximum.seconds(), u64::MAX);
  assert_eq!(maximum.header_value(), u64::MAX.to_string());
}

#[test]
fn age_accepts_http_optional_whitespace_padding() {
  for value in ["\t5\t", " 5 ", " \t5\t ", "5\t", "\t5"] {
    let age = Age::parse(value).expect("OWS-padded Age should parse");
    assert_eq!(age.seconds(), 5);
    assert_eq!(age.header_value(), "5");
  }
}

#[test]
fn age_rejects_missing_duplicate_and_invalid_values() {
  assert!(
    Age::parse_values([]).is_err(),
    "missing singleton must be rejected"
  );
  assert!(
    Age::parse_values(["60", "60"]).is_err(),
    "identical duplicates must be rejected"
  );
  assert!(
    Age::parse_values(["60", "120"]).is_err(),
    "differing duplicates must be rejected"
  );

  for value in [
    "",
    " ",
    "60, 120",
    "+60",
    "-60",
    "60.0",
    "sixy",
    "5 0",
    "5\t0",
    "18446744073709551616",
    "5\r\nX: y",
    "5\0",
    "5\u{7f}",
  ] {
    assert!(Age::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(Age::parse_values(["not-an-age", "60"]).is_err());
  assert!(Age::parse_values(["60", "not-an-age"]).is_err());
}

#[test]
fn age_enforces_inclusive_byte_limit() {
  let at_limit = format!("{}0", " ".repeat(MAX_AGE_VALUE_BYTES - 1));
  let over_limit = format!("{}0", " ".repeat(MAX_AGE_VALUE_BYTES));

  assert_eq!(
    Age::parse(&at_limit)
      .expect("exact limit should parse")
      .seconds(),
    0
  );
  assert!(
    Age::parse(over_limit).is_err(),
    "limit plus one byte must be rejected"
  );
}

#[test]
fn age_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_AGE_VALUE_BYTES + 1);

  assert!(
    Age::parse_values(["60", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
  assert!(
    Age::parse_values([oversized.as_str(), "60"]).is_err(),
    "oversized first fields must be rejected"
  );
}

#[test]
fn age_round_trips_canonical_decimal_values() {
  for seconds in [0, 1, 5, 60, 86_400, u64::MAX / 2, u64::MAX] {
    let age = Age::new(seconds);
    let serialized = age.header_value();
    assert_eq!(serialized, seconds.to_string());
    assert_eq!(
      Age::parse(&serialized).expect("canonical value should parse"),
      age
    );
    assert_eq!(
      Age::parse(&age.header_value()).expect("re-serialized value should parse"),
      age
    );
  }
}
