use rttp_protocol::rate_limit::{
  RateLimitLimit, RateLimitLimitItem, RateLimitLimitParseError, RateLimitRemaining,
  RateLimitRemainingParseError, RateLimitReset, RateLimitResetParseError,
  MAX_RATE_LIMIT_VALUE_BYTES,
};

#[test]
fn rate_limit_exposes_per_header_parse_error_aliases() {
  let _: Result<RateLimitLimit, RateLimitLimitParseError> = RateLimitLimit::parse("100");
  let _: Result<RateLimitRemaining, RateLimitRemainingParseError> = RateLimitRemaining::parse("42");
  let _: Result<RateLimitReset, RateLimitResetParseError> = RateLimitReset::parse("60");
}

#[test]
fn rate_limit_parses_response_values() {
  let limit =
    RateLimitLimit::parse("100, 50;w=3600").expect("RateLimit-Limit structured list should parse");
  let remaining = RateLimitRemaining::parse("42").expect("RateLimit-Remaining should parse");
  let reset = RateLimitReset::parse("60").expect("RateLimit-Reset should parse");

  assert_eq!(
    limit.items(),
    &[
      RateLimitLimitItem::new(100),
      RateLimitLimitItem::new(50).with_window(3600),
    ]
  );
  assert_eq!(remaining.value(), 42);
  assert_eq!(reset.value(), 60);
  assert_eq!(limit.header_value(), "100, 50;w=3600");
  assert_eq!(remaining.header_value(), "42");
  assert_eq!(reset.header_value(), "60");
}

#[test]
fn rate_limit_trims_optional_whitespace() {
  assert_eq!(
    RateLimitLimit::parse(" \t100;w=60\t ").expect("whitespace is allowed"),
    RateLimitLimit::new([RateLimitLimitItem::new(100).with_window(60)])
  );
  assert_eq!(
    RateLimitRemaining::parse(" 42 ").expect("whitespace is allowed"),
    RateLimitRemaining::new(42)
  );
  assert_eq!(
    RateLimitReset::parse("\t60 ").expect("whitespace is allowed"),
    RateLimitReset::new(60)
  );
}

#[test]
fn rate_limit_rejects_malformed_numeric_values() {
  for value in [
    "",
    "-1",
    "+1",
    "1.5",
    "one",
    "1000000000000000",
    "1\r\nX: y",
  ] {
    assert!(
      RateLimitLimit::parse(value).is_err(),
      "{value:?} must be rejected"
    );
    assert!(
      RateLimitRemaining::parse(value).is_err(),
      "{value:?} must be rejected"
    );
    assert!(
      RateLimitReset::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn rate_limit_rejects_invalid_list_and_duplicate_values() {
  assert!(RateLimitLimit::parse("100, (50)").is_err());
  assert!(RateLimitRemaining::parse("42, 21").is_err());
  assert!(RateLimitReset::parse("60, 30").is_err());

  assert_eq!(
    RateLimitLimit::parse_values(["100", "50;w=3600"])
      .expect("multiple header fields form one structured list"),
    RateLimitLimit::new([
      RateLimitLimitItem::new(100),
      RateLimitLimitItem::new(50).with_window(3600),
    ])
  );
  assert!(RateLimitRemaining::parse_values(["42", "21"]).is_err());
  assert!(RateLimitReset::parse_values(["60", "30"]).is_err());
}

#[test]
fn rate_limit_combines_all_list_fields() {
  let mut values = ["100", "50"].into_iter();
  let mut calls = 0;

  assert_eq!(
    RateLimitLimit::parse_values(std::iter::from_fn(|| {
      calls += 1;
      assert!(calls <= 3, "parser must inspect every list field");
      values.next()
    }))
    .expect("multiple fields form one structured list"),
    RateLimitLimit::new([RateLimitLimitItem::new(100), RateLimitLimitItem::new(50),])
  );
}

#[test]
fn rate_limit_remaining_zero_state_helper() {
  assert!(RateLimitRemaining::new(0).is_exhausted());
  assert!(RateLimitRemaining::parse("0")
    .expect("zero should parse")
    .is_exhausted());
  assert_eq!(RateLimitRemaining::new(0).header_value(), "0");

  assert!(!RateLimitRemaining::new(1).is_exhausted());
  assert!(!RateLimitRemaining::new(42).is_exhausted());
  assert!(!RateLimitRemaining::parse("1")
    .expect("positive should parse")
    .is_exhausted());
}

#[test]
fn rate_limit_reset_zero_state_helper() {
  assert!(RateLimitReset::new(0).is_immediate());
  assert!(RateLimitReset::parse("0")
    .expect("zero should parse")
    .is_immediate());
  assert_eq!(RateLimitReset::new(0).header_value(), "0");

  assert!(!RateLimitReset::new(1).is_immediate());
  assert!(!RateLimitReset::new(60).is_immediate());
  assert!(!RateLimitReset::parse("1")
    .expect("positive should parse")
    .is_immediate());
}

#[test]
fn rate_limit_enforces_value_bounds_for_every_field() {
  let oversized = "1".repeat(MAX_RATE_LIMIT_VALUE_BYTES + 1);

  assert!(RateLimitLimit::parse(&oversized).is_err());
  assert!(RateLimitRemaining::parse(&oversized).is_err());
  assert!(RateLimitReset::parse(&oversized).is_err());

  assert!(
    RateLimitLimit::parse_values(["100", oversized.as_str()]).is_err(),
    "an oversized duplicate must not bypass validation"
  );
}
