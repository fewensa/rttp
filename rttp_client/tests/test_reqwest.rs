use std::collections::HashMap;

#[test]
fn test_reqwest_basic() {
  let resp: HashMap<String, String> = reqwest::get("https://httpbin.org/ip")
    .expect("Build request fail")
    .json()
    .expect("Request fail");
  println!("{:#?}", resp);
}
