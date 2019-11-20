
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ProxyType {
  HTTP,
  HTTPS,
  SOCKS4,
  SOCKS5,
}

#[derive(Clone, Debug)]
pub struct Proxy {
  host: String,
  port: u32,
  username: Option<String>,
  password: Option<String>,
  type_: ProxyType,
}

impl Proxy {

  pub fn builder(type_: ProxyType) -> ProxyBuilder {
    ProxyBuilder::new(type_)
  }

  pub fn http_with_authorization<H: AsRef<str>, U: AsRef<str>, P: AsRef<str>>(host: H, port: u32, username: U, password: P) -> Self {
    Self::builder(ProxyType::HTTP)
      .host(host)
      .port(port)
      .username(username)
      .password(password)
      .build()
  }

  pub fn http<H: AsRef<str>>(host: H, port: u32) -> Self {
    Self::builder(ProxyType::HTTP)
      .host(host)
      .port(port)
      .build()
  }

  pub fn https_with_authorization<H: AsRef<str>, U: AsRef<str>, P: AsRef<str>>(host: H, port: u32, username: U, password: P) -> Self {
    Self::builder(ProxyType::HTTPS)
      .host(host)
      .port(port)
      .username(username)
      .password(password)
      .build()
  }

  pub fn https<H: AsRef<str>>(host: H, port: u32) -> Self {
    Self::builder(ProxyType::HTTPS)
      .host(host)
      .port(port)
      .build()
  }

  pub fn socks4<H: AsRef<str>>(host: H, port: u32) -> Self {
    Self::builder(ProxyType::SOCKS4)
      .host(host)
      .port(port)
      .build()
  }

  pub fn socks4_with_authorization<H: AsRef<str>, U: AsRef<str>, P: AsRef<str>>(host: H, port: u32, username: U, password: P) -> Self {
    Self::builder(ProxyType::SOCKS4)
      .host(host)
      .port(port)
      .username(username)
      .password(password)
      .build()
  }

  pub fn socks5<H: AsRef<str>>(host: H, port: u32) -> Self {
    Self::builder(ProxyType::SOCKS5)
      .host(host)
      .port(port)
      .build()
  }

  pub fn socks5_with_authorization<H: AsRef<str>, U: AsRef<str>, P: AsRef<str>>(host: H, port: u32, username: U, password: P) -> Self {
    Self::builder(ProxyType::SOCKS5)
      .host(host)
      .port(port)
      .username(username)
      .password(password)
      .build()
  }

  pub fn host(&self) -> &String { &self.host }
  pub fn port(&self) -> u32 { self.port }
  pub fn username(&self) -> &Option<String> { &self.username }
  pub fn password(&self) -> &Option<String> { &self.password }
  pub fn type_(&self) -> &ProxyType { &self.type_ }
}

pub struct ProxyBuilder {
  proxy: Proxy
}

impl ProxyBuilder {
  pub fn new(type_: ProxyType) -> Self {
    Self {
      proxy: Proxy {
        host: "".to_string(),
        port: 0,
        username: None,
        password: None,
        type_,
      }
    }
  }

  pub fn build(&self) -> Proxy {
    self.proxy.clone()
  }

  pub fn host<S: AsRef<str>>(&mut self, host: S) -> &mut Self {
    self.proxy.host = host.as_ref().into();
    self
  }

  pub fn port(&mut self, port: u32) -> &mut Self {
    self.proxy.port = port;
    self
  }

  pub fn username<S: AsRef<str>>(&mut self, username: S) -> &mut Self {
    self.proxy.username = Some(username.as_ref().into());
    self
  }

  pub fn password<S: AsRef<str>>(&mut self, password: S) -> &mut Self {
    self.proxy.password = Some(password.as_ref().into());
    self
  }

}


impl AsRef<Proxy> for Proxy {
  fn as_ref(&self) -> &Proxy {
    self
  }
}

impl AsRef<Proxy> for ProxyBuilder {
  fn as_ref(&self) -> &Proxy {
    &self.proxy
  }
}

