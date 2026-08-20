use rttp_protocol::negotiate::{
  Negotiate, NegotiateDirective, MAX_NEGOTIATE_MEMBERS, MAX_NEGOTIATE_TOTAL_BYTES,
  MAX_NEGOTIATE_VALUE_BYTES,
};

#[test]
fn parses_valid_negotiate_directives_in_wire_order() {
  let negotiate =
    Negotiate::parse("Trans, 1.0, feature-x=preview, *").expect("Negotiate should parse");

  assert_eq!(
    &[
      NegotiateDirective::Trans,
      NegotiateDirective::RvsaVersion { major: 1, minor: 0 },
      NegotiateDirective::Extension {
        name: "feature-x".to_owned(),
        value: Some("preview".to_owned()),
      },
      NegotiateDirective::Any,
    ],
    negotiate.members()
  );
  assert_eq!("trans, 1.0, feature-x=preview, *", negotiate.header_value());
}

#[test]
fn parses_rfc_example_negotiate_lists() {
  let versions = Negotiate::parse("1.0, 2.5").expect("RFC example versions should parse");
  assert_eq!(
    &[
      NegotiateDirective::RvsaVersion { major: 1, minor: 0 },
      NegotiateDirective::RvsaVersion { major: 2, minor: 5 },
    ],
    versions.members()
  );
  assert_eq!("1.0, 2.5", versions.header_value());

  let any = Negotiate::parse("*").expect("RFC example wildcard should parse");
  assert_eq!(&[NegotiateDirective::Any], any.members());
  assert_eq!("*", any.header_value());
}

#[test]
fn parses_multiple_negotiate_field_values_as_one_ordered_list() {
  let negotiate = Negotiate::parse_values(["trans", "1.0, feature-x=preview", "guess-small"])
    .expect("Negotiate should parse");

  assert_eq!(
    &[
      NegotiateDirective::Trans,
      NegotiateDirective::RvsaVersion { major: 1, minor: 0 },
      NegotiateDirective::Extension {
        name: "feature-x".to_owned(),
        value: Some("preview".to_owned()),
      },
      NegotiateDirective::GuessSmall,
    ],
    negotiate.members()
  );
  assert_eq!(
    "trans, 1.0, feature-x=preview, guess-small",
    negotiate.header_value()
  );
}

#[test]
fn formats_canonical_negotiate_directives() {
  let negotiate =
    Negotiate::parse("vlist, guess-small, vendor-cfg").expect("Negotiate should parse");
  assert_eq!(
    &[
      NegotiateDirective::Vlist,
      NegotiateDirective::GuessSmall,
      NegotiateDirective::Extension {
        name: "vendor-cfg".to_owned(),
        value: None,
      },
    ],
    negotiate.members()
  );
  assert_eq!("vlist, guess-small, vendor-cfg", negotiate.header_value());
  assert_eq!(3, negotiate.len());
  assert!(!negotiate.is_empty());
}

#[test]
fn normalizes_versions_without_leading_zeros() {
  let negotiate = Negotiate::parse("01.00, 2.50").expect("zero-padded versions should parse");
  assert_eq!(
    &[
      NegotiateDirective::RvsaVersion { major: 1, minor: 0 },
      NegotiateDirective::RvsaVersion {
        major: 2,
        minor: 50
      },
    ],
    negotiate.members()
  );
  assert_eq!("1.0, 2.50", negotiate.header_value());
}

#[test]
fn accepts_http_optional_whitespace_padding() {
  let negotiate = Negotiate::parse(" trans ,\t1.0 , feature-x = preview ")
    .expect("OWS-padded Negotiate should parse");
  assert_eq!(
    &[
      NegotiateDirective::Trans,
      NegotiateDirective::RvsaVersion { major: 1, minor: 0 },
      NegotiateDirective::Extension {
        name: "feature-x".to_owned(),
        value: Some("preview".to_owned()),
      },
    ],
    negotiate.members()
  );
  assert_eq!("trans, 1.0, feature-x=preview", negotiate.header_value());
}

#[test]
fn rejects_malformed_negotiate_values() {
  for value in [
    "",
    ",",
    "trans,",
    ",trans",
    "trans,,vlist",
    "   ",
    "trans=value",
    "vlist=1",
    "guess-small=yes",
    "*=value",
    "1.0=value",
    "trans=",
    "=value",
    "feature-x=",
    "trans;param",
    "feature-x=\"quoted\"",
  ] {
    assert!(
      Negotiate::parse(value).is_err(),
      "Negotiate should reject {value:?}"
    );
  }
}

#[test]
fn accepts_token_shaped_non_version_directives_as_extensions() {
  for value in ["1", "1.0.0", "1.", ".5", "vendor.com"] {
    let negotiate = Negotiate::parse(value).expect("token-shaped extension should parse");
    assert_eq!(
      &[NegotiateDirective::Extension {
        name: value.to_owned(),
        value: None,
      }],
      negotiate.members()
    );
    assert_eq!(value, negotiate.header_value());
  }

  let valued = Negotiate::parse("1.0.0=value").expect("token=token extension should parse");
  assert_eq!(
    &[NegotiateDirective::Extension {
      name: "1.0.0".to_owned(),
      value: Some("value".to_owned()),
    }],
    valued.members()
  );
  assert_eq!("1.0.0=value", valued.header_value());
}

#[test]
fn rejects_negotiate_version_overflow() {
  assert!(Negotiate::parse("18446744073709551616.0").is_err());
  assert!(Negotiate::parse("0.18446744073709551616").is_err());
  assert!(Negotiate::parse("18446744073709551616.18446744073709551616").is_err());
}

#[test]
fn rejects_duplicate_negotiate_directives() {
  assert!(Negotiate::parse("trans, TRANS").is_err());
  assert!(Negotiate::parse("vlist, Vlist").is_err());
  assert!(Negotiate::parse("guess-small, GUESS-SMALL").is_err());
  assert!(Negotiate::parse("*, *").is_err());
  assert!(Negotiate::parse("1.0, 01.00").is_err());
  assert!(Negotiate::parse_values(["1.0", "1.0"]).is_err());
  assert!(Negotiate::parse("feature-x=a, FEATURE-X=b").is_err());
  assert!(Negotiate::parse("vendor-cfg, vendor-cfg").is_err());
  assert!(Negotiate::parse("feature-x, FEATURE-X=value").is_err());
}

#[test]
fn accepts_distinct_versions_and_extensions() {
  let negotiate =
    Negotiate::parse("1.0, 2.5, feature-x=a, other=b").expect("distinct members should parse");
  assert_eq!(4, negotiate.len());
  assert_eq!("1.0, 2.5, feature-x=a, other=b", negotiate.header_value());
}

#[test]
fn rejects_too_many_negotiate_members() {
  let value = (0..=MAX_NEGOTIATE_MEMBERS)
    .map(|index| format!("feature-{index}"))
    .collect::<Vec<_>>()
    .join(", ");

  assert!(Negotiate::parse(value).is_err());
}

#[test]
fn rejects_oversized_negotiate_values() {
  let oversized = format!("feature-x={}", "a".repeat(MAX_NEGOTIATE_VALUE_BYTES));
  assert!(Negotiate::parse(oversized).is_err());
}

#[test]
fn rejects_oversized_negotiate_aggregate_values() {
  let first = format!("{}trans", " ".repeat(MAX_NEGOTIATE_TOTAL_BYTES / 2));
  let second = format!("{}vlist", " ".repeat(MAX_NEGOTIATE_TOTAL_BYTES / 2));

  assert!(Negotiate::parse_values([first.as_str(), second.as_str()]).is_err());
}

#[test]
fn rejects_negotiate_control_bytes_except_horizontal_tab() {
  assert!(Negotiate::parse("trans\r").is_err());
  assert!(Negotiate::parse("trans\n").is_err());
  assert!(Negotiate::parse("trans\0").is_err());
  assert_eq!(
    &[NegotiateDirective::Trans],
    Negotiate::parse("\ttrans\t")
      .expect("tab OWS is valid")
      .members()
  );
}
