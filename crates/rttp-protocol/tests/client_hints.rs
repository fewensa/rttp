use rttp_protocol::client_hints::{
  AcceptCh, CriticalCh, Dpr, MAX_CLIENT_HINT_NAMES, MAX_CLIENT_HINT_VALUE_BYTES,
  MAX_DPR_VALUE_BYTES,
};

#[test]
fn accept_ch_combines_values_and_preserves_client_hint_spelling() {
  let accept_ch = AcceptCh::parse_values(["Sec-CH-UA, DPR", "Viewport-Width, Example/token:1"])
    .expect("valid Accept-CH");

  assert_eq!(
    &["Sec-CH-UA", "DPR", "Viewport-Width", "Example/token:1"],
    accept_ch.client_hints()
  );
  assert_eq!(
    "Sec-CH-UA, DPR, Viewport-Width, Example/token:1",
    accept_ch.header_value()
  );
}

#[test]
fn critical_ch_round_trips_comma_separated_client_hints() {
  let critical_ch =
    CriticalCh::parse("Sec-CH-Prefers-Color-Scheme, Downlink").expect("valid Critical-CH");

  assert_eq!(
    &["Sec-CH-Prefers-Color-Scheme", "Downlink"],
    critical_ch.client_hints()
  );
  assert_eq!(
    "Sec-CH-Prefers-Color-Scheme, Downlink",
    critical_ch.header_value()
  );
  assert_eq!(
    critical_ch,
    CriticalCh::parse(critical_ch.header_value()).expect("serialized value is valid")
  );
}

#[test]
fn client_hint_headers_reject_invalid_and_empty_members() {
  for value in [
    "",
    "DPR,",
    ",DPR",
    "DPR,,Width",
    "DPR;Width",
    "1DPR",
    "DPR\r\nInjected: yes",
  ] {
    assert!(
      AcceptCh::parse(value).is_err(),
      "{value:?} must be rejected"
    );
    assert!(
      CriticalCh::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn client_hint_headers_enforce_value_and_member_bounds() {
  assert!(AcceptCh::parse("a".repeat(MAX_CLIENT_HINT_VALUE_BYTES + 1)).is_err());
  assert!(CriticalCh::parse("a".repeat(MAX_CLIENT_HINT_VALUE_BYTES + 1)).is_err());

  let too_many = std::iter::repeat_n("DPR", MAX_CLIENT_HINT_NAMES + 1)
    .collect::<Vec<_>>()
    .join(",");
  assert!(AcceptCh::parse(&too_many).is_err());
  assert!(CriticalCh::parse(&too_many).is_err());
}

#[test]
fn dpr_parses_positive_finite_decimal_and_round_trips() {
  for (value, ratio) in [("1", 1.0), ("2.0", 2.0), ("1.5", 1.5)] {
    let dpr = Dpr::parse(value).expect("valid DPR");
    assert_eq!(ratio, dpr.ratio());
    assert_eq!(value, dpr.header_value());
    assert_eq!(dpr, Dpr::parse(dpr.header_value()).expect("DPR roundtrip"));
  }
}

#[test]
fn dpr_trims_outer_optional_whitespace() {
  let dpr = Dpr::parse("\t 1.5 \t").expect("OWS-padded DPR");
  assert_eq!(1.5, dpr.ratio());
  assert_eq!("1.5", dpr.header_value());
}

#[test]
fn dpr_rejects_malformed_duplicate_empty_non_finite_and_non_positive_values() {
  assert!(Dpr::parse_values(["1", "2"]).is_err());
  assert!(Dpr::parse_values([]).is_err());

  for value in [
    "", " ", "0", "0.0", "00", "2.", ".5", "+1", "-1", "1e1", "1E1", "1.5.0", "1, 2", "1 5", "inf",
    "nan",
  ] {
    assert!(Dpr::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn dpr_rejects_oversized_and_control_byte_values() {
  assert!(Dpr::parse("1".repeat(MAX_DPR_VALUE_BYTES + 1)).is_err());
  assert!(Dpr::parse("1\r\nInjected: yes").is_err());
  assert!(Dpr::parse("1\u{7f}").is_err());
}

#[test]
fn dpr_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_DPR_VALUE_BYTES + 1);
  assert!(Dpr::parse_values(["1.5", oversized.as_str()]).is_err());
}

#[test]
fn dpr_rejects_non_finite_oversized_digits() {
  assert!(Dpr::parse("9".repeat(400)).is_err());
}
