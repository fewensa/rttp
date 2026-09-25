use rttp_server::server::HttpIfRange;

const DATE: &str = "Sun, 06 Nov 1994 08:49:37 GMT";

#[test]
fn accepts_http_ows_around_strong_tags_and_dates() {
  for padding in [" ", "\t", " \t", "\t "] {
    let entity_tag = format!("{padding}\"revision-42\"{padding}");
    assert!(
      matches!(
        HttpIfRange::parse(entity_tag),
        Ok(HttpIfRange::EntityTag(_))
      ),
      "HTTP OWS should be accepted around an entity tag"
    );

    let date = format!("{padding}{DATE}{padding}");
    assert!(
      matches!(HttpIfRange::parse(date), Ok(HttpIfRange::Date(_))),
      "HTTP OWS should be accepted around an HTTP-date"
    );
  }
}

#[test]
fn rejects_non_ows_padding_around_strong_tags_and_dates() {
  for whitespace in ["\r", "\n", "\u{000b}", "\u{000c}", "\u{00a0}", "\u{2003}"] {
    for value in [
      format!("{whitespace}\"revision-42\""),
      format!("\"revision-42\"{whitespace}"),
      format!("{whitespace}{DATE}"),
      format!("{DATE}{whitespace}"),
    ] {
      assert!(
        HttpIfRange::parse(&value).is_err(),
        "non-OWS padding should be rejected: {value:?}"
      );
    }
  }
}

#[test]
fn preserves_strong_entity_tag_requirement() {
  assert!(
    HttpIfRange::parse(" \tW/\"revision-42\"\t ").is_err(),
    "weak entity tags should remain invalid If-Range validators"
  );
}
