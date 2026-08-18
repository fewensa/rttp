use std::time::{Duration, UNIX_EPOCH};

use rttp_protocol::warning::{
  Warning, CODE_MISCELLANEOUS_PERSISTENT_WARNING, CODE_RESPONSE_IS_STALE, MAX_WARNING_ITEMS,
  MAX_WARNING_TEXT_BYTES, MAX_WARNING_VALUE_BYTES,
};

#[test]
fn warning_parses_multiple_fields_quoted_text_and_optional_http_date() {
  let warning = Warning::parse_values([
    r#"110 - "Response is Stale""#,
    r#"299 example.com:80 "Deprecated API" "Wed, 21 Oct 2015 07:28:00 GMT""#,
  ])
  .expect("multiple Warning fields should parse");

  assert_eq!(warning.len(), 2);
  assert!(!warning.is_empty());
  assert_eq!(warning.items()[0].code(), CODE_RESPONSE_IS_STALE);
  assert_eq!(warning.items()[0].agent(), "-");
  assert_eq!(warning.items()[0].text(), "Response is Stale");
  assert_eq!(warning.items()[0].date(), None);
  assert_eq!(
    warning.items()[1].code(),
    CODE_MISCELLANEOUS_PERSISTENT_WARNING
  );
  assert_eq!(warning.items()[1].agent(), "example.com:80");
  assert_eq!(warning.items()[1].text(), "Deprecated API");
  assert_eq!(
    warning.items()[1].date(),
    Some(UNIX_EPOCH + Duration::from_secs(1_445_412_480))
  );
  assert_eq!(
    warning.header_value(),
    r#"110 - "Response is Stale", 299 example.com:80 "Deprecated API" "Wed, 21 Oct 2015 07:28:00 GMT""#
  );
  assert_eq!(
    warning.items()[0].warning_value(),
    r#"110 - "Response is Stale""#
  );
}

#[test]
fn warning_unescapes_quoted_text_and_keeps_commas_inside_quotes() {
  let warning = Warning::parse(r#"199 proxy "say \"hi\", please" "Wed, 21 Oct 2015 07:28:00 GMT""#)
    .expect("escaped quotes and comma-in-date should parse");

  assert_eq!(warning.items()[0].text(), r#"say "hi", please"#);
  assert_eq!(
    warning.items()[0].date(),
    Some(UNIX_EPOCH + Duration::from_secs(1_445_412_480))
  );
  assert_eq!(
    warning.header_value(),
    r#"199 proxy "say \"hi\", please" "Wed, 21 Oct 2015 07:28:00 GMT""#
  );
}

#[test]
fn warning_accepts_empty_quoted_text_ows_and_opaque_ipv6_agents() {
  let warning = Warning::parse(
    "110\t-\t\"\" , 299 [2001:db8::1]:443 \"Deprecated API\"\t\"Wed, 21 Oct 2015 07:28:00 GMT\"",
  )
  .expect("empty text, OWS, and IPv6 agents should parse");

  assert_eq!(warning.items()[0].text(), "");
  assert_eq!(warning.items()[1].agent(), "[2001:db8::1]:443");
  assert_eq!(
    warning.items()[1].date(),
    Some(UNIX_EPOCH + Duration::from_secs(1_445_412_480))
  );
}

#[test]
fn warning_parse_values_combines_and_inspects_every_field() {
  let mut values = [
    r#"110 - "Response is Stale""#,
    r#"111 cache "Revalidation Failed""#,
  ]
  .into_iter();
  let mut calls = 0;

  let warning = Warning::parse_values(std::iter::from_fn(|| {
    calls += 1;
    assert!(calls <= 3, "parser must inspect every list field");
    values.next()
  }))
  .expect("multiple fields form one warning-value list");

  assert_eq!(warning.len(), 2);
  assert_eq!(warning.items()[1].code(), 111);
}

#[test]
fn warning_rejects_malformed_quoting_invalid_codes_and_empty_members() {
  for value in [
    "",
    " ",
    "\t",
    r#",110 - "ok""#,
    r#"110 - "ok","#,
    r#"110 - "ok",,111 - "later""#,
    r#"11 - "too short""#,
    r#"1100 - "too long""#,
    r#"abc - "not digits""#,
    r#"11a - "mixed""#,
    r#"110-"missing space""#,
    r#"110 - missing-quotes"#,
    r#"110 - "unterminated"#,
    r#"110 - "bad\"#,
    r#"110 - "ok" not-a-date"#,
    r#"110 - "ok" "not a date""#,
    r#"110  "missing-agent""#,
  ] {
    assert!(Warning::parse(value).is_err(), "{value:?} must be rejected");
  }

  assert!(
    Warning::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn warning_enforces_value_text_and_item_bounds_including_later_fields() {
  assert!(Warning::parse("x".repeat(MAX_WARNING_VALUE_BYTES + 1)).is_err());
  assert!(Warning::parse(format!(
    r#"110 - "{}""#,
    "x".repeat(MAX_WARNING_TEXT_BYTES + 1)
  ))
  .is_err());
  assert!(
    Warning::parse_values([
      r#"110 - "ok""#,
      "x".repeat(MAX_WARNING_VALUE_BYTES + 1).as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );

  let too_many = (0..=MAX_WARNING_ITEMS)
    .map(|index| format!(r#"110 - "item{index}""#))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(Warning::parse(too_many).is_err());
}
