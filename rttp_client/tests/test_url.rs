use rttp_client::types::{RoUrl, ToUrl};
use url::Url;

#[test]
fn test_url_gen() {
  let result = Url::parse("https://example.test/get?name=文山");
  let url = result.expect("INVALID URL");
  println!("{}  ", url.as_str());
}

#[test]
fn test_rourl_gen() {
  let url = RoUrl::with("https://example.test/get/?name=a&name=b")
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
  println!("{}", url);

  let rourl: RoUrl = url.into();
  println!("{:?}", rourl);
  println!("{}", rourl.to_url().expect("BAD URL"));
}
