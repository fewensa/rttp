use rttp_protocol::x_frame_options::{XFrameOptions, MAX_X_FRAME_OPTIONS_VALUE_BYTES};

#[test]
fn x_frame_options_parses_deny_and_same_origin_case_insensitively() {
  assert_eq!(
    XFrameOptions::Deny,
    XFrameOptions::parse("DENY").expect("DENY should parse")
  );
  assert_eq!(
    XFrameOptions::Deny,
    XFrameOptions::parse("deny").expect("deny should parse")
  );
  assert_eq!(
    XFrameOptions::SameOrigin,
    XFrameOptions::parse("SAMEORIGIN").expect("SAMEORIGIN should parse")
  );
  assert_eq!(
    XFrameOptions::SameOrigin,
    XFrameOptions::parse("SameOrigin").expect("SameOrigin should parse")
  );
  assert_eq!(
    XFrameOptions::SameOrigin,
    XFrameOptions::parse("sameorigin").expect("sameorigin should parse")
  );
  assert_eq!("DENY", XFrameOptions::Deny.header_value());
  assert_eq!("SAMEORIGIN", XFrameOptions::SameOrigin.header_value());
}

#[test]
fn x_frame_options_accepts_http_optional_whitespace_padding() {
  for value in ["\tDENY\t", " \tdeny\t ", "deny\t", "\tdeny"] {
    assert_eq!(
      XFrameOptions::Deny,
      XFrameOptions::parse(value).expect("OWS-padded deny should parse")
    );
  }
  for value in [
    "\tSAMEORIGIN\t",
    " \tsameorigin\t ",
    "SAMEORIGIN\t",
    "\tsameorigin",
  ] {
    assert_eq!(
      XFrameOptions::SameOrigin,
      XFrameOptions::parse(value).expect("OWS-padded SAMEORIGIN should parse")
    );
  }
}

#[test]
fn x_frame_options_rejects_empty_duplicate_malformed_and_ambiguous_values() {
  for value in [
    "",
    "   ",
    "ALLOW-FROM https://example.test",
    "DENY, SAMEORIGIN",
    "DENY; foo",
    "\"DENY\"",
    "SAME ORIGIN",
    "SAMEORIGIN\r\nX: y",
    "SAMEORIGIN\u{7f}",
    "deny, sameorigin",
    "deny; foo",
    "\"deny\"",
    "deny\r\nX: y",
    "deny\u{7f}",
    "same-origin",
    "unknown",
  ] {
    assert!(
      XFrameOptions::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    XFrameOptions::parse_values(["DENY", "SAMEORIGIN"]).is_err(),
    "duplicate singleton fields must be rejected"
  );
  assert!(
    XFrameOptions::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  assert!(
    XFrameOptions::parse("a".repeat(MAX_X_FRAME_OPTIONS_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );
}

#[test]
fn x_frame_options_checks_duplicate_values_against_its_bound() {
  let oversized = "a".repeat(MAX_X_FRAME_OPTIONS_VALUE_BYTES + 1);

  assert!(
    XFrameOptions::parse_values(["DENY", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}
