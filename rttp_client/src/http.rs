use crate::HttpClient;

pub struct Http;

impl Http {
  pub fn client() -> HttpClient {
    Default::default()
  }
}
