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
    Depth::parse("INFINITY").expect("case-insensitive infinity should parse")
  );
  assert_eq!(
    Depth::One,
    Depth::parse(" \t1\t ").expect("OWS should be trimmed")
  );
}

#[test]
fn formats_canonical_depth_values() {
  assert_eq!("0", Depth::Zero.header_value());
  assert_eq!("1", Depth::One.header_value());
  assert_eq!("infinity", Depth::Infinity.header_value());
}

#[test]
fn rejects_malformed_depth_values() {
  for value in ["", "2", "-1", "1.0", "infinite", "0, 1", "zero"] {
    assert!(
      Depth::parse(value).is_err(),
      "Depth should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_depth_fields() {
  assert!(Depth::parse_values(["0", "1"]).is_err());
}

#[test]
fn rejects_oversized_depth_values() {
  let oversized = "0".repeat(MAX_DEPTH_VALUE_BYTES + 1);
  assert!(Depth::parse(oversized).is_err());
}

#[test]
fn rejects_depth_control_bytes_except_horizontal_tab() {
  assert!(Depth::parse("1\r").is_err());
  assert!(Depth::parse("1\n").is_err());
  assert!(Depth::parse("1\0").is_err());
  assert_eq!(Depth::One, Depth::parse("\t1").expect("tab OWS is valid"));
}
