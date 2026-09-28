use rttp_protocol::accept_encoding::{
  AcceptEncoding, MAX_ACCEPT_ENCODINGS, MAX_ACCEPT_ENCODING_VALUE_BYTES,
};

#[test]
fn accept_encoding_parses_codings_and_quality_values() {
  let encodings =
    AcceptEncoding::parse("gzip, br;q=0.8, identity;q=0").expect("Accept-Encoding should parse");

  assert_eq!(3, encodings.len());
  assert_eq!("gzip", encodings.codings()[0].coding());
  assert_eq!(1000, encodings.codings()[0].quality());
  assert!(!encodings.codings()[0].is_wildcard());
  assert!(!encodings.codings()[0].is_identity());
  assert_eq!("br", encodings.codings()[1].coding());
  assert_eq!(800, encodings.codings()[1].quality());
  assert_eq!("identity", encodings.codings()[2].coding());
  assert_eq!(0, encodings.codings()[2].quality());
  assert!(encodings.codings()[2].is_identity());
  assert_eq!(encodings.header_value(), "gzip, br;q=0.8, identity;q=0");
}

#[test]
fn accept_encoding_round_trips_client_locked_header_value() {
  let encodings = AcceptEncoding::parse("gzip, br;q=0.8, identity;q=0")
    .expect("client-locked Accept-Encoding should parse");

  assert_eq!(encodings.header_value(), "gzip, br;q=0.8, identity;q=0");
  let round_trip = AcceptEncoding::parse(encodings.header_value())
    .expect("formatted Accept-Encoding should parse");
  assert_eq!(round_trip.header_value(), encodings.header_value());
}

#[test]
fn accept_encoding_accepts_multiple_fields_in_wire_order() {
  let encodings = AcceptEncoding::parse_values(["gzip, br;q=0.8", "identity; q=0"])
    .expect("multiple Accept-Encoding fields should parse");

  assert_eq!(3, encodings.len());
  assert_eq!("gzip", encodings.codings()[0].coding());
  assert_eq!(1000, encodings.codings()[0].quality());
  assert_eq!("br", encodings.codings()[1].coding());
  assert_eq!(800, encodings.codings()[1].quality());
  assert_eq!("identity", encodings.codings()[2].coding());
  assert_eq!(0, encodings.codings()[2].quality());
  assert_eq!(encodings.header_value(), "gzip, br;q=0.8, identity;q=0");
}

#[test]
fn accept_encoding_preserves_explicit_q_text_including_one() {
  let encodings = AcceptEncoding::parse("gzip;q=1, br;q=1.000, identity;q=0.80")
    .expect("explicit q-values should parse");

  assert_eq!(1000, encodings.codings()[0].quality());
  assert_eq!(1000, encodings.codings()[1].quality());
  assert_eq!(800, encodings.codings()[2].quality());
  assert_eq!(
    encodings.header_value(),
    "gzip;q=1, br;q=1.000, identity;q=0.80"
  );
}

#[test]
fn accept_encoding_accepts_wildcard_with_quality() {
  let encodings =
    AcceptEncoding::parse("gzip, *;q=0").expect("wildcard Accept-Encoding should parse");

  assert_eq!("gzip", encodings.codings()[0].coding());
  assert!(!encodings.codings()[0].is_wildcard());
  assert!(!encodings.codings()[0].is_identity());
  assert_eq!("*", encodings.codings()[1].coding());
  assert!(encodings.codings()[1].is_wildcard());
  assert!(!encodings.codings()[1].is_identity());
  assert_eq!(0, encodings.codings()[1].quality());
  assert_eq!(encodings.header_value(), "gzip, *;q=0");
}

#[test]
fn accept_encoding_normalizes_case_for_matching_but_preserves_codings() {
  let encodings = AcceptEncoding::parse("GZIP;Q=0.500, IDENTITY;Q=0, *;Q=0")
    .expect("mixed-case Accept-Encoding members should parse");

  assert_eq!("GZIP", encodings.codings()[0].coding());
  assert_eq!(500, encodings.codings()[0].quality());
  assert!(!encodings.codings()[0].is_identity());
  assert_eq!("IDENTITY", encodings.codings()[1].coding());
  assert!(encodings.codings()[1].is_identity());
  assert_eq!(0, encodings.codings()[1].quality());
  assert!(encodings.codings()[2].is_wildcard());
  assert_eq!(
    "GZIP;q=0.500, IDENTITY;q=0, *;q=0",
    encodings.header_value()
  );
}

#[test]
fn accept_encoding_accepts_qvalue_boundary_precision_and_empty_fractions() {
  let encodings = AcceptEncoding::parse(
    "zero;q=0, zero-empty;q=0., zero-three;q=0.123, one;q=1, one-empty;q=1., one-three;q=1.000",
  )
  .expect("RFC 9110 qvalue boundary forms should parse");

  assert_eq!(
    [0, 0, 123, 1000, 1000, 1000],
    encodings
      .codings()
      .iter()
      .map(|coding| coding.quality())
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(
    "zero;q=0, zero-empty;q=0., zero-three;q=0.123, one;q=1, one-empty;q=1., one-three;q=1.000",
    encodings.header_value()
  );
  let reparsed = AcceptEncoding::parse(encodings.header_value())
    .expect("canonical qvalue serialization should parse");
  assert_eq!(encodings, reparsed);
}

#[test]
fn accept_encoding_accepts_http_optional_whitespace_padding() {
  for value in ["\tgzip\t", " gzip "] {
    let encodings = AcceptEncoding::parse(value).expect("OWS-padded Accept-Encoding should parse");
    assert_eq!(encodings.codings()[0].coding(), "gzip");
    assert_eq!(encodings.header_value(), "gzip");
  }

  let encodings = AcceptEncoding::parse(" gzip ,\tbr\t; \tq\t=\t0.8\t ")
    .expect("OWS-padded Accept-Encoding members should parse");
  assert_eq!(encodings.codings()[0].coding(), "gzip");
  assert_eq!(encodings.codings()[1].coding(), "br");
  assert_eq!(800, encodings.codings()[1].quality());
  assert_eq!(encodings.header_value(), "gzip, br;q=0.8");
}

#[test]
fn accept_encoding_rejects_non_ows_whitespace_padding() {
  for whitespace in ["\u{000b}", "\u{000c}", "\r", "\n", "\u{00a0}", "\u{2003}"] {
    assert!(
      AcceptEncoding::parse(format!("{whitespace}gzip{whitespace}")).is_err(),
      "non-OWS whitespace around a coding must be rejected"
    );
    assert!(
      AcceptEncoding::parse(format!("gzip;q={whitespace}0.5{whitespace}")).is_err(),
      "non-OWS whitespace around a q-value must be rejected"
    );
  }
}

#[test]
fn accept_encoding_builds_default_quality_lists() {
  let encodings =
    AcceptEncoding::from_codings(["gzip", "br", "identity"]).expect("Accept-Encoding should build");

  assert_eq!(encodings.len(), 3);
  assert_eq!(encodings.codings()[0].coding(), "gzip");
  assert_eq!(encodings.codings()[0].quality(), 1000);
  assert_eq!(encodings.header_value(), "gzip, br, identity");
}

#[test]
fn accept_encoding_rejects_invalid_members() {
  for value in [
    "",
    "   ",
    ",gzip",
    "gzip,",
    "gzip,,br",
    "bad coding",
    "gzip;q=1.1",
    "gzip;q=1.001",
    "gzip;q=1.0000",
    "gzip;q=0.1234",
    "gzip;q=.5",
    "gzip;q=00",
    "gzip;q=-0",
    "gzip;foo=1",
    "gzip;q=0.8;foo=1",
    "gzip: br",
    "\u{0d}gzip",
    "gzip\r\nX: y",
    "gzip\u{7f}",
  ] {
    assert!(
      AcceptEncoding::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn accept_encoding_rejects_duplicates_case_insensitively() {
  assert!(
    AcceptEncoding::parse("gzip, GZIP").is_err(),
    "duplicate codings in one field must be rejected"
  );
  assert!(
    AcceptEncoding::parse_values(["gzip", "GZIP;q=0.5"]).is_err(),
    "duplicate codings across fields must be rejected"
  );
  assert!(
    AcceptEncoding::parse("*, *;q=0").is_err(),
    "duplicate wildcards must be rejected"
  );
}

#[test]
fn accept_encoding_rejects_empty_field_sets() {
  assert!(
    AcceptEncoding::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn accept_encoding_enforces_value_and_member_bounds() {
  assert!(
    AcceptEncoding::parse("x".repeat(MAX_ACCEPT_ENCODING_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_ACCEPT_ENCODING_VALUE_BYTES);
  assert!(
    AcceptEncoding::parse(&at_value_limit).is_ok(),
    "values at the 64 KiB bound must parse"
  );

  let oversized_duplicate = "x".repeat(MAX_ACCEPT_ENCODING_VALUE_BYTES + 1);
  assert!(
    AcceptEncoding::parse_values(["gzip", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_limit = (0..MAX_ACCEPT_ENCODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = AcceptEncoding::parse(&at_limit).expect("32 codings should parse");
  assert_eq!(parsed.len(), MAX_ACCEPT_ENCODINGS);

  let too_many = (0..=MAX_ACCEPT_ENCODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    AcceptEncoding::parse(&too_many).is_err(),
    "more than 32 codings must be rejected"
  );
}
