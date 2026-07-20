/// Default maximum number of bytes buffered for a client response body.
pub const DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Local settings for the bounded prior-knowledge h2c client path.
///
/// The default policy leaves both settings unadvertised, retaining the HTTP/2
/// protocol defaults of a 16,384-byte maximum frame size and a 4,096-byte
/// HPACK dynamic table. Configured values are validated before the client
/// opens its h2c socket.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct H2cClientPolicy {
  max_frame_size: Option<usize>,
  header_table_size: Option<usize>,
}

impl H2cClientPolicy {
  pub fn new() -> Self {
    Self::default()
  }

  /// Advertise a local h2c `SETTINGS_MAX_FRAME_SIZE` value.
  pub fn max_frame_size(mut self, max_frame_size: usize) -> Self {
    self.max_frame_size = Some(max_frame_size);
    self
  }

  /// Advertise a local h2c `SETTINGS_HEADER_TABLE_SIZE` value.
  pub fn header_table_size(mut self, header_table_size: usize) -> Self {
    self.header_table_size = Some(header_table_size);
    self
  }

  pub fn configured_max_frame_size(&self) -> Option<usize> {
    self.max_frame_size
  }

  pub fn configured_header_table_size(&self) -> Option<usize> {
    self.header_table_size
  }
}

#[derive(Clone, Debug)]
pub struct Config {
  connect_timeout: u64,
  read_timeout: u64,
  write_timeout: u64,
  auto_redirect: bool,
  max_redirect: u32,
  allow_https_to_http_redirects: bool,
  verify_ssl_hostname: bool,
  verify_ssl_cert: bool,
  max_buffered_response_body_bytes: usize,
  http2_max_frame_size: Option<usize>,
  http2_header_table_size: Option<usize>,
}

impl Default for Config {
  fn default() -> Self {
    Config::builder()
      .connect_timeout(10000)
      .read_timeout(10000)
      .write_timeout(10000)
      .auto_redirect(false)
      .max_redirect(0)
      .allow_https_to_http_redirects(false)
      .build()
  }
}

impl Config {
  pub fn builder() -> ConfigBuilder {
    ConfigBuilder::new()
  }
}

impl Config {
  /// Return the TCP connect timeout in milliseconds.
  ///
  /// Each resolved address receives this timeout independently. The default
  /// is 10,000 milliseconds.
  pub fn connect_timeout(&self) -> u64 {
    self.connect_timeout
  }
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
  pub fn allow_https_to_http_redirects(&self) -> bool {
    self.allow_https_to_http_redirects
  }
  pub fn verify_ssl_cert(&self) -> bool {
    self.verify_ssl_cert
  }
  pub fn verify_ssl_hostname(&self) -> bool {
    self.verify_ssl_hostname
  }
  pub fn max_buffered_response_body_bytes(&self) -> usize {
    self.max_buffered_response_body_bytes
  }
  pub fn http2_max_frame_size(&self) -> Option<usize> {
    self.http2_max_frame_size
  }
  pub fn http2_header_table_size(&self) -> Option<usize> {
    self.http2_header_table_size
  }
  /// Return the bounded prior-knowledge h2c settings configured for this request.
  pub fn h2c_policy(&self) -> H2cClientPolicy {
    H2cClientPolicy {
      max_frame_size: self.http2_max_frame_size,
      header_table_size: self.http2_header_table_size,
    }
  }

  pub(crate) fn h2c_policy_set(&mut self, policy: H2cClientPolicy) {
    self.http2_max_frame_size = policy.max_frame_size;
    self.http2_header_table_size = policy.header_table_size;
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
        connect_timeout: 10000,
        read_timeout: 10000,
        write_timeout: 10000,
        auto_redirect: false,
        max_redirect: 0,
        allow_https_to_http_redirects: false,
        verify_ssl_hostname: true,
        verify_ssl_cert: true,
        max_buffered_response_body_bytes: DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
        http2_max_frame_size: None,
        http2_header_table_size: None,
      },
    }
  }

  pub fn build(&self) -> Config {
    self.config.clone()
  }

  /// Set the TCP connect timeout in milliseconds for each resolved address.
  ///
  /// The value must be greater than zero. Addresses are attempted in resolver
  /// order, so the total connect time is bounded by the number of resolved
  /// addresses multiplied by this timeout.
  pub fn connect_timeout(&mut self, connect_timeout: u64) -> &mut Self {
    self.config.connect_timeout = connect_timeout;
    self
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
  /// Allow automatic redirects from HTTPS URLs to HTTP URLs.
  ///
  /// This is disabled by default because following such a redirect removes
  /// transport security from the request.
  pub fn allow_https_to_http_redirects(
    &mut self,
    allow_https_to_http_redirects: bool,
  ) -> &mut Self {
    self.config.allow_https_to_http_redirects = allow_https_to_http_redirects;
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
  /// Set the maximum number of wire or decoded body bytes buffered by a response.
  ///
  /// This limit does not apply when callers consume a streaming response body
  /// directly.
  pub fn max_buffered_response_body_bytes(
    &mut self,
    max_buffered_response_body_bytes: usize,
  ) -> &mut Self {
    self.config.max_buffered_response_body_bytes = max_buffered_response_body_bytes;
    self
  }
  /// Configure local settings for the bounded prior-knowledge h2c client path.
  pub fn h2c_policy(&mut self, policy: H2cClientPolicy) -> &mut Self {
    self.config.h2c_policy_set(policy);
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
  /// Configure the local h2c `SETTINGS_HEADER_TABLE_SIZE` value.
  ///
  /// This applies only to the bounded prior-knowledge h2c client path. The
  /// value is advertised to the peer and bounds the HPACK dynamic table used
  /// while decoding inbound response HEADERS and trailing HEADERS.
  pub fn http2_header_table_size(&mut self, header_table_size: usize) -> &mut Self {
    self.config.http2_header_table_size = Some(header_table_size);
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
      .connect_timeout(2468)
      .read_timeout(1234)
      .write_timeout(4321)
      .auto_redirect(true)
      .max_redirect(5)
      .allow_https_to_http_redirects(true)
      .verify_ssl_hostname(false)
      .verify_ssl_cert(false)
      .max_buffered_response_body_bytes(123)
      .http2_max_frame_size(16_384)
      .http2_header_table_size(64)
      .build();

    assert_eq!(config.connect_timeout(), 2468);
    assert_eq!(config.read_timeout(), 1234);
    assert_eq!(config.write_timeout(), 4321);
    assert!(config.auto_redirect());
    assert_eq!(config.max_redirect(), 5);
    assert!(config.allow_https_to_http_redirects());
    assert!(!config.verify_ssl_hostname());
    assert!(!config.verify_ssl_cert());
    assert_eq!(123, config.max_buffered_response_body_bytes());
    assert_eq!(Some(16_384), config.http2_max_frame_size());
    assert_eq!(Some(64), config.http2_header_table_size());
  }

  #[test]
  fn default_config_matches_builder_defaults() {
    let default_config = Config::default();
    let builder_config = Config::builder().build();

    assert_eq!(
      DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
      default_config.max_buffered_response_body_bytes()
    );
    assert!(default_config.max_buffered_response_body_bytes() > 0);
    assert_eq!(
      default_config.connect_timeout(),
      builder_config.connect_timeout()
    );
    assert_eq!(default_config.connect_timeout(), 10_000);
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
      default_config.allow_https_to_http_redirects(),
      builder_config.allow_https_to_http_redirects()
    );
    assert_eq!(
      default_config.verify_ssl_hostname(),
      builder_config.verify_ssl_hostname()
    );
    assert_eq!(
      default_config.verify_ssl_cert(),
      builder_config.verify_ssl_cert()
    );
    assert_eq!(
      default_config.max_buffered_response_body_bytes(),
      builder_config.max_buffered_response_body_bytes()
    );
    assert_eq!(
      default_config.http2_max_frame_size(),
      builder_config.http2_max_frame_size()
    );
    assert_eq!(
      default_config.http2_header_table_size(),
      builder_config.http2_header_table_size()
    );
  }

  #[test]
  fn enabling_redirects_sets_a_reasonable_default_limit() {
    let config = Config::builder().auto_redirect(true).build();

    assert!(config.auto_redirect());
    assert_eq!(config.max_redirect(), 5);
  }
}
