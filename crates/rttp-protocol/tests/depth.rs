use rttp_protocol::depth::{Depth, MAX_DEPTH_VALUE_BYTES};

#[test]
fn parses_valid_depth_values() {
  assert_eq!(Depth::Zero, Depth::parse("0").expect("0 should parse"));
  assert_eq!(Depth::One, Depth::parse("1").expect("1 should parse"));
  assert_eq!(
    Depth::Infinity,
    Depth::parse("infinity").expect("infinity should parse")
  );
  assert_eq!(
    Depth::Infinity,
    Depth::parse("INFINITY").expect("uppercase infinity should parse")
  );
  assert_eq!(
    Depth::Infinity,
    Depth::parse("InFiNiTy").expect("mixed-case infinity should parse")
  );
  assert_eq!(
    Depth::Zero,
    Depth::parse(" 0 ").expect("SP OWS should be trimmed")
  );
  assert_eq!(
    Depth::One,
    Depth::parse("\t1\t").expect("HTAB OWS should be trimmed")
  );
  assert_eq!(
    Depth::Infinity,
    Depth::parse(" \tinfinity\t ").expect("mixed SP/HTAB OWS should be trimmed")
  );
}

#[test]
fn formats_canonical_depth_values() {
  assert_eq!("0", Depth::Zero.header_value());
  assert_eq!("1", Depth::One.header_value());
  assert_eq!("infinity", Depth::Infinity.header_value());
}

#[test]
fn canonical_depth_values_round_trip() {
  for depth in [Depth::Zero, Depth::One, Depth::Infinity] {
    let canonical = depth.header_value();
    let parsed = Depth::parse(canonical).expect("canonical header_value should round-trip");
    assert_eq!(depth, parsed);
    assert_eq!(canonical, parsed.header_value());
  }
}

#[test]
fn rejects_malformed_depth_values() {
  for value in [
    "", "2", "-1", "1.0", "infinite", "0, 1", "zero", "01", "0 1",
  ] {
    assert!(
      Depth::parse(value).is_err(),
      "Depth should reject {value:?}"
    );
  }
}

#[test]
fn rejects_whitespace_only_depth_values() {
  for value in ["", " ", "\t", " \t ", "\t \t"] {
    assert!(
      Depth::parse(value).is_err(),
      "Depth should reject whitespace-only {value:?}"
    );
  }
}

#[test]
fn rejects_empty_and_duplicate_depth_fields() {
  assert!(
    Depth::parse_values([] as [&str; 0]).is_err(),
    "empty parse_values should be rejected"
  );
  assert!(
    Depth::parse_values(["0", "0"]).is_err(),
    "identical duplicate Depth fields should be rejected"
  );
  assert!(
    Depth::parse_values(["0", "1"]).is_err(),
    "distinct duplicate Depth fields should be rejected"
  );
  assert!(
    Depth::parse_values(["infinity", "INFINITY"]).is_err(),
    "case-variant duplicate Depth fields should be rejected"
  );
}

#[test]
fn accepts_exact_depth_value_byte_limit() {
  let at_limit = format!("{}0", " ".repeat(MAX_DEPTH_VALUE_BYTES - 1));
  assert_eq!(MAX_DEPTH_VALUE_BYTES, at_limit.len());
  assert_eq!(
    Depth::Zero,
    Depth::parse(&at_limit).expect("exact MAX_DEPTH_VALUE_BYTES should parse")
  );

  let at_limit_one = format!("1{}", "\t".repeat(MAX_DEPTH_VALUE_BYTES - 1));
  assert_eq!(MAX_DEPTH_VALUE_BYTES, at_limit_one.len());
  assert_eq!(
    Depth::One,
    Depth::parse(&at_limit_one).expect("exact HTAB-padded limit should parse")
  );
}

#[test]
fn rejects_oversized_depth_values() {
  let oversized = format!("{}0", " ".repeat(MAX_DEPTH_VALUE_BYTES));
  assert_eq!(MAX_DEPTH_VALUE_BYTES + 1, oversized.len());
  assert!(
    Depth::parse(&oversized).is_err(),
    "one byte over MAX_DEPTH_VALUE_BYTES must be rejected"
  );

  let oversized_repeat = "0".repeat(MAX_DEPTH_VALUE_BYTES + 1);
  assert!(Depth::parse(oversized_repeat).is_err());

  let oversized_duplicate = "1".repeat(MAX_DEPTH_VALUE_BYTES + 1);
  assert!(Depth::parse_values(["0", oversized_duplicate.as_str()]).is_err());
}

#[test]
fn rejects_depth_control_bytes_except_horizontal_tab() {
  assert_eq!(Depth::One, Depth::parse("\t1").expect("tab OWS is valid"));

  for byte in 0u8..=0x1f {
    if byte == b'\t' {
      continue;
    }
    let mut value = String::from("1");
    value.push(char::from(byte));
    assert!(
      Depth::parse(&value).is_err(),
      "Depth should reject control byte {byte:#04x}"
    );
  }

  assert!(Depth::parse("1\u{7f}").is_err(), "Depth should reject DEL");
}
