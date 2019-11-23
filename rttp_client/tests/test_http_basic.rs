use std::collections::HashMap;

use rttp_client::{Config, HttpClient};
use rttp_client::types::{Para, Proxy, RoUrl};

fn client() -> HttpClient {
  HttpClient::new()
}

#[test]
fn test_http() {
  let response = client()
    .url("http://httpbin.org/get")
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
  let response = client()
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
  let response = client()
    .get()
    .url("http://httpbin.org/get")
    .header(("Accept-Encoding", "gzip, deflate"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
fn test_upload() {
  let response = client()
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
  client()
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
  client()
    .method("post")
    .url("http://httpbin.org/post")
    .para(Para::new("name", "Chico"))
    .raw("name=Nick&name=Wendy")
    .content_type("application/x-www-form-urlencoded")
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(any(feature = "tls-rustls", feature = "tls-native"))]
fn test_https() {
  let response = client()
    .get()
    .url("https://bing.com")
    .para(Para::new("q", "News"))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}

#[test]
#[cfg(any(feature = "tls-native"))]
// feature = "tls-rustls",  // rustls request httpbin.org will throw exception // "CloseNotify alert received"
fn test_http_with_url() {
  client()
    .method("get")
    .url(RoUrl::with("https://httpbin.org").path("/get").para(("name", "Chico")))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[cfg(any(feature = "tls-rustls", feature = "tls-native"))]
#[ignore]
fn test_with_proxy_http() {
  client()
    .get()
    .url("https://google.com")
    .proxy(Proxy::http("127.0.0.1", 1081))
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
#[ignore]
fn test_with_proxy_socks5() {
  let response = client()
    .get()
    .url("http://google.com")
    .proxy(Proxy::socks5("127.0.0.1", 1080))
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("google.com", response.host());
  println!("{}", response);
}

#[test]
fn test_auto_redirect() {
  let response = client()
    .config(Config::builder().auto_redirect(true))
    .get()
    .url("http://bing.com")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_ne!("bing.com", response.host());
}

#[test]
fn test_connection_closed() {
  let mut client = client();
  let resp0 = client.url("http://httpbin.org/get").emit();
  assert!(resp0.is_ok());
  let resp1 = client.post().url("http://httpbin.org/post").emit();
  assert!(resp1.is_err());
  let resp2 = self::client().url("http://httpbin.org/get").emit();
  assert!(resp2.is_ok());
  let resp3 = self::client().post().url("http://httpbin.org/post").emit();
  assert!(resp3.is_ok());
  let resp4 = client.reset().post().url("http://httpbin.org/post").emit();
  assert!(resp4.is_ok());
}
