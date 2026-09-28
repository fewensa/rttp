use rttp_protocol::link::{
  LinkParameter, LinkParseError, LinkValue, LinkValues, MAX_LINK_PARAMETERS,
  MAX_LINK_PARAMETER_VALUE_BYTES, MAX_LINK_VALUES, MAX_LINK_VALUE_BYTES,
};

#[test]
fn link_parses_multi_field_values_in_order() {
  let links = LinkValues::parse_values([
    "</style.css>; rel=preload; as=style, <https://cdn.example.test/app.js>; rel=modulepreload",
    "<../manifest.json>; type=\"application/manifest+json\"; anchor=\"/app\"",
  ])
  .expect("multiple Link fields should parse");

  assert_eq!(3, links.len());
  assert!(!links.is_empty());
  assert_eq!("/style.css", links.values()[0].target());
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
  assert_eq!(Some("style"), links.values()[0].parameter("as"));
  assert_eq!(
    "https://cdn.example.test/app.js",
    links.values()[1].target()
  );
  assert_eq!(Some("modulepreload"), links.values()[1].parameter("rel"));
  assert_eq!("../manifest.json", links.values()[2].target());
  assert_eq!(
    Some("application/manifest+json"),
    links.values()[2].parameter("type")
  );
  assert_eq!(Some("/app"), links.values()[2].parameter("anchor"));
  assert_eq!(
    vec![("type", "application/manifest+json"), ("anchor", "/app")],
    links.values()[2]
      .parameters()
      .iter()
      .map(|parameter: &LinkParameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn link_preserves_repeated_relation_values() {
  let links = LinkValues::parse(
    "</a.css>; rel=stylesheet, </b.css>; rel=stylesheet; media=print, </c.css>; rel=\"preload prefetch\"",
  )
  .expect("repeated rel values should parse");

  assert_eq!(3, links.len());
  assert_eq!(Some("stylesheet"), links.values()[0].parameter("rel"));
  assert_eq!(Some("stylesheet"), links.values()[1].parameter("rel"));
  assert_eq!(Some("print"), links.values()[1].parameter("media"));
  assert_eq!(Some("preload prefetch"), links.values()[2].parameter("rel"));
}

#[test]
fn link_unescapes_quoted_parameter_values() {
  let links = LinkValues::parse(r#"</style.css>; rel=preload; title="say \"hi\" and \\""#)
    .expect("quoted-string escapes should parse");

  assert_eq!(
    Some(r#"say "hi" and \"#),
    links.values()[0].parameter("title")
  );
}

#[test]
fn link_accepts_obs_text_in_quoted_pair_escapes() {
  let links =
    LinkValues::parse(r#"</style.css>; title="\é""#).expect("escaped obs-text should parse");

  assert_eq!(Some("é"), links.values()[0].parameter("title"));
}

#[test]
fn link_preserves_valueless_parameters_and_lowercases_names() {
  let links =
    LinkValues::parse("</style.css>; rel=preload; NOPUSH").expect("valueless extensions parse");

  assert_eq!(
    vec![("rel", "preload"), ("nopush", "")],
    links.values()[0]
      .parameters()
      .iter()
      .map(|parameter: &LinkParameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
  assert_eq!(Some(""), links.values()[0].parameter("nopush"));
}

#[test]
fn link_keeps_targets_as_raw_unresolved_text() {
  let links = LinkValues::parse_values([
    "<HTTPS://EXAMPLE.TEST:443/A%2fB>",
    "<../images/logo.png?size=small#v1>",
    "<#fragment>",
  ])
  .expect("URI-references should parse");

  assert_eq!("HTTPS://EXAMPLE.TEST:443/A%2fB", links.values()[0].target());
  assert_eq!(
    "../images/logo.png?size=small#v1",
    links.values()[1].target()
  );
  assert_eq!("#fragment", links.values()[2].target());
}

#[test]
fn link_preserves_quoted_commas_escaped_quotes_obs_text_fragments_and_relative_targets() {
  let links = LinkValues::parse(
    r#"</style.css>; title="a,b\"c"; rel=preload, <../images/logo.png?size=small#v1>; title="\é""#,
  )
  .expect("quoted commas, escapes, obs-text, fragments, and relative targets should parse");

  assert_eq!(2, links.len());
  assert_eq!("/style.css", links.values()[0].target());
  assert_eq!(Some(r#"a,b"c"#), links.values()[0].parameter("title"));
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
  assert_eq!(
    "../images/logo.png?size=small#v1",
    links.values()[1].target()
  );
  assert_eq!(Some("é"), links.values()[1].parameter("title"));
}

#[test]
fn link_parse_values_combines_and_inspects_every_field() {
  let mut values = ["</a.css>", "</b.css>"].into_iter();
  let mut calls = 0;

  let links = LinkValues::parse_values(std::iter::from_fn(|| {
    calls += 1;
    assert!(calls <= 3, "parser must inspect every field");
    values.next()
  }))
  .expect("multiple fields form one Link list");

  assert_eq!(2, links.len());
  assert_eq!("/a.css", links.values()[0].target());
  assert_eq!("/b.css", links.values()[1].target());
}

#[test]
fn link_rejects_malformed_values() {
  for value in [
    "",
    "style.css; rel=preload",
    "<style.css; rel=preload",
    "</style.css> rel=preload",
    "</style.css>; =preload",
    "</style.css>; bad name=value",
    "</style.css>; rel=\"unterminated",
    "<>",
    "<http://>",
    "<http://exa mple.test/>",
    "<a\rb>",
    "</a.css>,, </b.css>",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  assert!(
    LinkValues::parse_values([]).is_err(),
    "empty field sets must be rejected"
  );
}

#[test]
fn link_accepts_http_ows_around_members_parameters_and_quoted_suffixes() {
  let padded = LinkValues::parse(" \t</style.css>\t;\trel\t=\t\"preload\" \t")
    .expect("HTTP OWS around members, parameters, and quoted suffixes should parse");
  assert_eq!(1, padded.len());
  assert_eq!("/style.css", padded.values()[0].target());
  assert_eq!(Some("preload"), padded.values()[0].parameter("rel"));

  let list = LinkValues::parse("\t</a.css>; rel=preload\t,\t</b.css>; rel=prefetch\t")
    .expect("HTTP OWS around list members should parse");
  assert_eq!(2, list.len());
  assert_eq!("/a.css", list.values()[0].target());
  assert_eq!(Some("preload"), list.values()[0].parameter("rel"));
  assert_eq!("/b.css", list.values()[1].target());
  assert_eq!(Some("prefetch"), list.values()[1].parameter("rel"));
}

#[test]
fn link_rejects_non_htab_controls_and_non_ows_member_whitespace() {
  for value in [
    "\r</style.css>; rel=preload",
    "</style.css>; rel=preload\r",
    "\n</style.css>; rel=preload",
    "</a.css>,\r</b.css>",
    "</a.css>,\n</b.css>",
    "</style.css>; rel=preload\u{0b}",
    "\u{0b}</style.css>; rel=preload",
    "</style.css>; rel=preload\u{0c}",
    "</style.css>; rel=preload\u{7f}",
    "</style.css>; rel=preload\u{00}",
    "</style.css>; rel=preload\u{01}",
    "\u{00a0}</style.css>; rel=preload",
    "</style.css>; rel=preload\u{00a0}",
    "</a.css>,\u{00a0}</b.css>",
    "</style.css>; rel=preload\u{0085}",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must reject CR, LF, VT, FF, DEL, other controls, and non-OWS whitespace"
    );
  }
}

#[test]
fn link_rejects_non_ows_quoted_parameter_suffixes() {
  for value in [
    r#"</style.css>; rel="preload"extra"#,
    r#"</style.css>; rel="preload" extra"#,
    "</style.css>; rel=\"preload\"\u{00a0}",
    "</style.css>; rel=\"preload\"\u{0b}",
    "</style.css>; rel=\"preload\"\n",
    "</style.css>; rel=\"preload\"\r",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must reject a non-OWS quoted-parameter suffix"
    );
  }

  let links = LinkValues::parse("</style.css>; rel=\"preload\" ")
    .expect("quoted-string OWS suffix should parse");
  assert_eq!(Some("preload"), links.values()[0].parameter("rel"));
}

#[test]
fn link_rejects_targets_url_parse_would_canonicalize() {
  for value in [
    "<foo bar>",
    "<foo\tbar>",
    r"<foo\bar>",
    "<a%zz>",
    "<a%2>",
    "<a%>",
    "<foo\"bar>",
    "<foo^bar>",
    "<foo`bar>",
    "<foo|bar>",
    "<café>",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }
}

#[test]
fn link_rejects_empty_assigned_parameter_values() {
  for value in [
    "</asset>; rel=",
    "</asset>; rel= ",
    "</asset>; rel =",
    "</asset>; rel = ",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must be rejected"
    );
  }

  let links = LinkValues::parse("</asset>; rel=\"\"")
    .expect("an empty quoted-string is a valid assigned value");
  assert_eq!(Some(""), links.values()[0].parameter("rel"));
}

#[test]
fn link_rejects_case_insensitive_duplicate_parameters_within_a_value() {
  assert!(LinkValues::parse("</a.css>; rel=preload; rel=prefetch").is_err());
  assert!(LinkValues::parse("</a.css>; rel=preload; REL=prefetch").is_err());

  let links = LinkValues::parse_values(["</a.css>; rel=preload", "</b.css>; rel=prefetch"])
    .expect("the same parameter on different values must be retained");
  assert_eq!(2, links.len());
}

#[test]
fn link_enforces_value_parameter_and_count_bounds() {
  assert!(LinkValues::parse("a".repeat(MAX_LINK_VALUE_BYTES + 1)).is_err());

  let too_many_values = (0..=MAX_LINK_VALUES)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(LinkValues::parse(too_many_values).is_err());

  let too_many_parameters = format!(
    "</asset>{}",
    (0..=MAX_LINK_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  assert!(LinkValues::parse(too_many_parameters).is_err());

  assert!(LinkValues::parse(format!(
    "</asset>; title={}",
    "a".repeat(MAX_LINK_PARAMETER_VALUE_BYTES + 1)
  ))
  .is_err());
}

#[test]
fn link_parse_error_is_public_and_displayable() {
  let error: LinkParseError =
    LinkValues::parse("not a link").expect_err("malformed Link must be rejected");
  assert!(!error.to_string().is_empty());
}

#[test]
fn link_values_expose_typed_accessors() {
  let links = LinkValues::parse("</style.css>; rel=preload").expect("valid Link must parse");
  let value: &LinkValue = &links.values()[0];

  assert_eq!("/style.css", value.target());
  assert_eq!(1, value.parameters().len());
  assert_eq!("rel", value.parameters()[0].name());
  assert_eq!("preload", value.parameters()[0].value());
  assert_eq!(Some("preload"), value.parameter("REL"));
}

#[test]
fn link_accepts_targets_containing_legal_uri_delimiters() {
  let links = LinkValues::parse(
    "</a,b>; rel=self, </search?q=foo=bar&x=1>; rel=search, \
     <https://ex.test:8443/a#frag>; rel=describedby, \
     </a!$&'()*+,;=>; rel=up, <../a/./b?size=small#v1>; rel=next",
  )
  .expect("URI-reference delimiters inside targets should parse");

  assert_eq!(5, links.len());
  assert_eq!("/a,b", links.values()[0].target());
  assert_eq!("/search?q=foo=bar&x=1", links.values()[1].target());
  assert_eq!("https://ex.test:8443/a#frag", links.values()[2].target());
  assert_eq!("/a!$&'()*+,;=", links.values()[3].target());
  assert_eq!("../a/./b?size=small#v1", links.values()[4].target());
}

#[test]
fn link_parses_quoted_and_extended_parameters() {
  let links = LinkValues::parse(
    "</TheBook/chapter2>; rel=previous; title*=UTF-8'de'letztes%20Kapitel; \
     type=\"application/json\"; title=\"a,b;c=d\"; media=\"screen and (min-width: 600px)\"",
  )
  .expect("quoted and extended parameters should parse");

  assert_eq!("/TheBook/chapter2", links.values()[0].target());
  assert_eq!(Some("previous"), links.values()[0].parameter("rel"));
  assert_eq!(
    Some("UTF-8'de'letztes%20Kapitel"),
    links.values()[0].parameter("title*")
  );
  assert_eq!(
    Some("application/json"),
    links.values()[0].parameter("type")
  );
  assert_eq!(Some("a,b;c=d"), links.values()[0].parameter("title"));
  assert_eq!(
    Some("screen and (min-width: 600px)"),
    links.values()[0].parameter("media")
  );
  assert_eq!(
    vec![
      ("rel", "previous"),
      ("title*", "UTF-8'de'letztes%20Kapitel"),
      ("type", "application/json"),
      ("title", "a,b;c=d"),
      ("media", "screen and (min-width: 600px)"),
    ],
    links.values()[0]
      .parameters()
      .iter()
      .map(|parameter: &LinkParameter| (parameter.name(), parameter.value()))
      .collect::<Vec<_>>()
  );
}

#[test]
fn link_handles_token_quoted_and_extension_relation_types() {
  let links = LinkValues::parse(
    "</a>; REL=Preload, </b>; rel=\"start next\", </c>; rel=\"http://example.net/foo\"",
  )
  .expect("relation types should parse");

  assert_eq!(Some("Preload"), links.values()[0].parameter("rel"));
  assert_eq!("rel", links.values()[0].parameters()[0].name());
  assert_eq!(Some("start next"), links.values()[1].parameter("rel"));
  assert_eq!(
    Some("http://example.net/foo"),
    links.values()[2].parameter("rel")
  );

  for value in [
    "</x>; rel=http://example.net/foo",
    "</x>; rel=preload prefetch",
    "</x>; type=application/json",
    "</x>; anchor=/app",
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must quote non-token parameter values"
    );
  }
}

#[test]
fn link_rejects_unterminated_quoting_and_dangling_escapes() {
  for value in [
    r#"</style.css>; title="unterminated"#,
    r#"</style.css>; title="abc\"#,
    r#"</style.css>; title="abc\""#,
    r#"</style.css>; title="a"b"#,
    r#"</a>; title="x, </b>"#,
  ] {
    assert!(
      LinkValues::parse(value).is_err(),
      "{value:?} must reject unterminated or dangling quoted-strings"
    );
  }
}

#[test]
fn link_enforces_aggregate_limits_across_repeated_fields() {
  let at_limit = (0..MAX_LINK_VALUES)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>();
  let parsed = LinkValues::parse_values(at_limit.iter().map(String::as_str))
    .expect("256 Link values across fields should parse");
  assert_eq!(MAX_LINK_VALUES, parsed.len());

  let too_many = (0..=MAX_LINK_VALUES)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>();
  assert!(
    LinkValues::parse_values(too_many.iter().map(String::as_str)).is_err(),
    "257 Link values across fields must be rejected"
  );

  let first_field = (0..MAX_LINK_VALUES)
    .map(|index| format!("</asset-{index}>"))
    .collect::<Vec<_>>()
    .join(", ");
  assert!(
    LinkValues::parse_values([first_field.as_str(), "</overflow>"]).is_err(),
    "the cumulative value cap must apply across repeated fields"
  );

  let oversized = "x".repeat(MAX_LINK_VALUE_BYTES + 1);
  assert!(
    LinkValues::parse_values(["</ok>", oversized.as_str()]).is_err(),
    "an oversized later field must still be rejected"
  );

  let at_parameter_limit = format!(
    "</asset>{}",
    (0..MAX_LINK_PARAMETERS)
      .map(|index| format!("; p{index}=v"))
      .collect::<String>()
  );
  let parameters = LinkValues::parse(&at_parameter_limit)
    .expect("256 parameters should parse")
    .values()[0]
    .parameters()
    .len();
  assert_eq!(MAX_LINK_PARAMETERS, parameters);
}

#[test]
fn link_serializes_canonical_header_values_and_round_trips() {
  let padded = LinkValues::parse(
    " \t</style.css>\t;\trel\t=\t\"preload\" \t, \
     </b.css>; NOPUSH; title=\"\"; type=\"application/json\"",
  )
  .expect("OWS, valueless, empty quoted, and quoted parameters should parse");
  assert_eq!(
    "</style.css>; rel=preload, </b.css>; nopush; title=\"\"; type=\"application/json\"",
    padded.header_value()
  );
  assert_eq!(
    padded,
    LinkValues::parse(padded.header_value()).expect("canonical Link output should reparse")
  );

  let escaped = LinkValues::parse(r#"</style.css>; title="say \"hi\" and \\""#)
    .expect("quoted-string escapes should parse");
  assert_eq!(
    r#"</style.css>; title="say \"hi\" and \\""#,
    escaped.header_value()
  );
  assert_eq!(
    escaped,
    LinkValues::parse(escaped.header_value()).expect("escaped canonical output should reparse")
  );

  let delimited = LinkValues::parse(
    "</a,b>; rel=self, </c;d?e=f#g>; rel=\"start next\", \
     </TheBook/chapter2>; title*=UTF-8'de'letztes%20Kapitel",
  )
  .expect("delimited targets and extended parameters should parse");
  assert_eq!(
    "</a,b>; rel=self, </c;d?e=f#g>; rel=\"start next\", \
     </TheBook/chapter2>; title*=UTF-8'de'letztes%20Kapitel",
    delimited.header_value()
  );
  assert_eq!(
    delimited,
    LinkValues::parse(delimited.header_value()).expect("delimited canonical output should reparse")
  );

  let repeated = LinkValues::parse("</a.css>; rel=stylesheet, </b.css>; rel=stylesheet")
    .expect("repeated links should parse");
  assert_eq!(
    "</a.css>; rel=stylesheet, </b.css>; rel=stylesheet",
    repeated.header_value()
  );
  assert_eq!(
    repeated,
    LinkValues::parse(repeated.header_value()).expect("repeated canonical output should reparse")
  );

  let obs_text =
    LinkValues::parse(r#"</style.css>; title="\é""#).expect("escaped obs-text should parse");
  assert_eq!(r#"</style.css>; title="é""#, obs_text.header_value());
  assert_eq!(
    obs_text,
    LinkValues::parse(obs_text.header_value()).expect("obs-text canonical output should reparse")
  );
}
