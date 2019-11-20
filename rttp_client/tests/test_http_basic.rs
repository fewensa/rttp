use rttp_client::Http;
use rttp_client::types::{RoUrl, Para};
use std::collections::HashMap;

#[test]
fn test_http() {
  let mut para_map = HashMap::new();
  para_map.insert("id", "1");
  para_map.insert("relation", "eq");
  Http::client()
    .method("post")
    .url(RoUrl::with("http://httpbin.org?id=1&name=jack#none").para("name=Julia"))
    .path("post")
    .header("User-Agent: Mozilla/5.0")
    .header(&format!("Host:{}", "httpbin.org"))
    .para("name=Chico")
    .para(&"name=文".to_string())
    .para(para_map)
    .form(("debug", "true", "name=Form&file=@cargo#../Cargo.toml"))
    .cookie("token=123234")
    .cookie("uid=abcdef")
    .content_type("application/x-www-form-urlencoded")
    .encode(true)
    .traditional(true)
    .emit()
    .expect("REQUEST FAIL");
}


#[test]
fn test_raw_json() {
  Http::client()
    .method("post")
    .url("http://httpbin.org/post?raw=json")
    .para("name=Chico")
    .content_type("application/json")
    .raw(r#"  {"from": "rttp"} "#)
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_raw_form_urlencoded() {
  Http::client()
    .method("post")
    .url("http://httpbin.org/post")
    .para(Para::new("name", "Chico"))
    .raw("name=Nick&name=Wendy")
    .content_type("application/x-www-form-urlencoded")
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_https() {
  Http::client()
    .get()
    .url("https://httpbin.org/get")
    .para(Para::new("name", "Chico"))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_http_with_url() {
  Http::client()
    .method("get")
    .url(RoUrl::with("https://httpbin.org").path("/get").para(("name", "Chico")))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_with_proxy_http() {
  Http::client()
    .get()
    .url("https://httpbin.org/get")
    .proxy()
    .emit()
    .expect("REQUEST FAIL");
}

