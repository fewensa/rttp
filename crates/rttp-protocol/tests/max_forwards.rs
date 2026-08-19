use rttp_protocol::max_forwards::{MaxForwards, MAX_FORWARDS_VALUE_BYTES};

#[test]
fn max_forwards_parses_zero_ordinary_and_unsigned_u32_boundaries() {
  let zero = MaxForwards::parse("0").expect("zero hop count");
  assert_eq!(zero.value(), 0);
  assert_eq!(zero.header_value(), "0");

  let ordinary = MaxForwards::parse("5").expect("ordinary hop count");
  assert_eq!(ordinary.value(), 5);
  assert_eq!(ordinary.header_value(), "5");

  let maximum = MaxForwards::parse(u32::MAX.to_string()).expect("maximum u32 hop count");
  assert_eq!(maximum.value(), u32::MAX);
  assert_eq!(maximum.header_value(), u32::MAX.to_string());
}

#[test]
fn max_forwards_accepts_http_optional_whitespace_padding() {
  for value in ["\t0\t", " 0 ", " \t0\t ", "0\t", "\t0"] {
    let max_forwards = MaxForwards::parse(value).expect("OWS-padded Max-Forwards should parse");
    assert_eq!(max_forwards.value(), 0);
    assert_eq!(max_forwards.header_value(), "0");
  }
}

#[test]
fn max_forwards_rejects_duplicate_and_invalid_values() {
  assert!(MaxForwards::parse_values(["0", "1"]).is_err());
  assert!(MaxForwards::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "0, 1",
    "+0",
    "-1",
    "1.5",
    "abc",
    "1 0",
    "4294967296",
    "0\r\nX: y",
    "0\u{7f}",
  ] {
    assert!(
      MaxForwards::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn max_forwards_enforces_value_bounds() {
  assert!(MaxForwards::parse("0".repeat(MAX_FORWARDS_VALUE_BYTES + 1)).is_err());
}

#[test]
fn max_forwards_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_FORWARDS_VALUE_BYTES + 1);

  assert!(
    MaxForwards::parse_values(["0", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
