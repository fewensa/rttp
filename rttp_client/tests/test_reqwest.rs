use std::collections::HashMap;

#[test]
fn test_reqwest_basic() {
  let resp: HashMap<String, String> = reqwest::Client::new()
    .get("https://httpbin.org/ip")
    .header("k", "v")
    .header("z", "y")
    .form(&[("one", "1")])
    .send()
    .expect("Request fail")
    .json()
    .expect("Request fail");
  println!("{:#?}", resp);
}

#[test]
fn test_reqwest_ip2() {
  let mut client = reqwest::Client::new()
    .get("https://httpbin.org/ip");
  client = client.header("k", "v")
    .header("z", "y")
    .form(&[("one", "1")]);
  let resp: HashMap<String, String> = client
    .send()
    .expect("Request fail")
    .json()
    .expect("Request fail");
  println!("{:#?}", resp);
}
