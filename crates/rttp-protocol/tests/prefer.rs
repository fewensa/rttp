use rttp_protocol::prefer::{
  Prefer, Preference, PreferenceApplied, PreferenceKind, MAX_PREFERENCES,
  MAX_PREFERENCE_PARAMETERS, MAX_PREFER_FIELD_BYTES, MAX_PREFER_VALUE_BYTES,
};

fn preference_names(prefer: &Prefer) -> Vec<&str> {
  prefer
    .preferences()
    .iter()
    .map(|preference| preference.name())
    .collect()
}

fn preference_names_applied(applied: &PreferenceApplied) -> Vec<&str> {
  applied
    .preferences()
    .iter()
    .map(|preference| preference.name())
    .collect()
}

fn field_at_limit() -> String {
  let mut members = (0..MAX_PREFERENCES)
    .map(|index| format!("p{index}={}", "x".repeat(2_040)))
    .collect::<Vec<_>>();
  let current_len = members.join(", ").len();
  members
    .last_mut()
    .expect("the preference bound must be non-zero")
    .push_str(&"x".repeat(MAX_PREFER_FIELD_BYTES - current_len));
  let field = members.join(", ");
  assert_eq!(field.len(), MAX_PREFER_FIELD_BYTES);
  assert!(
    members
      .last()
      .expect("the preference list must have a last member")
      .len()
      <= 2_040 + 8_192
  );
  field
}

fn preferences_at_limit() -> String {
  (0..MAX_PREFERENCES)
    .map(|index| format!("p{index}"))
    .collect::<Vec<_>>()
    .join(", ")
}

fn parameters_at_limit() -> String {
  format!(
    "extension{}",
    (0..MAX_PREFERENCE_PARAMETERS)
      .map(|index| format!("; p{index}=value"))
      .collect::<String>()
  )
}

#[test]
fn parses_ordered_preferences_across_fields() {
  let prefer = Prefer::parse_values([
    " return=minimal, wait=10; source=client ",
    " vendor=enabled; trace=\"a b\" ",
  ])
  .expect("ordered Prefer fields should parse");

  assert_eq!(preference_names(&prefer), ["return", "wait", "vendor"]);
  assert_eq!(prefer.preferences()[0].kind(), PreferenceKind::Return);
  assert_eq!(prefer.preferences()[1].kind(), PreferenceKind::Wait);
  assert_eq!(prefer.preferences()[2].kind(), PreferenceKind::Extension);
  assert_eq!(prefer.preferences()[1].parameters()[0].name(), "source");
  assert_eq!(
    prefer.preferences()[1].parameters()[0].value(),
    Some("client")
  );
  assert_eq!(prefer.preferences()[2].parameters()[0].name(), "trace");
  assert_eq!(prefer.preferences()[2].parameters()[0].value(), Some("a b"));
  assert_eq!(
    prefer.header_value(),
    "return=minimal, wait=10; source=client, vendor=enabled; trace=\"a b\""
  );
}

#[test]
fn validates_known_preferences_and_values() {
  let prefer = Prefer::parse("ReTuRn=representation, RESPOND-ASYNC, wait=10, HANDLING=lenient")
    .expect("known preferences should parse");
  assert_eq!(
    prefer
      .preferences()
      .iter()
      .map(Preference::kind)
      .collect::<Vec<_>>(),
    vec![
      PreferenceKind::Return,
      PreferenceKind::RespondAsync,
      PreferenceKind::Wait,
      PreferenceKind::Handling,
    ]
  );

  for value in [
    "return",
    "return=other",
    "respond-async=value",
    "wait",
    "wait=1.5",
    "wait=",
    "handling=relaxed",
  ] {
    assert!(
      Prefer::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
}

#[test]
fn preserves_quoted_escaping_and_round_trips() {
  let prefer =
    Prefer::parse(r#"vendor="say \"hi\" and \\path"; note="comma, semicolon"; flag; token=value"#)
      .expect("escaped quoted values should parse");

  assert_eq!(preference_names(&prefer), ["vendor"]);
  assert_eq!(
    prefer.preferences()[0].value(),
    Some(r#"say "hi" and \path"#)
  );
  assert_eq!(
    prefer.preferences()[0].parameters()[0].value(),
    Some("comma, semicolon")
  );
  assert_eq!(prefer.preferences()[0].parameters()[1].value(), None);
  assert_eq!(
    prefer.header_value(),
    r#"vendor="say \"hi\" and \\path"; note="comma, semicolon"; flag; token=value"#
  );

  let reparsed = Prefer::parse(prefer.header_value()).expect("formatted Prefer should reparse");
  assert_eq!(reparsed, prefer);
  assert_eq!(reparsed.header_value(), prefer.header_value());
}

#[test]
fn rejects_case_insensitive_duplicate_names_and_parameters() {
  for value in ["vendor=one, VENDOR=two", "vendor=one; trace=a; TRACE=b"] {
    assert!(
      Prefer::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }
  assert!(
    Prefer::parse_values(["vendor=one", "VENDOR=two"]).is_err(),
    "duplicate preference names across fields should be rejected"
  );

  assert!(
    PreferenceApplied::parse("return=minimal; source=cache").is_err(),
    "Preference-Applied parameters should be rejected"
  );
  assert!(
    PreferenceApplied::parse_values(["return=minimal", "RETURN=representation"]).is_err(),
    "duplicate Preference-Applied names across fields should be rejected"
  );
}

#[test]
fn rejects_malformed_separators_and_values() {
  for value in [
    "",
    " ",
    "\t",
    ",",
    ",vendor",
    "vendor,",
    "vendor,,trace",
    "vendor, ,trace",
    "vendor=",
    "vendor=one two",
    "vendor=one=two",
    "vendor;",
    "vendor; =value",
    "vendor;trace=",
    "vendor;trace=one two",
    "vendor;trace=value=extra",
    r#"vendor="unterminated"#,
    "vendor=\"bad\\\u{0}\"",
    "vendor=\"bad\u{7f}\"",
    "vendor\r\nX-Injected: yes",
  ] {
    assert!(
      Prefer::parse(value).is_err(),
      "{value:?} should be rejected"
    );
  }

  assert!(
    Prefer::parse_values([]).is_err(),
    "an empty Prefer field set should be rejected"
  );
  assert!(
    Prefer::parse_values(["vendor", " \t"]).is_err(),
    "a blank later Prefer field should be rejected"
  );
}

#[test]
fn enforces_field_value_preference_and_parameter_bounds() {
  assert_eq!(MAX_PREFER_FIELD_BYTES, 64 * 1024);
  assert_eq!(MAX_PREFER_VALUE_BYTES, 8 * 1024);
  assert_eq!(MAX_PREFERENCES, 32);
  assert_eq!(MAX_PREFERENCE_PARAMETERS, 256);

  let exact_value = format!("extension={}", "x".repeat(MAX_PREFER_VALUE_BYTES));
  let parsed = Prefer::parse(exact_value).expect("an 8 KiB preference value should parse");
  assert_eq!(
    parsed.preferences()[0].value().map(str::len),
    Some(8 * 1024)
  );
  assert!(
    Prefer::parse(format!(
      "extension={}x",
      "x".repeat(MAX_PREFER_VALUE_BYTES)
    ))
    .is_err(),
    "a preference value over 8 KiB should be rejected"
  );

  let exact_field = field_at_limit();
  let parsed = Prefer::parse(&exact_field).expect("an exact 64 KiB Prefer field should parse");
  assert_eq!(parsed.len(), MAX_PREFERENCES);
  assert!(
    Prefer::parse(format!("{exact_field}x")).is_err(),
    "a Prefer field over 64 KiB should be rejected"
  );
  assert!(
    Prefer::parse_values(["extension=one", &format!("{exact_field}x")]).is_err(),
    "an oversized later Prefer field should be rejected"
  );

  let exact_preferences = preferences_at_limit();
  assert_eq!(
    Prefer::parse(&exact_preferences)
      .expect("32 Prefer members should parse")
      .len(),
    MAX_PREFERENCES
  );
  assert!(
    Prefer::parse(format!("{exact_preferences}, over")).is_err(),
    "more than 32 Prefer members should be rejected"
  );

  let exact_parameters = parameters_at_limit();
  assert_eq!(
    Prefer::parse(&exact_parameters)
      .expect("256 preference parameters should parse")
      .preferences()[0]
      .parameters()
      .len(),
    MAX_PREFERENCE_PARAMETERS
  );
  let too_many_parameters = format!("{exact_parameters}; over=value");
  assert!(
    Prefer::parse(too_many_parameters).is_err(),
    "more than 256 preference parameters should be rejected"
  );
}

#[test]
fn preference_applied_validates_response_restrictions_and_bounds() {
  let applied = PreferenceApplied::parse_values(["return=minimal", "respond-async", "wait=10"])
    .expect("valid Preference-Applied values should parse");
  assert_eq!(
    preference_names_applied(&applied),
    ["return", "respond-async", "wait"]
  );
  assert_eq!(
    applied.header_value(),
    "return=minimal, respond-async, wait=10"
  );
  assert!(applied
    .preferences()
    .iter()
    .all(|preference| preference.parameters().is_empty()));

  for value in [
    "return",
    "return=other",
    "respond-async=value",
    "respond-async; source=server",
    "wait",
    "wait=10; source=server",
    "handling=relaxed",
    "vendor=enabled; trace=a",
  ] {
    assert!(
      PreferenceApplied::parse(value).is_err(),
      "{value:?} should be rejected in Preference-Applied"
    );
  }

  let exact_value = format!("extension={}", "x".repeat(MAX_PREFER_VALUE_BYTES));
  assert!(
    PreferenceApplied::parse(exact_value).is_ok(),
    "an 8 KiB Preference-Applied value should parse"
  );
  assert!(
    PreferenceApplied::parse(format!(
      "extension={}x",
      "x".repeat(MAX_PREFER_VALUE_BYTES)
    ))
    .is_err(),
    "an oversized Preference-Applied value should be rejected"
  );

  let exact_field = field_at_limit();
  assert_eq!(
    PreferenceApplied::parse(&exact_field)
      .expect("an exact 64 KiB Preference-Applied field should parse")
      .preferences()
      .len(),
    MAX_PREFERENCES
  );
  assert!(
    PreferenceApplied::parse(format!("{exact_field}x")).is_err(),
    "an oversized Preference-Applied field should be rejected"
  );

  let exact_preferences = preferences_at_limit();
  assert_eq!(
    PreferenceApplied::parse(&exact_preferences)
      .expect("32 Preference-Applied members should parse")
      .preferences()
      .len(),
    MAX_PREFERENCES
  );
  assert!(
    PreferenceApplied::parse(format!("{exact_preferences}, over")).is_err(),
    "more than 32 Preference-Applied members should be rejected"
  );
}
