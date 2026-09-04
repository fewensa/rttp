use rttp_protocol::rate_limit::MAX_RATE_LIMIT_VALUE_BYTES;
use rttp_server::server::{
  HttpRateLimitLimit, HttpRateLimitLimitItem, HttpRateLimitLimitParseError,
  HttpRateLimitParseError, HttpRateLimitRemaining, HttpRateLimitRemainingParseError,
  HttpRateLimitReset, HttpRateLimitResetParseError, HttpResponse,
};

const MAX_STRUCTURED_INTEGER: u64 = 999_999_999_999_999;

fn header_values<'a>(message: &'a str, name: &str) -> Vec<&'a str> {
  message
    .lines()
    .filter_map(|line| {
      let (header_name, value) = line.split_once(':')?;
      header_name
        .eq_ignore_ascii_case(name)
        .then_some(value.trim())
    })
    .collect()
}

#[test]
fn response_rate_limit_builders_serialize_typed_values_and_replace_fields() {
  let response = HttpResponse::ok("body")
    .header("RateLimit-Limit", "1")
    .header("ratelimit-limit", "2")
    .header("RateLimit-Remaining", "9")
    .header("ratelimit-remaining", "8")
    .header("RateLimit-Reset", "7")
    .header("ratelimit-reset", "6")
    .with_rate_limit_limit(HttpRateLimitLimit::new([
      HttpRateLimitLimitItem::new(100),
      HttpRateLimitLimitItem::new(50).with_window(3_600),
    ]))
    .expect("RateLimit-Limit declaration should parse")
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(0))
    .expect("RateLimit-Remaining declaration should parse")
    .with_rate_limit_reset(HttpRateLimitReset::new(0))
    .expect("RateLimit-Reset declaration should parse");

  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(
    vec!["100, 50;w=3600"],
    header_values(&serialized, "RateLimit-Limit")
  );
  assert_eq!(vec!["0"], header_values(&serialized, "RateLimit-Remaining"));
  assert_eq!(vec!["0"], header_values(&serialized, "RateLimit-Reset"));

  let limit = response
    .rate_limit_limit()
    .expect("RateLimit-Limit should parse")
    .expect("RateLimit-Limit should be present");
  assert_eq!(
    limit.items(),
    &[
      HttpRateLimitLimitItem::new(100),
      HttpRateLimitLimitItem::new(50).with_window(3_600),
    ]
  );
  assert_eq!(
    Some(HttpRateLimitRemaining::new(0)),
    response
      .rate_limit_remaining()
      .expect("RateLimit-Remaining should parse")
  );
  assert_eq!(
    Some(HttpRateLimitReset::new(0)),
    response
      .rate_limit_reset()
      .expect("RateLimit-Reset should parse")
  );
}

#[test]
fn response_rate_limit_accessors_flatten_list_fields_and_keep_duplicates() {
  let response = HttpResponse::ok("")
    .header("RateLimit-Limit", "100, 50;w=60")
    .header("ratelimit-limit", "100;w=3600");
  let limit = response
    .rate_limit_limit()
    .expect("RateLimit-Limit should parse")
    .expect("RateLimit-Limit should be present");

  assert_eq!(
    vec![(100, None), (50, Some(60)), (100, Some(3_600)),],
    limit
      .items()
      .iter()
      .map(|item| (item.value(), item.window()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn response_rate_limit_builders_round_trip_at_structured_field_boundaries() {
  let response = HttpResponse::ok("")
    .with_rate_limit_limit(HttpRateLimitLimit::new([HttpRateLimitLimitItem::new(
      MAX_STRUCTURED_INTEGER,
    )
    .with_window(MAX_STRUCTURED_INTEGER)]))
    .expect("maximum RateLimit-Limit integers should parse")
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(MAX_STRUCTURED_INTEGER))
    .expect("maximum RateLimit-Remaining integer should parse")
    .with_rate_limit_reset(HttpRateLimitReset::new(MAX_STRUCTURED_INTEGER))
    .expect("maximum RateLimit-Reset integer should parse");

  assert_eq!(
    Some(HttpRateLimitLimit::new([HttpRateLimitLimitItem::new(
      MAX_STRUCTURED_INTEGER,
    )
    .with_window(MAX_STRUCTURED_INTEGER)])),
    response
      .rate_limit_limit()
      .expect("RateLimit-Limit should parse")
  );
  assert_eq!(
    Some(HttpRateLimitRemaining::new(MAX_STRUCTURED_INTEGER)),
    response
      .rate_limit_remaining()
      .expect("RateLimit-Remaining should parse")
  );
  assert_eq!(
    Some(HttpRateLimitReset::new(MAX_STRUCTURED_INTEGER)),
    response
      .rate_limit_reset()
      .expect("RateLimit-Reset should parse")
  );

  let max_items = MAX_RATE_LIMIT_VALUE_BYTES.div_ceil(3);
  let limit = HttpRateLimitLimit::new(std::iter::repeat_n(
    HttpRateLimitLimitItem::new(0),
    max_items,
  ));
  assert_eq!(MAX_RATE_LIMIT_VALUE_BYTES, limit.header_value().len());
  let response = HttpResponse::ok("")
    .with_rate_limit_limit(limit)
    .expect("maximum RateLimit-Limit field value should parse");
  assert_eq!(
    max_items,
    response
      .rate_limit_limit()
      .expect("RateLimit-Limit should parse")
      .expect("RateLimit-Limit should be present")
      .items()
      .len()
  );
}

#[test]
fn response_rate_limit_builders_reject_invalid_values_without_replacing_fields() {
  let over_limit = MAX_STRUCTURED_INTEGER + 1;
  assert!(HttpResponse::ok("")
    .with_rate_limit_limit(HttpRateLimitLimit::new([HttpRateLimitLimitItem::new(
      over_limit,
    )]))
    .is_err());
  assert!(HttpResponse::ok("")
    .with_rate_limit_limit(HttpRateLimitLimit::new([
      HttpRateLimitLimitItem::new(1).with_window(over_limit)
    ]))
    .is_err());
  assert!(HttpResponse::ok("")
    .with_rate_limit_limit(HttpRateLimitLimit::new([]))
    .is_err());

  let max_items = MAX_RATE_LIMIT_VALUE_BYTES.div_ceil(3);
  assert!(HttpResponse::ok("")
    .with_rate_limit_limit(HttpRateLimitLimit::new(std::iter::repeat_n(
      HttpRateLimitLimitItem::new(0),
      max_items + 1
    ),))
    .is_err());

  assert!(HttpResponse::ok("")
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(over_limit))
    .is_err());
  assert!(HttpResponse::ok("")
    .with_rate_limit_reset(HttpRateLimitReset::new(over_limit))
    .is_err());

  let original = HttpResponse::ok("")
    .header("RateLimit-Limit", "1")
    .header("RateLimit-Remaining", "2")
    .header("RateLimit-Reset", "3");
  assert!(original
    .clone()
    .with_rate_limit_limit(HttpRateLimitLimit::new([]))
    .is_err());
  assert!(original
    .clone()
    .with_rate_limit_remaining(HttpRateLimitRemaining::new(over_limit))
    .is_err());
  assert!(original
    .clone()
    .with_rate_limit_reset(HttpRateLimitReset::new(over_limit))
    .is_err());

  let serialized = String::from_utf8(original.to_bytes()).expect("response should serialize");
  assert_eq!(vec!["1"], header_values(&serialized, "RateLimit-Limit"));
  assert_eq!(vec!["2"], header_values(&serialized, "RateLimit-Remaining"));
  assert_eq!(vec!["3"], header_values(&serialized, "RateLimit-Reset"));
}

#[test]
fn response_rate_limit_accessors_return_none_when_fields_are_absent() {
  let response = HttpResponse::ok("");
  assert_eq!(None, response.rate_limit_limit().expect("field is absent"));
  assert_eq!(
    None,
    response.rate_limit_remaining().expect("field is absent")
  );
  assert_eq!(None, response.rate_limit_reset().expect("field is absent"));
}

#[test]
fn response_rate_limit_singletons_reject_duplicates_and_preserve_raw_fields() {
  for (name, values) in [
    ("RateLimit-Remaining", ["1", "2"]),
    ("RateLimit-Reset", ["3", "4"]),
  ] {
    let response = HttpResponse::ok("")
      .header(name, values[0])
      .header(name.to_ascii_lowercase(), values[1]);
    let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

    match name {
      "RateLimit-Remaining" => assert!(response.rate_limit_remaining().is_err()),
      "RateLimit-Reset" => assert!(response.rate_limit_reset().is_err()),
      _ => unreachable!("only singleton fields are tested"),
    }
    assert_eq!(values, header_values(&serialized, name).as_slice());
  }
}

#[test]
fn response_rate_limit_accessors_reject_malformed_overflow_control_and_oversized_values() {
  let malformed = HttpResponse::ok("").header("RateLimit-Limit", "100, (50)");
  assert!(malformed.rate_limit_limit().is_err());
  let malformed_bytes = String::from_utf8(malformed.to_bytes()).expect("response should serialize");
  assert_eq!(
    vec!["100, (50)"],
    header_values(&malformed_bytes, "RateLimit-Limit")
  );

  let overflow = HttpResponse::ok("").header("RateLimit-Remaining", u64::MAX.to_string());
  assert!(overflow.rate_limit_remaining().is_err());
  let overflow_bytes = String::from_utf8(overflow.to_bytes()).expect("response should serialize");
  assert_eq!(
    vec![u64::MAX.to_string()],
    header_values(&overflow_bytes, "RateLimit-Remaining")
  );

  let controls = HttpResponse::ok("").header("RateLimit-Reset", "1\0");
  assert!(controls.rate_limit_reset().is_err());
  let controls_bytes = String::from_utf8(controls.to_bytes()).expect("response should serialize");
  assert_eq!(
    vec!["1\0"],
    header_values(&controls_bytes, "RateLimit-Reset")
  );

  let oversized_value = "1".repeat(64 * 1024 + 1);
  let oversized = HttpResponse::ok("").header("RateLimit-Limit", &oversized_value);
  assert!(oversized.rate_limit_limit().is_err());
  let oversized_bytes = String::from_utf8(oversized.to_bytes()).expect("response should serialize");
  assert_eq!(
    vec![oversized_value.as_str()],
    header_values(&oversized_bytes, "RateLimit-Limit")
  );
}

#[test]
fn response_rate_limit_facade_exports_shared_parse_errors() {
  let _: HttpRateLimitLimitParseError =
    HttpRateLimitLimit::parse("100, (50)").expect_err("malformed list should fail");
  let _: HttpRateLimitRemainingParseError =
    HttpRateLimitRemaining::parse("1, 2").expect_err("duplicate singleton should fail");
  let _: HttpRateLimitResetParseError =
    HttpRateLimitReset::parse("1, 2").expect_err("duplicate singleton should fail");
  let _: HttpRateLimitParseError =
    HttpRateLimitLimit::parse("100, (50)").expect_err("shared parse error should be usable");
}
