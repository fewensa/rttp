use rttp_protocol::surrogate_control::{
  SurrogateControl, MAX_SURROGATE_CONTROL_AGGREGATE_BYTES, MAX_SURROGATE_CONTROL_DIRECTIVES,
  MAX_SURROGATE_CONTROL_DIRECTIVE_VALUE_BYTES, MAX_SURROGATE_CONTROL_VALUE_BYTES,
};

#[test]
fn parses_repeated_fields_and_quoted_extension_directives() {
  let metadata = SurrogateControl::parse_values([
    "max-age=600, content=\"ESI/1.0\", surrogate-key=\"article 42\"",
    "stale-while-revalidate=30",
  ])
  .expect("Surrogate-Control should parse");

  assert_eq!(metadata.len(), 4);
  assert_eq!(metadata.directives()[0].name(), "max-age");
  assert_eq!(metadata.directives()[0].value(), Some("600"));
  assert_eq!(metadata.directives()[1].name(), "content");
  assert_eq!(metadata.directives()[1].value(), Some("ESI/1.0"));
  assert_eq!(metadata.directives()[2].name(), "surrogate-key");
  assert_eq!(metadata.directives()[2].value(), Some("article 42"));
  assert_eq!(
    metadata.header_value(),
    "max-age=600, content=\"ESI/1.0\", surrogate-key=\"article 42\", stale-while-revalidate=30"
  );
}

#[test]
fn rejects_malformed_surrogate_control_values() {
  for value in [
    "",
    "max-age=",
    "max-age=not a token",
    "content=\"unterminated",
    "max-age=60\r\nCache-Control: max-age=60",
  ] {
    assert!(
      SurrogateControl::parse(value).is_err(),
      "{value:?} should fail"
    );
  }
}

#[test]
fn rejects_duplicate_directives_case_insensitively_across_fields() {
  assert!(SurrogateControl::parse("max-age=60, Max-Age=120").is_err());
  assert!(SurrogateControl::parse_values(["content=ESI/1.0", "CONTENT=ESI/1.1"]).is_err());
}

#[test]
fn enforces_surrogate_control_bounds() {
  assert!(SurrogateControl::parse("x".repeat(MAX_SURROGATE_CONTROL_VALUE_BYTES + 1)).is_err());
  assert!(SurrogateControl::parse(format!(
    "x={}",
    "x".repeat(MAX_SURROGATE_CONTROL_DIRECTIVE_VALUE_BYTES + 1)
  ))
  .is_err());
  assert!(SurrogateControl::parse(
    (0..=MAX_SURROGATE_CONTROL_DIRECTIVES)
      .map(|index| format!("d{index}"))
      .collect::<Vec<_>>()
      .join(","),
  )
  .is_err());

  let first = format!(
    "a={}",
    "x".repeat((MAX_SURROGATE_CONTROL_AGGREGATE_BYTES / 2) - 2)
  );
  let second = format!(
    "b={}",
    "x".repeat((MAX_SURROGATE_CONTROL_AGGREGATE_BYTES / 2) - 1)
  );
  assert!(SurrogateControl::parse_values([first.as_str(), second.as_str()]).is_err());
}
