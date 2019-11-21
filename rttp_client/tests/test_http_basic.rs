use std::collections::HashMap;

use rttp_client::{Http, Config};
use rttp_client::types::{Para, Proxy, RoUrl};

#[test]
fn test_http() {
  let response = Http::client()
    .url("https://httpbin.org/get")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("httpbin.org", response.host());
  println!("{}", response);
}

#[test]
fn test_multi() {
  let mut para_map = HashMap::new();
  para_map.insert("id", "1");
  para_map.insert("relation", "eq");
  let response = Http::client()
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
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
fn test_gzip() {
  let response = Http::client()
    .get()
    .url("https://httpbin.org/get")
    .header(("Accept-Encoding", "gzip, deflate"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
fn test_upload() {
  let response = Http::client()
    .method("post")
    .url("http://httpbin.org")
    .path("post")
    .form(("debug", "true", "name=Form&file=@cargo#../Cargo.toml"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
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
#[ignore]
fn test_with_proxy_http() {
  Http::client()
    .get()
    .url("https://google.com")
    .proxy(Proxy::http("127.0.0.1", 1081))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[ignore]
fn test_with_proxy_socks5() {
  Http::client()
    .get()
    .url("http://google.com")
    .proxy(Proxy::socks5("127.0.0.1", 1080))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_auto_redirect() {
  let response = Http::client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url("http://bing.com")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_ne!("bing.com", response.host());
}
