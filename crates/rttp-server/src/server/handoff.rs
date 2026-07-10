use super::*;

pub struct HandoffStream {
  pub(crate) buffered: Cursor<Vec<u8>>,
  pub(crate) stream: TcpStream,
}

impl HandoffStream {
  pub(crate) fn new(buffered: Vec<u8>, stream: TcpStream) -> Self {
    Self {
      buffered: Cursor::new(buffered),
      stream,
    }
  }

  pub fn get_ref(&self) -> &TcpStream {
    &self.stream
  }

  pub fn get_mut(&mut self) -> &mut TcpStream {
    &mut self.stream
  }

  pub fn into_inner(self) -> TcpStream {
    self.stream
  }
}

impl Read for HandoffStream {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let read = self.buffered.read(buf)?;
    if read == 0 {
      self.stream.read(buf)
    } else {
      Ok(read)
    }
  }
}

impl Write for HandoffStream {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.stream.write(buf)
  }

  fn flush(&mut self) -> io::Result<()> {
    self.stream.flush()
  }
}

type HandoffHandler = Box<dyn FnOnce(HandoffStream) -> io::Result<()> + Send + 'static>;

pub enum HttpHandoff {
  Response(HttpResponse),
  Connect {
    response: HttpResponse,
    handler: HandoffHandler,
  },
  Upgrade {
    response: HttpResponse,
    handler: HandoffHandler,
  },
}

impl HttpHandoff {
  pub fn response(response: HttpResponse) -> Self {
    Self::Response(response)
  }

  pub fn connect<F>(response: HttpResponse, handler: F) -> Self
  where
    F: FnOnce(HandoffStream) -> io::Result<()> + Send + 'static,
  {
    Self::Connect {
      response,
      handler: Box::new(handler),
    }
  }

  pub fn upgrade<F>(response: HttpResponse, handler: F) -> Self
  where
    F: FnOnce(HandoffStream) -> io::Result<()> + Send + 'static,
  {
    Self::Upgrade {
      response,
      handler: Box::new(handler),
    }
  }

  pub(crate) fn valid_for(&self, request: &Request) -> bool {
    match self {
      Self::Response(_) => true,
      Self::Connect { .. } => {
        request.method().eq_ignore_ascii_case("CONNECT")
          && is_authority_form_request_target(request.target())
      }
      Self::Upgrade { .. } => {
        !request.method().eq_ignore_ascii_case("CONNECT")
          && request.header("Upgrade").is_some()
          && request.connection_header_has_token("upgrade")
      }
    }
  }
}
