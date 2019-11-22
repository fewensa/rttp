use rttp::Http;

#[test]
fn test_get() {
  let response = Http::client()
    .url("http://httpbin.org/get")
    .emit();
  assert!(response.is_ok());
  let response = response.unwrap();
  println!("{}", response);
}
