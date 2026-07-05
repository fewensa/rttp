pub struct Http {}

pub mod server;

impl Http {
  #[cfg(feature = "client")]
  pub fn client() -> rttp_client::HttpClient {
    rttp_client::HttpClient::new()
  }
}
