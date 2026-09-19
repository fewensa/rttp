use rttp_protocol::client_hints::{
  AcceptCh, CriticalCh, DeviceMemory, Downlink, Dpr, Ect, PrefersColorScheme, PrefersContrast,
  PrefersReducedMotion, Rtt, SecChUaArch, SecChUaBitness, SecChUaMobile, SecChUaModel,
  SecChUaPlatform, SecChUaWow64, ViewportWidth, Width, MAX_CLIENT_HINT_NAMES,
  MAX_CLIENT_HINT_VALUE_BYTES, MAX_DEVICE_MEMORY_VALUE_BYTES, MAX_DOWNLINK_VALUE_BYTES,
  MAX_DPR_VALUE_BYTES, MAX_ECT_VALUE_BYTES, MAX_PREFERS_COLOR_SCHEME_VALUE_BYTES,
  MAX_PREFERS_CONTRAST_VALUE_BYTES, MAX_PREFERS_REDUCED_MOTION_VALUE_BYTES, MAX_RTT_VALUE_BYTES,
  MAX_SEC_CH_UA_ARCH_VALUE_BYTES, MAX_SEC_CH_UA_BITNESS_VALUE_BYTES,
  MAX_SEC_CH_UA_MOBILE_VALUE_BYTES, MAX_SEC_CH_UA_MODEL_VALUE_BYTES,
  MAX_SEC_CH_UA_PLATFORM_VALUE_BYTES, MAX_SEC_CH_UA_WOW64_VALUE_BYTES,
  MAX_VIEWPORT_WIDTH_VALUE_BYTES, MAX_WIDTH_VALUE_BYTES,
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

#[test]
fn downlink_parses_non_negative_finite_decimal_and_round_trips() {
  for (value, mbps) in [
    ("0", 0.0),
    ("0.0", 0.0),
    ("1", 1.0),
    ("2.5", 2.5),
    ("10.25", 10.25),
  ] {
    let downlink = Downlink::parse(value).expect("valid Downlink");
    assert_eq!(mbps, downlink.mbps());
    assert_eq!(value, downlink.header_value());
    assert_eq!(
      downlink,
      Downlink::parse(downlink.header_value()).expect("Downlink roundtrip")
    );
  }
}

#[test]
fn downlink_trims_outer_optional_whitespace() {
  let downlink = Downlink::parse("\t 1.5 \t").expect("OWS-padded Downlink");
  assert_eq!(1.5, downlink.mbps());
  assert_eq!("1.5", downlink.header_value());
}

#[test]
fn downlink_rejects_malformed_duplicate_empty_non_finite_and_negative_values() {
  assert!(Downlink::parse_values(["1", "2"]).is_err());
  assert!(Downlink::parse_values([]).is_err());

  for value in [
    "", " ", "-0", "-1", "2.", ".5", "+1", "1e1", "1E1", "1.5.0", "1, 2", "1 5", "inf", "nan",
  ] {
    assert!(
      Downlink::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn downlink_rejects_oversized_and_control_byte_values() {
  assert!(Downlink::parse("1".repeat(MAX_DOWNLINK_VALUE_BYTES + 1)).is_err());
  assert!(Downlink::parse("1\r\nInjected: yes").is_err());
  assert!(Downlink::parse("1\u{7f}").is_err());
}

#[test]
fn downlink_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_DOWNLINK_VALUE_BYTES + 1);
  assert!(Downlink::parse_values(["1.5", oversized.as_str()]).is_err());
}

#[test]
fn downlink_rejects_non_finite_oversized_digits() {
  assert!(Downlink::parse("9".repeat(400)).is_err());
}

#[test]
fn device_memory_parses_non_negative_finite_decimal_and_round_trips() {
  for (value, gib) in [
    ("0", 0.0),
    ("0.0", 0.0),
    ("1", 1.0),
    ("2.5", 2.5),
    ("8", 8.0),
  ] {
    let device_memory = DeviceMemory::parse(value).expect("valid Device-Memory");
    assert_eq!(gib, device_memory.gib());
    assert_eq!(value, device_memory.header_value());
    assert_eq!(
      device_memory,
      DeviceMemory::parse(device_memory.header_value()).expect("Device-Memory roundtrip")
    );
  }
}

#[test]
fn device_memory_trims_outer_optional_whitespace() {
  let device_memory = DeviceMemory::parse("\t 8 \t").expect("OWS-padded Device-Memory");
  assert_eq!(8.0, device_memory.gib());
  assert_eq!("8", device_memory.header_value());
}

#[test]
fn device_memory_rejects_malformed_duplicate_empty_non_finite_and_negative_values() {
  assert!(DeviceMemory::parse_values(["1", "2"]).is_err());
  assert!(DeviceMemory::parse_values([]).is_err());

  for value in [
    "", " ", "-0", "-1", "2.", ".5", "+1", "1e1", "1E1", "1.5.0", "1, 2", "1 5", "inf", "nan",
  ] {
    assert!(
      DeviceMemory::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn device_memory_rejects_oversized_and_control_byte_values() {
  assert!(DeviceMemory::parse("1".repeat(MAX_DEVICE_MEMORY_VALUE_BYTES + 1)).is_err());
  assert!(DeviceMemory::parse("1\r\nInjected: yes").is_err());
  assert!(DeviceMemory::parse("1\u{7f}").is_err());
}

#[test]
fn device_memory_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_DEVICE_MEMORY_VALUE_BYTES + 1);
  assert!(DeviceMemory::parse_values(["8", oversized.as_str()]).is_err());
}

#[test]
fn device_memory_rejects_non_finite_oversized_digits() {
  assert!(DeviceMemory::parse("9".repeat(400)).is_err());
}

#[test]
fn prefers_color_scheme_accepts_case_insensitive_tokens_and_canonicalizes_them() {
  assert_eq!("light", PrefersColorScheme::Light.header_value());
  assert_eq!("dark", PrefersColorScheme::Dark.header_value());

  for (value, expected, canonical) in [
    ("LiGhT", PrefersColorScheme::Light, "light"),
    ("DARK", PrefersColorScheme::Dark, "dark"),
  ] {
    let scheme =
      PrefersColorScheme::parse(format!("\t{value} \t")).expect("valid prefers-color-scheme value");
    assert_eq!(expected, scheme);
    assert_eq!(canonical, scheme.header_value());
    assert_eq!(
      scheme,
      PrefersColorScheme::parse(scheme.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn prefers_color_scheme_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(PrefersColorScheme::parse_values(["light", "dark"]).is_err());
  assert!(PrefersColorScheme::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "system",
    "light, dark",
    "light\0",
    "dark\r\nInjected: yes",
    "dark\u{7f}",
  ] {
    assert!(
      PrefersColorScheme::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "a".repeat(MAX_PREFERS_COLOR_SCHEME_VALUE_BYTES + 1);
  assert!(PrefersColorScheme::parse(&oversized).is_err());
  assert!(PrefersColorScheme::parse_values(["light", oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_mobile_accepts_ows_around_structured_boolean_tokens_and_canonicalizes_them() {
  assert_eq!("?0", SecChUaMobile::NotMobile.header_value());
  assert_eq!("?1", SecChUaMobile::Mobile.header_value());
  assert!(!SecChUaMobile::NotMobile.is_mobile());
  assert!(SecChUaMobile::Mobile.is_mobile());

  for (value, expected, canonical) in [
    ("?0", SecChUaMobile::NotMobile, "?0"),
    ("?1", SecChUaMobile::Mobile, "?1"),
  ] {
    let mobile =
      SecChUaMobile::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-Mobile value");
    assert_eq!(expected, mobile);
    assert_eq!(canonical, mobile.header_value());
    assert_eq!(
      mobile,
      SecChUaMobile::parse(mobile.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_mobile_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaMobile::parse_values(["?0", "?1"]).is_err());
  assert!(SecChUaMobile::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "true",
    "false",
    "?2",
    "?1, ?1",
    "?1;foo=?1",
    "(?1)",
    "\"?1\"",
    "1",
    "?1\0",
    "?1\r\nInjected: yes",
    "?1\u{7f}",
  ] {
    assert!(
      SecChUaMobile::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "a".repeat(MAX_SEC_CH_UA_MOBILE_VALUE_BYTES + 1);
  assert!(SecChUaMobile::parse(&oversized).is_err());
  assert!(SecChUaMobile::parse_values(["?1", oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_wow64_accepts_ows_around_structured_boolean_tokens_and_canonicalizes_them() {
  assert_eq!("?0", SecChUaWow64::NotWow64.header_value());
  assert_eq!("?1", SecChUaWow64::Wow64.header_value());
  assert!(!SecChUaWow64::NotWow64.is_wow64());
  assert!(SecChUaWow64::Wow64.is_wow64());

  for (value, expected, canonical) in [
    ("?0", SecChUaWow64::NotWow64, "?0"),
    ("?1", SecChUaWow64::Wow64, "?1"),
  ] {
    let wow64 = SecChUaWow64::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-WoW64 value");
    assert_eq!(expected, wow64);
    assert_eq!(canonical, wow64.header_value());
    assert_eq!(
      wow64,
      SecChUaWow64::parse(wow64.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_wow64_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaWow64::parse_values(["?0", "?1"]).is_err());
  assert!(SecChUaWow64::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "true",
    "false",
    "?2",
    "?1, ?1",
    "?1;foo=?1",
    "(?1)",
    "\"?1\"",
    "1",
    "?1\0",
    "?1\r\nInjected: yes",
    "?1\u{7f}",
    "?\u{80}",
    "?1é",
  ] {
    assert!(
      SecChUaWow64::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "a".repeat(MAX_SEC_CH_UA_WOW64_VALUE_BYTES + 1);
  assert!(SecChUaWow64::parse(&oversized).is_err());
  assert!(SecChUaWow64::parse_values(["?1", oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_platform_accepts_structured_strings_and_canonicalizes_them() {
  for (value, expected_value, canonical) in [
    (r#""Windows""#, "Windows", r#""Windows""#),
    (r#""macOS""#, "macOS", r#""macOS""#),
    (r#""Chrome OS""#, "Chrome OS", r#""Chrome OS""#),
    (r#""Windows\\\"""#, "Windows\\\"", r#""Windows\\\"""#),
  ] {
    let platform =
      SecChUaPlatform::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-Platform value");
    assert_eq!(expected_value, platform.value());
    assert_eq!(canonical, platform.header_value());
    assert_eq!(
      platform,
      SecChUaPlatform::parse(platform.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_platform_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaPlatform::parse_values([r#""Windows""#, r#""Linux""#]).is_err());
  assert!(SecChUaPlatform::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "Windows",
    r#""Windows", "Linux""#,
    r#""Windows";foo=bar"#,
    r#""unterminated"#,
    r#""bad"quote""#,
    r#""bad\escape""#,
    "\"\u{1f34e}\"",
    "\"Windows\u{80}\"",
    "\"\u{65e5}\u{672c}\u{8a9e}\"",
    "\"Windows\0\"",
    "\"Windows\r\nInjected: yes\"",
    "\"Windows\u{7f}\"",
  ] {
    assert!(
      SecChUaPlatform::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = format!("\"{}\"", "x".repeat(MAX_SEC_CH_UA_PLATFORM_VALUE_BYTES));
  assert!(SecChUaPlatform::parse(&oversized).is_err());
  assert!(SecChUaPlatform::parse_values([r#""Windows""#, oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_model_accepts_structured_strings_and_canonicalizes_them() {
  for (value, expected_value, canonical) in [
    (r#""Pixel 8""#, "Pixel 8", r#""Pixel 8""#),
    (r#""Galaxy S24""#, "Galaxy S24", r#""Galaxy S24""#),
    (r#""Model\\\"""#, "Model\\\"", r#""Model\\\"""#),
  ] {
    let model = SecChUaModel::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-Model value");
    assert_eq!(expected_value, model.value());
    assert_eq!(canonical, model.header_value());
    assert_eq!(
      model,
      SecChUaModel::parse(model.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_model_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaModel::parse_values([r#""Pixel 8""#, r#""Galaxy S24""#]).is_err());
  assert!(SecChUaModel::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "Pixel 8",
    r#""Pixel 8", "Galaxy S24""#,
    r#""Pixel 8";foo=bar"#,
    r#""unterminated"#,
    r#""bad"quote""#,
    r#""bad\escape""#,
    "\"\u{1f34e}\"",
    "\"Pixel \u{80}\"",
    "\"\u{65e5}\u{672c}\u{8a9e}\"",
    "\"Pixel\0\"",
    "\"Pixel\r\nInjected: yes\"",
    "\"Pixel\u{7f}\"",
  ] {
    assert!(
      SecChUaModel::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = format!("\"{}\"", "x".repeat(MAX_SEC_CH_UA_MODEL_VALUE_BYTES));
  assert!(SecChUaModel::parse(&oversized).is_err());
  assert!(SecChUaModel::parse_values([r#""Pixel 8""#, oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_arch_accepts_structured_strings_and_canonicalizes_them() {
  for (value, expected_value, canonical) in [
    (r#""x86""#, "x86", r#""x86""#),
    (r#""arm64""#, "arm64", r#""arm64""#),
    (r#""wasm32""#, "wasm32", r#""wasm32""#),
    (r#""x86\\\"""#, "x86\\\"", r#""x86\\\"""#),
  ] {
    let arch = SecChUaArch::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-Arch value");
    assert_eq!(expected_value, arch.value());
    assert_eq!(canonical, arch.header_value());
    assert_eq!(
      arch,
      SecChUaArch::parse(arch.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_arch_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaArch::parse_values([r#""x86""#, r#""arm64""#]).is_err());
  assert!(SecChUaArch::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "x86",
    r#""x86", "arm64""#,
    r#""x86";foo=bar"#,
    r#""unterminated"#,
    r#""bad"quote""#,
    r#""bad\escape""#,
    "\"\u{1f34e}\"",
    "\"x86\u{80}\"",
    "\"\u{65e5}\u{672c}\u{8a9e}\"",
    "\"x86\0\"",
    "\"x86\r\nInjected: yes\"",
    "\"x86\u{7f}\"",
  ] {
    assert!(
      SecChUaArch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = format!("\"{}\"", "x".repeat(MAX_SEC_CH_UA_ARCH_VALUE_BYTES));
  assert!(SecChUaArch::parse(&oversized).is_err());
  assert!(SecChUaArch::parse_values([r#""x86""#, oversized.as_str()]).is_err());
}

#[test]
fn sec_ch_ua_bitness_accepts_structured_strings_and_canonicalizes_them() {
  for (value, expected_value, canonical) in [
    (r#""64""#, "64", r#""64""#),
    (r#""32""#, "32", r#""32""#),
    (r#""unknown""#, "unknown", r#""unknown""#),
    (r#""64\\\"""#, "64\\\"", r#""64\\\"""#),
  ] {
    let bitness =
      SecChUaBitness::parse(format!("\t{value} \t")).expect("valid Sec-CH-UA-Bitness value");
    assert_eq!(expected_value, bitness.value());
    assert_eq!(canonical, bitness.header_value());
    assert_eq!(
      bitness,
      SecChUaBitness::parse(bitness.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn sec_ch_ua_bitness_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(SecChUaBitness::parse_values([r#""64""#, r#""32""#]).is_err());
  assert!(SecChUaBitness::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "64",
    r#""64", "32""#,
    r#""64";foo=bar"#,
    r#""unterminated"#,
    r#""bad"quote""#,
    r#""bad\escape""#,
    "\"\u{1f34e}\"",
    "\"64\u{80}\"",
    "\"\u{65e5}\u{672c}\u{8a9e}\"",
    "\"64\0\"",
    "\"64\r\nInjected: yes\"",
    "\"64\u{7f}\"",
  ] {
    assert!(
      SecChUaBitness::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = format!("\"{}\"", "x".repeat(MAX_SEC_CH_UA_BITNESS_VALUE_BYTES));
  assert!(SecChUaBitness::parse(&oversized).is_err());
  assert!(SecChUaBitness::parse_values([r#""64""#, oversized.as_str()]).is_err());
}

#[test]
fn prefers_reduced_motion_accepts_case_insensitive_tokens_and_canonicalizes_them() {
  assert_eq!(
    "no-preference",
    PrefersReducedMotion::NoPreference.header_value()
  );
  assert_eq!("reduce", PrefersReducedMotion::Reduce.header_value());

  for (value, expected, canonical) in [
    (
      "No-Preference",
      PrefersReducedMotion::NoPreference,
      "no-preference",
    ),
    ("REDUCE", PrefersReducedMotion::Reduce, "reduce"),
  ] {
    let motion = PrefersReducedMotion::parse(format!("\t{value} \t"))
      .expect("valid prefers-reduced-motion value");
    assert_eq!(expected, motion);
    assert_eq!(canonical, motion.header_value());
    assert_eq!(
      motion,
      PrefersReducedMotion::parse(motion.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn prefers_reduced_motion_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(PrefersReducedMotion::parse_values(["no-preference", "reduce"]).is_err());
  assert!(PrefersReducedMotion::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "auto",
    "reduce, no-preference",
    "reduce\0",
    "reduce\r\nInjected: yes",
    "reduce{7f}",
  ] {
    assert!(
      PrefersReducedMotion::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "a".repeat(MAX_PREFERS_REDUCED_MOTION_VALUE_BYTES + 1);
  assert!(PrefersReducedMotion::parse(&oversized).is_err());
  assert!(PrefersReducedMotion::parse_values(["reduce", oversized.as_str()]).is_err());
}

#[test]
fn prefers_contrast_accepts_case_insensitive_tokens_and_canonicalizes_them() {
  assert_eq!(
    "no-preference",
    PrefersContrast::NoPreference.header_value()
  );
  assert_eq!("more", PrefersContrast::More.header_value());
  assert_eq!("less", PrefersContrast::Less.header_value());
  assert_eq!("custom", PrefersContrast::Custom.header_value());

  for (value, expected, canonical) in [
    (
      "No-Preference",
      PrefersContrast::NoPreference,
      "no-preference",
    ),
    ("MORE", PrefersContrast::More, "more"),
    ("LeSs", PrefersContrast::Less, "less"),
    ("CuStOm", PrefersContrast::Custom, "custom"),
  ] {
    let contrast =
      PrefersContrast::parse(format!("\t{value} \t")).expect("valid prefers-contrast value");
    assert_eq!(expected, contrast);
    assert_eq!(canonical, contrast.header_value());
    assert_eq!(
      contrast,
      PrefersContrast::parse(contrast.header_value()).expect("roundtrip")
    );
  }
}

#[test]
fn prefers_contrast_rejects_invalid_duplicate_oversized_and_control_values() {
  assert!(PrefersContrast::parse_values(["more", "less"]).is_err());
  assert!(PrefersContrast::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "auto",
    "more, less",
    "more\0",
    "more\r\nInjected: yes",
    "less\u{7f}",
  ] {
    assert!(
      PrefersContrast::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let oversized = "a".repeat(MAX_PREFERS_CONTRAST_VALUE_BYTES + 1);
  assert!(PrefersContrast::parse(&oversized).is_err());
  assert!(PrefersContrast::parse_values(["custom", oversized.as_str()]).is_err());
}

#[test]
fn ect_accepts_case_insensitive_tokens_and_canonicalizes_them() {
  const FOUR_G_HEADER: &str = Ect::FourG.header_value();
  assert_eq!("4g", FOUR_G_HEADER);

  for (value, canonical) in [
    ("sLoW-2G", "slow-2g"),
    ("2G", "2g"),
    ("3G", "3g"),
    ("4G", "4g"),
  ] {
    let ect = Ect::parse(format!("\t{value} \t")).expect("valid ECT");
    assert_eq!(canonical, ect.header_value());
    assert_eq!(ect, Ect::parse(ect.header_value()).expect("ECT roundtrip"));
  }
}

#[test]
fn ect_rejects_invalid_duplicate_oversized_and_list_values() {
  assert!(Ect::parse_values(["4g", "3g"]).is_err());
  assert!(Ect::parse("4g, 3g").is_err());
  assert!(Ect::parse("5g").is_err());
  assert!(Ect::parse("").is_err());
  assert!(Ect::parse("4g\r\nInjected: yes").is_err());
  assert!(Ect::parse("4g\u{7f}").is_err());
  assert!(Ect::parse("a".repeat(MAX_ECT_VALUE_BYTES + 1)).is_err());
}

#[test]
fn width_parses_non_negative_integer_and_round_trips() {
  for (value, expected) in [
    ("0", 0),
    ("1", 1),
    ("1440", 1440),
    ("4294967296", 4294967296),
  ] {
    let width = Width::parse(value).expect("valid Width");
    assert_eq!(expected, width.value());
    assert_eq!(value, width.header_value());
    assert_eq!(
      width,
      Width::parse(width.header_value()).expect("Width roundtrip")
    );
    assert_eq!(width, Width::new(expected));
  }
}

#[test]
fn width_trims_outer_optional_whitespace() {
  let width = Width::parse("\t 1440 \t").expect("OWS-padded Width");
  assert_eq!(1440, width.value());
  assert_eq!("1440", width.header_value());
}

#[test]
fn width_rejects_malformed_duplicate_empty_and_overflow_values() {
  assert!(Width::parse_values(["1", "2"]).is_err());
  assert!(Width::parse_values([]).is_err());

  for value in [
    "", " ", "-0", "-1", "+1", "1.0", "1e1", "1E1", "1, 2", "1 5", "1\0",
  ] {
    assert!(Width::parse(value).is_err(), "{value:?} must be rejected");
  }
  assert!(Width::parse("18446744073709551616").is_err());
}

#[test]
fn width_rejects_oversized_and_control_byte_values() {
  assert!(Width::parse("1".repeat(MAX_WIDTH_VALUE_BYTES + 1)).is_err());
  assert!(Width::parse("1\r\nInjected: yes").is_err());
  assert!(Width::parse("1\u{7f}").is_err());
}

#[test]
fn width_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_WIDTH_VALUE_BYTES + 1);
  assert!(Width::parse_values(["1440", oversized.as_str()]).is_err());
}

#[test]
fn viewport_width_parses_non_negative_integer_and_round_trips() {
  for (value, expected) in [
    ("0", 0),
    ("1", 1),
    ("1440", 1440),
    ("4294967296", 4294967296),
    ("18446744073709551615", u64::MAX),
  ] {
    let viewport_width = ViewportWidth::parse(value).expect("valid Viewport-Width");
    assert_eq!(expected, viewport_width.value());
    assert_eq!(value, viewport_width.header_value());
    assert_eq!(
      viewport_width,
      ViewportWidth::parse(viewport_width.header_value()).expect("Viewport-Width roundtrip")
    );
    assert_eq!(viewport_width, ViewportWidth::new(expected));
  }
}

#[test]
fn viewport_width_trims_outer_optional_whitespace() {
  let viewport_width = ViewportWidth::parse("\t 1440 \t").expect("OWS-padded Viewport-Width");
  assert_eq!(1440, viewport_width.value());
  assert_eq!("1440", viewport_width.header_value());
}

#[test]
fn viewport_width_rejects_malformed_duplicate_empty_and_overflow_values() {
  assert!(ViewportWidth::parse_values(["1", "2"]).is_err());
  assert!(ViewportWidth::parse_values([]).is_err());

  for value in [
    "", " ", "-0", "-1", "+1", "1.0", "1e1", "1E1", "1, 2", "1 5", "1\0",
  ] {
    assert!(
      ViewportWidth::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
  assert!(ViewportWidth::parse("18446744073709551616").is_err());
}

#[test]
fn viewport_width_rejects_oversized_and_control_byte_values() {
  assert!(ViewportWidth::parse("1".repeat(MAX_VIEWPORT_WIDTH_VALUE_BYTES + 1)).is_err());
  assert!(ViewportWidth::parse("1\r\nInjected: yes").is_err());
  assert!(ViewportWidth::parse("1{7f}").is_err());
}

#[test]
fn viewport_width_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_VIEWPORT_WIDTH_VALUE_BYTES + 1);
  assert!(ViewportWidth::parse_values(["1440", oversized.as_str()]).is_err());
}

#[test]
fn rtt_parses_non_negative_millisecond_integer_and_round_trips() {
  for (value, expected) in [
    ("0", 0),
    ("1", 1),
    ("150", 150),
    ("4294967296", 4294967296),
    ("18446744073709551615", u64::MAX),
  ] {
    let rtt = Rtt::parse(value).expect("valid RTT");
    assert_eq!(expected, rtt.value());
    assert_eq!(value, rtt.header_value());
    assert_eq!(rtt, Rtt::parse(rtt.header_value()).expect("RTT roundtrip"));
    assert_eq!(rtt, Rtt::new(expected));
  }
}

#[test]
fn rtt_trims_outer_optional_whitespace() {
  let rtt = Rtt::parse("\t 150 \t").expect("OWS-padded RTT");
  assert_eq!(150, rtt.value());
  assert_eq!("150", rtt.header_value());
}

#[test]
fn rtt_rejects_malformed_duplicate_empty_and_overflow_values() {
  assert!(Rtt::parse_values(["1", "2"]).is_err());
  assert!(Rtt::parse_values([]).is_err());

  for value in [
    "", " ", "-0", "-1", "+1", "1.0", "1e1", "1E1", "1, 2", "1 5", "1\0",
  ] {
    assert!(Rtt::parse(value).is_err(), "{value:?} must be rejected");
  }
  assert!(Rtt::parse("18446744073709551616").is_err());
}

#[test]
fn rtt_rejects_oversized_and_control_byte_values() {
  assert!(Rtt::parse("1".repeat(MAX_RTT_VALUE_BYTES + 1)).is_err());
  assert!(Rtt::parse("1\r\nInjected: yes").is_err());
  assert!(Rtt::parse("1\u{7f}").is_err());
}

#[test]
fn rtt_checks_duplicate_values_against_the_bound() {
  let oversized = "1".repeat(MAX_RTT_VALUE_BYTES + 1);
  assert!(Rtt::parse_values(["150", oversized.as_str()]).is_err());
}
