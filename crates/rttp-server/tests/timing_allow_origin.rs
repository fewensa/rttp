use rttp_protocol::timing_allow_origin::{
  TimingAllowOrigin, MAX_TIMING_ALLOW_ORIGIN_ORIGINS, MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES,
};
use rttp_server::server::{HttpResponse, HttpTimingAllowOrigin, HttpTimingAllowOriginParseError};

fn header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
  response.lines().find_map(|line| {
    let (header_name, value) = line.split_once(':')?;
    header_name
      .eq_ignore_ascii_case(name)
      .then_some(value.trim())
  })
}

fn assert_invalid_raw_header(value: &str) {
  let response = HttpResponse::ok("").header("Timing-Allow-Origin", value);
  let parse_error: HttpTimingAllowOriginParseError = response
    .timing_allow_origin()
    .expect_err("invalid Timing-Allow-Origin should return its typed parse error");
  assert!(!parse_error.to_string().is_empty());

  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  assert_eq!(
    Some(value),
    header_value(&serialized, "Timing-Allow-Origin")
  );
}

#[test]
fn response_timing_allow_origin_accessor_handles_absence_wildcard_and_order() {
  let absent: Option<HttpTimingAllowOrigin> = HttpResponse::ok("")
    .timing_allow_origin()
    .expect("absent Timing-Allow-Origin should parse");
  assert_eq!(None, absent);

  let wildcard = HttpResponse::ok("")
    .with_timing_allow_origin("*")
    .expect("wildcard Timing-Allow-Origin should parse");
  assert!(wildcard
    .timing_allow_origin()
    .expect("wildcard Timing-Allow-Origin should parse")
    .expect("Timing-Allow-Origin should be present")
    .is_wildcard());

  let response = HttpResponse::ok("")
    .header(
      "Timing-Allow-Origin",
      "https://example.test, https://api.example.test",
    )
    .header("timing-allow-origin", "https://static.example.test");
  let metadata = response
    .timing_allow_origin()
    .expect("attached Timing-Allow-Origin should parse")
    .expect("Timing-Allow-Origin should be present");
  assert_eq!(
    metadata.origins(),
    [
      "https://example.test",
      "https://api.example.test",
      "https://static.example.test",
    ]
  );
}

#[test]
fn response_timing_allow_origin_builder_replaces_fields_with_canonical_value() {
  let response = HttpResponse::ok("")
    .header("Timing-Allow-Origin", "https://legacy.test")
    .header("timing-allow-origin", "https://stale.test")
    .with_timing_allow_origin("https://example.test,https://api.example.test")
    .expect("Timing-Allow-Origin should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");

  assert_eq!(
    1,
    serialized
      .lines()
      .filter(|line| {
        line
          .split_once(':')
          .is_some_and(|(name, _)| name.eq_ignore_ascii_case("Timing-Allow-Origin"))
      })
      .count()
  );
  assert_eq!(
    Some("https://example.test, https://api.example.test"),
    header_value(&serialized, "Timing-Allow-Origin")
  );

  let metadata = response
    .timing_allow_origin()
    .expect("canonical Timing-Allow-Origin should parse")
    .expect("Timing-Allow-Origin should be present");
  assert_eq!(
    metadata.origins(),
    ["https://example.test", "https://api.example.test"]
  );
}

#[test]
fn response_timing_allow_origin_builder_rejects_invalid_input_atomically() {
  let original = HttpResponse::ok("").header("Timing-Allow-Origin", "https://legacy.test");
  let result = original
    .clone()
    .with_timing_allow_origin("https://example.test/path");
  let _: HttpTimingAllowOriginParseError =
    result.expect_err("malformed Timing-Allow-Origin should be rejected");

  let serialized = String::from_utf8(original.to_bytes()).expect("response should serialize");
  assert_eq!(
    Some("https://legacy.test"),
    header_value(&serialized, "Timing-Allow-Origin")
  );
}

#[test]
fn response_timing_allow_origin_accessor_returns_typed_errors_without_rewriting_raw_headers() {
  assert_invalid_raw_header("https://example.test/path");
  assert_invalid_raw_header("https://example.test, https://example.test");
  assert_invalid_raw_header("https://example.test, https://api.example.test\u{7f}");

  let too_many = (0..=MAX_TIMING_ALLOW_ORIGIN_ORIGINS)
    .map(|index| format!("https://{index}.example.test"))
    .collect::<Vec<_>>()
    .join(", ");
  assert_invalid_raw_header(&too_many);

  let too_large = format!(
    "https://example.test,{}",
    "a".repeat(MAX_TIMING_ALLOW_ORIGIN_VALUE_BYTES)
  );
  assert_invalid_raw_header(&too_large);
}

#[test]
fn response_timing_allow_origin_roundtrips_wire_value_through_protocol_parser() {
  let response = HttpResponse::ok("body")
    .with_timing_allow_origin(
      "https://example.test, https://api.example.test, https://static.example.test",
    )
    .expect("Timing-Allow-Origin should parse");
  let serialized = String::from_utf8(response.to_bytes()).expect("response should serialize");
  let wire_value = header_value(&serialized, "Timing-Allow-Origin")
    .expect("serialized response should contain Timing-Allow-Origin");
  assert_eq!(
    "https://example.test, https://api.example.test, https://static.example.test",
    wire_value
  );

  let protocol_metadata = TimingAllowOrigin::parse(wire_value)
    .expect("serialized Timing-Allow-Origin should parse through the protocol facade");
  let response_metadata = response
    .timing_allow_origin()
    .expect("serialized Timing-Allow-Origin should parse through the server facade")
    .expect("Timing-Allow-Origin should be present");
  assert_eq!(response_metadata.origins(), protocol_metadata.origins());
}
