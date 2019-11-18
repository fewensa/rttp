use rttp_client::Http;
use rttp_client::types::RoUrl;

#[test]
fn test_http() {
  Http::client()
    .method("get")
//    .url(&format!("Host:{}", "httpbin.org")[..])
    .url("http://httpbin.org/get?id=1&name=jack")
    .path("get")
    .header("User-Agent: Mozilla/5.0")
    .header(&format!("Host:{}", "httpbin.org"))
    .para("name=Chico")
    .para("name=文")
    .cookie("token=123234")
    .cookie("uid=abcdef")
    .content_type("application/x-www-form-urlencoded")
    .encode(true)
    .traditional(true)
    .raw("age=10&from=rttp")
    .emit()
    .expect("REQUEST FAIL");
}

#[test]
fn test_http_with_url() {
  Http::client()
    .method("get")
    .url(RoUrl::with("https://httpbin.org").path("/get").para(("name", "Chico")))
    .emit();
}
