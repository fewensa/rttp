use rttp_client::types::{IntoPara, RoUrl, ToUrl};
use url::Url;

#[test]
fn url_parse_percent_encodes_unicode_query_values() {
  let url = Url::parse("https://example.test/get?name=文山").expect("INVALID URL");

  assert_eq!(
    url.as_str(),
    "https://example.test/get?name=%E6%96%87%E5%B1%B1"
  );
}

#[test]
fn rourl_joins_paths_and_preserves_credentials_query_order_and_fragment() {
  let url = RoUrl::with("https://example.test/get/?name=a&name=b#section-1")
    .path("//test/")
    .path("/a")
    .para("name[]=文")
    .para(("name", "I"))
    .para(("name", "Z", "name=K", "name=O&name=P"))
    .username("Tom")
    .password("1123")
    .traditional(true)
    .to_url()
    .expect("BAD URL");

  assert_eq!(
    url.as_str(),
    "https://Tom:1123@example.test/get/test/a?name=a&name=b&name[]=%E6%96%87&name=I&name=Z&name=K&name=O&name=P#section-1"
  );
}

#[test]
fn rourl_uses_array_style_for_duplicate_query_parameters_when_not_traditional() {
  let url = RoUrl::with("https://example.test/get?tag=one")
    .para(("tag", "two"))
    .para("name[]=文")
    .para(("single", "value"))
    .traditional(false)
    .to_url()
    .expect("BAD URL");

  assert_eq!(
    url.as_str(),
    "https://example.test/get?tag[]=one&tag[]=two&name[]=%E6%96%87&single=value"
  );
}

#[test]
fn rourl_round_trips_generated_urls_without_dropping_query_parameters() {
  let url = RoUrl::with("https://example.test/get?name=a&name=b#section-1")
    .path("child")
    .para(("name", "c"))
    .to_url()
    .expect("BAD URL");

  let rourl: RoUrl = url.into();
  let round_tripped = rourl.to_url().expect("BAD URL");

  assert_eq!(
    round_tripped.as_str(),
    "https://example.test/get/child?name=a&name=b&name=c#section-1"
  );
}

#[test]
fn rourl_preserves_equals_signs_in_query_parameter_values() {
  let url = RoUrl::with("https://example.test/get")
    .para("token=a=b&signed=part1=part2")
    .to_url()
    .expect("BAD URL");

  let query_pairs: Vec<_> = url.query_pairs().into_owned().collect();

  assert_eq!(
    query_pairs,
    vec![
      ("token".to_string(), "a=b".to_string()),
      ("signed".to_string(), "part1=part2".to_string()),
    ]
  );
  assert!(url.as_str().contains("token=a=b"));
  assert!(url.as_str().contains("signed=part1=part2"));
}

#[test]
fn rourl_preserves_equals_signs_in_initial_url_query_values() {
  let url = RoUrl::with("https://example.test/get?token=a=b&signed=part1=part2")
    .to_url()
    .expect("BAD URL");

  let query_pairs: Vec<_> = url.query_pairs().into_owned().collect();

  assert_eq!(
    query_pairs,
    vec![
      ("token".to_string(), "a=b".to_string()),
      ("signed".to_string(), "part1=part2".to_string()),
    ]
  );
  assert!(url.as_str().contains("token=a=b"));
  assert!(url.as_str().contains("signed=part1=part2"));
}

#[test]
fn string_form_parameters_preserve_equals_signs_in_values() {
  let paras = "token=a=b&empty=&key-only&signed=part1=part2".into_paras();
  let parsed: Vec<_> = paras
    .iter()
    .map(|para| {
      (
        para.name().to_string(),
        para.value().clone().unwrap_or_default(),
      )
    })
    .collect();

  assert_eq!(
    parsed,
    vec![
      ("token".to_string(), "a=b".to_string()),
      ("empty".to_string(), "".to_string()),
      ("key-only".to_string(), "".to_string()),
      ("signed".to_string(), "part1=part2".to_string()),
    ]
  );
}
