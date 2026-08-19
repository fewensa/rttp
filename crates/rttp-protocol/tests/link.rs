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
