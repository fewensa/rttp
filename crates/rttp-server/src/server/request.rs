use super::*;

pub use rttp_protocol::forwarded::{
  Forwarded as HttpForwarded, ForwardedElement as HttpForwardedElement,
  ForwardedParameter as HttpForwardedParameter, ForwardedParseError as HttpForwardedParseError,
};

pub(crate) const MAX_REQUEST_HEAD_BYTES: usize = 64 * 1024;
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_ACCEPT_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ACCEPT_MEDIA_RANGES: usize = 256;
pub(crate) const MAX_AUTHORIZATION_VALUE_BYTES: usize = 64 * 1024;

/// Typed, bounded `Authorization` request metadata.
///
/// Credentials are opaque application-owned values. RTTP validates only the
/// generic HTTP header shape and does not select, verify, or log them.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpAuthorization {
  scheme: String,
  credentials: String,
}

impl HttpAuthorization {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpAuthorizationParseError> {
    let value = value.as_ref();
    if value.len() > MAX_AUTHORIZATION_VALUE_BYTES {
      return Err(HttpAuthorizationParseError::new(
        "Authorization header value is too large",
      ));
    }
    let Some(separator) = value.bytes().position(|byte| byte == b' ' || byte == b'\t') else {
      return Err(HttpAuthorizationParseError::new(
        "Authorization header requires credentials",
      ));
    };
    let scheme = &value[..separator];
    let credentials = value[separator..].trim_matches([' ', '\t']);
    if !is_http_token(scheme) {
      return Err(HttpAuthorizationParseError::new(
        "invalid Authorization authentication scheme",
      ));
    }
    if credentials.is_empty() || !credentials.bytes().all(is_header_value_byte) {
      return Err(HttpAuthorizationParseError::new(
        "invalid Authorization credentials",
      ));
    }
    Ok(Self {
      scheme: scheme.to_string(),
      credentials: credentials.to_string(),
    })
  }

  pub fn scheme(&self) -> &str {
    &self.scheme
  }

  pub fn credentials(&self) -> &str {
    &self.credentials
  }
}

impl fmt::Debug for HttpAuthorization {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter
      .debug_struct("HttpAuthorization")
      .field("scheme", &self.scheme)
      .field("credentials", &"[REDACTED]")
      .finish()
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpAuthorizationParseError {
  message: String,
}

impl HttpAuthorizationParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpAuthorizationParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAuthorizationParseError {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
  pub(crate) method: String,
  pub(crate) target: String,
  pub(crate) version: String,
  pub(crate) headers: Vec<(String, String)>,
  pub(crate) trailers: Vec<(String, String)>,
  pub(crate) body: Vec<u8>,
  pub(crate) extended_connect_protocol: Option<String>,
}

impl Request {
  pub fn method(&self) -> &str {
    &self.method
  }

  pub fn target(&self) -> &str {
    &self.target
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  pub fn extended_connect_protocol(&self) -> Option<&str> {
    self.extended_connect_protocol.as_deref()
  }

  pub fn trailers(&self) -> &[(String, String)] {
    &self.trailers
  }

  pub fn trailer(&self, name: &str) -> Option<&str> {
    self
      .trailers
      .iter()
      .find(|(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  pub fn body(&self) -> &[u8] {
    &self.body
  }

  pub fn closes_connection(&self) -> bool {
    if self.connection_header_has_token("close") {
      return true;
    }

    self.version == "HTTP/1.0" && !self.connection_header_has_token("keep-alive")
  }

  pub fn evaluate_conditional(
    &self,
    metadata: &HttpConditionalMetadata,
  ) -> HttpConditionalRequestOutcome {
    evaluate_conditional_request(self, metadata)
  }

  pub fn evaluate_if_range(
    &self,
    metadata: &HttpConditionalMetadata,
    entity_length: usize,
  ) -> Result<HttpIfRangeRequestOutcome, HttpByteRangeError> {
    evaluate_if_range_request(self, metadata, entity_length)
  }

  pub fn cache_control(
    &self,
  ) -> Result<Option<HttpRequestCacheControl>, HttpCacheControlParseError> {
    let values: Vec<&str> = self.headers_named("Cache-Control").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestCacheControl::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn authorization(&self) -> Result<Option<HttpAuthorization>, HttpAuthorizationParseError> {
    let mut values = self.headers_named("Authorization");
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(HttpAuthorizationParseError::new(
        "duplicate Authorization headers",
      ));
    }
    HttpAuthorization::parse(value).map(Some)
  }

  /// Parses received `Max-Forwards` request metadata without automatically
  /// decrementing or forwarding the request.
  ///
  /// The validated decimal count is returned verbatim so valid values are not
  /// constrained by a machine integer width.
  pub fn max_forwards(&self) -> Result<Option<String>, HttpMaxForwardsParseError> {
    parse_max_forwards_values(self.headers_named("Max-Forwards"))
  }

  /// Parses received `TE` request metadata without enabling transfer codings.
  pub fn te(&self) -> Result<Option<HttpRequestTe>, HttpTeParseError> {
    parse_te_values(self.headers_named("TE"))
  }

  /// Parses received `Prefer` request metadata without applying preferences.
  pub fn prefer(&self) -> Result<Option<HttpRequestPreferences>, HttpPreferParseError> {
    parse_prefer_values(self.headers_named("Prefer"))
  }

  /// Parses received `Accept-Encoding` request metadata without enabling
  /// automatic compression, decompression, or content negotiation.
  pub fn accept_encoding(
    &self,
  ) -> Result<Option<HttpRequestAcceptEncodings>, HttpAcceptEncodingParseError> {
    let values: Vec<&str> = self.headers_named("Accept-Encoding").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestAcceptEncodings::parse_values(values).map(Some)
  }

  pub fn accept(&self) -> Result<Option<HttpAccept>, HttpAcceptParseError> {
    let values: Vec<&str> = self.headers_named("Accept").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccept::parse_values(values).map(Some)
  }

  pub fn accept_language(
    &self,
  ) -> Result<Option<HttpAcceptLanguages>, HttpAcceptLanguageParseError> {
    let values: Vec<&str> = self.headers_named("Accept-Language").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptLanguages::parse_values(values).map(Some)
  }

  /// Parses received HTTP `Priority` metadata without changing transport scheduling.
  pub fn priority(&self) -> Result<Option<HttpPriority>, HttpPriorityParseError> {
    let values: Vec<&str> = self.headers_named("Priority").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpPriority::parse_values(values).map(Some)
  }

  /// Parses bounded RFC 7239 `Forwarded` request metadata without applying a
  /// proxy trust policy or rewriting request addresses.
  pub fn forwarded(&self) -> Result<Option<HttpForwarded>, HttpForwardedParseError> {
    let values: Vec<&str> = self.headers_named("Forwarded").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpForwarded::parse_values(values).map(Some)
  }

  pub fn vary_selection(&self, vary: &HttpVary) -> HttpVarySelection {
    if vary.is_wildcard() {
      return HttpVarySelection::wildcard();
    }

    HttpVarySelection::from_fields(vary.fields.iter().map(|field| {
      HttpVarySelectedField::new(
        field,
        self
          .headers_named(field)
          .map(ToString::to_string)
          .collect::<Vec<_>>(),
      )
    }))
  }

  pub(crate) fn connection_header_has_token(&self, token: &str) -> bool {
    self
      .headers
      .iter()
      .filter(|(name, _)| name.eq_ignore_ascii_case("Connection"))
      .any(|(_, value)| connection_header_has_token(Some(value), token))
  }

  pub(crate) fn headers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    self
      .headers
      .iter()
      .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
      .map(|(_, value)| value.as_str())
  }

  #[cfg(test)]
  pub(crate) fn read_next_from<R>(reader: &mut R) -> io::Result<Option<Self>>
  where
    R: BufRead,
  {
    Self::read_next_from_without_continue(reader)
  }

  pub(crate) fn read_next_from_with_continue<S>(
    reader: &mut BufReader<S>,
  ) -> io::Result<Option<Self>>
  where
    S: Read + Write,
  {
    let mut raw = Vec::new();
    let mut body_kind: Option<RequestBodyKind> = None;

    loop {
      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        if raw.len() == message_len {
          return Ok(Some(Self::from_raw_frame(&raw)?));
        }
      }

      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
          (find_header_end(&raw), body_kind)
        {
          let body_start = header_end + 4;
          let body_end = checked_request_message_len(header_end, content_length)?;
          if raw.len() < body_end || body_end < body_start {
            return Err(io::Error::new(
              io::ErrorKind::UnexpectedEof,
              "incomplete HTTP request body",
            ));
          }
        }
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "incomplete HTTP request",
        ));
      }

      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        let take = (message_len - raw.len()).min(available.len());
        raw.extend_from_slice(&available[..take]);
        reader.consume(take);
        continue;
      }

      let mut combined = raw.clone();
      combined.extend_from_slice(available);
      match find_header_end(&combined) {
        Some(header_end) => {
          let take = header_end + 4 - raw.len();
          reject_oversized_request_head(header_end + 4)?;
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          let parsed_body_kind = request_body_kind(&head.headers)?;
          if request_needs_continue(&head.headers, parsed_body_kind)? {
            write_continue_response(reader.get_mut())?;
          }
          match parsed_body_kind {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_and_body(head, Vec::new())));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader)?;
              return Ok(Some(Self::from_head_body_and_trailers(
                head,
                chunked.body,
                chunked.trailers,
              )));
            }
          }
        }
        None => {
          let take = available.len();
          reject_oversized_request_head(raw.len().saturating_add(take))?;
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  pub(crate) fn read_next_head_from_with_continue<S>(
    reader: &mut BufReader<S>,
  ) -> io::Result<Option<(Self, RequestBodyKind)>>
  where
    S: Read + Write,
  {
    Self::read_next_head_and_body_kind_from_with_continue(reader)?
      .map_or(Ok(None), |(head, kind)| {
        Ok(Some((Self::from_head_and_body(head, Vec::new()), kind)))
      })
  }

  pub(crate) fn read_next_head_and_body_kind_from_with_continue<S>(
    reader: &mut BufReader<S>,
  ) -> io::Result<Option<(RequestHead, RequestBodyKind)>>
  where
    S: Read + Write,
  {
    let mut raw = Vec::new();

    loop {
      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "incomplete HTTP request",
        ));
      }

      let mut combined = raw.clone();
      combined.extend_from_slice(available);
      match find_header_end(&combined) {
        Some(header_end) => {
          let take = header_end + 4 - raw.len();
          reject_oversized_request_head(header_end + 4)?;
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          let body_kind = request_body_kind(&head.headers)?;
          match body_kind {
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length)?;
            }
            RequestBodyKind::Chunked => {}
          }
          if request_needs_continue(&head.headers, body_kind)? {
            write_continue_response(reader.get_mut())?;
          }
          return Ok(Some((head, body_kind)));
        }
        None => {
          let take = available.len();
          reject_oversized_request_head(raw.len().saturating_add(take))?;
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  #[cfg(test)]
  pub(crate) fn read_next_from_without_continue<R>(reader: &mut R) -> io::Result<Option<Self>>
  where
    R: BufRead,
  {
    let mut raw = Vec::new();
    let mut body_kind: Option<RequestBodyKind> = None;

    loop {
      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        if raw.len() == message_len {
          return Ok(Some(Self::from_raw_frame(&raw)?));
        }
      }

      let available = reader.fill_buf()?;
      if available.is_empty() {
        if raw.is_empty() {
          return Ok(None);
        }
        if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
          (find_header_end(&raw), body_kind)
        {
          let body_start = header_end + 4;
          let body_end = checked_request_message_len(header_end, content_length)?;
          if raw.len() < body_end || body_end < body_start {
            return Err(io::Error::new(
              io::ErrorKind::UnexpectedEof,
              "incomplete HTTP request body",
            ));
          }
        }
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "incomplete HTTP request",
        ));
      }

      if let (Some(header_end), Some(RequestBodyKind::ContentLength(content_length))) =
        (find_header_end(&raw), body_kind)
      {
        let message_len = checked_request_message_len(header_end, content_length)?;
        let take = (message_len - raw.len()).min(available.len());
        raw.extend_from_slice(&available[..take]);
        reader.consume(take);
        continue;
      }

      let mut combined = raw.clone();
      combined.extend_from_slice(available);
      match find_header_end(&combined) {
        Some(header_end) => {
          let take = header_end + 4 - raw.len();
          reject_oversized_request_head(header_end + 4)?;
          raw.extend_from_slice(&available[..take]);
          reader.consume(take);
          let head = parse_request_head(&raw[..header_end])?;
          match request_body_kind(&head.headers)? {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_and_body(head, Vec::new())));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader)?;
              return Ok(Some(Self::from_head_body_and_trailers(
                head,
                chunked.body,
                chunked.trailers,
              )));
            }
          }
        }
        None => {
          let take = available.len();
          reject_oversized_request_head(raw.len().saturating_add(take))?;
          raw.extend_from_slice(available);
          reader.consume(take);
        }
      }
    }
  }

  pub(crate) fn from_raw_frame(raw: &[u8]) -> io::Result<Self> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))?;
    reject_oversized_request_head(header_end + 4)?;
    let head = parse_request_head(&raw[..header_end])?;
    let body_start = header_end + 4;
    let body = match request_body_kind(&head.headers)? {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length)?;
        let body_end = checked_request_message_len(header_end, content_length)?;

        if raw.len() < body_end {
          return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete HTTP request body",
          ));
        }

        raw[body_start..body_end].to_vec()
      }
      RequestBodyKind::Chunked => {
        return Err(io::Error::new(
          io::ErrorKind::InvalidData,
          "chunked request body requires streaming reader",
        ));
      }
    };

    Ok(Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers: Vec::new(),
      body,
      extended_connect_protocol: None,
    })
  }

  pub(crate) fn from_head_and_body(head: RequestHead, body: Vec<u8>) -> Self {
    Self::from_head_body_and_trailers(head, body, Vec::new())
  }

  pub(crate) fn from_head_body_and_trailers(
    head: RequestHead,
    body: Vec<u8>,
    trailers: Vec<(String, String)>,
  ) -> Self {
    Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers,
      body,
      extended_connect_protocol: None,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpAccept {
  media_ranges: Vec<HttpMediaRange>,
}

impl HttpAccept {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpAcceptParseError> {
    Self::parse_values(std::iter::once(value.as_ref()))
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpAcceptParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut media_ranges = Vec::new();
    for value in values {
      if value.len() > MAX_ACCEPT_VALUE_BYTES {
        return Err(HttpAcceptParseError::new(
          "Accept header value is too large",
        ));
      }
      if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(HttpAcceptParseError::new("invalid Accept header value"));
      }

      for member in split_accept_members(value)? {
        if media_ranges.len() >= MAX_ACCEPT_MEDIA_RANGES {
          return Err(HttpAcceptParseError::new("too many Accept media ranges"));
        }
        media_ranges.push(HttpMediaRange::parse(member)?);
      }
    }

    if media_ranges.is_empty() {
      return Err(HttpAcceptParseError::new("invalid Accept header value"));
    }

    Ok(Self { media_ranges })
  }

  pub fn media_ranges(&self) -> &[HttpMediaRange] {
    &self.media_ranges
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpMediaRange {
  media_type: String,
  parameters: Vec<(String, String)>,
  quality: Option<u16>,
}

impl HttpMediaRange {
  fn parse(value: &str) -> Result<Self, HttpAcceptParseError> {
    let mut parts = split_accept_parameters(value)?;
    let Some(media_type) = parts.first() else {
      return Err(HttpAcceptParseError::new("invalid Accept media range"));
    };
    let media_type = parse_accept_media_type(media_type.trim())?;
    parts.remove(0);

    let mut parameters = Vec::new();
    let mut quality = None;
    let mut parsing_extensions = false;
    for part in parts {
      let part = part.trim();
      let (name, value) = match part.split_once('=') {
        Some((name, value)) => (name.trim().to_ascii_lowercase(), Some(value.trim())),
        None if parsing_extensions => (part.to_ascii_lowercase(), None),
        None => return Err(HttpAcceptParseError::new("invalid Accept parameter")),
      };
      if !is_http_token(&name) {
        return Err(HttpAcceptParseError::new("invalid Accept parameter name"));
      }
      if parsing_extensions {
        if let Some(value) = value {
          parse_accept_parameter_value(value)?;
        }
        continue;
      }
      let Some(value) = value else {
        return Err(HttpAcceptParseError::new("invalid Accept parameter"));
      };
      if name == "q" {
        if quality.is_some() {
          return Err(HttpAcceptParseError::new("duplicate Accept quality value"));
        }
        quality = Some(parse_accept_quality(value)?);
        parsing_extensions = true;
        continue;
      }
      if parameters.iter().any(|(known, _)| known == &name) {
        return Err(HttpAcceptParseError::new("duplicate Accept parameter"));
      }
      parameters.push((name, parse_accept_parameter_value(value)?));
    }

    Ok(Self {
      media_type,
      parameters,
      quality,
    })
  }

  pub fn media_type(&self) -> &str {
    &self.media_type
  }

  pub fn parameters(&self) -> Vec<(&str, &str)> {
    self
      .parameters
      .iter()
      .map(|(name, value)| (name.as_str(), value.as_str()))
      .collect()
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|(known, _)| known.eq_ignore_ascii_case(name.as_ref()))
      .map(|(_, value)| value.as_str())
  }

  pub fn quality(&self) -> Option<u16> {
    self.quality
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpAcceptParseError {
  message: String,
}

impl HttpAcceptParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpAcceptParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAcceptParseError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpMaxForwardsParseError {
  message: String,
}

/// A validated `TE` coding received on an HTTP request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpTe {
  coding: String,
  quality: u16,
}

impl HttpTe {
  pub fn coding(&self) -> &str {
    &self.coding
  }

  /// Returns the q-value as thousandths, where `1000` is the default.
  pub fn quality(&self) -> u16 {
    self.quality
  }
}

/// Bounded `TE` request metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequestTe {
  codings: Vec<HttpTe>,
}

impl HttpRequestTe {
  pub fn codings(&self) -> &[HttpTe] {
    &self.codings
  }

  pub fn len(&self) -> usize {
    self.codings.len()
  }

  pub fn is_empty(&self) -> bool {
    self.codings.is_empty()
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpTeParseError {
  message: String,
}

impl HttpTeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpTeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpTeParseError {}

/// A validated `Prefer` item received on an HTTP request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpPreference {
  name: String,
  value: Option<String>,
}

impl HttpPreference {
  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

/// Bounded `Prefer` request metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequestPreferences {
  preferences: Vec<HttpPreference>,
}

impl HttpRequestPreferences {
  pub fn preferences(&self) -> &[HttpPreference] {
    &self.preferences
  }

  pub fn len(&self) -> usize {
    self.preferences.len()
  }

  pub fn is_empty(&self) -> bool {
    self.preferences.is_empty()
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpPreferParseError {
  message: String,
}

impl HttpPreferParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpPreferParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpPreferParseError {}

const MAX_REQUEST_CONTROL_VALUE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_CONTROL_MEMBERS: usize = 32;

fn parse_te_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpRequestTe>, HttpTeParseError> {
  let mut codings = Vec::new();
  for value in values {
    if value.len() > MAX_REQUEST_CONTROL_VALUE_BYTES {
      return Err(HttpTeParseError::new("TE header value is too large"));
    }
    for member in value.split(',') {
      let (coding, quality) = parse_te_member(member)?;
      if codings
        .iter()
        .any(|known: &HttpTe| known.coding.eq_ignore_ascii_case(coding))
      {
        return Err(HttpTeParseError::new("duplicate TE coding"));
      }
      if codings.len() >= MAX_REQUEST_CONTROL_MEMBERS {
        return Err(HttpTeParseError::new("too many TE codings"));
      }
      codings.push(HttpTe {
        coding: coding.to_string(),
        quality,
      });
    }
  }
  if codings.is_empty() {
    Ok(None)
  } else {
    Ok(Some(HttpRequestTe { codings }))
  }
}

fn parse_te_member(member: &str) -> Result<(&str, u16), HttpTeParseError> {
  let mut parts = member.split(';');
  let coding = parts.next().unwrap_or_default().trim();
  if !is_http_token(coding) || coding.eq_ignore_ascii_case("chunked") {
    return Err(HttpTeParseError::new("invalid TE coding"));
  }
  let Some(parameter) = parts.next() else {
    return Ok((coding, 1000));
  };
  if parts.next().is_some() {
    return Err(HttpTeParseError::new("invalid TE coding"));
  }
  let Some((name, value)) = parameter.trim().split_once('=') else {
    return Err(HttpTeParseError::new("invalid TE coding"));
  };
  if !name.trim().eq_ignore_ascii_case("q") {
    return Err(HttpTeParseError::new("invalid TE coding"));
  }
  Ok((
    coding,
    parse_request_control_qvalue(value.trim()).map_err(HttpTeParseError::new)?,
  ))
}

fn parse_prefer_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpRequestPreferences>, HttpPreferParseError> {
  let mut preferences = Vec::new();
  for value in values {
    if value.len() > MAX_REQUEST_CONTROL_VALUE_BYTES {
      return Err(HttpPreferParseError::new(
        "Prefer header value is too large",
      ));
    }
    for member in value.split(',') {
      let (name, preference_value) = parse_prefer_member(member)?;
      if preferences
        .iter()
        .any(|known: &HttpPreference| known.name.eq_ignore_ascii_case(name))
      {
        return Err(HttpPreferParseError::new("duplicate Prefer preference"));
      }
      if preferences.len() >= MAX_REQUEST_CONTROL_MEMBERS {
        return Err(HttpPreferParseError::new("too many Prefer preferences"));
      }
      preferences.push(HttpPreference {
        name: name.to_string(),
        value: preference_value.map(ToString::to_string),
      });
    }
  }
  if preferences.is_empty() {
    Ok(None)
  } else {
    Ok(Some(HttpRequestPreferences { preferences }))
  }
}

fn parse_prefer_member(member: &str) -> Result<(&str, Option<&str>), HttpPreferParseError> {
  let (name, value) = member
    .trim()
    .split_once('=')
    .map_or((member.trim(), None), |(name, value)| {
      (name.trim(), Some(value.trim()))
    });
  if !is_http_token(name) || value.is_some_and(|value| !is_http_token(value)) {
    return Err(HttpPreferParseError::new("invalid Prefer preference"));
  }
  Ok((name, value))
}

fn parse_request_control_qvalue(value: &str) -> Result<u16, &'static str> {
  let Some((whole, fraction)) = value.split_once('.') else {
    return match value {
      "0" => Ok(0),
      "1" => Ok(1000),
      _ => Err("invalid TE q-value"),
    };
  };
  if !matches!(whole, "0" | "1")
    || fraction.len() > 3
    || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    || (whole == "1" && !fraction.bytes().all(|byte| byte == b'0'))
  {
    return Err("invalid TE q-value");
  }
  let fractional = if fraction.is_empty() {
    0
  } else {
    fraction.parse::<u16>().expect("validated q-value")
  };
  Ok(if whole == "1" {
    1000
  } else {
    fractional * 10_u16.pow(3 - fraction.len() as u32)
  })
}

impl HttpMaxForwardsParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpMaxForwardsParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpMaxForwardsParseError {}

fn parse_max_forwards_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<String>, HttpMaxForwardsParseError> {
  let mut values = values.into_iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  if values.next().is_some() {
    return Err(HttpMaxForwardsParseError::new(
      "duplicate Max-Forwards headers",
    ));
  }

  let value = value.trim();
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(HttpMaxForwardsParseError::new(
      "invalid Max-Forwards header value",
    ));
  }
  Ok(Some(value.to_owned()))
}

fn split_accept_members(value: &str) -> Result<Vec<&str>, HttpAcceptParseError> {
  split_accept_delimited(value, b',', "invalid Accept header value")
}

fn split_accept_parameters(value: &str) -> Result<Vec<&str>, HttpAcceptParseError> {
  split_accept_delimited(value, b';', "invalid Accept parameter")
}

fn split_accept_delimited<'a>(
  value: &'a str,
  delimiter: u8,
  error: &'static str,
) -> Result<Vec<&'a str>, HttpAcceptParseError> {
  let mut members = Vec::new();
  let mut quoted = false;
  let mut escaped = false;
  let mut start = 0usize;

  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      if !is_content_type_quoted_pair_byte(byte) {
        return Err(HttpAcceptParseError::new(error));
      }
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' => quoted = !quoted,
      byte if byte == delimiter && !quoted => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err(HttpAcceptParseError::new(error));
        }
        members.push(member);
        start = index + 1;
      }
      _ => {}
    }
  }

  if quoted || escaped {
    return Err(HttpAcceptParseError::new(error));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(HttpAcceptParseError::new(error));
  }
  members.push(member);
  Ok(members)
}

fn parse_accept_media_type(value: &str) -> Result<String, HttpAcceptParseError> {
  let Some((type_name, subtype)) = value.split_once('/') else {
    return Err(HttpAcceptParseError::new("invalid Accept media range"));
  };
  if subtype.contains('/') {
    return Err(HttpAcceptParseError::new("invalid Accept media range"));
  }
  let type_name = type_name.trim().to_ascii_lowercase();
  let subtype = subtype.trim().to_ascii_lowercase();
  if type_name == "*" && subtype != "*" {
    return Err(HttpAcceptParseError::new("invalid Accept media range"));
  }
  if !(type_name == "*" || is_http_token(&type_name))
    || !(subtype == "*" || is_http_token(&subtype))
  {
    return Err(HttpAcceptParseError::new("invalid Accept media range"));
  }
  Ok(format!("{type_name}/{subtype}"))
}

fn parse_accept_parameter_value(value: &str) -> Result<String, HttpAcceptParseError> {
  parse_content_type_parameter_value(value)
    .map_err(|_| HttpAcceptParseError::new("invalid Accept parameter value"))
}

fn parse_accept_quality(value: &str) -> Result<u16, HttpAcceptParseError> {
  let value = value.trim();
  let valid = match value {
    "0" => Some(0),
    "1" => Some(1000),
    _ => {
      let Some((whole, fractional)) = value.split_once('.') else {
        return Err(HttpAcceptParseError::new("invalid Accept quality value"));
      };
      if fractional.is_empty()
        || fractional.len() > 3
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
      {
        return Err(HttpAcceptParseError::new("invalid Accept quality value"));
      }
      let scale = 10u16.pow((3 - fractional.len()) as u32);
      match whole {
        "0" => fractional
          .parse::<u16>()
          .ok()
          .map(|fraction| fraction * scale),
        "1" if fractional.bytes().all(|byte| byte == b'0') => Some(1000),
        _ => None,
      }
    }
  };
  valid.ok_or_else(|| HttpAcceptParseError::new("invalid Accept quality value"))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HttpConditionalMetadata {
  pub(crate) entity_tag: Option<HttpEntityTag>,
  pub(crate) last_modified: Option<SystemTime>,
}

impl HttpConditionalMetadata {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn entity_tag(mut self, entity_tag: HttpEntityTag) -> Self {
    self.entity_tag = Some(entity_tag);
    self
  }

  pub fn last_modified(mut self, last_modified: SystemTime) -> Self {
    self.last_modified = Some(last_modified);
    self
  }

  pub fn entity_tag_value(&self) -> Option<&HttpEntityTag> {
    self.entity_tag.as_ref()
  }

  pub fn last_modified_value(&self) -> Option<SystemTime> {
    self.last_modified
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpEntityTag {
  pub(crate) weak: bool,
  pub(crate) opaque_tag: String,
}

impl HttpEntityTag {
  pub fn strong<S: AsRef<str>>(opaque_tag: S) -> Self {
    Self::new(false, opaque_tag)
  }

  pub fn weak<S: AsRef<str>>(opaque_tag: S) -> Self {
    Self::new(true, opaque_tag)
  }

  pub fn parse<S: AsRef<str>>(value: S) -> Result<Self, HttpEntityTagParseError> {
    parse_entity_tag(value.as_ref().trim()).ok_or(HttpEntityTagParseError)
  }

  pub fn is_weak(&self) -> bool {
    self.weak
  }

  pub fn opaque_tag(&self) -> &str {
    &self.opaque_tag
  }

  pub fn header_value(&self) -> String {
    let mut value = String::new();
    if self.weak {
      value.push_str("W/");
    }
    value.push('"');
    value.push_str(&self.opaque_tag);
    value.push('"');
    value
  }

  pub(crate) fn new<S: AsRef<str>>(weak: bool, opaque_tag: S) -> Self {
    let opaque_tag = opaque_tag.as_ref();
    assert_valid_entity_tag_opaque_tag(opaque_tag);
    Self {
      weak,
      opaque_tag: opaque_tag.to_string(),
    }
  }

  pub(crate) fn strong_matches(&self, other: &Self) -> bool {
    !self.weak && !other.weak && self.opaque_tag == other.opaque_tag
  }

  pub(crate) fn weak_matches(&self, other: &Self) -> bool {
    self.opaque_tag == other.opaque_tag
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpEntityTagParseError;

impl fmt::Display for HttpEntityTagParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("invalid entity tag")
  }
}

impl Error for HttpEntityTagParseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpConditionalRequestOutcome {
  Proceed,
  NotModified,
  PreconditionFailed,
}

pub fn evaluate_conditional_request(
  request: &Request,
  metadata: &HttpConditionalMetadata,
) -> HttpConditionalRequestOutcome {
  if let Some(matches) = request_if_match_matches(request, metadata) {
    if !matches {
      return HttpConditionalRequestOutcome::PreconditionFailed;
    }
  } else if let Some(if_unmodified_since) = request_http_date(request, "If-Unmodified-Since") {
    if metadata
      .last_modified
      .is_some_and(|last_modified| http_date_seconds_after(last_modified, if_unmodified_since))
    {
      return HttpConditionalRequestOutcome::PreconditionFailed;
    }
  }

  if let Some(matches) = request_if_none_match_matches(request, metadata) {
    if matches {
      if method_uses_not_modified_for_if_none_match(request.method()) {
        return HttpConditionalRequestOutcome::NotModified;
      }
      return HttpConditionalRequestOutcome::PreconditionFailed;
    }
    return HttpConditionalRequestOutcome::Proceed;
  }

  if method_uses_not_modified_for_if_none_match(request.method()) {
    if let Some(if_modified_since) = request_http_date(request, "If-Modified-Since") {
      if metadata
        .last_modified
        .is_some_and(|last_modified| !http_date_seconds_after(last_modified, if_modified_since))
      {
        return HttpConditionalRequestOutcome::NotModified;
      }
    }
  }

  HttpConditionalRequestOutcome::Proceed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpIfRangeRequestOutcome {
  FullResponse,
  PartialContent(HttpByteRange),
  RangeNotSatisfiable,
}

pub fn evaluate_if_range_request(
  request: &Request,
  metadata: &HttpConditionalMetadata,
  entity_length: usize,
) -> Result<HttpIfRangeRequestOutcome, HttpByteRangeError> {
  evaluate_if_range_headers(
    request.header("Range"),
    request.header("If-Range"),
    metadata,
    entity_length,
  )
}

pub(crate) fn evaluate_if_range_headers(
  range_header: Option<&str>,
  if_range: Option<&str>,
  metadata: &HttpConditionalMetadata,
  entity_length: usize,
) -> Result<HttpIfRangeRequestOutcome, HttpByteRangeError> {
  let Some(range_header) = range_header else {
    return Ok(HttpIfRangeRequestOutcome::FullResponse);
  };

  if let Some(if_range) = if_range {
    if !if_range_matches(if_range, metadata) {
      return Ok(HttpIfRangeRequestOutcome::FullResponse);
    }
  }

  match HttpByteRange::parse(range_header, entity_length) {
    Ok(range) => Ok(HttpIfRangeRequestOutcome::PartialContent(range)),
    Err(HttpByteRangeError::UnsatisfiedRange) => Ok(HttpIfRangeRequestOutcome::RangeNotSatisfiable),
    Err(error) => Err(error),
  }
}

pub(crate) fn if_range_matches(if_range: &str, metadata: &HttpConditionalMetadata) -> bool {
  if let Ok(candidate) = HttpEntityTag::parse(if_range) {
    return metadata
      .entity_tag
      .as_ref()
      .is_some_and(|current| current.strong_matches(&candidate));
  }

  let Ok(candidate) = httpdate::parse_http_date(if_range) else {
    return false;
  };

  metadata
    .last_modified
    .is_some_and(|last_modified| http_date_seconds_equal(last_modified, candidate))
}

pub(crate) fn request_if_match_matches(
  request: &Request,
  metadata: &HttpConditionalMetadata,
) -> Option<bool> {
  request_entity_tag_validator_matches(request, "If-Match", metadata, EntityTagComparison::Strong)
}

pub(crate) fn request_if_none_match_matches(
  request: &Request,
  metadata: &HttpConditionalMetadata,
) -> Option<bool> {
  request_entity_tag_validator_matches(
    request,
    "If-None-Match",
    metadata,
    EntityTagComparison::Weak,
  )
}

pub(crate) fn request_entity_tag_validator_matches(
  request: &Request,
  header_name: &str,
  metadata: &HttpConditionalMetadata,
  comparison: EntityTagComparison,
) -> Option<bool> {
  let mut saw_header = false;
  let mut saw_valid_validator = false;

  for value in request.headers_named(header_name) {
    saw_header = true;
    for validator in EntityTagValidatorList::parse(value)? {
      saw_valid_validator = true;
      match validator {
        EntityTagValidator::Any => return Some(true),
        EntityTagValidator::Tag(candidate) => {
          let Some(current) = metadata.entity_tag.as_ref() else {
            continue;
          };
          let matches = match comparison {
            EntityTagComparison::Strong => current.strong_matches(&candidate),
            EntityTagComparison::Weak => current.weak_matches(&candidate),
          };
          if matches {
            return Some(true);
          }
        }
      }
    }
  }

  if saw_header && saw_valid_validator {
    Some(false)
  } else {
    None
  }
}

pub(crate) fn request_http_date(request: &Request, header_name: &str) -> Option<SystemTime> {
  httpdate::parse_http_date(request.header(header_name)?).ok()
}

pub(crate) fn http_date_seconds_after(left: SystemTime, right: SystemTime) -> bool {
  match (
    left.duration_since(UNIX_EPOCH),
    right.duration_since(UNIX_EPOCH),
  ) {
    (Ok(left), Ok(right)) => left.as_secs() > right.as_secs(),
    _ => left > right,
  }
}

pub(crate) fn http_date_seconds_equal(left: SystemTime, right: SystemTime) -> bool {
  match (
    left.duration_since(UNIX_EPOCH),
    right.duration_since(UNIX_EPOCH),
  ) {
    (Ok(left), Ok(right)) => left.as_secs() == right.as_secs(),
    _ => left == right,
  }
}

pub(crate) fn method_uses_not_modified_for_if_none_match(method: &str) -> bool {
  method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntityTagComparison {
  Strong,
  Weak,
}
pub(crate) enum EntityTagValidator {
  Any,
  Tag(HttpEntityTag),
}

pub(crate) struct EntityTagValidatorList {
  pub(crate) validators: Vec<EntityTagValidator>,
}

impl EntityTagValidatorList {
  pub(crate) fn parse(value: &str) -> Option<Self> {
    let value = value.trim();
    if value == "*" {
      return Some(Self {
        validators: vec![EntityTagValidator::Any],
      });
    }

    let mut validators = Vec::new();
    for part in value.split(',') {
      validators.push(EntityTagValidator::Tag(parse_entity_tag(part.trim())?));
    }
    if validators.is_empty() {
      None
    } else {
      Some(Self { validators })
    }
  }
}

impl IntoIterator for EntityTagValidatorList {
  type Item = EntityTagValidator;
  type IntoIter = std::vec::IntoIter<EntityTagValidator>;

  fn into_iter(self) -> Self::IntoIter {
    self.validators.into_iter()
  }
}

pub(crate) fn parse_entity_tag(value: &str) -> Option<HttpEntityTag> {
  let (weak, value) = if let Some(rest) = value.strip_prefix("W/") {
    (true, rest)
  } else {
    (false, value)
  };
  let opaque_tag = value.strip_prefix('"')?.strip_suffix('"')?;
  if !is_valid_entity_tag_opaque_tag(opaque_tag) {
    return None;
  }
  Some(HttpEntityTag {
    weak,
    opaque_tag: opaque_tag.to_string(),
  })
}

pub(crate) fn assert_valid_entity_tag_opaque_tag(opaque_tag: &str) {
  assert!(
    is_valid_entity_tag_opaque_tag(opaque_tag),
    "entity tag opaque value must be valid for an HTTP ETag header"
  );
}

pub(crate) fn is_valid_entity_tag_opaque_tag(opaque_tag: &str) -> bool {
  opaque_tag
    .bytes()
    .all(|byte| matches!(byte, b'\x21' | b'\x23'..=b'\x7e' | b'\x80'..=b'\xff'))
}

const MAX_ACCEPT_LANGUAGE_VALUE_BYTES: usize = 64 * 1024;
const MAX_ACCEPT_LANGUAGE_RANGES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptLanguages {
  ranges: Vec<String>,
  qualities: Vec<Option<String>>,
}

impl HttpAcceptLanguages {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpAcceptLanguageParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpAcceptLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let parsed = Self::parse_values_inner(values)?;
    if parsed.ranges.is_empty() {
      return Err(HttpAcceptLanguageParseError::new(
        "invalid Accept-Language range",
      ));
    }
    Ok(parsed)
  }

  fn parse_optional_values<'a, I>(values: I) -> Result<Option<Self>, HttpAcceptLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let parsed = Self::parse_values_inner(values)?;
    if parsed.ranges.is_empty() {
      Ok(None)
    } else {
      Ok(Some(parsed))
    }
  }

  fn parse_values_inner<'a, I>(values: I) -> Result<Self, HttpAcceptLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut ranges = Vec::new();
    let mut qualities = Vec::new();

    for value in values {
      if value.len() > MAX_ACCEPT_LANGUAGE_VALUE_BYTES {
        return Err(HttpAcceptLanguageParseError::new(
          "Accept-Language header value is too large",
        ));
      }
      for item in value.split(',') {
        let (range, quality) = parse_accept_language_item(item.trim())?;
        if ranges.len() >= MAX_ACCEPT_LANGUAGE_RANGES {
          return Err(HttpAcceptLanguageParseError::new(
            "too many Accept-Language ranges",
          ));
        }
        if ranges
          .iter()
          .any(|known: &String| known.eq_ignore_ascii_case(range))
        {
          return Err(HttpAcceptLanguageParseError::new(
            "duplicate Accept-Language range",
          ));
        }
        ranges.push(range.to_string());
        qualities.push(quality.map(ToString::to_string));
      }
    }

    Ok(Self { ranges, qualities })
  }

  pub fn ranges(&self) -> Vec<&str> {
    self.ranges.iter().map(String::as_str).collect()
  }

  pub fn qualities(&self) -> Vec<Option<&str>> {
    self
      .qualities
      .iter()
      .map(|quality| quality.as_deref())
      .collect()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptLanguageParseError {
  message: String,
}

impl HttpAcceptLanguageParseError {
  fn new(message: impl AsRef<str>) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpAcceptLanguageParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAcceptLanguageParseError {}

fn parse_accept_language_item(
  value: &str,
) -> Result<(&str, Option<&str>), HttpAcceptLanguageParseError> {
  let mut parts = value.split(';');
  let range = parts.next().unwrap_or_default().trim();
  if !is_valid_accept_language_range(range) {
    return Err(HttpAcceptLanguageParseError::new(
      "invalid Accept-Language range",
    ));
  }
  let Some(parameter) = parts.next() else {
    return Ok((range, None));
  };
  if parts.next().is_some() {
    return Err(HttpAcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  }
  let Some((name, quality)) = parameter.trim().split_once('=') else {
    return Err(HttpAcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  };
  let quality = quality.trim();
  if !name.trim().eq_ignore_ascii_case("q") || !is_valid_qvalue(quality) {
    return Err(HttpAcceptLanguageParseError::new(
      "invalid Accept-Language q-value",
    ));
  }
  Ok((range, Some(quality)))
}

fn is_valid_accept_language_range(value: &str) -> bool {
  if value == "*" {
    return true;
  }
  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };
  (1..=8).contains(&primary.len())
    && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
    && subtags.all(|subtag| {
      (1..=8).contains(&subtag.len()) && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn is_valid_qvalue(value: &str) -> bool {
  match value.split_once('.') {
    Some((whole, fraction)) => {
      (whole == "0" || whole == "1")
        && fraction.len() <= 3
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'))
    }
    None => value == "0" || value == "1",
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
  pub(crate) method: String,
  pub(crate) path: String,
  pub(crate) query: Option<String>,
  pub(crate) version: String,
  pub(crate) headers: Vec<HttpHeader>,
  pub(crate) body: Vec<u8>,
}

impl HttpRequest {
  pub fn parse(raw: &[u8]) -> Result<Self, HttpParseError> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| HttpParseError::new("request is missing header terminator"))?;
    reject_oversized_request_head(header_end + 4).map_err(HttpParseError::from_io_error)?;
    let head = parse_request_head(&raw[..header_end]).map_err(HttpParseError::from_io_error)?;
    let body_bytes = &raw[(header_end + 4)..];

    let (path, query) = match head.target.split_once('?') {
      Some((path, query)) => (path.to_string(), Some(query.to_string())),
      None => (head.target.clone(), None),
    };

    let body = match request_body_kind(&head.headers).map_err(HttpParseError::from_io_error)? {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length).map_err(HttpParseError::from_io_error)?;
        if body_bytes.len() != content_length {
          return Err(HttpParseError::new(
            "request body length does not match Content-Length",
          ));
        }
        body_bytes.to_vec()
      }
      RequestBodyKind::Chunked => {
        let mut reader = Cursor::new(body_bytes);
        let chunked =
          read_chunked_request_body(&mut reader).map_err(HttpParseError::from_io_error)?;
        if reader.position() as usize != body_bytes.len() {
          return Err(HttpParseError::new(
            "request body length does not match Transfer-Encoding",
          ));
        }
        chunked.body
      }
    };
    let headers = head
      .headers
      .into_iter()
      .map(|(name, value)| HttpHeader::new(name, value))
      .collect();

    Ok(Self {
      method: head.method,
      path,
      query,
      version: head.version,
      headers,
      body,
    })
  }

  pub fn method(&self) -> &str {
    &self.method
  }

  pub fn path(&self) -> &str {
    &self.path
  }

  pub fn query(&self) -> Option<&str> {
    self.query.as_deref()
  }

  pub fn version(&self) -> &str {
    &self.version
  }

  pub fn headers(&self) -> &[HttpHeader] {
    &self.headers
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value.as_str())
  }

  pub fn cache_control(
    &self,
  ) -> Result<Option<HttpRequestCacheControl>, HttpCacheControlParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Cache-Control"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestCacheControl::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn authorization(&self) -> Result<Option<HttpAuthorization>, HttpAuthorizationParseError> {
    let mut values = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Authorization"))
      .map(|header| header.value.as_str());
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(HttpAuthorizationParseError::new(
        "duplicate Authorization headers",
      ));
    }
    HttpAuthorization::parse(value).map(Some)
  }

  /// Parses received `Max-Forwards` request metadata without automatically
  /// decrementing or forwarding the request.
  ///
  /// The validated decimal count is returned verbatim so valid values are not
  /// constrained by a machine integer width.
  pub fn max_forwards(&self) -> Result<Option<String>, HttpMaxForwardsParseError> {
    parse_max_forwards_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Max-Forwards"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received `TE` request metadata without enabling transfer codings.
  pub fn te(&self) -> Result<Option<HttpRequestTe>, HttpTeParseError> {
    parse_te_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("TE"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received `Prefer` request metadata without applying preferences.
  pub fn prefer(&self) -> Result<Option<HttpRequestPreferences>, HttpPreferParseError> {
    parse_prefer_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Prefer"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received `Accept-Encoding` request metadata without enabling
  /// automatic compression, decompression, or content negotiation.
  pub fn accept_encoding(
    &self,
  ) -> Result<Option<HttpRequestAcceptEncodings>, HttpAcceptEncodingParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Encoding"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestAcceptEncodings::parse_values(values).map(Some)
  }

  pub fn accept(&self) -> Result<Option<HttpAccept>, HttpAcceptParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccept::parse_values(values).map(Some)
  }

  /// Parse bounded `Accept-Language` request metadata without selecting a locale.
  pub fn accept_language(
    &self,
  ) -> Result<Option<HttpAcceptLanguages>, HttpAcceptLanguageParseError> {
    let values = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Language"))
      .map(|header| header.value.as_str());
    HttpAcceptLanguages::parse_optional_values(values)
  }

  /// Parses bounded RFC 7239 `Forwarded` request metadata without applying a
  /// proxy trust policy or rewriting request addresses.
  pub fn forwarded(&self) -> Result<Option<HttpForwarded>, HttpForwardedParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Forwarded"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpForwarded::parse_values(values).map(Some)
  }

  pub fn vary_selection(&self, vary: &HttpVary) -> HttpVarySelection {
    if vary.is_wildcard() {
      return HttpVarySelection::wildcard();
    }

    HttpVarySelection::from_fields(vary.fields.iter().map(|field| {
      HttpVarySelectedField::new(
        field,
        self
          .headers
          .iter()
          .filter(|header| header.name.eq_ignore_ascii_case(field))
          .map(|header| header.value.clone())
          .collect::<Vec<_>>(),
      )
    }))
  }

  pub fn body(&self) -> &[u8] {
    &self.body
  }

  pub fn evaluate_if_range(
    &self,
    metadata: &HttpConditionalMetadata,
    entity_length: usize,
  ) -> Result<HttpIfRangeRequestOutcome, HttpByteRangeError> {
    evaluate_if_range_headers(
      self.header("Range"),
      self.header("If-Range"),
      metadata,
      entity_length,
    )
  }
}
