use rttp_client::Http;
use rttp_client::types::RoUrl;

#[test]
fn test_http() {
  Http::client()
    .method("get")
    .url("https://httpbin.org/get")
    .emit();
}

#[test]
fn test_http_with_url() {
  Http::client()
    .method("get")
    .url(RoUrl::with("https://httpbin.org").path("/get").para(("name", "Chico")))
    .emit();
}
