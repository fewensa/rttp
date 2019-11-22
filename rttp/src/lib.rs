
#[cfg(any(feature = "all", feature = "client"))]
use rttp_client::HttpClient;

pub struct Http {}


impl Http {
  #[cfg(any(feature = "all", feature = "client"))]
  pub fn client() -> HttpClient {
    HttpClient::new()
  }
}
