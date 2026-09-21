use rttp_protocol::expect::{Expect, MAX_EXPECTATIONS, MAX_EXPECT_VALUE_BYTES};

#[test]
fn expect_constructs_the_standardized_continue_singleton() {
  let expect = Expect::expect_continue();

  assert!(expect.expects_continue());
  assert!(expect.unsupported().is_empty());
  assert_eq!(expect.header_value(), "100-continue");
}

#[test]
fn expect_parses_continue_and_unsupported_extension_names() {
  let expect =
    Expect::parse_values(["100-continue", "preview"]).expect("mixed Expect fields should parse");

  assert!(expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "100-continue, preview");
}

#[test]
fn expect_preserves_unsupported_extension_names_with_values_and_parameters() {
  let expect = Expect::parse("\tpreview \t= \tsha256 \t; \tchunk=1\t ")
    .expect("parameterized Expect extension should parse");

  assert!(!expect.expects_continue());
  assert_eq!(["preview"], expect.unsupported());
  assert_eq!(expect.header_value(), "preview");
}

#[test]
fn expect_accepts_http_optional_whitespace_padding() {
  for value in [" 100-continue , preview ", "\t100-continue\t,\tpreview\t"] {
    let expect = Expect::parse(value).expect("OWS-padded Expect should parse");

    assert!(expect.expects_continue());
    assert_eq!(["preview"], expect.unsupported());
    assert_eq!(expect.header_value(), "100-continue, preview");
  }
}

#[test]
fn expect_accepts_quoted_extension_values_with_obs_text_beside_embedded_separators() {
  for whitespace in ["\u{00a0}", "\u{3000}"] {
    for value in [
      format!(r#"preview="x={whitespace}y""#),
      format!(r#"preview="x;{whitespace}y""#),
    ] {
      let expect = Expect::parse(&value).expect("quoted Expect obs-text should stay opaque");

      assert!(!expect.expects_continue());
      assert_eq!(["preview"], expect.unsupported());
      assert_eq!(expect.header_value(), "preview");
    }
  }

  assert!(
    Expect::parse("preview=\"x\";\u{00a0}parameter").is_err(),
    "non-OWS beside an unquoted parameter separator must still be rejected"
  );
}

#[test]
fn expect_rejects_non_ows_whitespace_at_members_and_separators() {
  for value in [
    "\u{00a0}preview",
    "preview\u{00a0}",
    "\u{3000}preview",
    "preview\u{3000}",
    "\rpreview",
    "preview\r",
    "\npreview",
    "preview\n",
  ] {
    assert!(Expect::parse(value).is_err(), "{value:?} must be rejected");
  }

  for whitespace in ["\u{00a0}", "\u{3000}", "\r", "\n"] {
    for value in [
      format!("preview{whitespace}=value"),
      format!("preview={whitespace}value"),
      format!("preview{whitespace};parameter"),
      format!("preview;{whitespace}parameter"),
    ] {
      assert!(Expect::parse(&value).is_err(), "{value:?} must be rejected");
    }
  }
}

#[test]
fn expect_rejects_duplicate_expectation_names() {
  assert!(Expect::parse("100-continue, 100-CONTINUE").is_err());
  assert!(Expect::parse_values(["100-continue", "100-CONTINUE"]).is_err());
  assert!(Expect::parse("preview, PREVIEW").is_err());
}

#[test]
fn expect_rejects_malformed_and_empty_values() {
  assert!(Expect::parse_values([]).is_err());

  for value in ["", "   ", ",", "100-continue,", ",preview", "not a token"] {
    assert!(Expect::parse(value).is_err(), "{value:?} must be rejected");
  }
}

#[test]
fn expect_retains_unsupported_extension_names() {
  let expect = Expect::parse("tea-time").expect("unsupported Expect names should parse");

  assert!(!expect.expects_continue());
  assert_eq!(["tea-time"], expect.unsupported());
  assert_eq!(expect.header_value(), "tea-time");
}

#[test]
fn expect_enforces_value_and_member_count_bounds() {
  assert!(
    Expect::parse("a".repeat(MAX_EXPECT_VALUE_BYTES + 1)).is_err(),
    "oversized values must be rejected"
  );

  let at_value_limit = "a".repeat(MAX_EXPECT_VALUE_BYTES);
  let at_limit = Expect::parse(&at_value_limit).expect("values at the 64 KiB bound must parse");
  assert_eq!(at_limit.unsupported(), [at_value_limit.as_str()]);

  let oversized_duplicate = "a".repeat(MAX_EXPECT_VALUE_BYTES + 1);
  assert!(
    Expect::parse_values(["preview", oversized_duplicate.as_str()]).is_err(),
    "oversized later fields must not bypass validation"
  );

  let at_count_limit = (0..MAX_EXPECTATIONS)
    .map(|index| format!("e{index}"))
    .collect::<Vec<_>>()
    .join(",");
  let parsed = Expect::parse(&at_count_limit).expect("32 expectations should parse");
  assert_eq!(parsed.unsupported().len(), MAX_EXPECTATIONS);

  let too_many = (0..=MAX_EXPECTATIONS)
    .map(|index| format!("e{index}"))
    .collect::<Vec<_>>()
    .join(",");
  assert!(
    Expect::parse(&too_many).is_err(),
    "more than 32 expectations must be rejected"
  );
}
