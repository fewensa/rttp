use rttp_protocol::sec_gpc::{SecGpc, MAX_SEC_GPC_VALUE_BYTES};

#[test]
fn sec_gpc_parses_defined_request_signal() {
  let metadata = SecGpc::parse("1").expect("valid Sec-GPC");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn sec_gpc_parse_values_accepts_single_ows_padded_field() {
  let metadata = SecGpc::parse_values(["\t1 "]).expect("single Sec-GPC field");

  assert_eq!("1", metadata.header_value());
}

#[test]
fn sec_gpc_rejects_malformed_values() {
  for value in ["", " ", "\t", "0", "2", "true", "?1", "1, 1", "1;q=1"] {
    assert!(
      SecGpc::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn sec_gpc_rejects_control_bytes() {
  for value in ["1\r", "1\n", "1\u{7f}"] {
    assert!(
      SecGpc::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn sec_gpc_rejects_duplicate_header_fields() {
  assert!(SecGpc::parse_values(["1", "1"]).is_err());
}

#[test]
fn sec_gpc_rejects_empty_value_lists() {
  assert!(SecGpc::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn sec_gpc_enforces_value_bounds() {
  let oversized = "1".repeat(MAX_SEC_GPC_VALUE_BYTES + 1);
  assert!(SecGpc::parse(oversized).is_err());
}
