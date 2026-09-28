use rttp_protocol::access_control_max_age::{
  AccessControlMaxAge, MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES,
};

#[test]
fn access_control_max_age_parses_zero_ordinary_and_unsigned_u64_boundaries() {
  let zero = AccessControlMaxAge::parse("0").expect("zero delta-seconds");
  assert_eq!(zero.seconds(), 0);
  assert_eq!(zero.header_value(), "0");

  let ordinary = AccessControlMaxAge::parse("60").expect("ordinary delta-seconds");
  assert_eq!(ordinary.seconds(), 60);
  assert_eq!(ordinary.header_value(), "60");

  let maximum =
    AccessControlMaxAge::parse(u64::MAX.to_string()).expect("maximum u64 delta-seconds");
  assert_eq!(maximum.seconds(), u64::MAX);
  assert_eq!(maximum.header_value(), u64::MAX.to_string());
}

#[test]
fn access_control_max_age_accepts_http_optional_whitespace_padding() {
  for value in ["\t0\t", " 0 ", " \t0\t ", "0\t", "\t0"] {
    let max_age =
      AccessControlMaxAge::parse(value).expect("OWS-padded Access-Control-Max-Age should parse");
    assert_eq!(max_age.seconds(), 0);
    assert_eq!(max_age.header_value(), "0");
  }
}

#[test]
fn access_control_max_age_emits_canonical_decimal_for_leading_zeros() {
  let with_leading_zeros = AccessControlMaxAge::parse("0005").expect("leading zeros should parse");
  assert_eq!(with_leading_zeros.seconds(), 5);
  assert_eq!(with_leading_zeros.header_value(), "5");

  let padded_zeros = AccessControlMaxAge::parse(" \t0000\t ").expect("OWS-padded leading zeros");
  assert_eq!(padded_zeros.seconds(), 0);
  assert_eq!(padded_zeros.header_value(), "0");
}

#[test]
fn access_control_max_age_header_value_round_trips() {
  for value in [0u64, 1, 5, 60, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
    let max_age = AccessControlMaxAge::new(value);
    let header_value = max_age.header_value();
    let reparsed =
      AccessControlMaxAge::parse(&header_value).expect("canonical header_value should round-trip");
    assert_eq!(reparsed, max_age);
    assert_eq!(reparsed.seconds(), value);
    assert_eq!(reparsed.header_value(), header_value);
  }

  let leading_zeros = AccessControlMaxAge::parse("00042").expect("leading zeros should parse");
  let canonical = leading_zeros.header_value();
  assert_eq!(canonical, "42");
  assert_eq!(
    AccessControlMaxAge::parse(&canonical).expect("canonical emit should reparse"),
    leading_zeros
  );
}

#[test]
fn access_control_max_age_rejects_duplicate_and_invalid_values() {
  assert!(AccessControlMaxAge::parse_values([]).is_err());
  assert!(AccessControlMaxAge::parse_values(["60", "120"]).is_err());
  assert!(AccessControlMaxAge::parse_values(["0", "0"]).is_err());

  for value in [
    "",
    " ",
    "\t",
    "60, 120",
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
    "60.0",
    "sixy",
    "18446744073709551616",
    "0\r\nX: y",
    "0\u{7f}",
  ] {
    assert!(
      AccessControlMaxAge::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn access_control_max_age_rejects_all_ascii_controls_except_horizontal_tab() {
  for control in (0u8..=31)
    .chain(std::iter::once(127))
    .filter(|&control| control != b'\t')
  {
    let value = format!("0{}", char::from(control));
    assert!(
      AccessControlMaxAge::parse(&value).is_err(),
      "control {control:#04x} must be rejected"
    );
  }

  assert!(AccessControlMaxAge::parse("0\r\nX: y").is_err());
  assert!(AccessControlMaxAge::parse_values(["0", "bad\0value"]).is_err());
  assert!(AccessControlMaxAge::parse_values(["0", "bad\rvalue"]).is_err());
  assert!(AccessControlMaxAge::parse_values(["0", "bad\u{7f}value"]).is_err());
}

#[test]
fn access_control_max_age_enforces_value_bounds() {
  let exact = "0".repeat(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES);
  let oversized = "0".repeat(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES + 1);
  let padding = " ".repeat(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES - 1);
  let exact_padded = format!("{padding}0");
  let oversized_padded = format!(" {exact_padded}");

  assert_eq!(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES, exact.len());
  assert_eq!(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES, exact_padded.len());
  assert_eq!(
    MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES + 1,
    oversized_padded.len()
  );

  assert_eq!(
    AccessControlMaxAge::new(0),
    AccessControlMaxAge::parse(&exact)
      .expect("exactly bounded Access-Control-Max-Age should parse")
  );
  assert_eq!(
    AccessControlMaxAge::new(0),
    AccessControlMaxAge::parse(&exact_padded)
      .expect("OWS-padded exact Access-Control-Max-Age bound should parse")
  );
  assert!(AccessControlMaxAge::parse(&oversized).is_err());
  assert!(AccessControlMaxAge::parse(&oversized_padded).is_err());
}

#[test]
fn access_control_max_age_checks_duplicate_values_against_its_bound() {
  let oversized = "0".repeat(MAX_ACCESS_CONTROL_MAX_AGE_VALUE_BYTES + 1);

  assert!(
    AccessControlMaxAge::parse_values(["0", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
