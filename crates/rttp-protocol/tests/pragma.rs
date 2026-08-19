use rttp_protocol::pragma::{
  Pragma, PragmaParseError, MAX_PRAGMA_DIRECTIVES, MAX_PRAGMA_DIRECTIVE_VALUE_BYTES,
  MAX_PRAGMA_VALUE_BYTES,
};

#[test]
fn parses_no_cache_and_extensions_preserving_spelling_and_order() {
  let pragma = Pragma::parse("no-cache, community=private, example=\"quoted, value\\\"\"")
    .expect("Pragma should parse");

  assert_eq!(pragma.len(), 3);
  assert!(pragma.no_cache());
  assert_eq!(pragma.directives()[0].name(), "no-cache");
  assert_eq!(pragma.directives()[0].value(), None);
  assert_eq!(pragma.directives()[1].name(), "community");
  assert_eq!(pragma.directives()[1].value(), Some("private"));
  assert_eq!(pragma.directives()[2].name(), "example");
  assert_eq!(pragma.directives()[2].value(), Some("quoted, value\""));
  assert_eq!(pragma.extensions().len(), 2);
  assert_eq!(pragma.extensions()[0].name(), "community");
  assert_eq!(
    pragma.header_value(),
    "no-cache, community=private, example=\"quoted, value\\\"\""
  );
}

#[test]
fn parses_uppercase_no_cache_as_defined_directive() {
  let pragma = Pragma::parse("No-Cache").expect("case-insensitive no-cache should parse");

  assert!(pragma.no_cache());
  assert_eq!(pragma.directives()[0].name(), "No-Cache");
  assert!(pragma.extensions().is_empty());
  assert_eq!(pragma.header_value(), "No-Cache");
}

#[test]
fn combines_multiple_field_values_in_wire_order() {
  let pragma = Pragma::parse_values(["no-cache", "community=private", "x-legacy=enabled"])
    .expect("Pragma fields should combine");

  assert_eq!(pragma.len(), 3);
  assert!(pragma.no_cache());
  assert_eq!(pragma.directives()[1].name(), "community");
  assert_eq!(pragma.directives()[2].name(), "x-legacy");
  assert_eq!(
    pragma.header_value(),
    "no-cache, community=private, x-legacy=enabled"
  );
}

#[test]
fn preserves_whitespace_trimming_and_quoted_round_trip() {
  let pragma = Pragma::parse(" \tno-cache , community = \"a b\" , example=token\t ")
    .expect("Pragma should trim optional whitespace");

  assert_eq!(pragma.len(), 3);
  assert_eq!(pragma.directives()[1].name(), "community");
  assert_eq!(pragma.directives()[1].value(), Some("a b"));
  assert_eq!(pragma.directives()[2].value(), Some("token"));
  assert_eq!(
    pragma.header_value(),
    "no-cache, community=\"a b\", example=token"
  );
}

#[test]
fn rejects_malformed_tokens_values_and_controls() {
  for value in [
    "",
    " ",
    "\t",
    ",",
    ",,",
    "no-cache,",
    ",no-cache",
    "bad name",
    "x=not a token",
    "x=\"unterminated",
    "x=\"invalid\\\x01\"",
    "x=value\r\ninjected",
    "x=\x7f",
    "no-cache=token",
    "no-cache=\"value\"",
  ] {
    assert!(Pragma::parse(value).is_err(), "{value:?} should fail");
  }
}

#[test]
fn rejects_case_insensitive_duplicate_directive_names() {
  for value in [
    "no-cache, no-cache",
    "no-cache, No-Cache",
    "community=private, COMMUNITY=public",
    "x=1, X=2",
  ] {
    let error: Result<Pragma, PragmaParseError> = Pragma::parse(value);
    assert!(error.is_err(), "{value:?} should be rejected");
    assert!(
      error.unwrap_err().to_string().contains("duplicate"),
      "{value:?} should fail as a duplicate"
    );
  }
}

#[test]
fn rejects_duplicate_names_across_combined_fields() {
  let error = Pragma::parse_values(["no-cache", "community=private", "NO-CACHE"])
    .expect_err("duplicate across fields should be rejected");
  assert!(error.to_string().contains("duplicate"));
}

#[test]
fn rejects_empty_present_fields_and_valuable_no_cache_forms() {
  let error: Result<Pragma, PragmaParseError> = Pragma::parse("");
  assert!(error.is_err());
  assert!(error
    .unwrap_err()
    .to_string()
    .contains("invalid Pragma directive"));
  let error = Pragma::parse("no-cache=").expect_err("valued no-cache should be rejected");
  assert!(
    error.to_string().contains("no-cache")
      || error.to_string().contains("invalid Pragma directive")
  );
  let error = Pragma::parse("no-cache=value").expect_err("valued no-cache should be rejected");
  assert!(error.to_string().contains("no-cache"));
}

#[test]
fn enforces_member_count_limit() {
  let too_many = (0..=MAX_PRAGMA_DIRECTIVES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let error = Pragma::parse(too_many).expect_err("too many directives should be rejected");
  assert!(error.to_string().contains("too many"));

  let at_limit = (0..MAX_PRAGMA_DIRECTIVES)
    .map(|index| format!("x{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let pragma = Pragma::parse(at_limit).expect("at the member limit should parse");
  assert_eq!(pragma.len(), MAX_PRAGMA_DIRECTIVES);
}

#[test]
fn enforces_field_and_directive_value_size_limits() {
  let error = Pragma::parse("x".repeat(MAX_PRAGMA_VALUE_BYTES + 1))
    .expect_err("oversized field should be rejected");
  assert!(error.to_string().contains("too large"));

  let error = Pragma::parse(format!(
    "x={}",
    "x".repeat(MAX_PRAGMA_DIRECTIVE_VALUE_BYTES + 1)
  ))
  .expect_err("oversized directive value should be rejected");
  assert!(error.to_string().contains("too large"));

  let error = Pragma::parse(format!(
    "x=\"{}\"",
    "x".repeat(MAX_PRAGMA_DIRECTIVE_VALUE_BYTES + 1)
  ))
  .expect_err("oversized quoted directive value should be rejected");
  assert!(error.to_string().contains("too large"));
}

#[test]
fn rejects_duplicate_member_count_and_oversized_forms_across_fields() {
  let duplicate_across_fields = Pragma::parse_values(["no-cache", "No-Cache"]);
  assert!(duplicate_across_fields.is_err());

  let member_overflow = Pragma::parse_values([
    "a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u, v, w, x, y, z",
    "aa, bb, cc, dd, ee, ff, gg, hh, ii, jj, kk, ll, mm, nn, oo, pp, qq, rr, ss, tt, uu, vv, ww, xx, yy, zz",
  ])
  .expect("combined fields below the member limit should parse");
  assert_eq!(member_overflow.len(), 52);

  let oversized_field = Pragma::parse_values(["x".repeat(MAX_PRAGMA_VALUE_BYTES + 1).as_str()]);
  assert!(oversized_field.is_err());
}
