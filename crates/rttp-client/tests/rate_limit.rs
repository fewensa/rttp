use rttp_client::response::{
  RateLimitLimit, RateLimitLimitItem, RateLimitLimitParseError, RateLimitParseError,
  RateLimitRemaining, RateLimitRemainingParseError, RateLimitReset, RateLimitResetParseError,
  Response,
};
use rttp_client::types::RoUrl;

fn response_with_values(name: &str, values: &[&str]) -> Response {
  let mut raw = String::from("HTTP/1.1 200 OK\r\n");
  for value in values {
    raw.push_str(name);
    raw.push_str(": ");
    raw.push_str(value);
    raw.push_str("\r\n");
  }
  raw.push_str("Content-Length: 0\r\n\r\n");
  Response::new(RoUrl::with("https://example.test"), raw.into_bytes())
    .expect("response should remain parseable")
}

#[test]
fn response_rate_limit_limit_flattens_repeated_fields_and_preserves_order() {
  let response = response_with_values("RateLimit-Limit", &["100, 50;w=3600", "25;w=60, 100"]);
  let limit = response
    .rate_limit_limit()
    .expect("RateLimit-Limit should parse")
    .expect("RateLimit-Limit should be present");

  let _: &[RateLimitLimitItem] = limit.items();
  assert_eq!(
    vec![(100, None), (50, Some(3_600)), (25, Some(60)), (100, None),],
    limit
      .items()
      .iter()
      .map(|item| (item.value(), item.window()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn response_rate_limit_singletons_parse_zero_values() {
  let response = response_with_values("RateLimit-Remaining", &["0"]);
  assert_eq!(
    Some(RateLimitRemaining::new(0)),
    response
      .rate_limit_remaining()
      .expect("RateLimit-Remaining should parse")
  );
  assert!(response
    .rate_limit_remaining()
    .expect("RateLimit-Remaining should parse")
    .expect("RateLimit-Remaining should be present")
    .is_exhausted());

  let response = response_with_values("RateLimit-Reset", &["0"]);
  assert_eq!(
    Some(RateLimitReset::new(0)),
    response
      .rate_limit_reset()
      .expect("RateLimit-Reset should parse")
  );
  assert!(response
    .rate_limit_reset()
    .expect("RateLimit-Reset should parse")
    .expect("RateLimit-Reset should be present")
    .is_immediate());
}

#[test]
fn response_rate_limit_helpers_return_none_when_fields_are_absent() {
  let response = Response::new(
    RoUrl::with("https://example.test"),
    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".to_vec(),
  )
  .expect("response without RateLimit fields should parse");
  assert_eq!(None, response.rate_limit_limit().expect("field is absent"));
  assert_eq!(
    None,
    response.rate_limit_remaining().expect("field is absent")
  );
  assert_eq!(None, response.rate_limit_reset().expect("field is absent"));
}

#[test]
fn response_rate_limit_singletons_reject_duplicates_and_preserve_raw_headers() {
  for (name, values) in [
    ("RateLimit-Remaining", ["1", "2"]),
    ("RateLimit-Reset", ["3", "4"]),
  ] {
    let response = response_with_values(name, &values);
    match name {
      "RateLimit-Remaining" => assert!(response.rate_limit_remaining().is_err()),
      "RateLimit-Reset" => assert!(response.rate_limit_reset().is_err()),
      _ => unreachable!("only singleton fields are tested"),
    }
    assert_eq!(
      values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>(),
      response
        .header_values(name)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
    );
  }
}

#[test]
fn response_rate_limit_parse_failures_preserve_raw_fields() {
  let cases = [
    ("RateLimit-Limit", "100, (50)"),
    ("RateLimit-Limit", "100;w=18446744073709551616"),
    ("RateLimit-Remaining", "18446744073709551616"),
    ("RateLimit-Reset", "1\0"),
  ];
  for (name, value) in cases {
    let response = response_with_values(name, &[value]);
    match name {
      "RateLimit-Limit" => assert!(response.rate_limit_limit().is_err()),
      "RateLimit-Remaining" => assert!(response.rate_limit_remaining().is_err()),
      "RateLimit-Reset" => assert!(response.rate_limit_reset().is_err()),
      _ => unreachable!("only RateLimit fields are tested"),
    }
    assert_eq!(Some(&value.to_string()), response.header_value(name));
  }

  let value = "1".repeat(64 * 1024 + 1);
  let response = response_with_values("RateLimit-Reset", &[&value]);
  assert!(response.rate_limit_reset().is_err());
  assert_eq!(Some(&value), response.header_value("RateLimit-Reset"));
}

#[test]
fn response_rate_limit_response_exports_shared_parse_errors() {
  let _: RateLimitLimitParseError =
    RateLimitLimit::parse("100, (50)").expect_err("malformed list should fail");
  let _: RateLimitRemainingParseError =
    RateLimitRemaining::parse("1, 2").expect_err("duplicate singleton should fail");
  let _: RateLimitResetParseError =
    RateLimitReset::parse("1, 2").expect_err("duplicate singleton should fail");
  let _: RateLimitParseError =
    RateLimitLimit::parse("100, (50)").expect_err("shared parse error should be usable");
}
