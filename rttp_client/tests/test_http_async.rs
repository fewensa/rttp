
use rttp_client::{Config, HttpClient};

fn client() -> HttpClient {
  HttpClient::new()
}

#[test]
fn test_async_http() {
  let response = client()
    .url("http://httpbin.org/get")
    .enqueue();
  assert!(response.is_ok());
  let response = response.unwrap();
  assert_eq!("httpbin.org", response.host());
  println!("{}", response);
}

