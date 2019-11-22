use rttp_client::HttpClient;

pub struct Http {}


impl Http {
  #[cfg(feature = "client")]
  pub fn client() -> HttpClient {
    HttpClient::new()
  }
}
