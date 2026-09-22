use rttp_protocol::priority::{Priority, MAX_PRIORITY_PARAMETERS, MAX_PRIORITY_VALUE_BYTES};

fn extension_pairs(priority: &Priority) -> Vec<(&str, Option<&str>)> {
  priority
    .extensions()
    .iter()
    .map(|extension| (extension.name(), extension.value()))
    .collect()
}

#[test]
fn priority_parses_ordered_multi_field_values_with_defaults() {
  let priority = Priority::parse_values(["u=3, x=first", "i=?0, y=second"])
    .expect("ordered Priority fields should parse");

  assert_eq!(Some(3), priority.urgency());
  assert!(!priority.incremental());
  assert_eq!(
    vec![("x", Some("first")), ("y", Some("second"))],
    extension_pairs(&priority)
  );
  assert_eq!("u=3, i=?0, x=first, y=second", priority.header_value());

  let urgency_only = Priority::parse("u=0").expect("urgency-only Priority should parse");
  assert_eq!(Some(0), urgency_only.urgency());
  assert!(
    !urgency_only.incremental(),
    "absent incremental must default to false"
  );
  assert!(urgency_only.extensions().is_empty());
  assert_eq!("u=0", urgency_only.header_value());
}

#[test]
fn priority_validates_known_parameter_types_and_ranges() {
  for urgency in 0..=7 {
    let priority = Priority::parse(format!("u={urgency}")).expect("urgency 0..=7 should parse");
    assert_eq!(Some(urgency), priority.urgency());
  }

  for value in ["u", "u=8", "u=-1", "u=1.0", "u=999"] {
    let priority = Priority::parse(value).expect("valid Structured Fields members should parse");
    assert_eq!(None, priority.urgency(), "{value:?} should be ignored");
  }
  for value in ["u=", "u=01a", "u=+1"] {
    assert!(
      Priority::parse(value).is_err(),
      "{value:?} must be rejected as malformed Structured Fields"
    );
  }

  let bare = Priority::parse("i").expect("bare incremental should parse");
  assert!(bare.incremental());
  assert_eq!("i", bare.header_value());

  let true_value = Priority::parse("i=?1").expect("incremental ?1 should parse");
  assert!(true_value.incremental());
  assert_eq!("i", true_value.header_value());

  let false_value = Priority::parse("i=?0").expect("incremental ?0 should parse");
  assert!(!false_value.incremental());
  assert_eq!("i=?0", false_value.header_value());

  for value in ["i=1", "i=true"] {
    let priority = Priority::parse(value).expect("valid Structured Fields members should parse");
    assert!(!priority.incremental(), "{value:?} should be ignored");
  }
  for value in ["i=", "i=?2", "i=?01"] {
    assert!(
      Priority::parse(value).is_err(),
      "{value:?} must be rejected as malformed Structured Fields"
    );
  }
}

#[test]
fn priority_uses_last_key_wins_for_known_and_extension_members() {
  let priority = Priority::parse_values(["u=1, i, x=first, y=keep", "u=7, i=?0, x=last"])
    .expect("duplicate Priority dictionary keys should parse");

  assert_eq!(Some(7), priority.urgency());
  assert!(!priority.incremental());
  assert_eq!(
    vec![("y", Some("keep")), ("x", Some("last"))],
    extension_pairs(&priority)
  );
  assert_eq!("u=7, i=?0, y=keep, x=last", priority.header_value());

  let same_field = Priority::parse("u=2, u=5, i=?0, i, a=1, a=2")
    .expect("in-field duplicate keys should parse with last-key-wins");
  assert_eq!(Some(5), same_field.urgency());
  assert!(same_field.incremental());
  assert_eq!(vec![("a", Some("2"))], extension_pairs(&same_field));
}

#[test]
fn priority_accepts_extension_bare_item_forms_and_quoted_escaping() {
  let priority = Priority::parse(
    r#"flag, tok=Token/path, str="say \"hi\" and \\path", bin=:YWJj:, num=-42, dec=12.345"#,
  )
  .expect("extension bare items should parse");

  assert_eq!(
    vec![
      ("flag", None),
      ("tok", Some("Token/path")),
      ("str", Some(r#""say \"hi\" and \\path""#)),
      ("bin", Some(":YWJj:")),
      ("num", Some("-42")),
      ("dec", Some("12.345")),
    ],
    extension_pairs(&priority)
  );
  assert_eq!(
    r#"flag, tok=Token/path, str="say \"hi\" and \\path", bin=:YWJj:, num=-42, dec=12.345"#,
    priority.header_value()
  );

  let with_comma_in_string =
    Priority::parse(r#"x="a,b", u=1"#).expect("quoted commas must not split members");
  assert_eq!(Some(1), with_comma_in_string.urgency());
  assert_eq!(
    Some(r#""a,b""#),
    with_comma_in_string.extensions()[0].value()
  );

  let reparsed =
    Priority::parse(priority.header_value()).expect("canonical Priority output should reparse");
  assert_eq!(priority, reparsed);
  assert_eq!(priority.header_value(), reparsed.header_value());

  let empty_bytes = Priority::parse("x=::").expect("an empty byte sequence should parse");
  assert_eq!(vec![("x", Some("::"))], extension_pairs(&empty_bytes));
  assert_eq!("x=::", empty_bytes.header_value());
  assert_eq!(
    empty_bytes,
    Priority::parse(empty_bytes.header_value()).expect("empty byte sequence should round-trip")
  );
}

#[test]
fn priority_rejects_malformed_keys_separators_and_items() {
  for value in [
    "",
    "   ",
    "\t",
    ",",
    ",u=1",
    "u=1,",
    "u=1,,i",
    "u=1, ",
    "u=1, ,i",
    "U=1",
    "1x=1",
    "-x=1",
    "x!=1",
    "=1",
    "x=",
    "u = 1",
    "u= 1",
    "x = token",
    "x=\"unterminated",
    "x=\"bad\\escape\"",
    "x=:@@@:",
    "x=:A:",
    "x=:Y=Q:",
    "x=?2",
    "x=1.2345",
    "x=+1",
    "x=01e2",
    "u=1\r\nX-Injected: 1",
  ] {
    assert!(
      Priority::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let with_ows = Priority::parse(" \tu=1\t , \tx=token \t").expect("outer and comma OWS is valid");
  assert_eq!(Some(1), with_ows.urgency());
  assert_eq!(vec![("x", Some("token"))], extension_pairs(&with_ows));

  assert!(
    Priority::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
  for (index, values) in [vec!["", "u=1"], vec!["u=1", ""], vec!["", ""]]
    .into_iter()
    .enumerate()
  {
    assert!(
      Priority::parse_values(values).is_err(),
      "empty combined fields must be rejected at index {index}"
    );
  }
}

#[test]
fn priority_enforces_exact_and_over_field_size_and_parameter_bounds() {
  let exact_token_len = MAX_PRIORITY_VALUE_BYTES - "x=".len();
  let exact_value = format!("x={}", "a".repeat(exact_token_len));
  assert_eq!(exact_value.len(), MAX_PRIORITY_VALUE_BYTES);
  let exact = Priority::parse(&exact_value).expect("a 64 KiB Priority field should parse");
  assert_eq!(
    Some(exact_token_len),
    exact.extensions()[0].value().map(str::len)
  );

  assert_eq!(
    Priority::parse("x".repeat(MAX_PRIORITY_VALUE_BYTES + 1))
      .expect_err("oversized Priority fields must be rejected")
      .to_string(),
    "Priority header value is too large"
  );
  assert!(
    Priority::parse_values(["u=1", "x".repeat(MAX_PRIORITY_VALUE_BYTES + 1).as_str(),]).is_err(),
    "an oversized later field must not bypass validation"
  );

  let at_parameter_limit = (0..MAX_PRIORITY_PARAMETERS)
    .map(|index| format!("x{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  let parsed_parameters =
    Priority::parse(&at_parameter_limit).expect("256 Priority parameters should parse");
  assert_eq!(
    MAX_PRIORITY_PARAMETERS,
    parsed_parameters.extensions().len()
  );

  let too_many = (0..=MAX_PRIORITY_PARAMETERS)
    .map(|index| format!("x{index}=?1"))
    .collect::<Vec<_>>()
    .join(", ");
  assert_eq!(
    Priority::parse(&too_many)
      .expect_err("more than 256 Priority parameters must be rejected")
      .to_string(),
    "too many Priority parameters"
  );

  let combined_at_limit = (0..MAX_PRIORITY_PARAMETERS)
    .map(|index| format!("x{index}=?1"))
    .collect::<Vec<_>>();
  let (first, rest) = combined_at_limit.split_at(128);
  let parsed_combined =
    Priority::parse_values([first.join(", ").as_str(), rest.join(", ").as_str()])
      .expect("256 parameters across Priority fields should parse");
  assert_eq!(MAX_PRIORITY_PARAMETERS, parsed_combined.extensions().len());

  let combined_over_limit = (0..=MAX_PRIORITY_PARAMETERS)
    .map(|index| format!("x{index}=?1"))
    .collect::<Vec<_>>();
  assert!(
    Priority::parse_values(
      combined_over_limit
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
    )
    .is_err(),
    "more than 256 parameters across fields must be rejected"
  );
}
