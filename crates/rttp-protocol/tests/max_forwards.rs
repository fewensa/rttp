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
fn max_forwards_emits_canonical_decimal_for_leading_zeros() {
  let with_leading_zeros = MaxForwards::parse("0005").expect("leading zeros should parse");
  assert_eq!(with_leading_zeros.value(), 5);
  assert_eq!(with_leading_zeros.header_value(), "5");

  let padded_zeros = MaxForwards::parse(" \t0000\t ").expect("OWS-padded leading zeros");
  assert_eq!(padded_zeros.value(), 0);
  assert_eq!(padded_zeros.header_value(), "0");
}

#[test]
fn max_forwards_header_value_round_trips() {
  for value in [0u32, 1, 5, 42, u32::MAX / 2, u32::MAX - 1, u32::MAX] {
    let max_forwards = MaxForwards::new(value);
    let header_value = max_forwards.header_value();
    let reparsed =
      MaxForwards::parse(&header_value).expect("canonical header_value should round-trip");
    assert_eq!(reparsed, max_forwards);
    assert_eq!(reparsed.value(), value);
    assert_eq!(reparsed.header_value(), header_value);
  }

  let leading_zeros = MaxForwards::parse("00042").expect("leading zeros should parse");
  let canonical = leading_zeros.header_value();
  assert_eq!(canonical, "42");
  assert_eq!(
    MaxForwards::parse(&canonical).expect("canonical emit should reparse"),
    leading_zeros
  );
}

#[test]
fn max_forwards_rejects_duplicate_and_invalid_values() {
  assert!(MaxForwards::parse_values([]).is_err());
  assert!(MaxForwards::parse_values(["0", "1"]).is_err());
  assert!(MaxForwards::parse_values(["0", "0"]).is_err());

  for value in [
    "",
    " ",
    "\t",
    "0, 1",
    "0,1",
    "+0",
    "+1",
    "-1",
    "-0",
    "1.5",
    ".1",
    "1.",
    "1e3",
    "1E3",
    "0x10",
    "0X10",
    "abc",
    "1 0",
    "4294967296",
    "18446744073709551616",
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
fn max_forwards_rejects_all_ascii_controls_except_horizontal_tab() {
  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let value = format!("0{}", char::from(control));
    assert!(
      MaxForwards::parse(&value).is_err(),
      "control {control:#04x} must be rejected"
    );
  }

  assert!(MaxForwards::parse("0\r\nX: y").is_err());
  assert!(MaxForwards::parse_values(["0", "bad\0value"]).is_err());
  assert!(MaxForwards::parse_values(["0", "bad\rvalue"]).is_err());
  assert!(MaxForwards::parse_values(["0", "bad\u{7f}value"]).is_err());
}

#[test]
fn max_forwards_enforces_value_bounds() {
  let exact = "0".repeat(MAX_FORWARDS_VALUE_BYTES);
  let oversized = "0".repeat(MAX_FORWARDS_VALUE_BYTES + 1);
  let padding = " ".repeat(MAX_FORWARDS_VALUE_BYTES - 1);
  let exact_padded = format!("{padding}0");
  let oversized_padded = format!(" {exact_padded}");

  assert_eq!(MAX_FORWARDS_VALUE_BYTES, exact.len());
  assert_eq!(MAX_FORWARDS_VALUE_BYTES, exact_padded.len());
  assert_eq!(MAX_FORWARDS_VALUE_BYTES + 1, oversized_padded.len());

  assert_eq!(
    MaxForwards::new(0),
    MaxForwards::parse(&exact).expect("exactly bounded Max-Forwards should parse")
  );
  assert_eq!(
    MaxForwards::new(0),
    MaxForwards::parse(&exact_padded).expect("OWS-padded exact Max-Forwards bound should parse")
  );
  assert!(MaxForwards::parse(&oversized).is_err());
  assert!(MaxForwards::parse(&oversized_padded).is_err());
}

#[test]
fn max_forwards_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_FORWARDS_VALUE_BYTES + 1);

  assert!(
    MaxForwards::parse_values(["0", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
