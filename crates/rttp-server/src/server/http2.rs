use super::*;

pub(crate) const HTTP2_CLIENT_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
pub(crate) const HTTP2_FRAME_DATA: u8 = 0x0;
pub(crate) const HTTP2_FRAME_HEADERS: u8 = 0x1;
pub(crate) const HTTP2_FRAME_PRIORITY: u8 = 0x2;
pub(crate) const HTTP2_FRAME_RST_STREAM: u8 = 0x3;
pub(crate) const HTTP2_FRAME_SETTINGS: u8 = 0x4;
pub(crate) const HTTP2_FRAME_PUSH_PROMISE: u8 = 0x5;
pub(crate) const HTTP2_FRAME_PING: u8 = 0x6;
pub(crate) const HTTP2_FRAME_GOAWAY: u8 = 0x7;
pub(crate) const HTTP2_FRAME_WINDOW_UPDATE: u8 = 0x8;
pub(crate) const HTTP2_FRAME_CONTINUATION: u8 = 0x9;
pub(crate) const HTTP2_FLAG_END_STREAM: u8 = 0x1;
pub(crate) const HTTP2_FLAG_ACK: u8 = 0x1;
pub(crate) const HTTP2_FLAG_END_HEADERS: u8 = 0x4;
pub(crate) const HTTP2_FLAG_PADDED: u8 = 0x8;
pub(crate) const HTTP2_FLAG_PRIORITY: u8 = 0x20;
pub(crate) const HTTP2_DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024;
pub(crate) const HTTP2_DEFAULT_INITIAL_WINDOW_SIZE: i32 = 65_535;
pub(crate) const HTTP2_DEFAULT_HEADER_TABLE_SIZE: usize = 4096;
pub(crate) const HTTP2_MAX_HEADER_LIST_SIZE: usize = MAX_REQUEST_HEAD_BYTES;
pub(crate) const HTTP2_STATIC_TABLE_LEN: usize = 61;
pub(crate) const HTTP2_SETTINGS_ENABLE_PUSH: u16 = 0x2;
pub(crate) const HTTP2_SETTINGS_HEADER_TABLE_SIZE: u16 = 0x1;
pub(crate) const HTTP2_SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
pub(crate) const HTTP2_SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
pub(crate) const HTTP2_SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
pub(crate) const HTTP2_SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x6;
pub(crate) const HTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL: u16 = 0x8;
pub(crate) const HTTP2_ERROR_NO_ERROR: u32 = 0x0;
pub(crate) const HTTP2_ERROR_REFUSED_STREAM: u32 = 0x7;

/// Bounds advertised and accepted HTTP/2 settings on the server's h2c path.
///
/// The policy applies to each bounded h2c connection accepted by an
/// [`HttpServer`]. It is fixed for that connection; changing policy at runtime
/// would require the session management that this server deliberately does not
/// provide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Http2ServerPolicy {
  max_frame_size: usize,
  max_header_list_size: usize,
}

impl Default for Http2ServerPolicy {
  fn default() -> Self {
    Self {
      max_frame_size: HTTP2_DEFAULT_MAX_FRAME_SIZE,
      max_header_list_size: HTTP2_MAX_HEADER_LIST_SIZE,
    }
  }
}

impl Http2ServerPolicy {
  /// Creates the default bounded h2c server policy.
  pub fn new() -> Self {
    Self::default()
  }

  /// Advertises and enforces a local `SETTINGS_MAX_FRAME_SIZE` value.
  pub fn with_max_frame_size(mut self, max_frame_size: usize) -> Self {
    self.max_frame_size = max_frame_size;
    self
  }

  /// Advertises and enforces a local `SETTINGS_MAX_HEADER_LIST_SIZE` value.
  pub fn with_max_header_list_size(mut self, max_header_list_size: usize) -> Self {
    self.max_header_list_size = max_header_list_size;
    self
  }

  pub fn max_frame_size(&self) -> usize {
    self.max_frame_size
  }

  pub fn max_header_list_size(&self) -> usize {
    self.max_header_list_size
  }

  pub(crate) fn validate(&self) -> io::Result<()> {
    if !(16_384..=16_777_215).contains(&self.max_frame_size)
      || u32::try_from(self.max_header_list_size).is_err()
    {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid bounded HTTP/2 server policy",
      ));
    }
    Ok(())
  }
}

pub(crate) enum AcceptedConnection {
  Http1(Http1Stream),
  Http2(TcpStream),
}

pub(crate) struct Http2InitialSettings {
  pub(crate) payload: Vec<u8>,
  pub(crate) acknowledge: bool,
  pub(crate) upgraded: bool,
}

pub(crate) enum Http1Stream {
  Plain(TcpStream),
  Prefixed(HandoffStream),
}

impl Http1Stream {
  pub(crate) fn into_handoff_stream(self, buffered: Vec<u8>) -> io::Result<HandoffStream> {
    match self {
      Self::Plain(stream) => Ok(HandoffStream::new(buffered, stream)),
      Self::Prefixed(mut stream) => {
        let mut combined = Vec::new();
        stream.buffered.read_to_end(&mut combined)?;
        combined.extend_from_slice(&buffered);
        Ok(HandoffStream::new(combined, stream.stream))
      }
    }
  }
}

impl Read for Http1Stream {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    match self {
      Self::Plain(stream) => stream.read(buf),
      Self::Prefixed(stream) => stream.read(buf),
    }
  }
}

impl Write for Http1Stream {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Self::Plain(stream) => stream.write(buf),
      Self::Prefixed(stream) => stream.write(buf),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Self::Plain(stream) => stream.flush(),
      Self::Prefixed(stream) => stream.flush(),
    }
  }
}

pub(crate) struct Http2Frame {
  pub(crate) frame_type: u8,
  pub(crate) flags: u8,
  pub(crate) stream_id: u32,
  pub(crate) payload: Vec<u8>,
}

pub(crate) struct Http2RequestStream {
  pub(crate) stream_id: u32,
  pub(crate) header_block: Vec<u8>,
  pub(crate) header_block_kind: Option<Http2HeaderBlockKind>,
  pub(crate) decoded_headers: Option<DecodedHttp2RequestHeaders>,
  pub(crate) decoded_trailers: Vec<(String, String)>,
  pub(crate) body: Vec<u8>,
  pub(crate) end_stream: bool,
  pub(crate) in_header_continuation: bool,
  pub(crate) receive_window: i32,
  pub(crate) send_window: Http2SendWindow,
}

pub(crate) struct Http2ClientStreamIds {
  pub(crate) max_opened: u32,
  pub(crate) closed: Vec<u32>,
}

impl Http2ClientStreamIds {
  pub(crate) fn new() -> Self {
    Self {
      max_opened: 0,
      closed: Vec::new(),
    }
  }

  pub(crate) fn after_http1_upgrade() -> Self {
    Self {
      max_opened: 1,
      closed: vec![1],
    }
  }

  pub(crate) fn open(&mut self, stream_id: u32) -> io::Result<()> {
    if stream_id.is_multiple_of(2) {
      return Err(invalid_http2_client_stream_id_error());
    }
    if self.is_closed(stream_id) {
      return Err(http2_closed_stream_error());
    }
    if stream_id <= self.max_opened {
      return Err(invalid_http2_client_stream_id_error());
    }
    self.max_opened = stream_id;
    Ok(())
  }

  pub(crate) fn close(&mut self, stream_id: u32) {
    if !self.closed.contains(&stream_id) {
      self.closed.push(stream_id);
    }
  }

  pub(crate) fn is_closed(&self, stream_id: u32) -> bool {
    self.closed.contains(&stream_id)
  }
}

#[derive(Clone, Copy)]
pub(crate) enum Http2HeaderBlockKind {
  RequestHeaders,
  RequestTrailers,
}

impl Http2RequestStream {
  pub(crate) fn new(stream_id: u32, send_window: i32) -> Self {
    Self {
      stream_id,
      header_block: Vec::new(),
      header_block_kind: None,
      decoded_headers: None,
      decoded_trailers: Vec::new(),
      body: Vec::new(),
      end_stream: false,
      in_header_continuation: false,
      receive_window: HTTP2_DEFAULT_INITIAL_WINDOW_SIZE,
      send_window: Http2SendWindow::new(send_window),
    }
  }

  pub(crate) fn is_complete(&self) -> bool {
    self.decoded_headers.is_some() && self.end_stream && !self.in_header_continuation
  }

  pub(crate) fn is_extended_connect(&self) -> bool {
    self
      .decoded_headers
      .as_ref()
      .is_some_and(DecodedHttp2RequestHeaders::is_extended_connect)
  }

  pub(crate) fn finish_header_block(
    &mut self,
    decoder: &mut Http2HeaderDecoder,
    max_header_list_size: usize,
    enable_connect_protocol: bool,
  ) -> io::Result<()> {
    match self.header_block_kind.take() {
      Some(Http2HeaderBlockKind::RequestHeaders) => {
        self.decoded_headers = Some(decode_http2_request_headers(
          &self.header_block,
          decoder,
          max_header_list_size,
          enable_connect_protocol,
        )?);
      }
      Some(Http2HeaderBlockKind::RequestTrailers) => {
        self.decoded_trailers =
          decode_http2_request_trailers(&self.header_block, decoder, max_header_list_size)?;
      }
      None => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "HTTP/2 header block completed without HEADERS frame",
        ));
      }
    }
    self.header_block.clear();
    self.in_header_continuation = false;
    Ok(())
  }

  pub(crate) fn into_request(self) -> io::Result<Request> {
    let decoded_headers = self.decoded_headers.ok_or_else(|| {
      io::Error::new(io::ErrorKind::InvalidData, "missing HTTP/2 request headers")
    })?;
    decoded_headers.into_request(self.body, self.decoded_trailers)
  }

  pub(crate) fn receive_flow_controlled_data(
    &mut self,
    connection_receive_window: &mut i32,
    len: i32,
  ) -> io::Result<()> {
    if len > *connection_receive_window || len > self.receive_window {
      return Err(http2_flow_control_error());
    }
    *connection_receive_window -= len;
    self.receive_window -= len;
    Ok(())
  }

  pub(crate) fn release_flow_controlled_data(
    &mut self,
    connection_receive_window: &mut i32,
    len: i32,
  ) -> io::Result<()> {
    *connection_receive_window = connection_receive_window
      .checked_add(len)
      .filter(|window| *window <= HTTP2_DEFAULT_INITIAL_WINDOW_SIZE)
      .ok_or_else(http2_flow_control_error)?;
    self.receive_window = self
      .receive_window
      .checked_add(len)
      .filter(|window| *window <= HTTP2_DEFAULT_INITIAL_WINDOW_SIZE)
      .ok_or_else(http2_flow_control_error)?;
    Ok(())
  }
}

#[derive(Clone, Copy)]
pub(crate) struct Http2SendWindow {
  pub(crate) size: i32,
}

impl Http2SendWindow {
  pub(crate) fn new(size: i32) -> Self {
    Self { size }
  }

  pub(crate) fn available(&self) -> usize {
    usize::try_from(self.size).unwrap_or(0)
  }

  pub(crate) fn consume(&mut self, len: usize) -> io::Result<()> {
    let len = i32::try_from(len).map_err(|_| http2_flow_control_error())?;
    if len > self.size {
      return Err(http2_flow_control_error());
    }
    self.size -= len;
    Ok(())
  }

  pub(crate) fn increase(&mut self, increment: u32) -> io::Result<()> {
    let increment = i32::try_from(increment).map_err(|_| http2_flow_control_overflow_error())?;
    self.size = self
      .size
      .checked_add(increment)
      .ok_or_else(http2_flow_control_overflow_error)?;
    Ok(())
  }

  pub(crate) fn adjust(&mut self, delta: i32) -> io::Result<()> {
    self.size = self
      .size
      .checked_add(delta)
      .ok_or_else(http2_flow_control_overflow_error)?;
    Ok(())
  }
}

pub(crate) fn http2_request_stream<'a>(
  streams: &'a mut Vec<Http2RequestStream>,
  stream_ids: &mut Http2ClientStreamIds,
  stream_id: u32,
  send_window: i32,
) -> io::Result<&'a mut Http2RequestStream> {
  if let Some(index) = streams
    .iter()
    .position(|request_stream| request_stream.stream_id == stream_id)
  {
    return Ok(&mut streams[index]);
  }

  stream_ids.open(stream_id)?;
  streams.push(Http2RequestStream::new(stream_id, send_window));
  Ok(streams.last_mut().expect("new HTTP/2 request stream"))
}

pub(crate) fn active_http2_header_continuation_stream(
  streams: &[Http2RequestStream],
) -> Option<u32> {
  streams
    .iter()
    .find(|request_stream| request_stream.in_header_continuation)
    .map(|request_stream| request_stream.stream_id)
}

pub(crate) fn http2_headers_payload_to_header_block_fragment(
  payload: &[u8],
  flags: u8,
) -> io::Result<&[u8]> {
  let mut start = 0;
  let pad_len = if flags & HTTP2_FLAG_PADDED == HTTP2_FLAG_PADDED {
    let Some((&pad_len, rest)) = payload.split_first() else {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid padded HTTP/2 HEADERS frame",
      ));
    };
    start = payload.len() - rest.len();
    pad_len as usize
  } else {
    0
  };

  if flags & HTTP2_FLAG_PRIORITY == HTTP2_FLAG_PRIORITY {
    if payload.len().saturating_sub(start) < 5 {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid priority HTTP/2 HEADERS frame",
      ));
    }
    start += 5;
  }

  let available = payload.len().saturating_sub(start);
  if pad_len > available {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid padded HTTP/2 HEADERS frame",
    ));
  }
  Ok(&payload[start..payload.len() - pad_len])
}

pub(crate) fn http2_data_payload_to_data(payload: &[u8], flags: u8) -> io::Result<&[u8]> {
  if flags & HTTP2_FLAG_PADDED != HTTP2_FLAG_PADDED {
    return Ok(payload);
  }
  let Some((&pad_len, data_and_padding)) = payload.split_first() else {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid padded HTTP/2 DATA frame",
    ));
  };
  let pad_len = pad_len as usize;
  if pad_len > data_and_padding.len() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid padded HTTP/2 DATA frame",
    ));
  }
  Ok(&data_and_padding[..data_and_padding.len() - pad_len])
}

pub(crate) fn read_http2_frame<S: Read>(
  stream: &mut S,
  max_frame_size: usize,
) -> io::Result<Http2Frame> {
  let mut header = [0; 9];
  stream.read_exact(&mut header)?;
  let length = ((header[0] as usize) << 16) | ((header[1] as usize) << 8) | header[2] as usize;
  if length > max_frame_size {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "HTTP/2 frame payload exceeds active max frame size",
    ));
  }
  let mut payload = vec![0; length];
  stream.read_exact(&mut payload).map_err(|err| {
    if err.kind() == io::ErrorKind::UnexpectedEof {
      io::Error::new(
        io::ErrorKind::InvalidData,
        "incomplete HTTP/2 frame payload",
      )
    } else {
      err
    }
  })?;
  Ok(Http2Frame {
    frame_type: header[3],
    flags: header[4],
    stream_id: u32::from_be_bytes([header[5] & 0x7f, header[6], header[7], header[8]]),
    payload,
  })
}

pub(crate) fn validate_http2_settings_payload(payload: &[u8]) -> io::Result<()> {
  if !payload.len().is_multiple_of(6) {
    return Err(invalid_http2_settings_error());
  }

  for setting in payload.chunks_exact(6) {
    let id = u16::from_be_bytes([setting[0], setting[1]]);
    let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
    match id {
      HTTP2_SETTINGS_ENABLE_PUSH if value > 1 => {
        return Err(invalid_http2_enable_push_settings_error());
      }
      HTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL if value > 1 => {
        return Err(invalid_http2_enable_connect_protocol_settings_error());
      }
      HTTP2_SETTINGS_INITIAL_WINDOW_SIZE if value > 0x7fff_ffff => {
        return Err(invalid_http2_settings_error());
      }
      HTTP2_SETTINGS_MAX_FRAME_SIZE if !(16_384..=16_777_215).contains(&value) => {
        return Err(invalid_http2_settings_error());
      }
      _ => {}
    }
  }

  Ok(())
}

pub(crate) fn h2c_upgrade_settings(request: &Request) -> io::Result<Option<Vec<u8>>> {
  if !request
    .header("Upgrade")
    .is_some_and(|value| connection_header_has_token(Some(value), "h2c"))
  {
    return Ok(None);
  }

  if request.version() != "HTTP/1.1"
    || request.method().eq_ignore_ascii_case("CONNECT")
    || !request.connection_header_has_token("upgrade")
    || !request.connection_header_has_token("http2-settings")
    || request.headers_named("HTTP2-Settings").count() != 1
    || !request.body().is_empty()
  {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid h2c upgrade request",
    ));
  }

  let settings = request
    .header("HTTP2-Settings")
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP2-Settings header"))?;
  let payload = decode_base64url_unpadded(settings.trim())?;
  validate_http2_settings_payload(&payload)?;
  Ok(Some(payload))
}

pub(crate) fn write_h2c_upgrade_response<S: Write>(stream: &mut S) -> io::Result<()> {
  HttpResponse::new(101, "Switching Protocols")
    .header("Connection", "Upgrade")
    .header("Upgrade", "h2c")
    .write_handoff_head_to(stream)
}

pub(crate) fn decode_base64url_unpadded(input: &str) -> io::Result<Vec<u8>> {
  if input.as_bytes().contains(&b'=') || input.len() % 4 == 1 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HTTP2-Settings header",
    ));
  }

  let mut output = Vec::with_capacity((input.len() * 3) / 4);
  for chunk in input.as_bytes().chunks(4) {
    let mut value = 0u32;
    for byte in chunk {
      value = (value << 6) | base64url_value(*byte)? as u32;
    }
    let missing = 4 - chunk.len();
    value <<= 6 * missing;
    output.push(((value >> 16) & 0xff) as u8);
    if chunk.len() >= 3 {
      output.push(((value >> 8) & 0xff) as u8);
    }
    if chunk.len() == 4 {
      output.push((value & 0xff) as u8);
    }
  }
  Ok(output)
}

pub(crate) fn base64url_value(byte: u8) -> io::Result<u8> {
  match byte {
    b'A'..=b'Z' => Ok(byte - b'A'),
    b'a'..=b'z' => Ok(byte - b'a' + 26),
    b'0'..=b'9' => Ok(byte - b'0' + 52),
    b'-' => Ok(62),
    b'_' => Ok(63),
    _ => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HTTP2-Settings header",
    )),
  }
}

pub(crate) fn http2_window_update_increment(payload: &[u8]) -> io::Result<u32> {
  if payload.len() != 4 {
    return Err(invalid_http2_window_update_error());
  }
  let increment = u32::from_be_bytes([payload[0] & 0x7f, payload[1], payload[2], payload[3]]);
  if increment == 0 {
    return Err(invalid_http2_window_update_error());
  }
  Ok(increment)
}

pub(crate) fn validate_http2_priority_frame(stream_id: u32, payload: &[u8]) -> io::Result<()> {
  if stream_id == 0 || stream_id.is_multiple_of(2) || payload.len() != 5 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HTTP/2 PRIORITY frame",
    ));
  }
  Ok(())
}

pub(crate) fn validate_http2_rst_stream_frame(stream_id: u32, payload: &[u8]) -> io::Result<()> {
  if stream_id == 0 || payload.len() != 4 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HTTP/2 RST_STREAM frame",
    ));
  }
  Ok(())
}

pub(crate) fn validate_http2_goaway_frame(stream_id: u32, payload: &[u8]) -> io::Result<()> {
  if stream_id != 0 || payload.len() < 8 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HTTP/2 GOAWAY frame",
    ));
  }
  Ok(())
}

pub(crate) fn invalid_http2_push_promise_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 PUSH_PROMISE frame is unsupported",
  )
}

pub(crate) fn invalid_http2_window_update_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "invalid HTTP/2 WINDOW_UPDATE frame",
  )
}

pub(crate) fn http2_flow_control_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 flow-control window exceeded",
  )
}

pub(crate) fn http2_flow_control_overflow_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 flow-control window overflow",
  )
}

pub(crate) fn http2_settings_max_frame_size(payload: &[u8]) -> Option<usize> {
  payload
    .chunks_exact(6)
    .fold(None, |max_frame_size, setting| {
      let id = u16::from_be_bytes([setting[0], setting[1]]);
      let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
      if id == HTTP2_SETTINGS_MAX_FRAME_SIZE {
        Some(value as usize)
      } else {
        max_frame_size
      }
    })
}

pub(crate) fn http2_settings_header_table_size(payload: &[u8]) -> Option<usize> {
  payload
    .chunks_exact(6)
    .fold(None, |header_table_size, setting| {
      let id = u16::from_be_bytes([setting[0], setting[1]]);
      let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
      if id == HTTP2_SETTINGS_HEADER_TABLE_SIZE {
        Some(value as usize)
      } else {
        header_table_size
      }
    })
}

pub(crate) fn http2_settings_initial_window_size(payload: &[u8]) -> Option<i32> {
  payload
    .chunks_exact(6)
    .fold(None, |initial_window_size, setting| {
      let id = u16::from_be_bytes([setting[0], setting[1]]);
      let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
      if id == HTTP2_SETTINGS_INITIAL_WINDOW_SIZE {
        Some(value as i32)
      } else {
        initial_window_size
      }
    })
}

pub(crate) fn http2_settings_enable_connect_protocol(payload: &[u8]) -> bool {
  payload.chunks_exact(6).fold(false, |enabled, setting| {
    let id = u16::from_be_bytes([setting[0], setting[1]]);
    let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
    if id == HTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL {
      value == 1
    } else {
      enabled
    }
  })
}

pub(crate) fn bounded_http2_max_concurrent_streams(request_limit: usize) -> u32 {
  u32::try_from(request_limit).unwrap_or(u32::MAX)
}

pub(crate) fn http2_setting(id: u16, value: u32) -> [u8; 6] {
  let mut setting = [0; 6];
  setting[..2].copy_from_slice(&id.to_be_bytes());
  setting[2..].copy_from_slice(&value.to_be_bytes());
  setting
}

pub(crate) fn invalid_http2_settings_error() -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP/2 SETTINGS frame")
}

pub(crate) fn invalid_http2_enable_push_settings_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "invalid HTTP/2 SETTINGS_ENABLE_PUSH value",
  )
}

pub(crate) fn invalid_http2_enable_connect_protocol_settings_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "invalid HTTP/2 SETTINGS_ENABLE_CONNECT_PROTOCOL value",
  )
}

pub(crate) fn unsupported_http2_extended_connect_body_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 extended CONNECT request bodies are unsupported",
  )
}

pub(crate) fn invalid_http2_client_stream_id_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "invalid HTTP/2 client stream id",
  )
}

pub(crate) fn http2_closed_stream_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 frame arrived after stream close",
  )
}

pub(crate) fn write_http2_frame<S: Write>(
  stream: &mut S,
  frame_type: u8,
  flags: u8,
  stream_id: u32,
  payload: &[u8],
) -> io::Result<()> {
  if payload.len() > 16_777_215 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "HTTP/2 frame payload is too large",
    ));
  }

  let length = payload.len();
  let mut header = [0; 9];
  header[0] = ((length >> 16) & 0xff) as u8;
  header[1] = ((length >> 8) & 0xff) as u8;
  header[2] = (length & 0xff) as u8;
  header[3] = frame_type;
  header[4] = flags;
  header[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
  stream.write_all(&header)?;
  stream.write_all(payload)
}

pub(crate) fn write_http2_window_update<S: Write>(
  stream: &mut S,
  stream_id: u32,
  increment: usize,
) -> io::Result<()> {
  let increment = u32::try_from(increment).map_err(|_| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      "HTTP/2 window update increment is too large",
    )
  })?;
  if increment == 0 || increment > 0x7fff_ffff {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "invalid HTTP/2 window update increment",
    ));
  }
  write_http2_frame(
    stream,
    HTTP2_FRAME_WINDOW_UPDATE,
    0,
    stream_id,
    &increment.to_be_bytes(),
  )
}

pub(crate) fn server_http2_settings_payload(
  request_limit: usize,
  policy: &Http2ServerPolicy,
) -> Vec<u8> {
  let mut payload = Vec::with_capacity(18);
  payload.extend_from_slice(&http2_setting(
    HTTP2_SETTINGS_MAX_FRAME_SIZE,
    policy.max_frame_size as u32,
  ));
  payload.extend_from_slice(&http2_setting(
    HTTP2_SETTINGS_MAX_CONCURRENT_STREAMS,
    bounded_http2_max_concurrent_streams(request_limit),
  ));
  payload.extend_from_slice(&http2_setting(
    HTTP2_SETTINGS_MAX_HEADER_LIST_SIZE,
    policy.max_header_list_size as u32,
  ));
  payload
}

pub(crate) fn write_http2_goaway<S: Write>(
  stream: &mut S,
  last_stream_id: u32,
  error_code: u32,
) -> io::Result<()> {
  let mut payload = [0; 8];
  payload[..4].copy_from_slice(&(last_stream_id & 0x7fff_ffff).to_be_bytes());
  payload[4..].copy_from_slice(&error_code.to_be_bytes());
  write_http2_frame(stream, HTTP2_FRAME_GOAWAY, 0, 0, &payload)
}

pub(crate) struct DecodedHttp2RequestHeaders {
  pub(crate) method: Option<String>,
  pub(crate) target: Option<String>,
  pub(crate) scheme: Option<String>,
  pub(crate) authority: Option<String>,
  pub(crate) extended_connect_protocol: Option<String>,
  pub(crate) headers: Vec<(String, String)>,
}

impl DecodedHttp2RequestHeaders {
  pub(crate) fn is_extended_connect(&self) -> bool {
    self.extended_connect_protocol.is_some()
  }

  pub(crate) fn into_request(
    self,
    body: Vec<u8>,
    trailers: Vec<(String, String)>,
  ) -> io::Result<Request> {
    let method = self
      .method
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP/2 :method"))?;
    if method.eq_ignore_ascii_case("CONNECT") && self.extended_connect_protocol.is_none() {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "HTTP/2 prior-knowledge CONNECT/proxy tunneling is unsupported",
      ));
    }
    if !method.eq_ignore_ascii_case("CONNECT") && self.extended_connect_protocol.is_some() {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "HTTP/2 extended CONNECT :protocol requires CONNECT",
      ));
    }
    if self.extended_connect_protocol.is_some() && (!body.is_empty() || !trailers.is_empty()) {
      return Err(unsupported_http2_extended_connect_body_error());
    }
    let target = self
      .target
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP/2 :path"))?;
    let _scheme = self
      .scheme
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP/2 :scheme"))?;
    let mut headers = self.headers;
    if let Some(authority) = self.authority {
      headers.push(("host".to_string(), authority));
    }

    Ok(Request {
      method,
      target,
      version: "HTTP/2".to_string(),
      headers,
      trailers,
      body,
      content_length: None,
      extended_connect_protocol: self.extended_connect_protocol,
    })
  }
}

pub(crate) struct Http2HeaderDecoder {
  pub(crate) dynamic_entries: Vec<(String, String)>,
  pub(crate) max_size: usize,
  pub(crate) current_size: usize,
}

impl Http2HeaderDecoder {
  pub(crate) fn new(max_size: usize) -> Self {
    Self {
      dynamic_entries: Vec::new(),
      max_size,
      current_size: 0,
    }
  }

  pub(crate) fn header(&self, index: usize) -> io::Result<(String, String)> {
    if index == 0 {
      return Err(invalid_hpack_index_error());
    }
    if index <= HTTP2_STATIC_TABLE_LEN {
      let (name, value) = http2_static_header(index)?;
      return Ok((name.to_string(), value.to_string()));
    }
    self
      .dynamic_header(index)
      .cloned()
      .ok_or_else(invalid_hpack_index_error)
  }

  pub(crate) fn header_name(&self, index: usize) -> io::Result<String> {
    if index == 0 {
      return Err(invalid_hpack_index_error());
    }
    if index <= HTTP2_STATIC_TABLE_LEN {
      return Ok(http2_static_header(index)?.0.to_string());
    }
    self
      .dynamic_header(index)
      .map(|(name, _)| name.clone())
      .ok_or_else(invalid_hpack_index_error)
  }

  pub(crate) fn update_max_size(&mut self, size: usize) -> io::Result<()> {
    if size > HTTP2_DEFAULT_HEADER_TABLE_SIZE {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "HPACK dynamic table size update exceeds configured size",
      ));
    }
    self.max_size = size;
    self.evict_to_max_size();
    Ok(())
  }

  pub(crate) fn insert(&mut self, name: String, value: String) {
    let entry_size = hpack_dynamic_entry_size(&name, &value);
    if entry_size > self.max_size {
      self.dynamic_entries.clear();
      self.current_size = 0;
      return;
    }
    self.dynamic_entries.insert(0, (name, value));
    self.current_size += entry_size;
    self.evict_to_max_size();
  }

  pub(crate) fn dynamic_header(&self, index: usize) -> Option<&(String, String)> {
    let dynamic_index = index.checked_sub(HTTP2_STATIC_TABLE_LEN + 1)?;
    self.dynamic_entries.get(dynamic_index)
  }

  pub(crate) fn evict_to_max_size(&mut self) {
    while self.current_size > self.max_size {
      let Some((name, value)) = self.dynamic_entries.pop() else {
        self.current_size = 0;
        return;
      };
      self.current_size -= hpack_dynamic_entry_size(&name, &value);
    }
  }
}

pub(crate) fn hpack_dynamic_entry_size(name: &str, value: &str) -> usize {
  name.len() + value.len() + 32
}

pub(crate) fn invalid_hpack_index_error() -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, "invalid HPACK table index")
}

pub(crate) fn decode_http2_request_headers(
  block: &[u8],
  decoder: &mut Http2HeaderDecoder,
  max_header_list_size: usize,
  enable_connect_protocol: bool,
) -> io::Result<DecodedHttp2RequestHeaders> {
  let mut decoded = DecodedHttp2RequestHeaders {
    method: None,
    target: None,
    scheme: None,
    authority: None,
    extended_connect_protocol: None,
    headers: Vec::new(),
  };
  let mut regular_header_seen = false;
  let mut pseudo_headers = Vec::<String>::new();

  let fields = decode_http2_header_fields(block, decoder)?;
  reject_oversized_http2_header_list(&fields, max_header_list_size)?;
  for (name, value) in fields {
    if name.starts_with(':') {
      if regular_header_seen {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "HTTP/2 pseudo-header appeared after a regular header",
        ));
      }
      if pseudo_headers
        .iter()
        .any(|pseudo_header| pseudo_header == &name)
      {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "duplicate HTTP/2 pseudo-header",
        ));
      }
      pseudo_headers.push(name.clone());
    } else {
      regular_header_seen = true;
    }

    match name.as_str() {
      ":method" => decoded.method = Some(value),
      ":path" => decoded.target = Some(value),
      ":scheme" => decoded.scheme = Some(value),
      ":authority" => decoded.authority = Some(value),
      ":protocol" => {
        if !enable_connect_protocol {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 extended CONNECT :protocol requires SETTINGS_ENABLE_CONNECT_PROTOCOL",
          ));
        }
        decoded.extended_connect_protocol = Some(value);
      }
      name if name.starts_with(':') => {}
      name if is_forbidden_http2_request_header_name(name) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "forbidden HTTP/2 request header",
        ));
      }
      name if name.eq_ignore_ascii_case("te") && value.eq_ignore_ascii_case("trailers") => {
        decoded.headers.push((name.to_string(), value))
      }
      name if name.eq_ignore_ascii_case("te") => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "forbidden HTTP/2 request header",
        ));
      }
      _ => decoded.headers.push((name, value)),
    }
  }

  Ok(decoded)
}

pub(crate) fn is_forbidden_http2_request_header_name(name: &str) -> bool {
  [
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
  ]
  .iter()
  .any(|forbidden| name.eq_ignore_ascii_case(forbidden))
}

pub(crate) fn decode_http2_request_trailers(
  block: &[u8],
  decoder: &mut Http2HeaderDecoder,
  max_header_list_size: usize,
) -> io::Result<Vec<(String, String)>> {
  let trailers = decode_http2_header_fields(block, decoder)?;
  reject_oversized_http2_header_list(&trailers, max_header_list_size)?;
  for (name, value) in &trailers {
    if name.starts_with(':') {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "HTTP/2 request trailer contained pseudo-header",
      ));
    }
    if !is_http_token(name)
      || is_forbidden_trailer_name(name)
      || !value.bytes().all(is_header_value_byte)
    {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "forbidden request trailer",
      ));
    }
  }
  Ok(trailers)
}

pub(crate) fn reject_oversized_http2_header_list(
  fields: &[(String, String)],
  max_header_list_size: usize,
) -> io::Result<()> {
  let mut size = 0usize;
  for (name, value) in fields {
    size = size
      .checked_add(name.len())
      .and_then(|size| size.checked_add(value.len()))
      .and_then(|size| size.checked_add(32))
      .ok_or_else(http2_header_list_size_error)?;
    if size > max_header_list_size {
      return Err(http2_header_list_size_error());
    }
  }
  Ok(())
}

pub(crate) fn http2_header_list_size_error() -> io::Error {
  io::Error::new(
    io::ErrorKind::InvalidData,
    "HTTP/2 header list size exceeded",
  )
}

pub(crate) fn decode_http2_header_fields(
  block: &[u8],
  decoder: &mut Http2HeaderDecoder,
) -> io::Result<Vec<(String, String)>> {
  let mut cursor = 0;
  let mut fields = Vec::new();
  let mut field_seen = false;
  while cursor < block.len() {
    let byte = block[cursor];
    let field = if byte & 0x80 == 0x80 {
      let index = decode_http2_integer(block, &mut cursor, 7)?;
      field_seen = true;
      decoder.header(index)?
    } else if byte & 0x40 == 0x40 {
      field_seen = true;
      let field = decode_http2_literal(block, &mut cursor, 6, decoder)?;
      decoder.insert(field.0.clone(), field.1.clone());
      field
    } else if byte & 0x20 == 0x20 {
      if field_seen {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "HPACK dynamic table size update after header field",
        ));
      }
      let size = decode_http2_integer(block, &mut cursor, 5)?;
      decoder.update_max_size(size)?;
      continue;
    } else {
      field_seen = true;
      decode_http2_literal(block, &mut cursor, 4, decoder)?
    };
    fields.push(field);
  }
  Ok(fields)
}

pub(crate) fn decode_http2_literal(
  block: &[u8],
  cursor: &mut usize,
  prefix_bits: u8,
  decoder: &Http2HeaderDecoder,
) -> io::Result<(String, String)> {
  let name_index = decode_http2_integer(block, cursor, prefix_bits)?;
  let name = if name_index == 0 {
    decode_http2_string(block, cursor)?
  } else {
    decoder.header_name(name_index)?
  };
  let value = decode_http2_string(block, cursor)?;
  Ok((name, value))
}

pub(crate) fn decode_http2_integer(
  block: &[u8],
  cursor: &mut usize,
  prefix_bits: u8,
) -> io::Result<usize> {
  if *cursor >= block.len() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "truncated HPACK integer",
    ));
  }

  let max_prefix = (1usize << prefix_bits) - 1;
  let mut value = (block[*cursor] as usize) & max_prefix;
  *cursor += 1;
  if value < max_prefix {
    return Ok(value);
  }

  let mut shift = 0u32;
  loop {
    if *cursor >= block.len() {
      return Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "truncated HPACK integer",
      ));
    }
    let byte = block[*cursor];
    *cursor += 1;
    let addition = ((byte & 0x7f) as usize)
      .checked_shl(shift)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HPACK integer overflow"))?;
    value = value
      .checked_add(addition)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HPACK integer overflow"))?;
    if byte & 0x80 == 0 {
      return Ok(value);
    }
    shift += 7;
  }
}

pub(crate) fn decode_http2_string(block: &[u8], cursor: &mut usize) -> io::Result<String> {
  if *cursor >= block.len() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "truncated HPACK string",
    ));
  }
  let huffman = block[*cursor] & 0x80 == 0x80;
  let len = decode_http2_integer(block, cursor, 7)?;
  let end = cursor
    .checked_add(len)
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HPACK string length overflow"))?;
  if end > block.len() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "truncated HPACK string",
    ));
  }
  let bytes = if huffman {
    decode_http2_huffman_string(&block[*cursor..end])?
  } else {
    block[*cursor..end].to_vec()
  };
  let value =
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
  *cursor = end;
  Ok(value)
}

pub(crate) fn decode_http2_huffman_string(encoded: &[u8]) -> io::Result<Vec<u8>> {
  let table = http2_huffman_decode_table();
  let mut decoded = Vec::with_capacity(encoded.len());
  let mut code = 0u32;
  let mut code_len = 0u8;
  let mut node = 0usize;

  for byte in encoded {
    for bit_index in (0..8).rev() {
      let bit = ((byte >> bit_index) & 1) as usize;
      code = (code << 1) | (bit as u32);
      code_len += 1;
      node = table
        .next_node(node, bit)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid HPACK Huffman code"))?;

      if let Some(symbol) = table.symbol(node) {
        if symbol == HTTP2_HUFFMAN_EOS {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HPACK Huffman EOS symbol used as data",
          ));
        }
        decoded.push(symbol as u8);
        code = 0;
        code_len = 0;
        node = 0;
      } else if code_len > HTTP2_HUFFMAN_EOS_BITS {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "invalid HPACK Huffman code",
        ));
      }
    }
  }

  if code_len == 0 {
    return Ok(decoded);
  }

  if code_len > 7 || code != (1u32 << code_len) - 1 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "invalid HPACK Huffman padding",
    ));
  }

  Ok(decoded)
}

pub(crate) fn http2_huffman_decode_table() -> &'static Http2HuffmanDecodeTable {
  static TABLE: OnceLock<Http2HuffmanDecodeTable> = OnceLock::new();
  TABLE.get_or_init(Http2HuffmanDecodeTable::from_codes)
}

pub(crate) struct Http2HuffmanDecodeTable {
  pub(crate) nodes: Vec<Http2HuffmanNode>,
}

impl Http2HuffmanDecodeTable {
  pub(crate) fn from_codes() -> Self {
    let mut table = Self {
      nodes: vec![Http2HuffmanNode::default()],
    };

    for (symbol, &(code_len, code)) in HTTP2_HUFFMAN_CODES.iter().enumerate() {
      table.insert(code, code_len, symbol as u16);
    }

    table
  }

  pub(crate) fn insert(&mut self, code: u32, code_len: u8, symbol: u16) {
    let mut node = 0usize;
    for bit_index in (0..code_len).rev() {
      let bit = ((code >> bit_index) & 1) as usize;
      node = match self.nodes[node].children[bit] {
        Some(child) => child,
        None => {
          let child = self.nodes.len();
          self.nodes.push(Http2HuffmanNode::default());
          self.nodes[node].children[bit] = Some(child);
          child
        }
      };
    }
    assert!(self.nodes[node].symbol.replace(symbol).is_none());
  }

  #[cfg(test)]
  pub(crate) fn decode_symbol(&self, code: u32, code_len: u8) -> Option<u16> {
    let mut node = 0usize;
    for bit_index in (0..code_len).rev() {
      let bit = ((code >> bit_index) & 1) as usize;
      node = self.next_node(node, bit)?;
    }
    self.symbol(node)
  }

  pub(crate) fn next_node(&self, node: usize, bit: usize) -> Option<usize> {
    self.nodes[node].children[bit]
  }

  pub(crate) fn symbol(&self, node: usize) -> Option<u16> {
    self.nodes[node].symbol
  }
}

#[derive(Default)]
pub(crate) struct Http2HuffmanNode {
  pub(crate) children: [Option<usize>; 2],
  pub(crate) symbol: Option<u16>,
}

pub(crate) const HTTP2_HUFFMAN_EOS: u16 = 256;
pub(crate) const HTTP2_HUFFMAN_EOS_BITS: u8 = 30;

pub(crate) const HTTP2_HUFFMAN_CODES: [(u8, u32); 257] = [
  (13, 0x1ff8),
  (23, 0x7ffd8),
  (28, 0xfffffe2),
  (28, 0xfffffe3),
  (28, 0xfffffe4),
  (28, 0xfffffe5),
  (28, 0xfffffe6),
  (28, 0xfffffe7),
  (28, 0xfffffe8),
  (24, 0xffffea),
  (30, 0x3ffffffc),
  (28, 0xfffffe9),
  (28, 0xfffffea),
  (30, 0x3ffffffd),
  (28, 0xfffffeb),
  (28, 0xfffffec),
  (28, 0xfffffed),
  (28, 0xfffffee),
  (28, 0xfffffef),
  (28, 0xffffff0),
  (28, 0xffffff1),
  (28, 0xffffff2),
  (30, 0x3ffffffe),
  (28, 0xffffff3),
  (28, 0xffffff4),
  (28, 0xffffff5),
  (28, 0xffffff6),
  (28, 0xffffff7),
  (28, 0xffffff8),
  (28, 0xffffff9),
  (28, 0xffffffa),
  (28, 0xffffffb),
  (6, 0x14),
  (10, 0x3f8),
  (10, 0x3f9),
  (12, 0xffa),
  (13, 0x1ff9),
  (6, 0x15),
  (8, 0xf8),
  (11, 0x7fa),
  (10, 0x3fa),
  (10, 0x3fb),
  (8, 0xf9),
  (11, 0x7fb),
  (8, 0xfa),
  (6, 0x16),
  (6, 0x17),
  (6, 0x18),
  (5, 0x0),
  (5, 0x1),
  (5, 0x2),
  (6, 0x19),
  (6, 0x1a),
  (6, 0x1b),
  (6, 0x1c),
  (6, 0x1d),
  (6, 0x1e),
  (6, 0x1f),
  (7, 0x5c),
  (8, 0xfb),
  (15, 0x7ffc),
  (6, 0x20),
  (12, 0xffb),
  (10, 0x3fc),
  (13, 0x1ffa),
  (6, 0x21),
  (7, 0x5d),
  (7, 0x5e),
  (7, 0x5f),
  (7, 0x60),
  (7, 0x61),
  (7, 0x62),
  (7, 0x63),
  (7, 0x64),
  (7, 0x65),
  (7, 0x66),
  (7, 0x67),
  (7, 0x68),
  (7, 0x69),
  (7, 0x6a),
  (7, 0x6b),
  (7, 0x6c),
  (7, 0x6d),
  (7, 0x6e),
  (7, 0x6f),
  (7, 0x70),
  (7, 0x71),
  (7, 0x72),
  (8, 0xfc),
  (7, 0x73),
  (8, 0xfd),
  (13, 0x1ffb),
  (19, 0x7fff0),
  (13, 0x1ffc),
  (14, 0x3ffc),
  (6, 0x22),
  (15, 0x7ffd),
  (5, 0x3),
  (6, 0x23),
  (5, 0x4),
  (6, 0x24),
  (5, 0x5),
  (6, 0x25),
  (6, 0x26),
  (6, 0x27),
  (5, 0x6),
  (7, 0x74),
  (7, 0x75),
  (6, 0x28),
  (6, 0x29),
  (6, 0x2a),
  (5, 0x7),
  (6, 0x2b),
  (7, 0x76),
  (6, 0x2c),
  (5, 0x8),
  (5, 0x9),
  (6, 0x2d),
  (7, 0x77),
  (7, 0x78),
  (7, 0x79),
  (7, 0x7a),
  (7, 0x7b),
  (15, 0x7ffe),
  (11, 0x7fc),
  (14, 0x3ffd),
  (13, 0x1ffd),
  (28, 0xffffffc),
  (20, 0xfffe6),
  (22, 0x3fffd2),
  (20, 0xfffe7),
  (20, 0xfffe8),
  (22, 0x3fffd3),
  (22, 0x3fffd4),
  (22, 0x3fffd5),
  (23, 0x7fffd9),
  (22, 0x3fffd6),
  (23, 0x7fffda),
  (23, 0x7fffdb),
  (23, 0x7fffdc),
  (23, 0x7fffdd),
  (23, 0x7fffde),
  (24, 0xffffeb),
  (23, 0x7fffdf),
  (24, 0xffffec),
  (24, 0xffffed),
  (22, 0x3fffd7),
  (23, 0x7fffe0),
  (24, 0xffffee),
  (23, 0x7fffe1),
  (23, 0x7fffe2),
  (23, 0x7fffe3),
  (23, 0x7fffe4),
  (21, 0x1fffdc),
  (22, 0x3fffd8),
  (23, 0x7fffe5),
  (22, 0x3fffd9),
  (23, 0x7fffe6),
  (23, 0x7fffe7),
  (24, 0xffffef),
  (22, 0x3fffda),
  (21, 0x1fffdd),
  (20, 0xfffe9),
  (22, 0x3fffdb),
  (22, 0x3fffdc),
  (23, 0x7fffe8),
  (23, 0x7fffe9),
  (21, 0x1fffde),
  (23, 0x7fffea),
  (22, 0x3fffdd),
  (22, 0x3fffde),
  (24, 0xfffff0),
  (21, 0x1fffdf),
  (22, 0x3fffdf),
  (23, 0x7fffeb),
  (23, 0x7fffec),
  (21, 0x1fffe0),
  (21, 0x1fffe1),
  (22, 0x3fffe0),
  (21, 0x1fffe2),
  (23, 0x7fffed),
  (22, 0x3fffe1),
  (23, 0x7fffee),
  (23, 0x7fffef),
  (20, 0xfffea),
  (22, 0x3fffe2),
  (22, 0x3fffe3),
  (22, 0x3fffe4),
  (23, 0x7ffff0),
  (22, 0x3fffe5),
  (22, 0x3fffe6),
  (23, 0x7ffff1),
  (26, 0x3ffffe0),
  (26, 0x3ffffe1),
  (20, 0xfffeb),
  (19, 0x7fff1),
  (22, 0x3fffe7),
  (23, 0x7ffff2),
  (22, 0x3fffe8),
  (25, 0x1ffffec),
  (26, 0x3ffffe2),
  (26, 0x3ffffe3),
  (26, 0x3ffffe4),
  (27, 0x7ffffde),
  (27, 0x7ffffdf),
  (26, 0x3ffffe5),
  (24, 0xfffff1),
  (25, 0x1ffffed),
  (19, 0x7fff2),
  (21, 0x1fffe3),
  (26, 0x3ffffe6),
  (27, 0x7ffffe0),
  (27, 0x7ffffe1),
  (26, 0x3ffffe7),
  (27, 0x7ffffe2),
  (24, 0xfffff2),
  (21, 0x1fffe4),
  (21, 0x1fffe5),
  (26, 0x3ffffe8),
  (26, 0x3ffffe9),
  (28, 0xffffffd),
  (27, 0x7ffffe3),
  (27, 0x7ffffe4),
  (27, 0x7ffffe5),
  (20, 0xfffec),
  (24, 0xfffff3),
  (20, 0xfffed),
  (21, 0x1fffe6),
  (22, 0x3fffe9),
  (21, 0x1fffe7),
  (21, 0x1fffe8),
  (23, 0x7ffff3),
  (22, 0x3fffea),
  (22, 0x3fffeb),
  (25, 0x1ffffee),
  (25, 0x1ffffef),
  (24, 0xfffff4),
  (24, 0xfffff5),
  (26, 0x3ffffea),
  (23, 0x7ffff4),
  (26, 0x3ffffeb),
  (27, 0x7ffffe6),
  (26, 0x3ffffec),
  (26, 0x3ffffed),
  (27, 0x7ffffe7),
  (27, 0x7ffffe8),
  (27, 0x7ffffe9),
  (27, 0x7ffffea),
  (27, 0x7ffffeb),
  (28, 0xffffffe),
  (27, 0x7ffffec),
  (27, 0x7ffffed),
  (27, 0x7ffffee),
  (27, 0x7ffffef),
  (27, 0x7fffff0),
  (26, 0x3ffffee),
  (30, 0x3fffffff),
];

pub(crate) struct Http2ResponseFlowControl<'a> {
  pub(crate) max_inbound_frame_size: usize,
  pub(crate) max_header_list_size: usize,
  pub(crate) max_frame_size: &'a mut usize,
  pub(crate) peer_header_table_size: &'a mut usize,
  pub(crate) peer_initial_stream_send_window: &'a mut i32,
  pub(crate) peer_enable_connect_protocol: &'a mut bool,
  pub(crate) connection_send_window: &'a mut Http2SendWindow,
  pub(crate) connection_receive_window: &'a mut i32,
  pub(crate) stream_send_window: &'a mut Http2SendWindow,
  pub(crate) streams: &'a mut Vec<Http2RequestStream>,
  pub(crate) reset_streams: &'a mut Vec<u32>,
  pub(crate) stream_ids: &'a mut Http2ClientStreamIds,
  pub(crate) request_header_decoder: &'a mut Http2HeaderDecoder,
  pub(crate) accepted_stream_count: &'a mut usize,
  pub(crate) last_accepted_stream_id: &'a mut u32,
}

pub(crate) enum Http2ResponseFlowControlRead {
  WindowAvailable,
  ResponseReset,
}

pub(crate) fn write_http2_response<S: Read + Write>(
  stream: &mut S,
  stream_id: u32,
  response: &HttpResponse,
  write_body: bool,
  flow_control: &mut Http2ResponseFlowControl<'_>,
) -> io::Result<()> {
  let mut header_encoder = Http2HeaderEncoder::new(*flow_control.peer_header_table_size);
  let dynamic_candidates = repeated_http2_response_fields(response);
  let headers = encode_http2_response_headers(response, &mut header_encoder, &dynamic_candidates)?;
  if !write_body {
    write_http2_header_block(
      stream,
      stream_id,
      HTTP2_FLAG_END_STREAM,
      &headers,
      *flow_control.max_frame_size,
    )?;
    return stream.flush();
  }
  let write_trailers = write_body && response.allows_body() && !response.trailers().is_empty();
  write_http2_header_block(stream, stream_id, 0, &headers, *flow_control.max_frame_size)?;
  let body = if write_body && response.allows_body() {
    response.body.as_slice()
  } else {
    &[]
  };
  if body.is_empty() {
    let flags = if write_trailers {
      0
    } else {
      HTTP2_FLAG_END_STREAM
    };
    write_http2_frame(stream, HTTP2_FRAME_DATA, flags, stream_id, &[])?;
  } else {
    let mut offset = 0;
    while offset < body.len() {
      while flow_control.connection_send_window.available() == 0
        || flow_control.stream_send_window.available() == 0
      {
        match read_http2_response_flow_control_frame(stream, stream_id, flow_control)? {
          Http2ResponseFlowControlRead::WindowAvailable => {}
          Http2ResponseFlowControlRead::ResponseReset => return Ok(()),
        }
      }

      let chunk_len = (body.len() - offset)
        .min(*flow_control.max_frame_size)
        .min(flow_control.connection_send_window.available())
        .min(flow_control.stream_send_window.available());
      let final_data = offset + chunk_len == body.len();
      let flags = if final_data && !write_trailers {
        HTTP2_FLAG_END_STREAM
      } else {
        0
      };
      let chunk = &body[offset..offset + chunk_len];
      flow_control.connection_send_window.consume(chunk_len)?;
      flow_control.stream_send_window.consume(chunk_len)?;
      write_http2_frame(stream, HTTP2_FRAME_DATA, flags, stream_id, chunk)?;
      stream.flush()?;
      offset += chunk_len;
    }
  }
  if write_trailers {
    header_encoder.set_max_size(*flow_control.peer_header_table_size);
    let trailers =
      encode_http2_response_trailers(response, &mut header_encoder, &dynamic_candidates)?;
    write_http2_header_block(
      stream,
      stream_id,
      HTTP2_FLAG_END_STREAM,
      &trailers,
      *flow_control.max_frame_size,
    )?;
  }
  stream.flush()
}

pub(crate) fn read_http2_response_flow_control_frame<S: Read + Write>(
  stream: &mut S,
  response_stream_id: u32,
  flow_control: &mut Http2ResponseFlowControl<'_>,
) -> io::Result<Http2ResponseFlowControlRead> {
  loop {
    let frame = read_http2_frame(stream, flow_control.max_inbound_frame_size)?;
    if let Some(stream_id) = active_http2_header_continuation_stream(flow_control.streams) {
      if frame.frame_type != HTTP2_FRAME_CONTINUATION || frame.stream_id != stream_id {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "HTTP/2 frame interleaved before END_HEADERS",
        ));
      }
    }
    match (frame.frame_type, frame.stream_id) {
      (HTTP2_FRAME_WINDOW_UPDATE, 0) => {
        flow_control
          .connection_send_window
          .increase(http2_window_update_increment(&frame.payload)?)?;
        return Ok(Http2ResponseFlowControlRead::WindowAvailable);
      }
      (HTTP2_FRAME_WINDOW_UPDATE, id) if id == response_stream_id => {
        flow_control
          .stream_send_window
          .increase(http2_window_update_increment(&frame.payload)?)?;
        return Ok(Http2ResponseFlowControlRead::WindowAvailable);
      }
      (HTTP2_FRAME_WINDOW_UPDATE, id) => {
        let increment = http2_window_update_increment(&frame.payload)?;
        if let Some(request_stream) = flow_control
          .streams
          .iter_mut()
          .find(|request_stream| request_stream.stream_id == id)
        {
          request_stream.send_window.increase(increment)?;
        }
      }
      (HTTP2_FRAME_SETTINGS, 0) => {
        if frame.flags & HTTP2_FLAG_ACK == HTTP2_FLAG_ACK {
          if !frame.payload.is_empty() {
            return Err(io::Error::new(
              io::ErrorKind::InvalidData,
              "HTTP/2 SETTINGS ACK frame must not contain payload",
            ));
          }
        } else {
          validate_http2_settings_payload(&frame.payload)?;
          if let Some(updated_max_frame_size) = http2_settings_max_frame_size(&frame.payload) {
            *flow_control.max_frame_size = updated_max_frame_size;
          }
          if let Some(header_table_size) = http2_settings_header_table_size(&frame.payload) {
            *flow_control.peer_header_table_size = header_table_size;
          }
          if let Some(initial_window_size) = http2_settings_initial_window_size(&frame.payload) {
            let delta = initial_window_size - *flow_control.peer_initial_stream_send_window;
            flow_control.stream_send_window.adjust(delta)?;
            for request_stream in &mut *flow_control.streams {
              request_stream.send_window.adjust(delta)?;
            }
            *flow_control.peer_initial_stream_send_window = initial_window_size;
          }
          if http2_settings_enable_connect_protocol(&frame.payload) {
            *flow_control.peer_enable_connect_protocol = true;
          }
          write_http2_frame(stream, HTTP2_FRAME_SETTINGS, HTTP2_FLAG_ACK, 0, &[])?;
          stream.flush()?;
        }
      }
      (HTTP2_FRAME_SETTINGS, _) => return Err(invalid_http2_settings_error()),
      (HTTP2_FRAME_PING, 0) => {
        if frame.payload.len() != 8 {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid HTTP/2 PING frame",
          ));
        }
        if frame.flags & HTTP2_FLAG_ACK != HTTP2_FLAG_ACK {
          write_http2_frame(stream, HTTP2_FRAME_PING, HTTP2_FLAG_ACK, 0, &frame.payload)?;
          stream.flush()?;
        }
      }
      (HTTP2_FRAME_PING, _) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "invalid HTTP/2 PING frame",
        ));
      }
      (HTTP2_FRAME_GOAWAY, 0) => {
        return Err(io::Error::new(
          io::ErrorKind::UnexpectedEof,
          "HTTP/2 connection received GOAWAY",
        ));
      }
      (HTTP2_FRAME_HEADERS, id) if id != 0 => {
        let header_block_fragment =
          http2_headers_payload_to_header_block_fragment(&frame.payload, frame.flags)?;
        let is_new_stream = flow_control
          .streams
          .iter()
          .all(|request_stream| request_stream.stream_id != id);
        let request_stream = http2_request_stream(
          flow_control.streams,
          flow_control.stream_ids,
          id,
          *flow_control.peer_initial_stream_send_window,
        )?;
        if request_stream.end_stream {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 HEADERS frame arrived after END_STREAM",
          ));
        }
        let header_block_kind = if request_stream.decoded_headers.is_some() {
          if request_stream.is_extended_connect() {
            return Err(unsupported_http2_extended_connect_body_error());
          }
          if frame.flags & HTTP2_FLAG_END_STREAM != HTTP2_FLAG_END_STREAM {
            return Err(io::Error::new(
              io::ErrorKind::InvalidData,
              "HTTP/2 request trailers must end the stream",
            ));
          }
          Http2HeaderBlockKind::RequestTrailers
        } else {
          Http2HeaderBlockKind::RequestHeaders
        };
        request_stream.header_block_kind = Some(header_block_kind);
        request_stream
          .header_block
          .extend_from_slice(header_block_fragment);
        if frame.flags & HTTP2_FLAG_END_HEADERS == HTTP2_FLAG_END_HEADERS {
          request_stream.finish_header_block(
            flow_control.request_header_decoder,
            flow_control.max_header_list_size,
            *flow_control.peer_enable_connect_protocol,
          )?;
        } else {
          request_stream.in_header_continuation = true;
        }
        if frame.flags & HTTP2_FLAG_END_STREAM == HTTP2_FLAG_END_STREAM {
          request_stream.end_stream = true;
        }
        if is_new_stream {
          *flow_control.accepted_stream_count += 1;
          *flow_control.last_accepted_stream_id = id;
        }
      }
      (HTTP2_FRAME_CONTINUATION, id) if id != 0 => {
        let Some(request_stream) = flow_control
          .streams
          .iter_mut()
          .find(|request_stream| request_stream.stream_id == id)
        else {
          if flow_control.stream_ids.is_closed(id) {
            return Err(http2_closed_stream_error());
          }
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 CONTINUATION frame arrived without request headers",
          ));
        };
        if !request_stream.in_header_continuation {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 CONTINUATION frame arrived without request headers",
          ));
        }
        request_stream
          .header_block
          .extend_from_slice(&frame.payload);
        if frame.flags & HTTP2_FLAG_END_HEADERS == HTTP2_FLAG_END_HEADERS {
          request_stream.finish_header_block(
            flow_control.request_header_decoder,
            flow_control.max_header_list_size,
            *flow_control.peer_enable_connect_protocol,
          )?;
        }
      }
      (HTTP2_FRAME_CONTINUATION, _) => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "HTTP/2 CONTINUATION frame arrived without request headers",
        ));
      }
      (HTTP2_FRAME_DATA, id) if id != 0 => {
        let Some(request_stream) = flow_control
          .streams
          .iter_mut()
          .find(|request_stream| request_stream.stream_id == id)
        else {
          if flow_control.reset_streams.contains(&id) {
            continue;
          }
          if flow_control.stream_ids.is_closed(id) {
            return Err(http2_closed_stream_error());
          }
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 DATA frame arrived before request headers",
          ));
        };
        if request_stream.decoded_headers.is_none() || request_stream.in_header_continuation {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 DATA frame arrived before request headers",
          ));
        }
        if request_stream.is_extended_connect() {
          return Err(unsupported_http2_extended_connect_body_error());
        }

        let data_payload = http2_data_payload_to_data(&frame.payload, frame.flags)?;
        let flow_controlled_len =
          i32::try_from(frame.payload.len()).map_err(|_| http2_flow_control_error())?;
        request_stream.receive_flow_controlled_data(
          flow_control.connection_receive_window,
          flow_controlled_len,
        )?;
        let new_len = request_stream
          .body
          .len()
          .checked_add(data_payload.len())
          .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request body is too large"))?;
        reject_oversized_request_body(new_len, MAX_REQUEST_BODY_BYTES)?;
        request_stream.body.extend_from_slice(data_payload);
        if !frame.payload.is_empty() {
          write_http2_window_update(stream, 0, frame.payload.len())?;
          write_http2_window_update(stream, id, frame.payload.len())?;
          request_stream.release_flow_controlled_data(
            flow_control.connection_receive_window,
            flow_controlled_len,
          )?;
        }
        if frame.flags & HTTP2_FLAG_END_STREAM == HTTP2_FLAG_END_STREAM {
          request_stream.end_stream = true;
        }
      }
      (HTTP2_FRAME_RST_STREAM, id) if id != 0 => {
        validate_http2_rst_stream_frame(id, &frame.payload)?;
        flow_control
          .streams
          .retain(|request_stream| request_stream.stream_id != id);
        if !flow_control.reset_streams.contains(&id) {
          flow_control.reset_streams.push(id);
        }
        if id == response_stream_id {
          return Ok(Http2ResponseFlowControlRead::ResponseReset);
        }
      }
      (HTTP2_FRAME_RST_STREAM, id) => {
        validate_http2_rst_stream_frame(id, &frame.payload)?;
      }
      (_, 0) => {}
      _ => {}
    }
    if flow_control.connection_send_window.available() > 0
      && flow_control.stream_send_window.available() > 0
    {
      return Ok(Http2ResponseFlowControlRead::WindowAvailable);
    }
  }
}

pub(crate) fn write_http2_header_block<S: Write>(
  stream: &mut S,
  stream_id: u32,
  first_frame_flags: u8,
  block: &[u8],
  max_frame_size: usize,
) -> io::Result<()> {
  if block.len() <= max_frame_size {
    return write_http2_frame(
      stream,
      HTTP2_FRAME_HEADERS,
      first_frame_flags | HTTP2_FLAG_END_HEADERS,
      stream_id,
      block,
    );
  }

  let mut chunks = block.chunks(max_frame_size);
  let first = chunks.next().unwrap_or(&[]);
  write_http2_frame(
    stream,
    HTTP2_FRAME_HEADERS,
    first_frame_flags,
    stream_id,
    first,
  )?;

  while let Some(chunk) = chunks.next() {
    let flags = if chunks.len() == 0 {
      HTTP2_FLAG_END_HEADERS
    } else {
      0
    };
    write_http2_frame(stream, HTTP2_FRAME_CONTINUATION, flags, stream_id, chunk)?;
  }

  Ok(())
}

pub(crate) struct Http2HeaderEncoder {
  pub(crate) dynamic_entries: Vec<(String, String)>,
  pub(crate) max_size: usize,
  pub(crate) current_size: usize,
  pub(crate) pending_max_size_update: Option<usize>,
}

impl Http2HeaderEncoder {
  pub(crate) fn new(max_size: usize) -> Self {
    Self {
      dynamic_entries: Vec::new(),
      max_size,
      current_size: 0,
      pending_max_size_update: None,
    }
  }

  pub(crate) fn dynamic_header_index(&self, name: &str, value: &str) -> Option<usize> {
    self
      .dynamic_entries
      .iter()
      .position(|(entry_name, entry_value)| entry_name == name && entry_value == value)
      .map(|position| HTTP2_STATIC_TABLE_LEN + position + 1)
  }

  pub(crate) fn can_insert(&self, name: &str, value: &str) -> bool {
    hpack_dynamic_entry_size(name, value) <= self.max_size
  }

  pub(crate) fn set_max_size(&mut self, max_size: usize) {
    if self.max_size == max_size {
      return;
    }
    self.max_size = max_size;
    self.pending_max_size_update = Some(max_size);
    self.evict_to_max_size();
  }

  pub(crate) fn encode_pending_max_size_update(&mut self, block: &mut Vec<u8>) -> io::Result<()> {
    if let Some(max_size) = self.pending_max_size_update.take() {
      encode_http2_dynamic_table_size_update(block, max_size)?;
    }
    Ok(())
  }

  pub(crate) fn insert(&mut self, name: String, value: String) {
    let entry_size = hpack_dynamic_entry_size(&name, &value);
    if entry_size > self.max_size {
      self.dynamic_entries.clear();
      self.current_size = 0;
      return;
    }
    self.dynamic_entries.insert(0, (name, value));
    self.current_size += entry_size;
    self.evict_to_max_size();
  }

  pub(crate) fn evict_to_max_size(&mut self) {
    while self.current_size > self.max_size {
      let Some((name, value)) = self.dynamic_entries.pop() else {
        self.current_size = 0;
        return;
      };
      self.current_size -= hpack_dynamic_entry_size(&name, &value);
    }
  }
}

pub(crate) fn repeated_http2_response_fields(response: &HttpResponse) -> Vec<(String, String)> {
  let mut fields = Vec::<(String, String)>::new();
  for header in &response.headers {
    let name = header.name.to_ascii_lowercase();
    if !is_http2_skipped_response_header_name(&name) {
      fields.push((name, header.value.clone()));
    }
  }
  for trailer in response.trailers() {
    let name = trailer.name.to_ascii_lowercase();
    if !is_http2_skipped_response_trailer_name(&name) {
      fields.push((name, trailer.value.clone()));
    }
  }

  let mut repeated = Vec::<(String, String)>::new();
  for (index, (name, value)) in fields.iter().enumerate() {
    if repeated
      .iter()
      .any(|(repeated_name, repeated_value)| repeated_name == name && repeated_value == value)
    {
      continue;
    }
    if fields[index + 1..]
      .iter()
      .any(|(other_name, other_value)| other_name == name && other_value == value)
    {
      repeated.push((name.clone(), value.clone()));
    }
  }
  repeated
}

pub(crate) fn is_repeated_http2_response_field(
  dynamic_candidates: &[(String, String)],
  name: &str,
  value: &str,
) -> bool {
  dynamic_candidates
    .iter()
    .any(|(candidate_name, candidate_value)| candidate_name == name && candidate_value == value)
}

pub(crate) fn encode_http2_response_headers(
  response: &HttpResponse,
  encoder: &mut Http2HeaderEncoder,
  dynamic_candidates: &[(String, String)],
) -> io::Result<Vec<u8>> {
  let mut block = Vec::new();
  encoder.encode_pending_max_size_update(&mut block)?;
  match response.status_code {
    200 => block.push(0x88),
    204 => block.push(0x89),
    206 => block.push(0x8a),
    304 => block.push(0x8b),
    400 => block.push(0x8c),
    404 => block.push(0x8d),
    500 => block.push(0x8e),
    status => encode_http2_literal_indexed_name_without_indexing(
      &mut block,
      8,
      status.to_string().as_bytes(),
    )?,
  }

  let content_length = if response.allows_body() {
    response.body.len()
  } else {
    0
  };
  encode_http2_literal_new_name_without_indexing(
    &mut block,
    b"content-length",
    content_length.to_string().as_bytes(),
  )?;

  for header in &response.headers {
    let name = header.name.to_ascii_lowercase();
    if is_http2_skipped_response_header_name(&name) {
      continue;
    }
    encode_http2_response_field(
      &mut block,
      encoder,
      dynamic_candidates,
      &name,
      &header.value,
    )?;
  }

  Ok(block)
}

pub(crate) fn encode_http2_response_trailers(
  response: &HttpResponse,
  encoder: &mut Http2HeaderEncoder,
  dynamic_candidates: &[(String, String)],
) -> io::Result<Vec<u8>> {
  let mut block = Vec::new();
  encoder.encode_pending_max_size_update(&mut block)?;
  for trailer in response.trailers() {
    let name = trailer.name.to_ascii_lowercase();
    if is_http2_skipped_response_trailer_name(&name) {
      continue;
    }
    encode_http2_response_field(
      &mut block,
      encoder,
      dynamic_candidates,
      &name,
      &trailer.value,
    )?;
  }
  Ok(block)
}

pub(crate) fn encode_http2_response_field(
  block: &mut Vec<u8>,
  encoder: &mut Http2HeaderEncoder,
  dynamic_candidates: &[(String, String)],
  name: &str,
  value: &str,
) -> io::Result<()> {
  let literal_without_indexing_len =
    encoded_http2_literal_new_name_without_indexing_len(name.as_bytes(), value.as_bytes())?;
  if let Some(index) = encoder.dynamic_header_index(name, value) {
    let indexed_len = encoded_http2_indexed_len(index)?;
    if indexed_len < literal_without_indexing_len {
      return encode_http2_indexed(block, index);
    }
  }

  if is_repeated_http2_response_field(dynamic_candidates, name, value)
    && encoder.can_insert(name, value)
  {
    let indexed_len = encoded_http2_indexed_len(HTTP2_STATIC_TABLE_LEN + 1)?;
    if indexed_len < literal_without_indexing_len {
      encode_http2_literal_new_name_with_incremental_indexing(
        block,
        name.as_bytes(),
        value.as_bytes(),
      )?;
      encoder.insert(name.to_string(), value.to_string());
      return Ok(());
    }
  }

  encode_http2_literal_new_name_without_indexing(block, name.as_bytes(), value.as_bytes())
}

pub(crate) fn encode_http2_dynamic_table_size_update(
  block: &mut Vec<u8>,
  size: usize,
) -> io::Result<()> {
  encode_http2_integer(block, size, 5, 0x20)
}

pub(crate) fn is_http2_skipped_response_header_name(name: &str) -> bool {
  matches!(
    name,
    "connection"
      | "keep-alive"
      | "proxy-connection"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
      | "content-length"
  )
}

pub(crate) fn is_http2_skipped_response_trailer_name(name: &str) -> bool {
  matches!(
    name,
    "authorization"
      | "connection"
      | "content-length"
      | "cookie"
      | "host"
      | "keep-alive"
      | "proxy-authenticate"
      | "proxy-authorization"
      | "proxy-connection"
      | "set-cookie"
      | "te"
      | "trailer"
      | "transfer-encoding"
      | "upgrade"
      | "www-authenticate"
  )
}

pub(crate) fn encode_http2_indexed(block: &mut Vec<u8>, index: usize) -> io::Result<()> {
  encode_http2_integer(block, index, 7, 0x80)
}

pub(crate) fn encode_http2_literal_new_name_with_incremental_indexing(
  block: &mut Vec<u8>,
  name: &[u8],
  value: &[u8],
) -> io::Result<()> {
  block.push(0x40);
  encode_http2_string(block, name)?;
  encode_http2_string(block, value)
}

pub(crate) fn encoded_http2_indexed_len(index: usize) -> io::Result<usize> {
  let mut encoded = Vec::new();
  encode_http2_indexed(&mut encoded, index)?;
  Ok(encoded.len())
}

pub(crate) fn encoded_http2_literal_new_name_without_indexing_len(
  name: &[u8],
  value: &[u8],
) -> io::Result<usize> {
  let mut encoded = Vec::new();
  encode_http2_literal_new_name_without_indexing(&mut encoded, name, value)?;
  Ok(encoded.len())
}

pub(crate) fn encode_http2_literal_indexed_name_without_indexing(
  block: &mut Vec<u8>,
  name_index: u8,
  value: &[u8],
) -> io::Result<()> {
  if name_index > 15 {
    return Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "HPACK literal indexed name is too large for the minimal encoder",
    ));
  }
  block.push(name_index);
  encode_http2_string(block, value)
}

pub(crate) fn encode_http2_literal_new_name_without_indexing(
  block: &mut Vec<u8>,
  name: &[u8],
  value: &[u8],
) -> io::Result<()> {
  block.push(0);
  encode_http2_string(block, name)?;
  encode_http2_string(block, value)
}

pub(crate) fn encode_http2_string(block: &mut Vec<u8>, value: &[u8]) -> io::Result<()> {
  let huffman = encode_http2_huffman_string(value);
  if huffman.len() < value.len() {
    encode_http2_integer(block, huffman.len(), 7, 0x80)?;
    block.extend_from_slice(&huffman);
    return Ok(());
  }

  encode_http2_integer(block, value.len(), 7, 0)?;
  block.extend_from_slice(value);
  Ok(())
}

pub(crate) fn encode_http2_huffman_string(value: &[u8]) -> Vec<u8> {
  let mut encoded = Vec::new();
  let mut bits = 0u64;
  let mut bit_len = 0u8;

  for byte in value {
    let (code_len, code) = HTTP2_HUFFMAN_CODES[*byte as usize];
    bits = (bits << code_len) | u64::from(code);
    bit_len += code_len;

    while bit_len >= 8 {
      let shift = bit_len - 8;
      encoded.push((bits >> shift) as u8);
      bit_len -= 8;
      bits &= (1u64 << shift) - 1;
    }
  }

  if bit_len > 0 {
    let padding = 8 - bit_len;
    bits = (bits << padding) | ((1u64 << padding) - 1);
    encoded.push(bits as u8);
  }

  encoded
}

pub(crate) fn encode_http2_integer(
  block: &mut Vec<u8>,
  mut value: usize,
  prefix_bits: u8,
  first_byte_prefix: u8,
) -> io::Result<()> {
  let max_prefix = (1usize << prefix_bits) - 1;
  if value < max_prefix {
    block.push(first_byte_prefix | value as u8);
    return Ok(());
  }

  block.push(first_byte_prefix | max_prefix as u8);
  value -= max_prefix;
  while value >= 128 {
    block.push((value % 128) as u8 + 128);
    value /= 128;
  }
  block.push(value as u8);
  Ok(())
}

pub(crate) fn http2_static_header(index: usize) -> io::Result<(&'static str, &'static str)> {
  match index {
    1 => Ok((":authority", "")),
    2 => Ok((":method", "GET")),
    3 => Ok((":method", "POST")),
    4 => Ok((":path", "/")),
    5 => Ok((":path", "/index.html")),
    6 => Ok((":scheme", "http")),
    7 => Ok((":scheme", "https")),
    8 => Ok((":status", "200")),
    9 => Ok((":status", "204")),
    10 => Ok((":status", "206")),
    11 => Ok((":status", "304")),
    12 => Ok((":status", "400")),
    13 => Ok((":status", "404")),
    14 => Ok((":status", "500")),
    31 => Ok(("content-type", "")),
    33 => Ok(("date", "")),
    54 => Ok(("server", "")),
    _ => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "unsupported HPACK static table index",
    )),
  }
}
