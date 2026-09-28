use rttp_protocol::content_length::{
  HttpContentLength, HttpContentLengthParseError, MAX_CONTENT_LENGTH_TOTAL_BYTES,
  MAX_CONTENT_LENGTH_VALUE_BYTES,
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn content_length_preserves_existing_value_behavior() {
  let value = HttpContentLength::new(123);

  assert_eq!(value.len(), 123);
  assert!(!value.is_zero());
  assert!(!value.is_empty());
  assert_eq!(value.header_value(), "123");
  assert_eq!(HttpContentLength::new(0).header_value(), "0");
}

#[test]
fn content_length_parses_comma_lists_and_repeated_fields() {
  let parsed = HttpContentLength::parse_values([" 001,\t1 ", "1"])
    .expect("equal Content-Length members should parse");

  assert_eq!(parsed.len(), 1);
  assert_eq!(parsed.header_value(), "1");
}

#[test]
fn content_length_accepts_ows_around_members() {
  for value in ["0", " 0 ", "\t0\t", " \t00042\t ", "42,\t42"] {
    let parsed = HttpContentLength::parse(value).expect("OWS-padded value should parse");
    let expected = value
      .split(',')
      .next()
      .unwrap()
      .trim_matches([' ', '\t'])
      .parse::<usize>()
      .unwrap();
    assert_eq!(parsed.len(), expected);
    assert_eq!(parsed.header_value(), parsed.len().to_string());
  }
}

#[test]
fn content_length_rejects_empty_leading_trailing_and_mismatched_members() {
  for value in [",1", "1,", "1,,1", "", " ", "\t"] {
    assert!(
      HttpContentLength::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(HttpContentLength::parse_values(["1", "01"]).is_ok());
  assert!(HttpContentLength::parse_values(["1", "2"]).is_err());
  assert!(HttpContentLength::parse("1, 2").is_err());
  assert!(HttpContentLength::parse_values([]).is_err());
}

#[test]
fn content_length_rejects_non_decimal_syntax_and_controls() {
  for value in [
    "+1",
    "-1",
    "1.0",
    "1 0",
    "1\r\nX: y",
    "1\u{7f}",
    "1\u{00b2}",
  ] {
    assert!(
      HttpContentLength::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_length_rejects_overflow_and_accepts_usize_boundary() {
  let maximum = usize::MAX.to_string();
  assert_eq!(
    HttpContentLength::parse(&maximum).unwrap().len(),
    usize::MAX
  );

  let overflow = format!("{maximum}0");
  assert!(HttpContentLength::parse(overflow).is_err());
}

#[test]
fn content_length_enforces_field_and_aggregate_limits() {
  let at_field_limit = "0".repeat(MAX_CONTENT_LENGTH_VALUE_BYTES);
  assert!(HttpContentLength::parse(&at_field_limit).is_ok());

  let oversized_field = "0".repeat(MAX_CONTENT_LENGTH_VALUE_BYTES + 1);
  assert!(HttpContentLength::parse(&oversized_field).is_err());

  let field = "0".repeat(MAX_CONTENT_LENGTH_TOTAL_BYTES / 2);
  assert!(HttpContentLength::parse_values([field.as_str(), field.as_str()]).is_ok());
  assert!(HttpContentLength::parse_values([field.as_str(), field.as_str(), "0"]).is_err());
}

#[test]
fn content_length_round_trips_canonical_output_and_is_sync_safe() {
  assert_send_sync::<HttpContentLength>();
  assert_send_sync::<HttpContentLengthParseError>();

  for input in ["0000", "\t00042 ", "42, 042"] {
    let parsed = HttpContentLength::parse(input).expect("input should parse");
    let reparsed =
      HttpContentLength::parse(parsed.header_value()).expect("canonical output should parse");
    assert_eq!(reparsed, parsed);
    assert_eq!(reparsed.header_value(), parsed.len().to_string());
  }
}
