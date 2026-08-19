use rttp_protocol::save_data::{SaveData, MAX_SAVE_DATA_VALUE_BYTES};

#[test]
fn save_data_parses_on_request_form() {
  let metadata = SaveData::parse("on").expect("valid Save-Data");

  assert_eq!("on", metadata.header_value());
}

#[test]
fn save_data_parse_values_accepts_single_ows_padded_field() {
  let metadata = SaveData::parse_values(["\ton "]).expect("single Save-Data field");

  assert_eq!("on", metadata.header_value());
}

#[test]
fn save_data_rejects_malformed_values() {
  for value in [
    "", " ", "\t", "off", "false", "ON", "On", "?1", "on, on", "on;q=1",
  ] {
    assert!(
      SaveData::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn save_data_rejects_control_bytes() {
  for value in ["on\r", "on\n", "on\u{7f}"] {
    assert!(
      SaveData::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn save_data_rejects_duplicate_header_fields() {
  assert!(SaveData::parse_values(["on", "on"]).is_err());
}

#[test]
fn save_data_rejects_empty_value_lists() {
  assert!(SaveData::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn save_data_enforces_value_bounds() {
  let oversized = "o".repeat(MAX_SAVE_DATA_VALUE_BYTES + 1);
  assert!(SaveData::parse(oversized).is_err());
}
