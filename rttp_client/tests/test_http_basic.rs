use rttp_client::Http;

#[test]
fn test_http() {
  Http::client()
    .method("get")
//    .url("https://httpbin.org/get")
    .emit();
}
