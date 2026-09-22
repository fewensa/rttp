use rttp_protocol::alt_svc::{
  AltSvc, MAX_ALT_SVC_ALTERNATIVES, MAX_ALT_SVC_PARAMETERS, MAX_ALT_SVC_PARAMETER_VALUE_BYTES,
  MAX_ALT_SVC_VALUE_BYTES,
};

#[test]
fn parses_ordered_alternatives_across_fields() {
  let alt_svc = AltSvc::parse_values([
    r#"h3=":443"; ma=60; persist=1; region=us-east, h2="alt.example:8443"; note="two words""#,
    r#"h3-29="[2001:db8::1]:443"; persist=0"#,
  ])
  .expect("combined Alt-Svc fields should parse");

  assert!(!alt_svc.is_clear());
  assert_eq!(3, alt_svc.len());
  assert_eq!(
    vec!["h3", "h2", "h3-29"],
    alt_svc
      .alternatives()
      .iter()
      .map(|alternative| alternative.protocol_id())
      .collect::<Vec<_>>()
  );
  assert_eq!(":443", alt_svc.alternatives()[0].authority());
  assert_eq!(Some(60), alt_svc.alternatives()[0].max_age());
  assert_eq!(Some(true), alt_svc.alternatives()[0].persist());
  assert_eq!("alt.example:8443", alt_svc.alternatives()[1].authority());
  assert_eq!(
    vec![("note", Some("two words"))],
    alt_svc.alternatives()[1]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!("[2001:db8::1]:443", alt_svc.alternatives()[2].authority());
  assert_eq!(Some(false), alt_svc.alternatives()[2].persist());
  assert_eq!(
    r#"h3=":443"; ma=60; persist=1; region=us-east, h2="alt.example:8443"; note="two words", h3-29="[2001:db8::1]:443"; persist=0"#,
    alt_svc.header_value()
  );
  assert_eq!(
    alt_svc,
    AltSvc::parse(alt_svc.header_value()).expect("canonical Alt-Svc should round-trip")
  );
}

#[test]
fn clear_is_exclusive_across_fields_and_list_members() {
  let clear = AltSvc::parse_values([" \tclear\t"]).expect("clear should parse");
  assert!(clear.is_clear());
  assert!(clear.is_empty());
  assert_eq!("clear", clear.header_value());

  for values in [
    vec!["clear", "h3=\":443\""],
    vec!["h3=\":443\"", "clear"],
    vec!["clear", "clear"],
    vec!["clear, h3=\":443\""],
    vec!["h3=\":443\", clear"],
    vec!["clear, clear"],
  ] {
    assert!(
      AltSvc::parse_values(values.iter().copied()).is_err(),
      "clear must be exclusive in {values:?}"
    );
  }
}

#[test]
fn unescapes_quoted_authorities_and_round_trips_ipv6_forms() {
  let alt_svc = AltSvc::parse(r#"h3="\[2001:db8::1\]\:8443"; note="say \"hi\" and \\path""#)
    .expect("escaped authority and extension values should parse");

  assert_eq!("[2001:db8::1]:8443", alt_svc.alternatives()[0].authority());
  assert_eq!(
    Some("say \"hi\" and \\path"),
    alt_svc.alternatives()[0].parameters()[0].value()
  );
  assert_eq!(
    r#"h3="[2001:db8::1]:8443"; note="say \"hi\" and \\path""#,
    alt_svc.header_value()
  );
  assert_eq!(
    alt_svc,
    AltSvc::parse(alt_svc.header_value()).expect("escaped authority should round-trip")
  );
}

#[test]
fn validates_ma_persist_and_duplicate_parameters() {
  let valid = AltSvc::parse(format!("h3=\":443\"; ma={}; persist=0", u64::MAX))
    .expect("maximum ma and persist=0 should parse");
  assert_eq!(Some(u64::MAX), valid.alternatives()[0].max_age());
  assert_eq!(Some(false), valid.alternatives()[0].persist());

  for value in [
    "h3=\":443\"; ma",
    "h3=\":443\"; ma=",
    "h3=\":443\"; ma=forever",
    "h3=\":443\"; ma=-1",
    "h3=\":443\"; ma=\"60\"",
    "h3=\":443\"; ma=18446744073709551616",
    "h3=\":443\"; persist",
    "h3=\":443\"; persist=",
    "h3=\":443\"; persist=2",
    "h3=\":443\"; persist=true",
    "h3=\":443\"; persist=\"1\"",
    "h3=\":443\"; ma=1; MA=2",
    "h3=\":443\"; persist=0; PERSIST=1",
    "h3=\":443\"; extension=one; EXTENSION=two",
  ] {
    assert!(AltSvc::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn preserves_extension_token_and_quoted_forms() {
  let alt_svc = AltSvc::parse(r#"h3=":443"; token=abc; quoted="two words, \"quoted\""; empty="""#)
    .expect("extension parameter forms should parse");

  assert_eq!(
    vec![
      ("token", Some("abc")),
      ("quoted", Some("two words, \"quoted\"")),
      ("empty", Some("")),
    ],
    alt_svc.alternatives()[0]
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(
    r#"h3=":443"; token=abc; quoted="two words, \"quoted\""; empty="""#,
    alt_svc.header_value()
  );
  assert_eq!(
    alt_svc,
    AltSvc::parse(alt_svc.header_value()).expect("extension forms should round-trip")
  );
}

#[test]
fn accepts_obs_text_in_quoted_pair_escapes() {
  let alt_svc = AltSvc::parse(r#"h3=":443"; note="\é""#).expect("escaped obs-text should parse");

  assert_eq!(Some("é"), alt_svc.alternatives()[0].parameters()[0].value());
  assert_eq!(r#"h3=":443"; note="é""#, alt_svc.header_value());
  assert_eq!(
    alt_svc,
    AltSvc::parse(alt_svc.header_value()).expect("escaped obs-text should round-trip")
  );
}

#[test]
fn accepts_uri_host_authority_forms() {
  for authority in ["foo_bar:443", "foo%2Dbar:443", "[v1.fe80::a]:443"] {
    let alt_svc = AltSvc::parse(format!(r#"h3="{authority}""#))
      .unwrap_or_else(|_| panic!("{authority} should parse as an Alt-Svc authority"));
    assert_eq!(authority, alt_svc.alternatives()[0].authority());
    assert_eq!(
      alt_svc,
      AltSvc::parse(alt_svc.header_value()).expect("uri-host authority should round-trip")
    );
  }
}

#[test]
fn rejects_malformed_separators_protocol_ids_and_authorities() {
  for value in [
    "",
    " ",
    "\t",
    ",",
    ", h3=\":443\"",
    "h3=\":443\",",
    "h3=\":443\",, h2=\":8443\"",
    "h3=\":443\";",
    "h3=\":443\";;flag",
    "h3=\":443\"; flag",
    "h3=\":443\" x=1",
    "h3=\":443\"; x=1 y=2",
    "=\":443\"",
    "h 3=\":443\"",
    "h3/1=\":443\"",
    "h3",
    "h3==\":443\"",
    "h3=:443",
    "h3=alt.example:443",
    "h3=\"alt.example\"",
    "h3=\"alt.example:\"",
    "h3=\"alt.example:https\"",
    "h3=\"alt.example:65536\"",
    "h3=\"2001:db8::1:443\"",
    "h3=\"[2001:db8::1:443",
    "h3=\"[not-an-ip]:443\"",
    "h3=\"[]:443\"",
    "h3=\"[2001:db8:::1]:443\"",
    "h3=\"foo/bar:443\"",
    "h3=\"foo%GG:443\"",
  ] {
    assert!(AltSvc::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn enforces_exact_and_over_field_alternative_and_parameter_bounds() {
  let prefix = "h3=\":443\"; note=\"";
  let suffix = "\"";
  let exact_field = format!(
    "{prefix}{}{suffix}",
    "a".repeat(MAX_ALT_SVC_VALUE_BYTES - prefix.len() - suffix.len())
  );
  assert_eq!(MAX_ALT_SVC_VALUE_BYTES, exact_field.len());
  let parsed = AltSvc::parse(&exact_field).expect("an exact-size field should parse");
  assert_eq!(
    Some(MAX_ALT_SVC_VALUE_BYTES - prefix.len() - suffix.len()),
    parsed.alternatives()[0].parameters()[0]
      .value()
      .map(str::len)
  );
  assert!(
    AltSvc::parse(format!("{exact_field}x")).is_err(),
    "an over-size field must be rejected"
  );

  let alternatives = (0..MAX_ALT_SVC_ALTERNATIVES)
    .map(|index| format!("h{index}=\":443\""))
    .collect::<Vec<_>>();
  let at_alternative_limit = alternatives.join(", ");
  let parsed = AltSvc::parse(&at_alternative_limit).expect("256 alternatives should parse");
  assert_eq!(MAX_ALT_SVC_ALTERNATIVES, parsed.len());

  let too_many_alternatives = (0..=MAX_ALT_SVC_ALTERNATIVES)
    .map(|index| format!("h{index}=\":443\""))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    AltSvc::parse(&too_many_alternatives).is_err(),
    "more than 256 alternatives must be rejected"
  );

  let at_parameter_limit = format!(
    "h3=\":443\"{}",
    (0..MAX_ALT_SVC_PARAMETERS)
      .map(|index| format!("; ext{index}=value"))
      .collect::<Vec<_>>()
      .join("")
  );
  let parsed = AltSvc::parse(&at_parameter_limit).expect("256 parameters should parse");
  assert_eq!(
    MAX_ALT_SVC_PARAMETERS,
    parsed.alternatives()[0].parameters().len()
  );

  let too_many_parameters = format!(
    "h3=\":443\"{}",
    (0..=MAX_ALT_SVC_PARAMETERS)
      .map(|index| format!("; ext{index}=value"))
      .collect::<Vec<_>>()
      .join("")
  );
  assert!(
    AltSvc::parse(&too_many_parameters).is_err(),
    "more than 256 parameters must be rejected"
  );

  assert_eq!(
    MAX_ALT_SVC_PARAMETER_VALUE_BYTES, MAX_ALT_SVC_VALUE_BYTES,
    "the public parameter-value bound is capped by the equal field-size bound"
  );
  let over_parameter_value = format!(
    "h3=\":443\"; note={}",
    "a".repeat(MAX_ALT_SVC_PARAMETER_VALUE_BYTES + 1)
  );
  assert!(
    over_parameter_value.len() > MAX_ALT_SVC_VALUE_BYTES,
    "a parameter value over the equal public bound is field-oversized"
  );
  assert!(AltSvc::parse(&over_parameter_value).is_err());
}
