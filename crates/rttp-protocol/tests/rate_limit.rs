use rttp_protocol::rate_limit::{
  RateLimitLimit, RateLimitLimitParseError, RateLimitRemaining, RateLimitRemainingParseError,
  RateLimitReset, RateLimitResetParseError, MAX_RATE_LIMIT_VALUE_BYTES,
};

#[test]
fn rate_limit_exposes_per_header_parse_error_aliases() {
  let _: Result<RateLimitLimit, RateLimitLimitParseError> = RateLimitLimit::parse("100");
  let _: Result<RateLimitRemaining, RateLimitRemainingParseError> = RateLimitRemaining::parse("42");
  let _: Result<RateLimitReset, RateLimitResetParseError> = RateLimitReset::parse("60");
}

#[test]
fn rate_limit_parses_singleton_response_values() {
  let limit = RateLimitLimit::parse("100").expect("RateLimit-Limit should parse");
  let remaining = RateLimitRemaining::parse("42").expect("RateLimit-Remaining should parse");
  let reset = RateLimitReset::parse("60").expect("RateLimit-Reset should parse");

  assert_eq!(limit.value(), 100);
  assert_eq!(remaining.value(), 42);
  assert_eq!(reset.value(), 60);
  assert_eq!(limit.header_value(), "100");
  assert_eq!(remaining.header_value(), "42");
  assert_eq!(reset.header_value(), "60");
}

#[test]
fn rate_limit_trims_optional_whitespace() {
  assert_eq!(
    RateLimitLimit::parse(" \t100\t ").expect("whitespace is allowed"),
    RateLimitLimit::new(100)
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
    "18446744073709551616",
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
fn rate_limit_rejects_duplicate_and_list_values() {
  assert!(RateLimitLimit::parse("100, 50").is_err());
  assert!(RateLimitRemaining::parse("42, 21").is_err());
  assert!(RateLimitReset::parse("60, 30").is_err());

  assert!(RateLimitLimit::parse_values(["100", "50"]).is_err());
  assert!(RateLimitRemaining::parse_values(["42", "21"]).is_err());
  assert!(RateLimitReset::parse_values(["60", "30"]).is_err());
}

#[test]
fn rate_limit_stops_after_the_first_duplicate_field() {
  let mut values = ["100", "50", "must not be inspected"].into_iter();
  let mut calls = 0;

  assert!(RateLimitLimit::parse_values(std::iter::from_fn(|| {
    calls += 1;
    assert!(
      calls <= 2,
      "parser must not inspect fields after a duplicate"
    );
    values.next()
  }))
  .is_err());
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
