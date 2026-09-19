use std::error::Error;

use rttp_protocol::clear_site_data::{
  ClearSiteData, ClearSiteDataDirective, ClearSiteDataParseError, MAX_CLEAR_SITE_DATA_DIRECTIVES,
  MAX_CLEAR_SITE_DATA_VALUE_BYTES,
};

fn parse_error(value: &str) -> ClearSiteDataParseError {
  let error = ClearSiteData::parse(value).expect_err("Clear-Site-Data value should be rejected");
  let displayed: &dyn Error = &error;
  assert_eq!(displayed.to_string(), error.to_string());
  error
}

fn assert_rejected(value: &str, fragment: &str) {
  let message = parse_error(value).to_string();
  assert!(
    message.contains(fragment),
    "{value:?} should mention {fragment:?}, got {message:?}"
  );
}

fn assert_helpers(
  value: &str,
  directives: &[ClearSiteDataDirective],
  wildcard: bool,
  cache: bool,
  cookies: bool,
  storage: bool,
  execution_contexts: bool,
) {
  let metadata = ClearSiteData::parse(value).unwrap_or_else(|error| {
    panic!("Clear-Site-Data {value:?} should parse: {error}");
  });
  assert_eq!(metadata.directives(), directives, "{value}");
  assert_eq!(metadata.is_wildcard(), wildcard, "{value}");
  assert_eq!(metadata.clears_cache(), cache, "{value}");
  assert_eq!(metadata.clears_cookies(), cookies, "{value}");
  assert_eq!(metadata.clears_storage(), storage, "{value}");
  assert_eq!(
    metadata.clears_execution_contexts(),
    execution_contexts,
    "{value}"
  );
  let canonical = directives
    .iter()
    .map(|directive| format!("\"{}\"", directive.as_str()))
    .collect::<Vec<_>>()
    .join(", ");
  assert_eq!(metadata.header_value(), canonical, "{value}");
}

#[test]
fn clear_site_data_preserves_field_order_ows_and_canonical_quoting() {
  assert_eq!("cache", ClearSiteDataDirective::Cache.as_str());
  assert_eq!("cookies", ClearSiteDataDirective::Cookies.as_str());
  assert_eq!("storage", ClearSiteDataDirective::Storage.as_str());
  assert_eq!(
    "executionContexts",
    ClearSiteDataDirective::ExecutionContexts.as_str()
  );
  assert_eq!("*", ClearSiteDataDirective::Wildcard.as_str());

  let metadata = ClearSiteData::parse_values([
    "\t\"executionContexts\" , \"storage\"\t",
    " \"cookies\",\t\"cache\" ",
    "\"*\"",
  ])
  .expect("ordered Clear-Site-Data fields should parse");

  assert_eq!(
    metadata.directives(),
    &[
      ClearSiteDataDirective::ExecutionContexts,
      ClearSiteDataDirective::Storage,
      ClearSiteDataDirective::Cookies,
      ClearSiteDataDirective::Cache,
      ClearSiteDataDirective::Wildcard,
    ]
  );
  assert_eq!(
    metadata.header_value(),
    "\"executionContexts\", \"storage\", \"cookies\", \"cache\", \"*\""
  );
  let round_trip =
    ClearSiteData::parse(metadata.header_value()).expect("canonical Clear-Site-Data should parse");
  assert_eq!(round_trip.directives(), metadata.directives());
  assert_eq!(round_trip.header_value(), metadata.header_value());

  let spaced = ClearSiteData::parse(" \"cache\" ,\t\"cookies\" ")
    .expect("OWS around quoted directives should parse");
  assert_eq!(
    spaced.directives(),
    &[
      ClearSiteDataDirective::Cache,
      ClearSiteDataDirective::Cookies,
    ]
  );
  assert_eq!(spaced.header_value(), "\"cache\", \"cookies\"");
}

#[test]
fn clear_site_data_wildcard_helpers_follow_named_truth_table() {
  assert_helpers(
    "\"cache\"",
    &[ClearSiteDataDirective::Cache],
    false,
    true,
    false,
    false,
    false,
  );
  assert_helpers(
    "\"cookies\"",
    &[ClearSiteDataDirective::Cookies],
    false,
    false,
    true,
    false,
    false,
  );
  assert_helpers(
    "\"storage\"",
    &[ClearSiteDataDirective::Storage],
    false,
    false,
    false,
    true,
    false,
  );
  assert_helpers(
    "\"executionContexts\"",
    &[ClearSiteDataDirective::ExecutionContexts],
    false,
    false,
    false,
    false,
    true,
  );
  assert_helpers(
    "\"*\"",
    &[ClearSiteDataDirective::Wildcard],
    true,
    true,
    true,
    true,
    true,
  );
  assert_helpers(
    "\"cache\", \"cookies\"",
    &[
      ClearSiteDataDirective::Cache,
      ClearSiteDataDirective::Cookies,
    ],
    false,
    true,
    true,
    false,
    false,
  );
  assert_helpers(
    "\"storage\", \"executionContexts\"",
    &[
      ClearSiteDataDirective::Storage,
      ClearSiteDataDirective::ExecutionContexts,
    ],
    false,
    false,
    false,
    true,
    true,
  );
  assert_helpers(
    "\"*\", \"cache\"",
    &[
      ClearSiteDataDirective::Wildcard,
      ClearSiteDataDirective::Cache,
    ],
    true,
    true,
    true,
    true,
    true,
  );
  assert_helpers(
    "\"cookies\", \"*\"",
    &[
      ClearSiteDataDirective::Cookies,
      ClearSiteDataDirective::Wildcard,
    ],
    true,
    true,
    true,
    true,
    true,
  );

  let across_fields = ClearSiteData::parse_values(["\"storage\"", " \"*\" "])
    .expect("wildcard across fields should parse");
  assert_eq!(
    across_fields.directives(),
    &[
      ClearSiteDataDirective::Storage,
      ClearSiteDataDirective::Wildcard,
    ]
  );
  assert!(across_fields.is_wildcard());
  assert!(across_fields.clears_cache());
  assert!(across_fields.clears_cookies());
  assert!(across_fields.clears_storage());
  assert!(across_fields.clears_execution_contexts());
  assert_eq!(across_fields.header_value(), "\"storage\", \"*\"");
}

#[test]
fn clear_site_data_rejects_duplicates_empty_commas_quotes_and_unknown_names() {
  let htab = ClearSiteData::parse("\t\"cache\"\t,\t\"cookies\"\t")
    .expect("HTAB optional whitespace should parse");
  assert_eq!(
    htab.directives(),
    &[
      ClearSiteDataDirective::Cache,
      ClearSiteDataDirective::Cookies,
    ]
  );
  assert_eq!(htab.header_value(), "\"cache\", \"cookies\"");

  for value in [
    "\"cache\", \"cache\"",
    "\"cookies\",\"cookies\"",
    "\"*\", \"*\"",
    "\"executionContexts\", \"storage\", \"executionContexts\"",
  ] {
    assert_rejected(value, "duplicate");
  }

  let duplicate_fields =
    ClearSiteData::parse_values(["\"cache\", \"cookies\"", "\"storage\", \"cache\""])
      .expect_err("duplicate directives across fields should be rejected");
  assert!(duplicate_fields.to_string().contains("duplicate"));
  let duplicate_wildcard =
    ClearSiteData::parse_values(["\"*\"", "\"cache\", \"*\""]).expect_err("repeated wildcard");
  assert!(duplicate_wildcard.to_string().contains("duplicate"));

  for value in ["", " ", "\t", "\"cache\",", "\"cache\", ", "\"cache\",\t"] {
    assert_rejected(value, "invalid Clear-Site-Data directive");
  }
  for value in [",", ",\"cache\"", "\"cache\",,", "\"cache\", , \"cookies\""] {
    assert_rejected(value, "quoted");
  }
  let empty_fields = ClearSiteData::parse_values(std::iter::empty())
    .expect_err("an empty field list should be rejected");
  assert!(empty_fields
    .to_string()
    .contains("invalid Clear-Site-Data directive"));
  let blank_second = ClearSiteData::parse_values(["\"cache\"", " \t"])
    .expect_err("a blank field should be rejected");
  assert!(blank_second
    .to_string()
    .contains("invalid Clear-Site-Data directive"));
  let trailing_second = ClearSiteData::parse_values(["\"cookies\"", "\"storage\","])
    .expect_err("a trailing comma in a later field should be rejected");
  assert!(trailing_second
    .to_string()
    .contains("invalid Clear-Site-Data directive"));

  for value in ["cache", "*", "cookies", "'cache'", "cache\""] {
    assert_rejected(value, "quoted");
  }
  for value in [
    "\"cache",
    "\"",
    "\"cache\\\"\"",
    "\"\\cache\"",
    "\"ca\\che\"",
    "\"\\*\"",
    "\"ca\tche\"",
  ] {
    assert_rejected(value, "malformed");
  }
  assert_rejected("\"cache\" \"cookies\"", "invalid Clear-Site-Data directive");

  for value in [
    "\"unknown\"",
    "\"Cache\"",
    "\"COOKIES\"",
    "\"executioncontexts\"",
    "\"clientHints\"",
    "\"\"",
    "\" cache\"",
    "\"* \"",
  ] {
    assert_rejected(value, "invalid Clear-Site-Data directive");
  }
}

#[test]
fn clear_site_data_rejects_escaped_non_ascii_and_control_bytes() {
  for value in [
    "\"cache\\\"\"",
    "\"\\\\*\"",
    "\"cookies\\\"",
    "\"s\\torage\"",
  ] {
    assert_rejected(value, "malformed");
  }
  for value in ["\"caf\u{e9}\"", "\"\u{80}\"", "\"storage\u{ff}\""] {
    assert_rejected(value, "malformed");
  }
  assert_rejected("\u{e9}", "quoted");
  assert_rejected("\"cache\"\u{e9}", "invalid Clear-Site-Data directive");

  for value in [
    "\"cache\"\n",
    "\"cache\"\r",
    "\u{0}\"cache\"",
    "\"cache\u{1f}\"",
    "\"\u{7f}\"",
    "\"cache\"\r\nX: injected",
    "\"cookies\"\u{1}",
  ] {
    assert_rejected(value, "control byte");
  }
}

#[test]
fn clear_site_data_enforces_value_byte_and_directive_bounds() {
  assert_eq!(64 * 1024, MAX_CLEAR_SITE_DATA_VALUE_BYTES);
  assert_eq!(5, MAX_CLEAR_SITE_DATA_DIRECTIVES);

  let quoted = "\"executionContexts\"";
  let padding = MAX_CLEAR_SITE_DATA_VALUE_BYTES - quoted.len();
  let exact = format!("\t{}{quoted}", " ".repeat(padding - 1));
  assert_eq!(MAX_CLEAR_SITE_DATA_VALUE_BYTES, exact.len());
  let parsed = ClearSiteData::parse(&exact).expect("exact 64 KiB OWS-padded field should parse");
  assert_eq!(
    parsed.directives(),
    &[ClearSiteDataDirective::ExecutionContexts]
  );
  assert_eq!(parsed.header_value(), "\"executionContexts\"");
  assert!(parsed.clears_execution_contexts());
  assert!(!parsed.clears_cache());
  assert!(!parsed.is_wildcard());

  let with_neighbor = ClearSiteData::parse_values([exact.as_str(), "\t\"cache\" "])
    .expect("a second field should remain independent of the per-field byte bound");
  assert_eq!(
    with_neighbor.directives(),
    &[
      ClearSiteDataDirective::ExecutionContexts,
      ClearSiteDataDirective::Cache,
    ]
  );

  let oversized = format!("{exact} ");
  assert_eq!(MAX_CLEAR_SITE_DATA_VALUE_BYTES + 1, oversized.len());
  assert_rejected(&oversized, "too large");
  let oversized_field = ClearSiteData::parse_values(["\"cookies\"", oversized.as_str()])
    .expect_err("an oversized later field should be rejected");
  assert!(oversized_field.to_string().contains("too large"));

  let at_limit = "\"cache\", \"cookies\", \"storage\", \"executionContexts\", \"*\"";
  let parsed = ClearSiteData::parse(at_limit).expect("the five recognized directives should parse");
  assert_eq!(
    parsed.directives(),
    &[
      ClearSiteDataDirective::Cache,
      ClearSiteDataDirective::Cookies,
      ClearSiteDataDirective::Storage,
      ClearSiteDataDirective::ExecutionContexts,
      ClearSiteDataDirective::Wildcard,
    ]
  );
  assert_eq!(parsed.directives().len(), MAX_CLEAR_SITE_DATA_DIRECTIVES);

  let at_limit_fields = [
    "\"cache\"",
    "\"cookies\"",
    "\"storage\"",
    "\"executionContexts\"",
    "\"*\"",
  ];
  let parsed_fields = ClearSiteData::parse_values(at_limit_fields)
    .expect("the five recognized directives should parse across fields");
  assert_eq!(parsed_fields.directives(), parsed.directives());
  assert_eq!(
    parsed_fields.directives().len(),
    MAX_CLEAR_SITE_DATA_DIRECTIVES
  );

  let overflow = format!("{at_limit}, \"cache\"");
  let overflow_error = parse_error(&overflow);
  let overflow_message = overflow_error.to_string();
  assert!(
    overflow_message.contains("too many Clear-Site-Data directives"),
    "over-limit list should mention the directive bound, got {overflow_error}"
  );
  assert!(
    !overflow_message.contains("duplicate"),
    "the count limit must reject item six before duplicate policy, got {overflow_error}"
  );

  let unknown_overflow = format!("{at_limit}, \"unknown\"");
  let unknown_overflow_error = parse_error(&unknown_overflow);
  let unknown_overflow_message = unknown_overflow_error.to_string();
  assert!(
    unknown_overflow_message.contains("too many Clear-Site-Data directives"),
    "a sixth unknown name must hit the count limit, got {unknown_overflow_error}"
  );
  assert!(
    !unknown_overflow_message.contains("invalid"),
    "the count limit must reject item six before unknown-name policy, got {unknown_overflow_error}"
  );

  let fields = [
    "\"cache\", \"cookies\"",
    "\"storage\"",
    "\"executionContexts\"",
    "\"*\"",
    "\"cache\"",
  ];
  let field_overflow = ClearSiteData::parse_values(fields)
    .expect_err("a sixth directive across fields should be rejected");
  let field_overflow_message = field_overflow.to_string();
  assert!(
    field_overflow_message.contains("too many Clear-Site-Data directives"),
    "over-limit fields should mention the directive bound, got {field_overflow}"
  );
  assert!(
    !field_overflow_message.contains("duplicate"),
    "a later field past the cap must hit the count limit, got {field_overflow}"
  );

  let repeated = std::iter::repeat_n("\"cache\"", MAX_CLEAR_SITE_DATA_DIRECTIVES + 1)
    .collect::<Vec<_>>()
    .join(", ");
  let repeated_error = parse_error(&repeated);
  assert!(
    repeated_error.to_string().contains("duplicate"),
    "repeated directives past the cap must stay rejected as duplicates, got {repeated_error}"
  );

  let repeated_fields = vec!["\"cookies\""; MAX_CLEAR_SITE_DATA_DIRECTIVES + 1];
  let repeated_field_overflow = ClearSiteData::parse_values(repeated_fields)
    .expect_err("repeated directives across fields past the cap should be rejected");
  assert!(
    repeated_field_overflow.to_string().contains("duplicate"),
    "duplicate policy must still apply across fields, got {repeated_field_overflow}"
  );

  let pair = parse_error("\"storage\", \"storage\"");
  assert!(pair.to_string().contains("duplicate"));
}
