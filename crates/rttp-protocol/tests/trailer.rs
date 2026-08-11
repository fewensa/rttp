use rttp_protocol::trailer::{Trailer, MAX_TRAILER_FIELD_NAMES, MAX_TRAILER_VALUE_BYTES};

#[test]
fn trailer_normalizes_singleton_field_names_case_insensitively() {
  let trailer = Trailer::parse("X-Checksum").expect("valid Trailer field name");

  assert_eq!(vec!["x-checksum"], trailer.field_names());
  assert_eq!(1, trailer.len());
  assert!(!trailer.is_empty());
  assert_eq!("x-checksum", trailer.header_value());
}

#[test]
fn trailer_parses_comma_lists_and_multiple_values() {
  let trailer = Trailer::parse_values([" X-Checksum , X-Trace-Id ", "Expires"])
    .expect("valid Trailer field names");

  assert_eq!(
    vec!["x-checksum", "x-trace-id", "expires"],
    trailer.field_names()
  );
  assert_eq!("x-checksum, x-trace-id, expires", trailer.header_value());
}

#[test]
fn trailer_deduplicates_field_names_case_insensitively_in_first_seen_order() {
  let trailer =
    Trailer::parse_values(["X-Checksum, X-Trace-Id", "x-checksum, EXPIRES, x-trace-id"])
      .expect("duplicate Trailer field names are valid");

  assert_eq!(
    vec!["x-checksum", "x-trace-id", "expires"],
    trailer.field_names()
  );
  assert_eq!("x-checksum, x-trace-id, expires", trailer.header_value());
}

#[test]
fn trailer_rejects_empty_members() {
  for value in ["", "X-Checksum,", ",X-Checksum", "X-Checksum,,Expires"] {
    assert!(Trailer::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn trailer_rejects_invalid_tokens() {
  for value in ["X Checksum", "X-Checksum: sha256", "\rX-Checksum"] {
    assert!(Trailer::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn trailer_rejects_forbidden_field_names() {
  for value in ["Content-Length", "Trailer", "Transfer-Encoding"] {
    assert!(Trailer::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn trailer_rejects_values_over_the_byte_limit() {
  let too_large = "x".repeat(MAX_TRAILER_VALUE_BYTES + 1);

  assert!(Trailer::parse(too_large).is_err());
}

#[test]
fn trailer_rejects_field_name_list_overflow() {
  let too_many = (0..=MAX_TRAILER_FIELD_NAMES)
    .map(|index| format!("X-Trailer-{index}"))
    .collect::<Vec<_>>()
    .join(",");

  assert!(Trailer::parse(too_many).is_err());
}
