pub struct Http {}

impl Http {
  #[cfg(any(
    feature = "all",
    feature = "client",
    feature = "client_tls_rustls",
    feature = "client_tls_native",
  ))]
  pub fn client() -> rttp_client::HttpClient {
    rttp_client::HttpClient::new()
  }
}
