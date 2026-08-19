/// Read-only HTTP `Content-Length` framing metadata.
///
/// This value is intended to be populated from already validated message
/// framing state. It does not parse headers or decide body framing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct HttpContentLength {
  len: usize,
}

impl HttpContentLength {
  pub fn new(len: usize) -> Self {
    Self { len }
  }

  pub fn len(&self) -> usize {
    self.len
  }

  pub fn is_zero(&self) -> bool {
    self.len == 0
  }

  pub fn is_empty(&self) -> bool {
    self.is_zero()
  }

  pub fn header_value(&self) -> String {
    self.len.to_string()
  }
}
