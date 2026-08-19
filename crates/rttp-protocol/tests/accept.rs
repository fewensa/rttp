use rttp_protocol::accept::{
  Accept, AcceptMediaRange, MAX_ACCEPT_MEDIA_RANGES, MAX_ACCEPT_VALUE_BYTES,
  MAX_CLIENT_ACCEPT_MEDIA_RANGES,
};

#[test]
fn accept_parses_repeated_fields_in_wire_order() {
  let accept = Accept::parse_values([
    "Text/HTML; Level=1; q=0.7, application/json",
    "application/*; profile=\"compact\"; q=1.000; foo; bar=quoted, */*; q=0",
  ])
  .expect("Accept should parse");

  assert_eq!(accept.len(), 4);
  assert_eq!(
    ["text/html", "application/json", "application/*", "*/*"],
    accept
      .media_ranges()
      .iter()
      .map(AcceptMediaRange::media_type)
      .collect::<Vec<_>>()
      .as_slice()
  );
  assert_eq!(Some(700), accept.media_ranges()[0].quality());
  assert_eq!(vec![("level", "1")], accept.media_ranges()[0].parameters());
  assert_eq!(
    Some("compact"),
    accept.media_ranges()[2].parameter("profile")
  );
  assert_eq!(Some(1000), accept.media_ranges()[2].quality());
  assert_eq!(Some(0), accept.media_ranges()[3].quality());
  assert_eq!(
    "text/html; level=1;q=0.7, application/json, application/*; profile=compact;q=1, */*;q=0",
    accept.header_value()
  );
}

#[test]
fn accept_request_builder_member_preserves_wire_spelling() {
  let member = AcceptMediaRange::request_builder_member("text/plain; charset=utf-8", Some("0.5"))
    .expect("builder member should parse");

  assert_eq!("text/plain; charset=utf-8;q=0.5", member);
  assert_eq!(
    "application/json",
    AcceptMediaRange::request_builder_member("application/json", None)
      .expect("builder member should parse")
  );
  assert_eq!(
    "text/plain; feature=\"\"",
    AcceptMediaRange::request_builder_member("text/plain; feature=\"\"", None)
      .expect("empty quoted-string parameter should parse")
  );
  assert_eq!(
    "text/plain;q=0.",
    AcceptMediaRange::request_builder_member("text/plain", Some("0."))
      .expect("empty fractional q-value should preserve client compatibility")
  );
  assert_eq!(
    "text/html;q=1.",
    AcceptMediaRange::request_builder_member("text/html", Some("1."))
      .expect("empty fractional q-value should preserve client compatibility")
  );
}

#[test]
fn accept_request_builder_member_rejects_padded_qvalues() {
  for qvalue in [" 0.8", "0.8 ", "\t0.8"] {
    assert!(
      AcceptMediaRange::request_builder_member("text/plain", Some(qvalue)).is_err(),
      "{qvalue:?} should preserve raw client q-value rejection"
    );
  }
}

#[test]
fn accept_allows_empty_quoted_string_parameter_values() {
  let accept = Accept::parse("text/plain; feature=\"\"").expect("Accept should parse");

  assert_eq!(Some(""), accept.media_ranges()[0].parameter("feature"));
  assert_eq!("text/plain; feature=\"\"", accept.header_value());
}

#[test]
fn accept_rejects_duplicate_parameters_and_quality_values() {
  for value in [
    "text/plain; charset=utf-8; charset=utf-16",
    "text/plain; q=0.8; q=0.7",
    "text/plain; q=0.8; ext=1; q=0.7",
  ] {
    assert!(
      Accept::parse(value).is_err(),
      "{value:?} should reject duplicates"
    );
  }

  assert!(
    AcceptMediaRange::request_builder_member("text/plain; q=0.5", Some("0.4")).is_err(),
    "builder q-value should reject an existing q parameter"
  );
}

#[test]
fn accept_distinguishes_server_extensions_from_builder_validation() {
  let server_accept =
    Accept::parse("text/html; q=0.8; foo").expect("server Accept should allow extension tokens");

  assert_eq!(Some(800), server_accept.media_ranges()[0].quality());
  assert!(
    Accept::parse_request_builder_values_with_limit(
      ["text/html; q=0.8; foo"],
      MAX_CLIENT_ACCEPT_MEDIA_RANGES,
    )
    .is_err(),
    "client builder validation should reject valueless extension tokens"
  );
}

#[test]
fn accept_rejects_malformed_media_ranges_parameters_and_qvalues() {
  for value in [
    "",
    "text",
    "*/json",
    "text/html; level",
    "text/html; level=\"unterminated",
    "text/html; level=a\"b",
    "text/html; q=1.001",
    "text/html; q=0.",
    "text/html\n;level=1",
  ] {
    assert!(Accept::parse(value).is_err(), "{value:?} must fail");
  }
}

#[test]
fn accept_enforces_size_and_count_bounds() {
  assert!(Accept::parse("a".repeat(MAX_ACCEPT_VALUE_BYTES + 1)).is_err());

  let at_server_limit = (0..MAX_ACCEPT_MEDIA_RANGES)
    .map(|index| format!("application/x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert_eq!(
    MAX_ACCEPT_MEDIA_RANGES,
    Accept::parse(&at_server_limit)
      .expect("server Accept range limit should parse")
      .len()
  );

  let too_many_server = (0..=MAX_ACCEPT_MEDIA_RANGES)
    .map(|index| format!("application/x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Accept::parse(&too_many_server).is_err());

  let too_many_client = (0..=MAX_CLIENT_ACCEPT_MEDIA_RANGES)
    .map(|index| format!("application/x-{index}"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Accept::parse_request_builder_values_with_limit(
    [too_many_client.as_str()],
    MAX_CLIENT_ACCEPT_MEDIA_RANGES
  )
  .is_err());
}

#[test]
fn accept_parse_error_implements_display_and_error() {
  let error = Accept::parse("text/plain; q=1.001").expect_err("bad q should fail");
  assert_eq!("invalid Accept quality value", error.to_string());
  let _: &dyn std::error::Error = &error;
}
