use rttp_protocol::dnt::{Dnt, MAX_DNT_VALUE_BYTES};

#[test]
fn dnt_parses_both_defined_preference_tokens() {
  let allow = Dnt::parse("0").expect("valid DNT allow-tracking");
  let do_not_track = Dnt::parse("1").expect("valid DNT do-not-track");

  assert_eq!(Dnt::AllowTracking, allow);
  assert_eq!("0", allow.header_value());
  assert_eq!(Dnt::DoNotTrack, do_not_track);
  assert_eq!("1", do_not_track.header_value());
}

#[test]
fn dnt_parse_values_accepts_single_ows_padded_field() {
  let metadata = Dnt::parse_values(["\t1 "]).expect("single DNT field");

  assert_eq!(Dnt::DoNotTrack, metadata);
  assert_eq!("1", metadata.header_value());
}

#[test]
fn dnt_rejects_malformed_values() {
  for value in [
    "", " ", "\t", "on", "off", "true", "false", "?1", "ON", "On", "null", "2", "1abc", "1, 0",
    "1;foo", "\"1\"",
  ] {
    assert!(Dnt::parse(value).is_err(), "{value:?} should be rejected");
  }
}

#[test]
fn dnt_rejects_control_bytes() {
  for value in ["0\r", "1\n", "1\u{7f}"] {
    assert!(Dnt::parse(value).is_err(), "{value:?} should be rejected");
  }
}

#[test]
fn dnt_rejects_duplicate_header_fields() {
  assert!(Dnt::parse_values(["0", "0"]).is_err());
  assert!(Dnt::parse_values(["1", "0"]).is_err());
}

#[test]
fn dnt_rejects_empty_value_lists() {
  assert!(Dnt::parse_values([] as [&str; 0]).is_err());
}

#[test]
fn dnt_enforces_value_bounds() {
  let oversized = "0".repeat(MAX_DNT_VALUE_BYTES + 1);
  assert!(Dnt::parse(oversized).is_err());
  let oversized_duplicate = "1".repeat(MAX_DNT_VALUE_BYTES + 1);
  assert!(Dnt::parse_values(["0", oversized_duplicate.as_str()]).is_err());
}
