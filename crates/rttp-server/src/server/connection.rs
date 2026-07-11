use super::*;

pub struct HttpServer {
  pub(crate) listener: TcpListener,
  pub(crate) read_timeout: Option<Duration>,
  pub(crate) write_timeout: Option<Duration>,
  pub(crate) http2_policy: Http2ServerPolicy,
}

impl HttpServer {
  pub fn bind<A>(addr: A) -> io::Result<Self>
  where
    A: ToSocketAddrs,
  {
    let mut last_err = None;

    for addr in addr.to_socket_addrs()? {
      let socket = match Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP)) {
        Ok(socket) => socket,
        Err(err) => {
          last_err = Some(err);
          continue;
        }
      };

      if let Err(err) = socket.set_reuse_address(true) {
        last_err = Some(err);
        continue;
      }
      if let Err(err) = socket.bind(&addr.into()) {
        last_err = Some(err);
        continue;
      }
      if let Err(err) = socket.listen(128) {
        last_err = Some(err);
        continue;
      }

      return Ok(Self {
        listener: TcpListener::from(socket),
        read_timeout: None,
        write_timeout: None,
        http2_policy: Http2ServerPolicy::default(),
      });
    }

    Err(
      last_err
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "address did not resolve")),
    )
  }

  pub fn local_addr(&self) -> io::Result<SocketAddr> {
    self.listener.local_addr()
  }

  /// Sets the read timeout applied to each accepted connection before parsing requests.
  pub fn with_read_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.read_timeout = timeout;
    self
  }

  /// Sets the write timeout applied to each accepted connection before writing responses.
  pub fn with_write_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.write_timeout = timeout;
    self
  }

  /// Sets the fixed bounds advertised and enforced for accepted h2c connections.
  pub fn with_http2_policy(mut self, policy: Http2ServerPolicy) -> Self {
    self.http2_policy = policy;
    self
  }

  pub fn accept_one<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpResponse,
  {
    self.handle_next_connection(handler)
  }

  pub fn accept_one_streaming<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request, RequestBodyReader<'_, BufReader<TcpStream>>) -> HttpResponse,
  {
    self.handle_next_streaming_connection(handler)
  }

  pub fn accept_one_handoff<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpHandoff,
  {
    self.handle_next_handoff_connection(handler)
  }

  pub fn serve_requests<F>(&self, request_count: usize, mut handler: F) -> io::Result<()>
  where
    F: FnMut(Request) -> HttpResponse,
  {
    let mut served = 0;

    while served < request_count {
      let (stream, _) = self.listener.accept()?;
      self.configure_stream(&stream)?;
      let handled = self.handle_connection(stream, request_count - served, &mut handler)?;
      served += handled.max(1);
    }

    Ok(())
  }

  pub(crate) fn handle_connection<F>(
    &self,
    stream: TcpStream,
    request_limit: usize,
    handler: &mut F,
  ) -> io::Result<usize>
  where
    F: FnMut(Request) -> HttpResponse,
  {
    let stream = match self.detect_connection_protocol(stream)? {
      AcceptedConnection::Http1(stream) => stream,
      AcceptedConnection::Http2(stream) => {
        return self.handle_http2_connection(stream, request_limit, handler);
      }
    };
    let mut reader = BufReader::new(stream);
    let mut served = 0;

    while served < request_limit {
      let request = match self
        .normalize_connection_error(Request::read_next_from_with_continue(&mut reader))
      {
        Ok(Some(request)) => request,
        Ok(None) => break,
        Err(err) if is_expectation_failed_error(&err) => {
          self
            .normalize_connection_error(expectation_failed_response().write_to(reader.get_mut()))?;
          served += 1;
          break;
        }
        Err(err) if is_bad_request_error(&err) => {
          self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()))?;
          served += 1;
          break;
        }
        Err(err) => return Err(err),
      };
      let request_closes_connection = request.closes_connection();
      let request_uses_http10_defaults = request.version() == "HTTP/1.0";
      let request_is_head = request.method() == "HEAD";
      if let Some(settings_payload) = match h2c_upgrade_settings(&request) {
        Ok(settings_payload) => settings_payload,
        Err(err) if is_bad_request_error(&err) => {
          self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()))?;
          served += 1;
          break;
        }
        Err(err) => return Err(err),
      } {
        self.normalize_connection_error(write_h2c_upgrade_response(reader.get_mut()))?;
        let buffered = reader.buffer().to_vec();
        let stream = reader.into_inner().into_handoff_stream(buffered)?;
        let handled = self.handle_http2_connection_with_initial(
          stream,
          request_limit - served,
          handler,
          Some(Http2InitialSettings {
            payload: settings_payload,
            acknowledge: false,
            upgraded: true,
          }),
        )?;
        return Ok(served + handled);
      }
      let response = handler(request);
      let response_closes_connection = response.closes_connection();
      served += 1;

      let close_after_response =
        request_closes_connection || response_closes_connection || served == request_limit;
      let default_connection = if close_after_response {
        DefaultConnectionHeader::ForceClose
      } else if request_uses_http10_defaults {
        DefaultConnectionHeader::KeepAlive
      } else {
        DefaultConnectionHeader::Omit
      };
      self.normalize_connection_error(response.write_to_with_default_connection_and_body(
        reader.get_mut(),
        default_connection,
        !request_is_head,
      ))?;

      if close_after_response {
        break;
      }
    }

    Ok(served)
  }

  pub(crate) fn handle_next_connection<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpResponse,
  {
    let (stream, _) = self.listener.accept()?;
    self.configure_stream(&stream)?;
    let stream = match self.detect_connection_protocol(stream)? {
      AcceptedConnection::Http1(stream) => stream,
      AcceptedConnection::Http2(stream) => {
        let mut handler = Some(handler);
        return self
          .handle_http2_connection(stream, 1, &mut |request| {
            handler.take().expect("single h2 request handler")(request)
          })
          .map(|_| ());
      }
    };
    let mut reader = BufReader::new(stream);
    let request = match self.normalize_connection_error(
      Request::read_next_from_with_continue(&mut reader).and_then(|request| {
        request.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))
      }),
    ) {
      Ok(request) => request,
      Err(err) if is_expectation_failed_error(&err) => {
        return self
          .normalize_connection_error(expectation_failed_response().write_to(reader.get_mut()));
      }
      Err(err) if is_bad_request_error(&err) => {
        return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
      }
      Err(err) => return Err(err),
    };
    if let Some(settings_payload) = match h2c_upgrade_settings(&request) {
      Ok(settings_payload) => settings_payload,
      Err(err) if is_bad_request_error(&err) => {
        return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
      }
      Err(err) => return Err(err),
    } {
      self.normalize_connection_error(write_h2c_upgrade_response(reader.get_mut()))?;
      let buffered = reader.buffer().to_vec();
      let stream = reader.into_inner().into_handoff_stream(buffered)?;
      let mut handler = Some(handler);
      return self
        .handle_http2_connection_with_initial(
          stream,
          1,
          &mut |request| handler.take().expect("single h2 request handler")(request),
          Some(Http2InitialSettings {
            payload: settings_payload,
            acknowledge: false,
            upgraded: true,
          }),
        )
        .map(|_| ());
    }
    let request_is_head = request.method() == "HEAD";
    let response = handler(request);
    self.normalize_connection_error(response.write_to_with_default_connection_and_body(
      reader.get_mut(),
      DefaultConnectionHeader::ForceClose,
      !request_is_head,
    ))
  }

  pub(crate) fn handle_next_streaming_connection<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request, RequestBodyReader<'_, BufReader<TcpStream>>) -> HttpResponse,
  {
    let (stream, _) = self.listener.accept()?;
    self.configure_stream(&stream)?;
    let mut reader = BufReader::new(stream);
    let (request, body_kind) = match self.normalize_connection_error(
      Request::read_next_head_from_with_continue(&mut reader).and_then(|request| {
        request.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))
      }),
    ) {
      Ok(request) => request,
      Err(err) if is_expectation_failed_error(&err) => {
        return self
          .normalize_connection_error(expectation_failed_response().write_to(reader.get_mut()));
      }
      Err(err) if is_bad_request_error(&err) => {
        return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
      }
      Err(err) => return Err(err),
    };
    if h2c_upgrade_settings(&request).is_err() || h2c_upgrade_settings(&request)?.is_some() {
      return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
    }
    let request_is_head = request.method() == "HEAD";
    let body = RequestBodyReader::new(&mut reader, body_kind, self.read_timeout.is_some());
    let response = handler(request, body);
    self.normalize_connection_error(response.write_to_with_default_connection_and_body(
      reader.get_mut(),
      DefaultConnectionHeader::ForceClose,
      !request_is_head,
    ))
  }

  pub(crate) fn handle_next_handoff_connection<F>(&self, handler: F) -> io::Result<()>
  where
    F: FnOnce(Request) -> HttpHandoff,
  {
    let (stream, _) = self.listener.accept()?;
    self.configure_stream(&stream)?;
    let mut reader = BufReader::new(stream);
    let request = match self.normalize_connection_error(
      Request::read_next_head_from_with_continue(&mut reader).and_then(|request| {
        request
          .map(|(request, _)| request)
          .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))
      }),
    ) {
      Ok(request) => request,
      Err(err) if is_expectation_failed_error(&err) => {
        return self
          .normalize_connection_error(expectation_failed_response().write_to(reader.get_mut()));
      }
      Err(err) if is_bad_request_error(&err) => {
        return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
      }
      Err(err) => return Err(err),
    };

    if h2c_upgrade_settings(&request).is_err() || h2c_upgrade_settings(&request)?.is_some() {
      return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
    }

    let handoff = handler(request.clone());
    if !handoff.valid_for(&request) {
      return self.normalize_connection_error(bad_request_response().write_to(reader.get_mut()));
    }

    match handoff {
      HttpHandoff::Response(response) => self.normalize_connection_error(
        response.write_to_with_default_connection(reader.get_mut(), DefaultConnectionHeader::Close),
      ),
      HttpHandoff::Connect { response, handler } | HttpHandoff::Upgrade { response, handler } => {
        self.normalize_connection_error(response.write_handoff_head_to(reader.get_mut()))?;
        let buffered = reader.buffer().to_vec();
        let stream = HandoffStream::new(buffered, reader.into_inner());
        handler(stream)
      }
    }
  }

  pub(crate) fn configure_stream(&self, stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(self.read_timeout)?;
    stream.set_write_timeout(self.write_timeout)
  }

  pub(crate) fn detect_connection_protocol(
    &self,
    mut stream: TcpStream,
  ) -> io::Result<AcceptedConnection> {
    let mut peeked = [0; 24];

    loop {
      match self.normalize_connection_error(stream.peek(&mut peeked)) {
        Ok(0) => break,
        Ok(read) => {
          if read == HTTP2_CLIENT_PREFACE.len() && peeked[..read] == HTTP2_CLIENT_PREFACE[..] {
            let mut preface = [0; 24];
            stream.read_exact(&mut preface)?;
            return Ok(AcceptedConnection::Http2(stream));
          }
          if HTTP2_CLIENT_PREFACE.starts_with(&peeked[..read]) {
            let mut preface = [0; 24];
            stream.read_exact(&mut preface)?;
            if preface == *HTTP2_CLIENT_PREFACE {
              return Ok(AcceptedConnection::Http2(stream));
            }
            return Ok(AcceptedConnection::Http1(Http1Stream::Prefixed(
              HandoffStream::new(preface.to_vec(), stream),
            )));
          }
          if !HTTP2_CLIENT_PREFACE.starts_with(&peeked[..read]) {
            return Ok(AcceptedConnection::Http1(Http1Stream::Plain(stream)));
          }
        }
        Err(err) => return Err(err),
      }
    }

    Ok(AcceptedConnection::Http1(Http1Stream::Plain(stream)))
  }

  pub(crate) fn handle_http2_connection<F>(
    &self,
    stream: TcpStream,
    request_limit: usize,
    handler: &mut F,
  ) -> io::Result<usize>
  where
    F: FnMut(Request) -> HttpResponse,
  {
    self.handle_http2_connection_with_initial(stream, request_limit, handler, None)
  }

  pub(crate) fn handle_http2_connection_with_initial<S, F>(
    &self,
    mut stream: S,
    request_limit: usize,
    handler: &mut F,
    initial_settings: Option<Http2InitialSettings>,
  ) -> io::Result<usize>
  where
    S: Read + Write,
    F: FnMut(Request) -> HttpResponse,
  {
    self.http2_policy.validate()?;
    let (initial_payload, acknowledge_initial_settings, upgraded) =
      if let Some(initial_settings) = initial_settings {
        (
          initial_settings.payload,
          initial_settings.acknowledge,
          initial_settings.upgraded,
        )
      } else {
        let frame = self.normalize_connection_error(read_http2_frame(
          &mut stream,
          self.http2_policy.max_frame_size(),
        ))?;
        if frame.frame_type != HTTP2_FRAME_SETTINGS
          || frame.flags & HTTP2_FLAG_ACK == HTTP2_FLAG_ACK
          || frame.stream_id != 0
        {
          return Err(invalid_http2_settings_error());
        }
        (frame.payload, true, false)
      };
    validate_http2_settings_payload(&initial_payload)?;
    let mut peer_max_frame_size =
      http2_settings_max_frame_size(&initial_payload).unwrap_or(HTTP2_DEFAULT_MAX_FRAME_SIZE);
    let mut peer_initial_stream_send_window = http2_settings_initial_window_size(&initial_payload)
      .unwrap_or(HTTP2_DEFAULT_INITIAL_WINDOW_SIZE);
    let mut peer_header_table_size =
      http2_settings_header_table_size(&initial_payload).unwrap_or(HTTP2_DEFAULT_HEADER_TABLE_SIZE);
    let mut peer_enable_connect_protocol = http2_settings_enable_connect_protocol(&initial_payload);

    self.normalize_connection_error(write_http2_frame(
      &mut stream,
      HTTP2_FRAME_SETTINGS,
      0,
      0,
      &server_http2_settings_payload(request_limit, &self.http2_policy),
    ))?;
    if acknowledge_initial_settings {
      self.normalize_connection_error(write_http2_frame(
        &mut stream,
        HTTP2_FRAME_SETTINGS,
        HTTP2_FLAG_ACK,
        0,
        &[],
      ))?;
    }
    self.normalize_connection_error(stream.flush())?;
    if upgraded {
      let mut preface = [0; HTTP2_CLIENT_PREFACE.len()];
      self.normalize_connection_error(stream.read_exact(&mut preface))?;
      if preface != *HTTP2_CLIENT_PREFACE {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "invalid HTTP/2 client preface after h2c upgrade",
        ));
      }
    }

    let mut streams = Vec::<Http2RequestStream>::new();
    let mut reset_streams = Vec::<u32>::new();
    let mut stream_ids = if upgraded {
      Http2ClientStreamIds::after_http1_upgrade()
    } else {
      Http2ClientStreamIds::new()
    };
    let mut connection_receive_window = HTTP2_DEFAULT_INITIAL_WINDOW_SIZE;
    let mut connection_send_window = Http2SendWindow::new(HTTP2_DEFAULT_INITIAL_WINDOW_SIZE);
    let mut request_header_decoder = Http2HeaderDecoder::new(HTTP2_DEFAULT_HEADER_TABLE_SIZE);
    let mut served = 0;
    let mut last_processed_stream_id = 0;
    let mut accepted_stream_count = 0;
    let mut last_accepted_stream_id = 0;
    let mut graceful_goaway_sent = false;
    let mut peer_goaway_received = false;

    while served < request_limit
      && ((!graceful_goaway_sent && !peer_goaway_received) || !streams.is_empty())
    {
      let frame = match self.normalize_connection_error(read_http2_frame(
        &mut stream,
        self.http2_policy.max_frame_size(),
      )) {
        Ok(frame) => frame,
        Err(err)
          if err.kind() == io::ErrorKind::UnexpectedEof
            && active_http2_header_continuation_stream(&streams).is_some() =>
        {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete HTTP/2 header block",
          ));
        }
        Err(err)
          if err.kind() == io::ErrorKind::UnexpectedEof && served > 0 && streams.is_empty() =>
        {
          break;
        }
        Err(err) => return Err(err),
      };
      if let Some(stream_id) = active_http2_header_continuation_stream(&streams) {
        if frame.frame_type != HTTP2_FRAME_CONTINUATION || frame.stream_id != stream_id {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP/2 frame interleaved before END_HEADERS",
          ));
        }
      }
      match (frame.frame_type, frame.stream_id) {
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
            if let Some(max_frame_size) = http2_settings_max_frame_size(&frame.payload) {
              peer_max_frame_size = max_frame_size;
            }
            if let Some(header_table_size) = http2_settings_header_table_size(&frame.payload) {
              peer_header_table_size = header_table_size;
            }
            if let Some(initial_window_size) = http2_settings_initial_window_size(&frame.payload) {
              let delta = initial_window_size - peer_initial_stream_send_window;
              for request_stream in &mut streams {
                request_stream.send_window.adjust(delta)?;
              }
              peer_initial_stream_send_window = initial_window_size;
            }
            if http2_settings_enable_connect_protocol(&frame.payload) {
              peer_enable_connect_protocol = true;
            }
            self.normalize_connection_error(write_http2_frame(
              &mut stream,
              HTTP2_FRAME_SETTINGS,
              HTTP2_FLAG_ACK,
              0,
              &[],
            ))?;
            self.normalize_connection_error(stream.flush())?;
          }
        }
        (HTTP2_FRAME_SETTINGS, _) => {
          return Err(invalid_http2_settings_error());
        }
        (HTTP2_FRAME_PING, 0) => {
          if frame.payload.len() != 8 {
            return Err(io::Error::new(
              io::ErrorKind::InvalidData,
              "invalid HTTP/2 PING frame",
            ));
          }
          if frame.flags & HTTP2_FLAG_ACK != HTTP2_FLAG_ACK {
            self.normalize_connection_error(write_http2_frame(
              &mut stream,
              HTTP2_FRAME_PING,
              HTTP2_FLAG_ACK,
              0,
              &frame.payload,
            ))?;
            self.normalize_connection_error(stream.flush())?;
          }
        }
        (HTTP2_FRAME_PING, _) => {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid HTTP/2 PING frame",
          ));
        }
        (HTTP2_FRAME_PRIORITY, id) => {
          validate_http2_priority_frame(id, &frame.payload)?;
        }
        (HTTP2_FRAME_PUSH_PROMISE, _) => {
          return Err(invalid_http2_push_promise_error());
        }
        (HTTP2_FRAME_HEADERS, id) if id != 0 => {
          let header_block_fragment =
            http2_headers_payload_to_header_block_fragment(&frame.payload, frame.flags)?;
          let is_new_stream = streams
            .iter()
            .all(|request_stream| request_stream.stream_id != id);
          if streams
            .iter()
            .all(|request_stream| request_stream.stream_id != id)
            && (graceful_goaway_sent
              || peer_goaway_received
              || served.saturating_add(streams.len()) >= request_limit)
          {
            self.normalize_connection_error(write_http2_frame(
              &mut stream,
              HTTP2_FRAME_RST_STREAM,
              0,
              id,
              &HTTP2_ERROR_REFUSED_STREAM.to_be_bytes(),
            ))?;
            self.normalize_connection_error(stream.flush())?;
            if !reset_streams.contains(&id) {
              reset_streams.push(id);
            }
            continue;
          }
          {
            let request_stream = http2_request_stream(
              &mut streams,
              &mut stream_ids,
              id,
              peer_initial_stream_send_window,
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
                &mut request_header_decoder,
                self.http2_policy.max_header_list_size(),
                peer_enable_connect_protocol,
              )?;
            } else {
              request_stream.in_header_continuation = true;
            }
            if frame.flags & HTTP2_FLAG_END_STREAM == HTTP2_FLAG_END_STREAM {
              request_stream.end_stream = true;
            }
          }
          if is_new_stream {
            accepted_stream_count += 1;
            last_accepted_stream_id = id;
          }
        }
        (HTTP2_FRAME_CONTINUATION, id) if id != 0 => {
          let Some(request_stream) = streams
            .iter_mut()
            .find(|request_stream| request_stream.stream_id == id)
          else {
            if stream_ids.is_closed(id) {
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
              &mut request_header_decoder,
              self.http2_policy.max_header_list_size(),
              peer_enable_connect_protocol,
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
          let Some(request_stream) = streams
            .iter_mut()
            .find(|request_stream| request_stream.stream_id == id)
          else {
            if reset_streams.contains(&id) {
              continue;
            }
            if stream_ids.is_closed(id) {
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
          request_stream
            .receive_flow_controlled_data(&mut connection_receive_window, flow_controlled_len)?;
          let new_len = request_stream
            .body
            .len()
            .checked_add(data_payload.len())
            .ok_or_else(|| {
              io::Error::new(io::ErrorKind::InvalidData, "request body is too large")
            })?;
          reject_oversized_request_body(new_len)?;
          request_stream.body.extend_from_slice(data_payload);
          if !frame.payload.is_empty() {
            write_http2_window_update(&mut stream, 0, frame.payload.len())?;
            write_http2_window_update(&mut stream, id, frame.payload.len())?;
            request_stream
              .release_flow_controlled_data(&mut connection_receive_window, flow_controlled_len)?;
          }
          if frame.flags & HTTP2_FLAG_END_STREAM == HTTP2_FLAG_END_STREAM {
            request_stream.end_stream = true;
          }
        }
        (HTTP2_FRAME_RST_STREAM, id) if id != 0 => {
          validate_http2_rst_stream_frame(id, &frame.payload)?;
          streams.retain(|request_stream| request_stream.stream_id != id);
          if !reset_streams.contains(&id) {
            reset_streams.push(id);
          }
        }
        (HTTP2_FRAME_RST_STREAM, id) => {
          validate_http2_rst_stream_frame(id, &frame.payload)?;
        }
        (HTTP2_FRAME_GOAWAY, id) => {
          validate_http2_goaway_frame(id, &frame.payload)?;
          peer_goaway_received = true;
        }
        (HTTP2_FRAME_WINDOW_UPDATE, _) => {
          let increment = http2_window_update_increment(&frame.payload)?;
          if frame.stream_id == 0 {
            connection_send_window.increase(increment)?;
          } else if let Some(request_stream) = streams
            .iter_mut()
            .find(|request_stream| request_stream.stream_id == frame.stream_id)
          {
            request_stream.send_window.increase(increment)?;
          }
        }
        (_, 0) => {}
        _ => {}
      }

      while served < request_limit {
        let Some(index) = streams
          .iter()
          .position(|request_stream| request_stream.is_complete())
        else {
          break;
        };
        let request_stream = streams.remove(index);
        let stream_id = request_stream.stream_id;
        stream_ids.close(stream_id);
        let mut stream_send_window = request_stream.send_window;
        let request = request_stream.into_request()?;
        let request_is_head = request.method() == "HEAD";
        let response = handler(request);
        self.normalize_connection_error(write_http2_response(
          &mut stream,
          stream_id,
          &response,
          !request_is_head,
          &mut Http2ResponseFlowControl {
            max_inbound_frame_size: self.http2_policy.max_frame_size(),
            max_header_list_size: self.http2_policy.max_header_list_size(),
            max_frame_size: &mut peer_max_frame_size,
            peer_header_table_size: &mut peer_header_table_size,
            peer_initial_stream_send_window: &mut peer_initial_stream_send_window,
            connection_send_window: &mut connection_send_window,
            connection_receive_window: &mut connection_receive_window,
            stream_send_window: &mut stream_send_window,
            streams: &mut streams,
            reset_streams: &mut reset_streams,
            stream_ids: &mut stream_ids,
            request_header_decoder: &mut request_header_decoder,
            accepted_stream_count: &mut accepted_stream_count,
            last_accepted_stream_id: &mut last_accepted_stream_id,
            peer_enable_connect_protocol: &mut peer_enable_connect_protocol,
          },
        ))?;
        last_processed_stream_id = stream_id;
        served += 1;
        if !graceful_goaway_sent
          && accepted_stream_count >= request_limit
          && !streams.is_empty()
          && last_accepted_stream_id != 0
        {
          self.normalize_connection_error(write_http2_goaway(
            &mut stream,
            last_accepted_stream_id,
            HTTP2_ERROR_NO_ERROR,
          ))?;
          self.normalize_connection_error(stream.flush())?;
          graceful_goaway_sent = true;
        }
      }
    }

    if served == request_limit && last_processed_stream_id != 0 && !graceful_goaway_sent {
      self.normalize_connection_error(write_http2_goaway(
        &mut stream,
        last_processed_stream_id,
        HTTP2_ERROR_NO_ERROR,
      ))?;
      self.normalize_connection_error(stream.flush())?;
    }

    Ok(served)
  }

  pub(crate) fn normalize_connection_error<T>(&self, result: io::Result<T>) -> io::Result<T> {
    result.map_err(|err| {
      if err.kind() == io::ErrorKind::WouldBlock
        && (self.read_timeout.is_some() || self.write_timeout.is_some())
      {
        io::Error::new(io::ErrorKind::TimedOut, err)
      } else {
        err
      }
    })
  }
}
