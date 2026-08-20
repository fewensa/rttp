use rttp_protocol::overwrite::{Overwrite, MAX_OVERWRITE_VALUE_BYTES};

#[test]
fn parses_valid_overwrite_values() {
  assert_eq!(Overwrite::T, Overwrite::parse("T").expect("T should parse"));
  assert_eq!(Overwrite::F, Overwrite::parse("F").expect("F should parse"));
  assert_eq!(
    Overwrite::T,
    Overwrite::parse(" \tT\t ").expect("OWS should be trimmed")
  );
  assert_eq!(
    Overwrite::F,
    Overwrite::parse(" \tF\t ").expect("OWS should be trimmed")
  );
}

#[test]
fn formats_canonical_overwrite_values() {
  assert_eq!("T", Overwrite::T.header_value());
  assert_eq!("F", Overwrite::F.header_value());
}

#[test]
fn rejects_malformed_overwrite_values() {
  for value in ["", "t", "f", "true", "false", "T, F", "0", "1", "TF", "   "] {
    assert!(
      Overwrite::parse(value).is_err(),
      "Overwrite should reject {value:?}"
    );
  }
}

#[test]
fn rejects_duplicate_overwrite_fields() {
  assert!(Overwrite::parse_values(["T", "F"]).is_err());
  assert!(Overwrite::parse_values(["T", "T"]).is_err());
}

#[test]
fn rejects_oversized_overwrite_values() {
  let oversized = "T".repeat(MAX_OVERWRITE_VALUE_BYTES + 1);
  assert!(Overwrite::parse(oversized).is_err());
}

#[test]
fn rejects_overwrite_control_bytes_except_horizontal_tab() {
  assert!(Overwrite::parse("T\r").is_err());
  assert!(Overwrite::parse("T\n").is_err());
  assert!(Overwrite::parse("T\0").is_err());
  assert_eq!(
    Overwrite::T,
    Overwrite::parse("\tT").expect("tab OWS is valid")
  );
}
