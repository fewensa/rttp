use rttp_protocol::nel::{Nel, MAX_NEL_DEPTH, MAX_NEL_MEMBERS, MAX_NEL_VALUE_BYTES};

#[test]
fn nel_parses_full_policy_with_checked_members() {
  let nel = Nel::parse(
    r#"{"report_to":"network-errors","max_age":2592000,"include_subdomains":true,"success_fraction":0.1,"failure_fraction":1.0}"#,
  )
  .expect("full NEL policy should parse");

  assert_eq!(nel.max_age(), 2592000);
  assert_eq!(nel.report_to(), Some("network-errors"));
  assert_eq!(nel.include_subdomains(), Some(true));
  assert_eq!(nel.success_fraction(), Some(0.1));
  assert_eq!(nel.failure_fraction(), Some(1.0));
  assert_eq!(nel.unknown_members(), &[]);
}

#[test]
fn nel_accepts_whitespace_and_normalizes_known_member_order() {
  let nel =
    Nel::parse(" { \"failure_fraction\" : 0.5 , \"max_age\" : 60 , \"report_to\" : \"errors\" } ")
      .expect("whitespace-tolerant JSON should parse");

  assert_eq!(nel.max_age(), 60);
  assert_eq!(nel.report_to(), Some("errors"));
  assert_eq!(nel.failure_fraction(), Some(0.5));
  assert_eq!(
    nel.header_value(),
    r#"{"max_age":60,"report_to":"errors","failure_fraction":0.5}"#
  );
}

#[test]
fn nel_round_trips_typed_members_through_header_value() {
  let nel = Nel::parse(
    r#"{"report_to":"network-errors","max_age":2592000,"include_subdomains":true,"success_fraction":0.25,"failure_fraction":0.75}"#,
  )
  .expect("full NEL policy should parse");

  let reparsed = Nel::parse(nel.header_value()).expect("header_value should reparse");
  assert_eq!(nel, reparsed);
  assert_eq!(
    nel.header_value(),
    r#"{"max_age":2592000,"report_to":"network-errors","include_subdomains":true,"success_fraction":0.25,"failure_fraction":0.75}"#
  );
}

#[test]
fn nel_preserves_unknown_members_verbatim_without_policy_semantics() {
  let nel = Nel::parse(r#"{"max_age":1,"x":{"a":[1,2,"s"]},"y":null,"z":-1.5e2,"w":true}"#)
    .expect("unknown members should parse as raw metadata");

  assert_eq!(nel.unknown_members().len(), 4);
  assert_eq!(nel.unknown_members()[0].name(), "x");
  assert_eq!(nel.unknown_members()[0].value(), r#"{"a":[1,2,"s"]}"#);
  assert_eq!(nel.unknown_members()[1].name(), "y");
  assert_eq!(nel.unknown_members()[1].value(), "null");
  assert_eq!(nel.unknown_members()[2].name(), "z");
  assert_eq!(nel.unknown_members()[2].value(), "-1.5e2");
  assert_eq!(nel.unknown_members()[3].name(), "w");
  assert_eq!(nel.unknown_members()[3].value(), "true");
  assert_eq!(
    nel.header_value(),
    r#"{"max_age":1,"x":{"a":[1,2,"s"]},"y":null,"z":-1.5e2,"w":true}"#
  );
}

#[test]
fn nel_unescapes_json_strings_including_surrogate_pairs() {
  let nel = Nel::parse(r#"{"max_age":1,"report_to":"a\"b\\c\nd\u0041\u00e9\ud83d\ude00"}"#)
    .expect("JSON escapes should parse");

  assert_eq!(nel.report_to(), Some("a\"b\\c\ndA\u{00e9}\u{1f600}"));
  assert_eq!(
    Nel::parse(nel.header_value())
      .expect("escaped header_value should reparse")
      .report_to(),
    Some("a\"b\\c\ndA\u{00e9}\u{1f600}")
  );
}

#[test]
fn nel_rejects_malformed_json() {
  for value in [
    "",
    " ",
    "\t",
    "{",
    "}",
    r#"{"max_age":}"#,
    r#"{"max_age":1"#,
    r#"{"max_age":1,}"#,
    r#"{max_age:1}"#,
    r"{'max_age':1}",
    r#"{"max_age":01}"#,
    r#"{"max_age":+1}"#,
    r#"{"max_age":1.}"#,
    r#"{"max_age":.5}"#,
    r#"{"max_age":1e}"#,
    r#"{"max_age":1e+}"#,
    r#"{"max_age":1 "x"}"#,
    r#"{"max_age":1}{}"#,
    r#"{"max_age":1} x"#,
    r#"{"max_age":1,"report_to""#,
    r#"{"max_age":1,"report_to":}"#,
    r#"{"max_age":1,,}"#,
    r#"{"max_age":1,"a":,"b":2}"#,
    r#"{"max_age":"1"}"#,
    r#"{"max_age":null}"#,
    r#"{"max_age":[1]}"#,
    r#"{"max_age":{"v":1}}"#,
    r#"{"max_age":true}"#,
    r#"{"max_age":false}"#,
    r#"{"max_age":"unterminated}"#,
    r#"{"max_age":1,"a":"\x"}"#,
    r#"{"max_age":1,"a":"\u12g4"}"#,
    r#"{"max_age":1,"a":"\ud800"}"#,
    r#"{"max_age":1,"a":"\udc00"}"#,
    r#"{"max_age":1,"a":"\ud800\u0041"}"#,
    r#"{"max_age":1,"a":"tab	control"}"#,
    "{\"max_age\":1,\"x\":{ \"a\":\r\n1 }}",
    "{\"max_age\":1,\"x\":[1,\r\n2]}",
    "{\"max_age\":1}\r\n",
    "{\"max_age\":1}\n",
    "{\r\n\"max_age\":1}",
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn nel_rejects_raw_crlf_in_field_value_and_header_value_stays_clean() {
  for value in [
    "{\"max_age\":1,\"x\":{ \"a\":\r\n1 }}",
    "{\"max_age\":1,\"x\":[1,\r\n2]}",
    "{\"max_age\":1}\r\n",
    "{\"max_age\":1}\n",
    "{\"max_age\":1,\"x\":1}\r",
    "{\r\n\"max_age\":1}",
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }

  let nel = Nel::parse(r#"{"max_age":1,"x":{"a":[1,2],"b":null},"y":"ok"}"#)
    .expect("valid policy with unknown members should parse");
  let header_value = nel.header_value();
  assert!(
    !header_value.contains('\r') && !header_value.contains('\n'),
    "header_value must not contain raw CR or LF: {header_value:?}"
  );
}

#[test]
fn nel_rejects_non_object_top_level_and_duplicate_header_fields() {
  for value in [
    "[]", "[1]", r#"["x"]"#, r#""x""#, "42", "-1", "true", "false", "null",
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    Nel::parse_values([r#"{"max_age":1}"#, r#"{"max_age":1}"#,]).is_err(),
    "duplicate NEL header fields must be rejected"
  );
  assert!(
    Nel::parse_values([]).is_err(),
    "empty NEL field sets must be rejected"
  );
}

#[test]
fn nel_rejects_invalid_member_types() {
  for value in [
    r#"{"report_to":5,"max_age":1}"#,
    r#"{"report_to":true,"max_age":1}"#,
    r#"{"report_to":null,"max_age":1}"#,
    r#"{"report_to":[],"max_age":1}"#,
    r#"{"report_to":{},"max_age":1}"#,
    r#"{"include_subdomains":"true","max_age":1}"#,
    r#"{"include_subdomains":1,"max_age":1}"#,
    r#"{"include_subdomains":null,"max_age":1}"#,
    r#"{"success_fraction":"0.5","max_age":1}"#,
    r#"{"success_fraction":null,"max_age":1}"#,
    r#"{"success_fraction":true,"max_age":1}"#,
    r#"{"failure_fraction":[],"max_age":1}"#,
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn nel_rejects_duplicate_singleton_members() {
  for value in [
    r#"{"max_age":1,"max_age":2}"#,
    r#"{"report_to":"a","report_to":"b","max_age":1}"#,
    r#"{"include_subdomains":true,"include_subdomains":false,"max_age":1}"#,
    r#"{"success_fraction":0.1,"success_fraction":0.2,"max_age":1}"#,
    r#"{"failure_fraction":1.0,"failure_fraction":0.5,"max_age":1}"#,
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn nel_requires_max_age_and_rejects_non_integer_or_out_of_range_values() {
  for value in [
    "{}",
    r#"{"report_to":"network-errors"}"#,
    r#"{"max_age":-1}"#,
    r#"{"max_age":-0}"#,
    r#"{"max_age":1.5}"#,
    r#"{"max_age":1.0}"#,
    r#"{"max_age":1e3}"#,
    r#"{"max_age":1E2}"#,
    r#"{"max_age":18446744073709551616}"#,
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }
  assert_eq!(
    Nel::parse(r#"{"max_age":18446744073709551615}"#)
      .expect("u64::MAX should be accepted")
      .max_age(),
    u64::MAX
  );
}

#[test]
fn nel_rejects_non_finite_and_out_of_range_fractions() {
  for value in [
    r#"{"success_fraction":1.5,"max_age":1}"#,
    r#"{"success_fraction":-0.1,"max_age":1}"#,
    r#"{"success_fraction":1e999,"max_age":1}"#,
    r#"{"success_fraction":-1e999,"max_age":1}"#,
    r#"{"success_fraction":2,"max_age":1}"#,
    r#"{"failure_fraction":1.5,"max_age":1}"#,
    r#"{"failure_fraction":-0.01,"max_age":1}"#,
    r#"{"failure_fraction":1e999,"max_age":1}"#,
  ] {
    assert!(Nel::parse(value).is_err(), "{value:?} must be rejected");
  }

  let nel = Nel::parse(r#"{"success_fraction":0,"failure_fraction":1,"max_age":1}"#)
    .expect("inclusive fraction bounds should parse");
  assert_eq!(nel.success_fraction(), Some(0.0));
  assert_eq!(nel.failure_fraction(), Some(1.0));
}

#[test]
fn nel_enforces_value_member_count_and_depth_bounds() {
  let oversized = format!("{{\"max_age\":1{}}}", " ".repeat(MAX_NEL_VALUE_BYTES));
  assert!(
    Nel::parse(oversized).is_err(),
    "an oversized NEL field value must be rejected"
  );

  let too_many = format!(
    "{{\"max_age\":1,{}}}",
    (0..=MAX_NEL_MEMBERS)
      .map(|index| format!("\"k{index}\":0"))
      .collect::<Vec<_>>()
      .join(",")
  );
  assert!(Nel::parse(too_many).is_err());

  let nested = format!(
    "{{\"max_age\":1,\"a\":{}0{}}}",
    "[".repeat(MAX_NEL_DEPTH + 8),
    "]".repeat(MAX_NEL_DEPTH + 8)
  );
  assert!(
    Nel::parse(nested).is_err(),
    "excessive nesting must be rejected"
  );

  let acceptable = format!(
    "{{\"max_age\":1,\"a\":{}0{}}}",
    "[".repeat(MAX_NEL_DEPTH / 2),
    "]".repeat(MAX_NEL_DEPTH / 2)
  );
  assert!(
    Nel::parse(acceptable).is_ok(),
    "bounded nesting should parse"
  );
}
