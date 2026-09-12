use rttp_protocol::early_data::{EarlyData, MAX_EARLY_DATA_VALUE_BYTES};

#[test]
fn early_data_parses_defined_request_signal() {
  let metadata = EarlyData::parse("1").expect("valid Early-Data");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn early_data_parse_values_accepts_single_ows_padded_field() {
  let metadata = EarlyData::parse_values(["\t1 "]).expect("single Early-Data field");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn early_data_rejects_malformed_and_unsupported_values() {
  for value in ["", " ", "\t", "0", "2", "true", "?1", "1, 1", "1;foo=bar"] {
    assert!(
      EarlyData::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn early_data_rejects_control_bytes() {
  for value in ["1\r", "1\n", "1\u{7f}"] {
    assert!(
      EarlyData::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn early_data_rejects_duplicate_header_fields() {
  assert!(EarlyData::parse_values(["1", "1"]).is_err());
}

#[test]
fn early_data_rejects_empty_value_lists() {
  assert!(EarlyData::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn early_data_enforces_value_bounds() {
  let oversized = "1".repeat(MAX_EARLY_DATA_VALUE_BYTES + 1);
  assert!(EarlyData::parse(oversized).is_err());
}
