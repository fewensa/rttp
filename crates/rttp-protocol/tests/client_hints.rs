use rttp_protocol::client_hints::{
  AcceptCh, CriticalCh, MAX_CLIENT_HINT_NAMES, MAX_CLIENT_HINT_VALUE_BYTES,
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
