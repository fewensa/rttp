use rttp_protocol::expect::{Expect, MAX_EXPECTATIONS, MAX_EXPECT_VALUE_BYTES};

#[test]
fn expect_constructs_the_standardized_continue_singleton() {
  let expect = Expect::expect_continue();

  assert!(expect.expects_continue());
  assert!(expect.unsupported().is_empty());
  assert_eq!(expect.header_value(), "100-continue");
}

#[test]
fn expect_parses_continue_and_unsupported_extension_names() {
  let expect =
    Expect::parse_values(["100-continue", "preview"]).expect("mixed Expect fields should parse");

  assert!(expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "100-continue, preview");
}

#[test]
fn expect_preserves_unsupported_extension_names_with_values_and_parameters() {
  let expect =
    Expect::parse("preview=sha256; chunk=1").expect("parameterized Expect extension should parse");

  assert!(!expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "preview");
}

#[test]
fn expect_accepts_http_optional_whitespace_padding() {
  let expect = Expect::parse(" 100-continue , preview ").expect("OWS-padded Expect should parse");

  assert!(expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "100-continue, preview");
}

#[test]
fn expect_rejects_duplicate_expectation_names() {
  assert!(Expect::parse("100-continue, 100-CONTINUE").is_err());
  assert!(Expect::parse_values(["100-continue", "100-CONTINUE"]).is_err());
  assert!(Expect::parse("preview, PREVIEW").is_err());
}

#[test]
fn expect_rejects_malformed_and_empty_values() {
  assert!(Expect::parse_values([]).is_err());

  for value in ["", "   ", ",", "100-continue,", ",preview", "not a token"] {
    assert!(Expect::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn expect_retains_unsupported_extension_names() {
  let expect = Expect::parse("tea-time").expect("unsupported Expect names should parse");

  assert!(!expect.expects_continue());
  assert_eq!(["tea-time"], expect.unsupported());
  assert_eq!(expect.header_value(), "tea-time");
}

#[test]
fn expect_enforces_value_and_member_count_bounds() {
  assert!(
    Expect::parse("a".repeat(MAX_EXPECT_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "a".repeat(MAX_EXPECT_VALUE_BYTES);
  let at_limit = Expect::parse(&at_value_limit).expect("values at the 64 KiB bound must parse");
  assert_eq!(at_limit.unsupported(), [at_value_limit.as_str()]);

  let oversized_duplicate = "a".repeat(MAX_EXPECT_VALUE_BYTES + 1);
  assert!(
    Expect::parse_values(["preview", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_count_limit = (0..MAX_EXPECTATIONS)
    .map(|index| format!("e{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Expect::parse(&at_count_limit).expect("32 expectations should parse");
  assert_eq!(parsed.unsupported().len(), MAX_EXPECTATIONS);

  let too_many = (0..=MAX_EXPECTATIONS)
    .map(|index| format!("e{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Expect::parse(&too_many).is_err(),
    "more than 32 expectations must be rejected"
  );
}
