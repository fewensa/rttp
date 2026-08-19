use rttp_protocol::transfer_encoding::{
  TransferEncoding, MAX_TRANSFER_ENCODING_CODINGS, MAX_TRANSFER_ENCODING_VALUE_BYTES,
};

#[test]
fn transfer_encoding_preserves_chunked_spelling() {
  let transfer_encoding =
    TransferEncoding::parse("CHUNKED").expect("Transfer-Encoding should parse");

  assert_eq!(transfer_encoding.codings(), ["CHUNKED"]);
  assert_eq!(transfer_encoding.header_value(), "CHUNKED");
  assert_eq!(transfer_encoding.len(), 1);
  assert!(!transfer_encoding.is_empty());
}

#[test]
fn transfer_encoding_accepts_http_optional_whitespace_padding() {
  for value in ["\tchunked\t", " chunked "] {
    let transfer_encoding =
      TransferEncoding::parse(value).expect("OWS-padded Transfer-Encoding should parse");
    assert_eq!(transfer_encoding.codings(), ["chunked"]);
  }
}

#[test]
fn transfer_encoding_rejects_non_sole_chunked_order() {
  for value in ["gzip", "gzip, chunked", "chunked, gzip", "chunked, chunked"] {
    assert!(
      TransferEncoding::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn transfer_encoding_combines_duplicate_fields_before_sole_chunked_check() {
  assert!(
    TransferEncoding::parse_values(["chunked", "chunked"]).is_err(),
    "combined duplicate chunked fields must be rejected"
  );
  assert!(
    TransferEncoding::parse_values(["gzip", "chunked"]).is_err(),
    "combined gzip then chunked fields must be rejected"
  );

  let transfer_encoding =
    TransferEncoding::parse_values(["chunked"]).expect("a single chunked field should parse");
  assert_eq!(transfer_encoding.codings(), ["chunked"]);
}

#[test]
fn transfer_encoding_rejects_invalid_values() {
  for value in [
    "",
    "   ",
    ",chunked",
    "chunked,",
    "chunked,,gzip",
    "chun ked",
    "chunked; q=1",
    "chunked: gzip",
    "\u{0d}chunked",
    "chunked\r\nX: y",
    "chunked\u{7f}",
  ] {
    assert!(
      TransferEncoding::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn transfer_encoding_rejects_empty_field_sets() {
  assert!(
    TransferEncoding::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn transfer_encoding_enforces_value_and_coding_bounds() {
  assert!(
    TransferEncoding::parse("x".repeat(MAX_TRANSFER_ENCODING_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "x".repeat(MAX_TRANSFER_ENCODING_VALUE_BYTES);
  assert!(
    TransferEncoding::parse(&at_value_limit).is_err(),
    "a 64 KiB non-chunked token must still be rejected as unsupported"
  );

  let oversized_duplicate = "x".repeat(MAX_TRANSFER_ENCODING_VALUE_BYTES + 1);
  assert!(
    TransferEncoding::parse_values(["chunked", oversized_duplicate.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );

  let too_many = (0..=MAX_TRANSFER_ENCODING_CODINGS)
    .map(|index| format!("c{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    TransferEncoding::parse(&too_many).is_err(),
    "more than 256 codings must be rejected"
  );
}
