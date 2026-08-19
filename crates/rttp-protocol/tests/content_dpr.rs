use rttp_protocol::content_dpr::{ContentDpr, MAX_CONTENT_DPR_VALUE_BYTES};

#[test]
fn content_dpr_parses_integer_and_decimal_ratios() {
  let one = ContentDpr::parse("1").expect("integer Content-DPR");
  assert_eq!(one.ratio(), 1.0);
  assert_eq!(one.header_value(), "1");

  let two = ContentDpr::parse("2.0").expect("trailing-zero Content-DPR");
  assert_eq!(two.ratio(), 2.0);
  assert_eq!(two.header_value(), "2.0");

  let one_and_half = ContentDpr::parse("1.5").expect("fractional Content-DPR");
  assert_eq!(one_and_half.ratio(), 1.5);
  assert_eq!(one_and_half.header_value(), "1.5");
}

#[test]
fn content_dpr_accepts_http_optional_whitespace_padding() {
  for value in ["\t1.5\t", " 2.0 ", " \t1\t ", "1.5\t", "\t2"] {
    let content_dpr = ContentDpr::parse(value).expect("OWS-padded Content-DPR should parse");
    assert_eq!(content_dpr.header_value(), value.trim_matches([' ', '\t']));
  }
}

#[test]
fn content_dpr_rejects_zero_invalid_grammar_and_duplicates() {
  assert!(ContentDpr::parse_values(["1", "2.0"]).is_err());
  assert!(ContentDpr::parse_values([]).is_err());

  for value in [
    "",
    " ",
    "0",
    "0.0",
    "00",
    "2.",
    ".5",
    "+1",
    "-1",
    "1e1",
    "1E1",
    "1.5.0",
    "1, 2",
    "1 5",
    "inf",
    "nan",
    "1\r\nX: y",
    "1\u{7f}",
  ] {
    assert!(
      ContentDpr::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn content_dpr_enforces_value_bounds() {
  assert!(ContentDpr::parse("1".repeat(MAX_CONTENT_DPR_VALUE_BYTES + 1)).is_err());
}

#[test]
fn content_dpr_checks_duplicate_values_against_its_bound() {
  let oversized = "1".repeat(MAX_CONTENT_DPR_VALUE_BYTES + 1);

  assert!(
    ContentDpr::parse_values(["1.5", oversized.as_str()]).is_err(),
    "oversized duplicate fields must not bypass validation"
  );
}

#[test]
fn content_dpr_rejects_non_finite_oversized_digits() {
  assert!(
    ContentDpr::parse("9".repeat(400)).is_err(),
    "digit strings that overflow f64 must be rejected as non-finite"
  );
}
