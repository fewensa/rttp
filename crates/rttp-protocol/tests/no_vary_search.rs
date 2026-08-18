use rttp_protocol::no_vary_search::{
  NoVarySearch, NoVarySearchParams, MAX_NO_VARY_SEARCH_PARAMETERS, MAX_NO_VARY_SEARCH_VALUE_BYTES,
};

#[test]
fn parses_structured_no_vary_search_metadata() {
  let metadata = NoVarySearch::parse(r#"key-order=?0, params=("utm_source" "q"), x-token=42"#)
    .expect("valid No-Vary-Search should parse");

  assert_eq!(Some(false), metadata.key_order());
  assert_eq!(
    Some(&["utm_source".to_owned(), "q".to_owned()][..]),
    metadata.ignored_params()
  );
  assert_eq!(metadata.except(), Vec::<String>::new().as_slice());
  assert_eq!(metadata.extensions()[0].key(), "x-token");
  assert_eq!(metadata.extensions()[0].value(), Some("42"));
  assert_eq!(
    metadata.header_value(),
    r#"key-order=?0, params=("utm_source" "q"), x-token=42"#
  );
}

#[test]
fn parses_params_all_with_exceptions_across_header_fields() {
  let metadata = NoVarySearch::parse_values([r#"params"#, r#"except=("session" "debug")"#])
    .expect("multiple No-Vary-Search fields should parse");

  assert!(metadata.ignores_all_query_params());
  assert_eq!(metadata.params(), Some(&NoVarySearchParams::All));
  assert_eq!(metadata.except(), ["session", "debug"]);
  assert_eq!(
    metadata.header_value(),
    r#"params, except=("session" "debug")"#
  );
}

#[test]
fn parses_explicit_params_false() {
  let metadata = NoVarySearch::parse("params=?0").expect("No-Vary-Search should parse");

  assert_eq!(metadata.params(), Some(&NoVarySearchParams::None));
  assert!(!metadata.ignores_all_query_params());
  assert_eq!(metadata.ignored_params(), None);
  assert_eq!(metadata.header_value(), "params=?0");
}

#[test]
fn duplicate_members_use_last_value() {
  let metadata = NoVarySearch::parse_values([
    r#"key-order=?0, params=("a"), except=("old"), x=first"#,
    r#"key-order, params, except=("new"), x=last"#,
  ])
  .expect("duplicate dictionary keys should use the last value");

  assert_eq!(metadata.key_order(), Some(true));
  assert_eq!(metadata.params(), Some(&NoVarySearchParams::All));
  assert_eq!(metadata.except(), ["new"]);
  assert_eq!(metadata.extensions()[0].value(), Some("last"));
  assert_eq!(
    metadata.header_value(),
    r#"key-order, params, except=("new"), x=last"#
  );
}

#[test]
fn validates_extension_values_as_structured_fields() {
  for value in ["x", "x=?0", "x=42", r#"x=("a" token ?1)"#, r#"x="quoted""#] {
    NoVarySearch::parse(value).expect("valid extension value should parse");
  }

  for value in ["x=", "x=not valid", "x=(?2)", r#"x="unterminated"#] {
    assert!(
      NoVarySearch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn parses_parameterized_implicit_true_extensions() {
  let metadata = NoVarySearch::parse("x;flag, y;flag=1")
    .expect("parameterized implicit true extensions should parse");

  assert_eq!(metadata.extensions()[0].key(), "x");
  assert_eq!(metadata.extensions()[0].value(), Some("?1;flag"));
  assert_eq!(metadata.extensions()[1].key(), "y");
  assert_eq!(metadata.extensions()[1].value(), Some("?1;flag=1"));
  assert_eq!(metadata.header_value(), "x=?1;flag, y=?1;flag=1");
}

#[test]
fn parameterized_extension_duplicates_use_last_member() {
  let metadata = NoVarySearch::parse("x;old, x;flag=1")
    .expect("duplicate parameterized extension keys should use the last member");

  assert_eq!(1, metadata.extensions().len());
  assert_eq!(metadata.extensions()[0].key(), "x");
  assert_eq!(metadata.extensions()[0].value(), Some("?1;flag=1"));
  assert_eq!(metadata.header_value(), "x=?1;flag=1");
}

#[test]
fn rejects_parameterized_reserved_members() {
  for value in [
    "params;flag",
    "key-order;flag",
    r#"except;flag=("session")"#,
    r#"except=("session");flag"#,
    r#"params=("a"), params;flag"#,
  ] {
    assert!(
      NoVarySearch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn rejects_invalid_no_vary_search_values() {
  for value in [
    "",
    "Params",
    "params=utm",
    "params=()",
    r#"params=("a", "b")"#,
    r#"params=("a"), except=("b")"#,
    r#"except=("b")"#,
    "key-order=false",
    "key-order=?2",
    "x key",
  ] {
    assert!(
      NoVarySearch::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn enforces_no_vary_search_bounds() {
  assert!(NoVarySearch::parse("x".repeat(MAX_NO_VARY_SEARCH_VALUE_BYTES + 1)).is_err());

  let too_many = format!(
    "params=({})",
    (0..=MAX_NO_VARY_SEARCH_PARAMETERS)
      .map(|index| format!("\"p{index}\""))
      .collect::<Vec<_>>()
      .join(" ")
  );
  assert!(NoVarySearch::parse(too_many).is_err());
}
