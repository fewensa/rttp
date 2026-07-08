#[derive(Clone, Debug)]
pub struct Config {
  read_timeout: u64,
  write_timeout: u64,
  auto_redirect: bool,
  max_redirect: u32,
  verify_ssl_hostname: bool,
  verify_ssl_cert: bool,
  http2_max_frame_size: Option<usize>,
}

impl Default for Config {
  fn default() -> Self {
    Config::builder()
      .read_timeout(10000)
      .write_timeout(10000)
      .auto_redirect(false)
      .max_redirect(0)
      .build()
  }
}

impl Config {
  pub fn builder() -> ConfigBuilder {
    ConfigBuilder::new()
  }
}

impl Config {
  pub fn read_timeout(&self) -> u64 {
    self.read_timeout
  }
  pub fn write_timeout(&self) -> u64 {
    self.write_timeout
  }
  pub fn auto_redirect(&self) -> bool {
    self.auto_redirect
  }
  pub fn max_redirect(&self) -> u32 {
    self.max_redirect
  }
  pub fn verify_ssl_cert(&self) -> bool {
    self.verify_ssl_cert
  }
  pub fn verify_ssl_hostname(&self) -> bool {
    self.verify_ssl_hostname
  }
  pub fn http2_max_frame_size(&self) -> Option<usize> {
    self.http2_max_frame_size
  }
}

#[derive(Clone, Debug)]
pub struct ConfigBuilder {
  config: Config,
}

impl Default for ConfigBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl ConfigBuilder {
  pub fn new() -> Self {
    Self {
      config: Config {
        read_timeout: 10000,
        write_timeout: 10000,
        auto_redirect: false,
        max_redirect: 0,
        verify_ssl_hostname: true,
        verify_ssl_cert: true,
        http2_max_frame_size: None,
      },
    }
  }

  pub fn build(&self) -> Config {
    self.config.clone()
  }

  pub fn read_timeout(&mut self, read_timeout: u64) -> &mut Self {
    self.config.read_timeout = read_timeout;
    self
  }
  pub fn write_timeout(&mut self, write_timeout: u64) -> &mut Self {
    self.config.write_timeout = write_timeout;
    self
  }
  pub fn auto_redirect(&mut self, auto_redirect: bool) -> &mut Self {
    self.config.auto_redirect = auto_redirect;
    if auto_redirect && self.config.max_redirect == 0 {
      self.config.max_redirect = 5;
    }
    self
  }
  pub fn max_redirect(&mut self, max_redirect: u32) -> &mut Self {
    self.config.max_redirect = max_redirect;
    self
  }
  pub fn verify_ssl_hostname(&mut self, verify_ssl_hostname: bool) -> &mut Self {
    self.config.verify_ssl_hostname = verify_ssl_hostname;
    self
  }
  pub fn verify_ssl_cert(&mut self, verify_ssl_cert: bool) -> &mut Self {
    self.config.verify_ssl_cert = verify_ssl_cert;
    self
  }
  /// Configure the local h2c `SETTINGS_MAX_FRAME_SIZE` value.
  ///
  /// This applies only to the bounded prior-knowledge h2c client path. Values
  /// must be in the legal HTTP/2 range of 16,384 through 16,777,215 bytes; an
  /// out-of-range value is rejected before a socket is opened. When set, the
  /// value is advertised to the peer and inbound frame payloads above that
  /// active local limit are rejected. Outbound request HEADERS, DATA, and
  /// trailing HEADERS are still split to the peer's active
  /// `SETTINGS_MAX_FRAME_SIZE`.
  pub fn http2_max_frame_size(&mut self, max_frame_size: usize) -> &mut Self {
    self.config.http2_max_frame_size = Some(max_frame_size);
    self
  }
}

impl AsRef<Config> for Config {
  fn as_ref(&self) -> &Config {
    self
  }
}

impl AsRef<Config> for ConfigBuilder {
  fn as_ref(&self) -> &Config {
    &self.config
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builder_updates_values() {
    let config = Config::builder()
      .read_timeout(1234)
      .write_timeout(4321)
      .auto_redirect(true)
      .max_redirect(5)
      .verify_ssl_hostname(false)
      .verify_ssl_cert(false)
      .http2_max_frame_size(16_384)
      .build();

    assert_eq!(config.read_timeout(), 1234);
    assert_eq!(config.write_timeout(), 4321);
    assert!(config.auto_redirect());
    assert_eq!(config.max_redirect(), 5);
    assert!(!config.verify_ssl_hostname());
    assert!(!config.verify_ssl_cert());
    assert_eq!(Some(16_384), config.http2_max_frame_size());
  }

  #[test]
  fn default_config_matches_builder_defaults() {
    let default_config = Config::default();
    let builder_config = Config::builder().build();

    assert_eq!(default_config.read_timeout(), builder_config.read_timeout());
    assert_eq!(
      default_config.write_timeout(),
      builder_config.write_timeout()
    );
    assert_eq!(
      default_config.auto_redirect(),
      builder_config.auto_redirect()
    );
    assert_eq!(default_config.max_redirect(), builder_config.max_redirect());
    assert_eq!(
      default_config.verify_ssl_hostname(),
      builder_config.verify_ssl_hostname()
    );
    assert_eq!(
      default_config.verify_ssl_cert(),
      builder_config.verify_ssl_cert()
    );
    assert_eq!(
      default_config.http2_max_frame_size(),
      builder_config.http2_max_frame_size()
    );
  }

  #[test]
  fn enabling_redirects_sets_a_reasonable_default_limit() {
    let config = Config::builder().auto_redirect(true).build();

    assert!(config.auto_redirect());
    assert_eq!(config.max_redirect(), 5);
  }
}
