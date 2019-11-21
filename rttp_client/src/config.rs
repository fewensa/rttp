//use std::{collections::HashMap, sync::Mutex};
//
//use once_cell::sync::Lazy;
//
//static DEFAULT_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| {
//  let mut config = ;
//  Mutex::new(config)
//});


#[derive(Clone, Debug)]
pub struct Config {
  read_timeout: u64,
  write_timeout: u64,
  auto_redirect: bool,
  max_redirect: u32,
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
  pub fn read_timeout(&self) -> u64 { self.read_timeout }
  pub fn write_timeout(&self) -> u64 { self.write_timeout }
  pub fn auto_redirect(&self) -> bool { self.auto_redirect }
  pub fn max_redirect(&self) -> u32 { self.max_redirect }
}


#[derive(Clone, Debug)]
pub struct ConfigBuilder {
  config: Config
}

impl ConfigBuilder {
  pub fn new() -> Self {
    Self {
      config: Config {
        read_timeout: 5000,
        write_timeout: 5000,
        auto_redirect: false,
        max_redirect: 3,
      }
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
    self
  }
  pub fn max_redirect(&mut self, max_redirect: u32) -> &mut Self {
    self.config.max_redirect = max_redirect;
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
