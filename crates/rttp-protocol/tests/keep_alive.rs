use rttp_protocol::keep_alive::{KeepAlive, MAX_KEEP_ALIVE_ITEMS, MAX_KEEP_ALIVE_VALUE_BYTES};

#[test]
fn keep_alive_parses_timeout_and_optional_max() {
  let timeout_only = KeepAlive::parse("timeout=5").expect("timeout-only should parse");
  assert_eq!(timeout_only.timeout(), 5);
  assert_eq!(timeout_only.max(), None);
  assert_eq!(timeout_only.header_value(), "timeout=5");

  let with_max = KeepAlive::parse("timeout=5, max=100").expect("timeout and max should parse");
  assert_eq!(with_max.timeout(), 5);
  assert_eq!(with_max.max(), Some(100));
  assert_eq!(with_max.header_value(), "timeout=5, max=100");
}

#[test]
fn keep_alive_parses_values_combines_fields_and_inspects_every_field() {
  let mut values = ["timeout=5", "max=100"].into_iter();
  let mut calls = 0;

  let keep_alive = KeepAlive::parse_values(std::iter::from_fn(|| {
    calls += 1;
    assert!(calls <= 3, "parser must inspect every list field");
    values.next()
  }))
  .expect("multiple fields form one Keep-Alive parameter set");

  assert_eq!(keep_alive.timeout(), 5);
  assert_eq!(keep_alive.max(), Some(100));
}

#[test]
fn keep_alive_accepts_ows_around_separators_and_case_insensitive_names() {
  let keep_alive =
    KeepAlive::parse("  TIMEOUT = 5 , MAX = 100  ").expect("OWS and uppercase names should parse");
  assert_eq!(keep_alive.timeout(), 5);
  assert_eq!(keep_alive.max(), Some(100));
  assert_eq!(
    keep_alive.header_value(),
    "timeout=5, max=100",
    "formatted output is canonical lowercase"
  );

  let tab_separated =
    KeepAlive::parse("timeout\t=\t5,\tmax\t=\t100").expect("tabs count as OWS around separators");
  assert_eq!(tab_separated.timeout(), 5);
  assert_eq!(tab_separated.max(), Some(100));
}

#[test]
fn keep_alive_parses_checked_integers_with_leading_zeros_and_round_trips() {
  let keep_alive =
    KeepAlive::parse("timeout=0005, max=000100").expect("leading zeros should parse");
  assert_eq!(keep_alive.timeout(), 5);
  assert_eq!(keep_alive.max(), Some(100));
  assert_eq!(
    KeepAlive::parse(keep_alive.header_value())
      .expect("formatted Keep-Alive should round-trip")
      .header_value(),
    "timeout=5, max=100"
  );
}

#[test]
fn keep_alive_rejects_malformed_missing_duplicate_unknown_and_overflow() {
  for value in [
    "",
    " ",
    "\t",
    "timeout=5,",
    ",timeout=5",
    "timeout=5,, max=100",
    "timeout",
    "timeout=",
    "=5",
    "timeout=abc",
    "timeout=-5",
    "timeout=5.0",
    "timeout=5 max=100",
    "max=100",
    "timeout=5, timeout=6",
    "timeout=5, max=100, max=200",
    "keep=alive",
    "timeout=5, keep=alive",
    "timeout=18446744073709551616",
    "max=18446744073709551616",
  ] {
    assert!(
      KeepAlive::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    KeepAlive::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn keep_alive_enforces_value_and_item_bounds() {
  assert!(KeepAlive::parse("x".repeat(MAX_KEEP_ALIVE_VALUE_BYTES + 1)).is_err());
  assert!(
    KeepAlive::parse_values([
      "timeout=5",
      "x".repeat(MAX_KEEP_ALIVE_VALUE_BYTES + 1).as_str(),
    ])
    .is_err(),
    "an oversized later field must not bypass validation"
  );

  let excessive = (0..=MAX_KEEP_ALIVE_ITEMS)
    .map(|index| {
      if index % 2 == 0 {
        "timeout=1".to_string()
      } else {
        "max=2".to_string()
      }
    })
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    KeepAlive::parse(excessive).is_err(),
    "excessive keep-alive elements must be rejected"
  );
}
