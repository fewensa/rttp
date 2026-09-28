use rttp_protocol::rate_limit::{
  RateLimitLimit, RateLimitLimitItem, RateLimitLimitParseError, RateLimitRemaining,
  RateLimitRemainingParseError, RateLimitReset, RateLimitResetParseError, MAX_RATE_LIMIT_INTEGER,
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
fn rate_limit_accepts_list_separators_and_valid_window_parameters() {
  let limit = RateLimitLimit::parse("\t100; w=60\t,\t50\t")
    .expect("SP/HTAB OWS around list members and parameters should parse");
  assert_eq!(
    limit,
    RateLimitLimit::new([
      RateLimitLimitItem::new(100).with_window(60),
      RateLimitLimitItem::new(50),
    ])
  );
  assert_eq!("100;w=60, 50", limit.header_value());

  for value in ["100,,50", ",100", "100,", "100;w", "100;w=1.5"] {
    assert!(
      RateLimitLimit::parse(value).is_err(),
      "malformed list or parameter {value:?} must be rejected"
    );
  }
}

#[test]
fn rate_limit_enforces_structured_integer_bounds() {
  let maximum = MAX_RATE_LIMIT_INTEGER.to_string();
  let over_maximum = (MAX_RATE_LIMIT_INTEGER + 1).to_string();

  assert_eq!(
    RateLimitLimit::parse(format!("{maximum};w={maximum}"))
      .expect("maximum Structured Fields integers should parse")
      .header_value(),
    format!("{maximum};w={maximum}")
  );
  assert_eq!(
    RateLimitRemaining::parse(&maximum)
      .expect("maximum Structured Fields integer should parse")
      .header_value(),
    maximum
  );
  assert_eq!(
    RateLimitReset::parse(&maximum)
      .expect("maximum Structured Fields integer should parse")
      .header_value(),
    maximum
  );

  for value in [
    over_maximum.clone(),
    format!("1;w={over_maximum}"),
    format!("1;w={}", u64::MAX),
  ] {
    assert!(
      RateLimitLimit::parse(&value).is_err(),
      "over-limit RateLimit-Limit value {value:?} must be rejected"
    );
  }
  assert!(RateLimitRemaining::parse(&over_maximum).is_err());
  assert!(RateLimitReset::parse(&over_maximum).is_err());
}

#[test]
fn rate_limit_rejects_unsupported_and_duplicate_parameters() {
  for value in ["100;foo=1", "100;w=1;w=2", "100;w=true"] {
    assert!(
      RateLimitLimit::parse(value).is_err(),
      "unsupported or duplicate parameter {value:?} must be rejected"
    );
  }
}

#[test]
fn rate_limit_rejects_forbidden_controls_but_accepts_ows() {
  for control in [0x00_u8, 0x0b, 0x0c, 0x0d, 0x0e, 0x1f, 0x7f] {
    let value = format!("1{}", char::from(control));
    assert!(RateLimitLimit::parse(&value).is_err());
    assert!(RateLimitRemaining::parse(&value).is_err());
    assert!(RateLimitReset::parse(&value).is_err());
  }
  assert_eq!(1, RateLimitRemaining::parse("\t1 ").unwrap().value());
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

  let oversized = "1".repeat(MAX_RATE_LIMIT_VALUE_BYTES + 1);
  let error = RateLimitRemaining::parse_values(["1", "2", &oversized])
    .expect_err("every duplicate field must be validated");
  assert!(error.to_string().contains("too large"));
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
fn rate_limit_remaining_positive_state_helper() {
  assert!(!RateLimitRemaining::new(0).has_quota());
  assert!(!RateLimitRemaining::parse("0")
    .expect("zero should parse")
    .has_quota());

  assert!(RateLimitRemaining::new(1).has_quota());
  assert!(RateLimitRemaining::new(42).has_quota());
  assert!(RateLimitRemaining::parse("1")
    .expect("positive should parse")
    .has_quota());
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
fn rate_limit_reset_positive_state_helper() {
  assert!(!RateLimitReset::new(0).is_delayed());
  assert!(!RateLimitReset::parse("0")
    .expect("zero should parse")
    .is_delayed());

  assert!(RateLimitReset::new(1).is_delayed());
  assert!(RateLimitReset::new(60).is_delayed());
  assert!(RateLimitReset::parse("1")
    .expect("positive should parse")
    .is_delayed());
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

#[test]
fn rate_limit_bounds_combined_limit_fields_and_round_trips_canonically() {
  let max_items = MAX_RATE_LIMIT_VALUE_BYTES.div_ceil(3);
  let values = std::iter::repeat_n("0", max_items).collect::<Vec<_>>();
  let limit = RateLimitLimit::parse_values(values.iter().copied())
    .expect("canonical combined list at the byte limit should parse");
  assert_eq!(MAX_RATE_LIMIT_VALUE_BYTES, limit.header_value().len());
  assert_eq!(limit, RateLimitLimit::parse(limit.header_value()).unwrap());

  let too_many = std::iter::repeat_n("0", max_items + 1).collect::<Vec<_>>();
  assert!(
    RateLimitLimit::parse_values(too_many.iter().copied()).is_err(),
    "combined list serialization must stay within the aggregate bound"
  );
}
