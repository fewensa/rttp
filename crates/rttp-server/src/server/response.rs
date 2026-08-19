use super::*;

pub use rttp_protocol::accept_ranges::{
  AcceptRanges as HttpAcceptRanges, AcceptRangesParseError as HttpAcceptRangesParseError,
};
pub use rttp_protocol::access_control_allow_credentials::{
  AccessControlAllowCredentials as HttpAccessControlAllowCredentials,
  AccessControlAllowCredentialsParseError as HttpAccessControlAllowCredentialsParseError,
};
pub use rttp_protocol::access_control_allow_headers::{
  AccessControlAllowHeaders as HttpAccessControlAllowHeaders,
  AccessControlAllowHeadersParseError as HttpAccessControlAllowHeadersParseError,
};
pub use rttp_protocol::access_control_allow_methods::{
  AccessControlAllowMethods as HttpAccessControlAllowMethods,
  AccessControlAllowMethodsParseError as HttpAccessControlAllowMethodsParseError,
};
pub use rttp_protocol::access_control_allow_origin::{
  AccessControlAllowOrigin as HttpAccessControlAllowOrigin,
  AccessControlAllowOriginParseError as HttpAccessControlAllowOriginParseError,
};
pub use rttp_protocol::allow::{
  Allow as HttpAllowedMethods, AllowParseError as HttpAllowParseError,
};
pub use rttp_protocol::alt_svc::{
  AltSvc as HttpAltSvc, AltSvcAlternative as HttpAltSvcAlternative,
  AltSvcParameter as HttpAltSvcParameter, AltSvcParseError as HttpAltSvcParseError,
};
pub use rttp_protocol::alt_used::{
  AltUsed as HttpAltUsed, AltUsedParseError as HttpAltUsedParseError,
};
pub use rttp_protocol::authentication_info::{
  AuthenticationInfo as HttpAuthenticationInfo,
  AuthenticationInfoParseError as HttpAuthenticationInfoParseError,
};
pub use rttp_protocol::cache_status::{
  CacheStatus as HttpCacheStatus, CacheStatusIdentifier as HttpCacheStatusIdentifier,
  CacheStatusMember as HttpCacheStatusMember, CacheStatusParameter as HttpCacheStatusParameter,
  CacheStatusParseError as HttpCacheStatusParseError,
};
pub use rttp_protocol::cdn_cache_control::{
  CdnCacheControl as HttpCdnCacheControl,
  CdnCacheControlParseError as HttpCdnCacheControlParseError,
};
pub use rttp_protocol::clear_site_data::{
  ClearSiteData as HttpClearSiteData, ClearSiteDataDirective as HttpClearSiteDataDirective,
  ClearSiteDataParseError as HttpClearSiteDataParseError,
};
pub use rttp_protocol::client_hints::{
  AcceptCh as HttpAcceptCh, AcceptChParseError as HttpAcceptChParseError,
  CriticalCh as HttpCriticalCh, CriticalChParseError as HttpCriticalChParseError,
};
pub use rttp_protocol::content_disposition::ContentDispositionParseError as HttpContentDispositionParseError;
pub use rttp_protocol::content_dpr::{
  ContentDpr as HttpContentDpr, ContentDprParseError as HttpContentDprParseError,
};
pub use rttp_protocol::content_encoding::{
  ContentEncoding as HttpResponseContentEncodings,
  ContentEncodingParseError as HttpContentEncodingParseError,
};
pub use rttp_protocol::content_language::ContentLanguageParseError as HttpContentLanguageParseError;
pub use rttp_protocol::content_location::{
  ContentLocation as HttpContentLocation,
  ContentLocationParseError as HttpContentLocationParseError,
};
pub use rttp_protocol::content_type::ContentTypeParseError as HttpContentTypeParseError;
pub use rttp_protocol::cross_origin_embedder_policy::{
  CrossOriginEmbedderPolicy as HttpCrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyParseError as HttpCrossOriginEmbedderPolicyParseError,
};
pub use rttp_protocol::cross_origin_embedder_policy_report_only::{
  CrossOriginEmbedderPolicyReportOnly as HttpCrossOriginEmbedderPolicyReportOnly,
  CrossOriginEmbedderPolicyReportOnlyParseError as HttpCrossOriginEmbedderPolicyReportOnlyParseError,
};
pub use rttp_protocol::cross_origin_opener_policy::{
  CrossOriginOpenerPolicy as HttpCrossOriginOpenerPolicy,
  CrossOriginOpenerPolicyParseError as HttpCrossOriginOpenerPolicyParseError,
};
pub use rttp_protocol::cross_origin_resource_policy::{
  CrossOriginResourcePolicy as HttpCrossOriginResourcePolicy,
  CrossOriginResourcePolicyParseError as HttpCrossOriginResourcePolicyParseError,
};
pub use rttp_protocol::deprecation::{
  Deprecation as HttpDeprecation, DeprecationParseError as HttpDeprecationParseError,
};
pub use rttp_protocol::digest::{
  Digest as HttpDigest, DigestEntry as HttpDigestEntry, DigestParseError as HttpDigestParseError,
  ReprDigest as HttpReprDigest, ReprDigestEntry as HttpReprDigestEntry,
};
pub use rttp_protocol::keep_alive::{
  KeepAlive as HttpKeepAlive, KeepAliveExtension as HttpKeepAliveExtension,
  KeepAliveParseError as HttpKeepAliveParseError,
};
pub use rttp_protocol::memento_datetime::{
  MementoDatetime as HttpMementoDatetime,
  MementoDatetimeParseError as HttpMementoDatetimeParseError,
};
pub use rttp_protocol::nel::{
  Nel as HttpNel, NelParseError as HttpNelParseError, NelUnknownMember as HttpNelUnknownMember,
};
pub use rttp_protocol::no_vary_search::{
  NoVarySearch as HttpNoVarySearch, NoVarySearchExtension as HttpNoVarySearchExtension,
  NoVarySearchParams as HttpNoVarySearchParams,
  NoVarySearchParseError as HttpNoVarySearchParseError,
};
pub use rttp_protocol::pragma::{Pragma as HttpPragma, PragmaParseError as HttpPragmaParseError};
pub use rttp_protocol::priority::{
  Priority as HttpPriority, PriorityExtension as HttpPriorityExtension,
  PriorityParseError as HttpPriorityParseError,
};
pub use rttp_protocol::proxy_authentication_info::{
  ProxyAuthenticationInfo as HttpProxyAuthenticationInfo,
  ProxyAuthenticationInfoParseError as HttpProxyAuthenticationInfoParseError,
};
pub use rttp_protocol::proxy_status::{
  ProxyStatus as HttpProxyStatus, ProxyStatusBareItem as HttpProxyStatusBareItem,
  ProxyStatusIdentifier as HttpProxyStatusIdentifier, ProxyStatusMember as HttpProxyStatusMember,
  ProxyStatusParameter as HttpProxyStatusParameter,
  ProxyStatusParseError as HttpProxyStatusParseError,
};
pub use rttp_protocol::range::{
  ContentRange as HttpContentRange, ContentRangeParseError as HttpContentRangeParseError,
};
pub use rttp_protocol::reporting_endpoints::{
  ReportingEndpoints as HttpReportingEndpoints,
  ReportingEndpointsParseError as HttpReportingEndpointsParseError,
};
pub use rttp_protocol::server_timing::{
  ServerTiming as HttpServerTiming, ServerTimingMetric as HttpServerTimingMetric,
  ServerTimingParameter as HttpServerTimingParameter,
  ServerTimingParseError as HttpServerTimingParseError,
};
pub use rttp_protocol::signature_input::{
  SignatureInput as HttpSignatureInput, SignatureInputParseError as HttpSignatureInputParseError,
};
pub use rttp_protocol::strict_transport_security::{
  StrictTransportSecurity as HttpStrictTransportSecurity,
  StrictTransportSecurityParseError as HttpStrictTransportSecurityParseError,
};
pub use rttp_protocol::sunset::SunsetParseError as HttpSunsetParseError;
pub use rttp_protocol::upgrade::{
  Upgrade as HttpUpgrade, UpgradeParseError as HttpUpgradeParseError,
};
pub use rttp_protocol::www_authenticate::{
  WwwAuthenticate as HttpWwwAuthenticate, WwwAuthenticateChallenge as HttpWwwAuthenticateChallenge,
  WwwAuthenticateParameter as HttpWwwAuthenticateParameter,
  WwwAuthenticateParseError as HttpWwwAuthenticateParseError,
};
pub use rttp_protocol::x_content_type_options::{
  XContentTypeOptions as HttpXContentTypeOptions,
  XContentTypeOptionsParseError as HttpXContentTypeOptionsParseError,
};
pub use rttp_protocol::x_frame_options::{
  XFrameOptions as HttpXFrameOptions, XFrameOptionsParseError as HttpXFrameOptionsParseError,
};

/// Server `Content-Type` representation metadata backed by the shared protocol
/// parser, preserving the server facade accessor surface and normalized
/// output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpContentType {
  inner: rttp_protocol::content_type::ContentType,
  essence: String,
}

impl HttpContentType {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpContentTypeParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpContentTypeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let parsed = rttp_protocol::content_type::ContentType::parse_values(values)?;
    Self::normalize(parsed)
  }

  pub fn new<T, S>(type_name: T, subtype: S) -> Result<Self, HttpContentTypeParseError>
  where
    T: AsRef<str>,
    S: AsRef<str>,
  {
    Ok(Self::from_protocol(
      rttp_protocol::content_type::ContentType::new(type_name, subtype)?,
    ))
  }

  pub fn with_parameter<N, V>(
    mut self,
    name: N,
    value: V,
  ) -> Result<Self, HttpContentTypeParseError>
  where
    N: AsRef<str>,
    V: AsRef<str>,
  {
    self.inner = self.inner.with_parameter(name, value)?;
    Ok(self)
  }

  /// Returns the normalized `type/subtype` media type string.
  pub fn media_type(&self) -> &str {
    &self.essence
  }

  pub fn type_(&self) -> &str {
    self.inner.type_()
  }

  pub fn subtype(&self) -> &str {
    self.inner.subtype()
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self.inner.parameter(name)
  }

  pub fn parameters(&self) -> Vec<(&str, &str)> {
    self
      .inner
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect()
  }

  pub fn header_value(&self) -> String {
    self.inner.header_value()
  }

  fn normalize(
    parsed: rttp_protocol::content_type::ContentType,
  ) -> Result<Self, HttpContentTypeParseError> {
    let mut normalized = rttp_protocol::content_type::ContentType::new(
      parsed.type_().to_ascii_lowercase(),
      parsed.subtype().to_ascii_lowercase(),
    )?;
    for parameter in parsed.parameters() {
      normalized =
        normalized.with_parameter(parameter.name().to_ascii_lowercase(), parameter.value())?;
    }
    Ok(Self::from_protocol(normalized))
  }

  fn from_protocol(inner: rttp_protocol::content_type::ContentType) -> Self {
    let essence = format!("{}/{}", inner.type_(), inner.subtype());
    Self { inner, essence }
  }
}

/// Server `Content-Language` representation metadata backed by the shared
/// protocol parser, preserving the server facade accessor surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpContentLanguages {
  inner: rttp_protocol::content_language::ContentLanguage,
}

impl HttpContentLanguages {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpContentLanguageParseError> {
    Ok(Self {
      inner: rttp_protocol::content_language::ContentLanguage::parse(value)?,
    })
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpContentLanguageParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      inner: rttp_protocol::content_language::ContentLanguage::parse_values(values)?,
    })
  }

  pub fn from_languages<I, L>(languages: I) -> Result<Self, HttpContentLanguageParseError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    Ok(Self {
      inner: rttp_protocol::content_language::ContentLanguage::from_languages(languages)?,
    })
  }

  /// Returns the parsed language tags in wire order.
  pub fn languages(&self) -> Vec<&str> {
    self.inner.tags()
  }

  /// Returns the parsed language tags in wire order.
  pub fn tags(&self) -> Vec<&str> {
    self.inner.tags()
  }

  pub fn len(&self) -> usize {
    self.inner.len()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  pub fn header_value(&self) -> String {
    self.inner.header_value()
  }
}

pub(crate) const MAX_CACHE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_CACHE_CONTROL_DIRECTIVES: usize = 256;
pub(crate) const MAX_VARY_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_VARY_FIELDS: usize = 256;
pub(crate) const MAX_ACCEPT_PATCH_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ACCEPT_PATCH_MEDIA_TYPES: usize = 32;
pub(crate) const MAX_ACCEPT_POST_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ACCEPT_POST_MEDIA_TYPES: usize = 32;
pub(crate) const MAX_RETRY_AFTER_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_EARLY_HINTS_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_LINK_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_LINK_VALUES: usize = 256;
pub(crate) const MAX_LINK_PARAMETERS: usize = 256;
pub(crate) const MAX_LINK_PARAMETER_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_BROWSER_POLICY_VALUE_BYTES: usize = 64 * 1024;

macro_rules! browser_policy_metadata {
  ($name:ident, $header:literal) => {
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct $name(String);

    impl $name {
      /// Parses bounded response metadata without applying browser behavior.
      pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpBrowserPolicyParseError> {
        let value = value.as_ref();
        validate_browser_policy_value($header, value)?;
        Ok(Self(value.to_owned()))
      }

      pub fn as_str(&self) -> &str {
        &self.0
      }

      pub fn header_value(&self) -> &str {
        self.as_str()
      }
    }

    impl AsRef<str> for $name {
      fn as_ref(&self) -> &str {
        self.as_str()
      }
    }
  };
}

browser_policy_metadata!(HttpContentSecurityPolicy, "Content-Security-Policy");
browser_policy_metadata!(HttpPermissionsPolicy, "Permissions-Policy");
browser_policy_metadata!(HttpReferrerPolicy, "Referrer-Policy");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpBrowserPolicyParseError {
  message: String,
}

impl HttpBrowserPolicyParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpBrowserPolicyParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpBrowserPolicyParseError {}

fn validate_browser_policy_value(
  header: &str,
  value: &str,
) -> Result<(), HttpBrowserPolicyParseError> {
  if value.is_empty() {
    return Err(HttpBrowserPolicyParseError::new(format!(
      "{header} header value is empty"
    )));
  }
  if value.len() > MAX_BROWSER_POLICY_VALUE_BYTES {
    return Err(HttpBrowserPolicyParseError::new(format!(
      "{header} header value is too large"
    )));
  }
  if value
    .bytes()
    .any(|byte| byte != b'\t' && (byte <= 0x1f || byte == 0x7f))
  {
    return Err(HttpBrowserPolicyParseError::new(format!(
      "invalid {header} control byte"
    )));
  }
  Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
  pub(crate) version: String,
  pub(crate) status_code: u16,
  pub(crate) reason: String,
  pub(crate) headers: Vec<HttpHeader>,
  pub(crate) trailers: Vec<HttpHeader>,
  pub(crate) body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpByteRange {
  pub(crate) start: usize,
  pub(crate) end: usize,
}

impl HttpByteRange {
  pub fn new(start: usize, end: usize) -> Self {
    assert!(start <= end, "byte range start must not exceed end");
    Self { start, end }
  }

  pub fn parse<S: AsRef<str>>(
    range_header: S,
    entity_length: usize,
  ) -> Result<Self, HttpByteRangeError> {
    let range_header = range_header.as_ref().trim();
    let Some((unit, range_spec)) = range_header.split_once('=') else {
      return Err(HttpByteRangeError::InvalidRange);
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") {
      return Err(HttpByteRangeError::UnsupportedUnit);
    }

    let range_spec = range_spec.trim();
    if range_spec.contains(',') {
      return Err(HttpByteRangeError::MultipleRanges);
    }

    let Some((first, last)) = range_spec.split_once('-') else {
      return Err(HttpByteRangeError::InvalidRange);
    };
    if last.contains('-') {
      return Err(HttpByteRangeError::InvalidRange);
    }

    let first = first.trim();
    let last = last.trim();
    if first.is_empty() {
      return parse_suffix_byte_range(last, entity_length);
    }

    let start = parse_byte_range_position(first)?;
    let end = if last.is_empty() {
      None
    } else {
      let requested_end = parse_byte_range_position(last)?;
      if start > requested_end {
        return Err(HttpByteRangeError::InvalidRange);
      }
      Some(requested_end)
    };

    if start >= entity_length {
      return Err(HttpByteRangeError::UnsatisfiedRange);
    }
    let end = end.unwrap_or(entity_length - 1).min(entity_length - 1);

    Ok(Self { start, end })
  }

  pub fn start(&self) -> usize {
    self.start
  }

  pub fn end(&self) -> usize {
    self.end
  }

  pub fn len(&self) -> usize {
    self.end - self.start + 1
  }

  pub fn is_empty(&self) -> bool {
    false
  }

  pub fn slice<'a>(&self, body: &'a [u8]) -> Option<&'a [u8]> {
    body.get(self.start..=self.end)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpByteRangeError {
  UnsupportedUnit,
  MultipleRanges,
  InvalidRange,
  UnsatisfiedRange,
}

impl fmt::Display for HttpByteRangeError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    let message = match self {
      Self::UnsupportedUnit => "unsupported Range unit",
      Self::MultipleRanges => "multiple byte ranges are not supported",
      Self::InvalidRange => "invalid byte range",
      Self::UnsatisfiedRange => "byte range is not satisfiable",
    };
    formatter.write_str(message)
  }
}

impl Error for HttpByteRangeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpEarlyHintsError {
  pub(crate) message: String,
}

impl HttpEarlyHintsError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpEarlyHintsError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpEarlyHintsError {}

/// Bounded `Link` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpLinkValues {
  values: Vec<HttpLinkValue>,
}

impl HttpLinkValues {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpLinkParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpLinkParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parsed = Vec::new();
    for value in values {
      if value.len() > MAX_LINK_VALUE_BYTES {
        return Err(HttpLinkParseError::new("Link header value is too large"));
      }
      for member in split_http_link_members(value, b',')? {
        if parsed.len() >= MAX_LINK_VALUES {
          return Err(HttpLinkParseError::new("too many Link values"));
        }
        parsed.push(HttpLinkValue::parse_member(&member)?);
      }
    }
    if parsed.is_empty() {
      return Err(HttpLinkParseError::new("invalid Link value"));
    }
    Ok(Self { values: parsed })
  }

  pub fn values(&self) -> &[HttpLinkValue] {
    &self.values
  }

  pub fn len(&self) -> usize {
    self.values.len()
  }

  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpLinkValue {
  target: String,
  parameters: Vec<(String, String)>,
}

impl HttpLinkValue {
  fn parse_member(member: &str) -> Result<Self, HttpLinkParseError> {
    let member = member.trim();
    let Some(target_and_tail) = member.strip_prefix('<') else {
      return Err(HttpLinkParseError::new("invalid Link target"));
    };
    let Some(target_end) = target_and_tail.find('>') else {
      return Err(HttpLinkParseError::new("invalid Link target"));
    };
    let target = &target_and_tail[..target_end];
    validate_http_link_target(target)?;

    let mut parameters = Vec::new();
    let tail = target_and_tail[target_end + 1..].trim();
    if !tail.is_empty() {
      if !tail.starts_with(';') {
        return Err(HttpLinkParseError::new("invalid Link parameter"));
      }
      for parameter in split_http_link_members(&tail[1..], b';')? {
        if parameters.len() >= MAX_LINK_PARAMETERS {
          return Err(HttpLinkParseError::new("too many Link parameters"));
        }
        let (name, value) = parse_http_link_parameter(&parameter)?;
        if parameters
          .iter()
          .any(|(known, _): &(String, String)| known.eq_ignore_ascii_case(&name))
        {
          return Err(HttpLinkParseError::new("duplicate Link parameter"));
        }
        parameters.push((name, value));
      }
    }
    Ok(Self {
      target: target.to_string(),
      parameters,
    })
  }

  pub fn target(&self) -> &str {
    &self.target
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpLinkParseError {
  pub(crate) message: String,
}

impl HttpLinkParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpLinkParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpLinkParseError {}

fn validate_http_link_target(target: &str) -> Result<(), HttpLinkParseError> {
  if target.is_empty()
    || target.bytes().any(|byte| !is_uri_reference_byte(byte))
    || !has_valid_percent_escapes(target)
  {
    return Err(HttpLinkParseError::new("invalid Link target"));
  }
  let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
  Url::options()
    .base_url(Some(&base))
    .parse(target)
    .map_err(|_| HttpLinkParseError::new("invalid Link target"))?;
  Ok(())
}

/// Whether `byte` is permitted raw in an RFC 3986 URI-reference.
fn is_uri_reference_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'%'
      | b':'
      | b'/'
      | b'?'
      | b'#'
      | b'['
      | b']'
      | b'@'
      | b'!'
      | b'$'
      | b'&'
      | b'\''
      | b'('
      | b')'
      | b'*'
      | b'+'
      | b','
      | b';'
      | b'='
      | b'-'
      | b'.'
      | b'_'
      | b'~'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

/// Whether every `%` in `target` starts a well-formed percent-encoding.
fn has_valid_percent_escapes(target: &str) -> bool {
  let mut bytes = target.bytes();
  while let Some(byte) = bytes.next() {
    if byte != b'%' {
      continue;
    }
    let Some(high) = bytes.next() else {
      return false;
    };
    let Some(low) = bytes.next() else {
      return false;
    };
    if !high.is_ascii_hexdigit() || !low.is_ascii_hexdigit() {
      return false;
    }
  }
  true
}

fn split_http_link_members(value: &str, delimiter: u8) -> Result<Vec<String>, HttpLinkParseError> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut quoted = false;
  let mut escaped = false;
  let mut in_target = false;
  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' if !in_target => quoted = !quoted,
      b'<' if !quoted => in_target = true,
      b'>' if !quoted => in_target = false,
      byte if byte == delimiter && !quoted && !in_target => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err(HttpLinkParseError::new("invalid Link value"));
        }
        members.push(member.to_string());
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped || in_target {
    return Err(HttpLinkParseError::new("invalid Link value"));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(HttpLinkParseError::new("invalid Link value"));
  }
  members.push(member.to_string());
  Ok(members)
}

fn parse_http_link_parameter(value: &str) -> Result<(String, String), HttpLinkParseError> {
  let (name, value) = match value.split_once('=') {
    Some((name, value)) => (name, Some(value.trim())),
    None => (value, None),
  };
  let name = name.trim();
  if !is_http_token(name) {
    return Err(HttpLinkParseError::new("invalid Link parameter name"));
  }
  let value = match value {
    Some("") => {
      return Err(HttpLinkParseError::new("invalid Link parameter value"));
    }
    Some(value) => {
      if value.len() > MAX_LINK_PARAMETER_VALUE_BYTES {
        return Err(HttpLinkParseError::new("Link parameter value is too large"));
      }
      if value.starts_with('"') {
        parse_http_link_quoted_string(value)?
      } else if value.contains('"') || !is_http_token(value) {
        return Err(HttpLinkParseError::new("invalid Link parameter value"));
      } else {
        value.to_string()
      }
    }
    None => String::new(),
  };
  Ok((name.to_ascii_lowercase(), value))
}

fn parse_http_link_quoted_string(value: &str) -> Result<String, HttpLinkParseError> {
  if !value.ends_with('"') || value.len() < 2 {
    return Err(HttpLinkParseError::new("invalid Link quoted-string"));
  }

  let inner = &value[1..value.len() - 1];
  let mut parsed = String::new();
  let mut escaped = false;
  for ch in inner.chars() {
    if escaped {
      if !is_http_link_quoted_pair_char(ch) {
        return Err(HttpLinkParseError::new("invalid Link quoted-string"));
      }
      parsed.push(ch);
      escaped = false;
    } else if ch == '\\' {
      escaped = true;
    } else if ch == '"' || !is_http_link_quoted_text_char(ch) {
      return Err(HttpLinkParseError::new("invalid Link quoted-string"));
    } else {
      parsed.push(ch);
    }
  }

  if escaped {
    return Err(HttpLinkParseError::new("invalid Link quoted-string"));
  }
  Ok(parsed)
}

fn is_http_link_quoted_text_char(ch: char) -> bool {
  matches!(ch, '\t' | ' ' | '!' | '#'..='[' | ']'..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn is_http_link_quoted_pair_char(ch: char) -> bool {
  matches!(ch, '\t' | ' '..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefaultConnectionHeader {
  Close,
  ForceClose,
  KeepAlive,
  Omit,
}

impl HttpResponse {
  pub fn new<S: AsRef<str>>(status_code: u16, reason: S) -> Self {
    Self {
      version: "HTTP/1.1".to_string(),
      status_code,
      reason: reason.as_ref().to_string(),
      headers: Vec::new(),
      trailers: Vec::new(),
      body: Vec::new(),
    }
  }

  pub fn ok(body: impl AsRef<[u8]>) -> Self {
    Self::new(200, "OK").body(body)
  }

  pub fn partial_content<B: AsRef<[u8]>>(body: B, range: HttpByteRange) -> Self {
    let body = body.as_ref();
    let partial = range
      .slice(body)
      .expect("partial content range must be satisfiable for body");

    Self::new(206, "Partial Content")
      .header(
        "Content-Range",
        HttpContentRange::Bytes {
          start: range.start() as u64,
          end: range.end() as u64,
          complete_length: Some(body.len() as u64),
        }
        .header_value(),
      )
      .body(partial)
  }

  pub fn range_not_satisfiable(entity_length: usize) -> Self {
    Self::new(416, "Range Not Satisfiable").header(
      "Content-Range",
      HttpContentRange::Unsatisfied {
        complete_length: entity_length as u64,
      }
      .header_value(),
    )
  }

  pub fn not_modified(metadata: &HttpConditionalMetadata) -> Self {
    let mut response = Self::new(304, "Not Modified");
    if let Some(entity_tag) = metadata.entity_tag_value() {
      response = response.header("ETag", entity_tag.header_value());
    }
    if let Some(last_modified) = metadata.last_modified_value() {
      response = response.header("Last-Modified", httpdate::fmt_http_date(last_modified));
    }
    response
  }

  pub fn precondition_failed() -> Self {
    Self::new(412, "Precondition Failed")
  }

  pub fn early_hints<I, L>(links: I) -> Result<Self, HttpEarlyHintsError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    Self::early_hints_with_headers(links, std::iter::empty::<(&str, &str)>())
  }

  pub fn early_hints_with_headers<I, L, H, N, V>(
    links: I,
    metadata: H,
  ) -> Result<Self, HttpEarlyHintsError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
    H: IntoIterator<Item = (N, V)>,
    N: AsRef<str>,
    V: AsRef<str>,
  {
    let mut response = Self::new(103, "Early Hints");
    let mut link_count = 0usize;
    for link in links {
      let link = validate_early_hints_link_value(link.as_ref())?;
      response.headers.push(HttpHeader::new("Link", link));
      link_count += 1;
    }
    if link_count == 0 {
      return Err(HttpEarlyHintsError::new(
        "Early Hints requires at least one Link header",
      ));
    }

    for (name, value) in metadata {
      let name = validate_early_hints_metadata_name(name.as_ref())?;
      let value = validate_early_hints_header_value(value.as_ref())?;
      response.headers.push(HttpHeader::new(name, value));
    }

    Ok(response)
  }

  pub fn header<N: AsRef<str>, V: AsRef<str>>(mut self, name: N, value: V) -> Self {
    let name = name.as_ref();
    let value = value.as_ref();
    assert_valid_header_component(name);
    assert_valid_header_component(value);
    self.headers.push(HttpHeader::new(name, value));
    self
  }

  pub fn trailer<N: AsRef<str>, V: AsRef<str>>(mut self, name: N, value: V) -> Self {
    let name = name.as_ref();
    let value = value.as_ref();
    assert_valid_header_component(name);
    assert_valid_header_component(value);
    assert_allowed_trailer_name(name);
    self.trailers.push(HttpHeader::new(name, value));
    self
  }

  pub fn with_vary<V: AsRef<str>>(mut self, value: V) -> Result<Self, HttpVaryParseError> {
    let vary = HttpVary::parse(value)?;
    self
      .headers
      .push(HttpHeader::new("Vary", vary.header_value()));
    Ok(self)
  }

  /// Validates and replaces `No-Vary-Search` response metadata without applying
  /// cache-key or URL-normalization policy.
  pub fn with_no_vary_search<V: AsRef<str>>(
    mut self,
    value: V,
  ) -> Result<Self, HttpNoVarySearchParseError> {
    let no_vary_search = HttpNoVarySearch::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("No-Vary-Search"));
    self.headers.push(HttpHeader::new(
      "No-Vary-Search",
      no_vary_search.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Upgrade` response metadata without changing
  /// handoff behavior or adding `Connection: Upgrade`.
  pub fn with_upgrade<I, P>(mut self, protocols: I) -> Result<Self, HttpUpgradeParseError>
  where
    I: IntoIterator<Item = P>,
    P: AsRef<str>,
  {
    let protocols: Vec<String> = protocols
      .into_iter()
      .map(|protocol| protocol.as_ref().to_string())
      .collect();
    let upgrade = HttpUpgrade::parse_values(protocols.iter().map(String::as_str))?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Upgrade"));
    self
      .headers
      .push(HttpHeader::new("Upgrade", upgrade.header_value()));
    Ok(self)
  }

  pub fn with_allow<I, M>(mut self, methods: I) -> Result<Self, HttpAllowParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let allow = HttpAllowedMethods::from_methods(methods)?;
    self
      .headers
      .push(HttpHeader::new("Allow", allow.header_value()));
    Ok(self)
  }

  pub fn with_content_language<I, L>(
    mut self,
    languages: I,
  ) -> Result<Self, HttpContentLanguageParseError>
  where
    I: IntoIterator<Item = L>,
    L: AsRef<str>,
  {
    let content_languages = HttpContentLanguages::from_languages(languages)?;
    self.headers.push(HttpHeader::new(
      "Content-Language",
      content_languages.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Access-Control-Allow-Methods` response metadata
  /// without applying CORS policy.
  pub fn with_access_control_allow_methods(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpAccessControlAllowMethodsParseError> {
    let allow_methods = HttpAccessControlAllowMethods::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Access-Control-Allow-Methods")
    });
    self.headers.push(HttpHeader::new(
      "Access-Control-Allow-Methods",
      allow_methods.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Access-Control-Allow-Origin` response metadata
  /// without applying CORS policy.
  pub fn with_access_control_allow_origin(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpAccessControlAllowOriginParseError> {
    let allow_origin = HttpAccessControlAllowOrigin::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Access-Control-Allow-Origin")
    });
    self.headers.push(HttpHeader::new(
      "Access-Control-Allow-Origin",
      allow_origin.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Access-Control-Allow-Credentials` response metadata
  /// without granting credentials automatically.
  pub fn with_access_control_allow_credentials(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpAccessControlAllowCredentialsParseError> {
    let allow_credentials = HttpAccessControlAllowCredentials::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Access-Control-Allow-Credentials")
    });
    self.headers.push(HttpHeader::new(
      "Access-Control-Allow-Credentials",
      allow_credentials.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Access-Control-Allow-Headers` response metadata
  /// without applying CORS policy.
  pub fn with_access_control_allow_headers(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpAccessControlAllowHeadersParseError> {
    let allow_headers = HttpAccessControlAllowHeaders::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Access-Control-Allow-Headers")
    });
    self.headers.push(HttpHeader::new(
      "Access-Control-Allow-Headers",
      allow_headers.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Cross-Origin-Resource-Policy` response metadata
  /// without applying resource isolation policy.
  pub fn with_cross_origin_resource_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpCrossOriginResourcePolicyParseError> {
    let policy = HttpCrossOriginResourcePolicy::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Cross-Origin-Resource-Policy")
    });
    self.headers.push(HttpHeader::new(
      "Cross-Origin-Resource-Policy",
      policy.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `NEL` response metadata without sending reports
  /// or persisting policy.
  pub fn with_nel(mut self, value: impl AsRef<str>) -> Result<Self, HttpNelParseError> {
    let nel = HttpNel::parse(value)?;
    let header_value = nel.header_value();
    assert_valid_header_component(&header_value);
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("NEL"));
    self.headers.push(HttpHeader::new("NEL", header_value));
    Ok(self)
  }

  /// Validates and replaces `Cross-Origin-Embedder-Policy` response metadata
  /// without applying embedder policy.
  pub fn with_cross_origin_embedder_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpCrossOriginEmbedderPolicyParseError> {
    let policy = HttpCrossOriginEmbedderPolicy::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Cross-Origin-Embedder-Policy")
    });
    self.headers.push(HttpHeader::new(
      "Cross-Origin-Embedder-Policy",
      policy.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Cross-Origin-Embedder-Policy-Report-Only`
  /// response metadata without applying embedder policy or scheduling reports.
  pub fn with_cross_origin_embedder_policy_report_only(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpCrossOriginEmbedderPolicyReportOnlyParseError> {
    let policy = HttpCrossOriginEmbedderPolicyReportOnly::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Cross-Origin-Embedder-Policy-Report-Only")
    });
    self.headers.push(HttpHeader::new(
      "Cross-Origin-Embedder-Policy-Report-Only",
      policy.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Cross-Origin-Opener-Policy` response metadata
  /// without applying opener policy.
  pub fn with_cross_origin_opener_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpCrossOriginOpenerPolicyParseError> {
    let policy = HttpCrossOriginOpenerPolicy::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Cross-Origin-Opener-Policy")
    });
    self.headers.push(HttpHeader::new(
      "Cross-Origin-Opener-Policy",
      policy.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Strict-Transport-Security` response metadata
  /// without applying HTTPS-only policy.
  pub fn with_strict_transport_security(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpStrictTransportSecurityParseError> {
    let policy = HttpStrictTransportSecurity::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Strict-Transport-Security")
    });
    self.headers.push(HttpHeader::new(
      "Strict-Transport-Security",
      policy.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `X-Content-Type-Options` response metadata
  /// without applying MIME-sniffing protection.
  pub fn with_x_content_type_options(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpXContentTypeOptionsParseError> {
    let options = HttpXContentTypeOptions::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("X-Content-Type-Options"));
    self.headers.push(HttpHeader::new(
      "X-Content-Type-Options",
      options.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `X-Frame-Options` response metadata without
  /// applying clickjacking protection.
  pub fn with_x_frame_options(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpXFrameOptionsParseError> {
    let options = HttpXFrameOptions::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("X-Frame-Options"));
    self
      .headers
      .push(HttpHeader::new("X-Frame-Options", options.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Reporting-Endpoints` response metadata.
  pub fn with_reporting_endpoints<I, N, U>(
    mut self,
    endpoints: I,
  ) -> Result<Self, HttpReportingEndpointsParseError>
  where
    I: IntoIterator<Item = (N, U)>,
    N: AsRef<str>,
    U: AsRef<str>,
  {
    let endpoints = HttpReportingEndpoints::from_endpoints(endpoints)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Reporting-Endpoints"));
    self.headers.push(HttpHeader::new(
      "Reporting-Endpoints",
      endpoints.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Content-Security-Policy` metadata without enforcing it.
  pub fn with_content_security_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpBrowserPolicyParseError> {
    let policy = HttpContentSecurityPolicy::parse(value)?;
    self.set_browser_policy_header("Content-Security-Policy", policy.header_value());
    Ok(self)
  }

  /// Validates and replaces `Permissions-Policy` metadata without enforcing it.
  pub fn with_permissions_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpBrowserPolicyParseError> {
    let policy = HttpPermissionsPolicy::parse(value)?;
    self.set_browser_policy_header("Permissions-Policy", policy.header_value());
    Ok(self)
  }

  /// Validates and replaces `Referrer-Policy` metadata without altering requests.
  pub fn with_referrer_policy(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpBrowserPolicyParseError> {
    let policy = HttpReferrerPolicy::parse(value)?;
    self.set_browser_policy_header("Referrer-Policy", policy.header_value());
    Ok(self)
  }

  pub fn with_content_encoding<I, C>(
    mut self,
    codings: I,
  ) -> Result<Self, HttpContentEncodingParseError>
  where
    I: IntoIterator<Item = C>,
    C: AsRef<str>,
  {
    let content_encodings = HttpResponseContentEncodings::from_codings(codings)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-Encoding"));
    self.headers.push(HttpHeader::new(
      "Content-Encoding",
      content_encodings.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `WWW-Authenticate` response metadata.
  pub fn with_www_authenticate(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpWwwAuthenticateParseError> {
    let challenges = HttpWwwAuthenticate::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("WWW-Authenticate"));
    self.headers.push(HttpHeader::new(
      "WWW-Authenticate",
      challenges.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Authentication-Info` response metadata.
  pub fn with_authentication_info(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpAuthenticationInfoParseError> {
    let info = HttpAuthenticationInfo::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Authentication-Info"));
    self
      .headers
      .push(HttpHeader::new("Authentication-Info", info.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Proxy-Authentication-Info` response metadata.
  pub fn with_proxy_authentication_info(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpProxyAuthenticationInfoParseError> {
    let info = HttpProxyAuthenticationInfo::parse(value)?;
    self.headers.retain(|header| {
      !header
        .name
        .eq_ignore_ascii_case("Proxy-Authentication-Info")
    });
    self.headers.push(HttpHeader::new(
      "Proxy-Authentication-Info",
      info.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces `Content-Digest` response metadata.
  pub fn with_digest(mut self, value: impl AsRef<str>) -> Result<Self, HttpDigestParseError> {
    let digest = HttpDigest::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-Digest"));
    self
      .headers
      .push(HttpHeader::new("Content-Digest", digest.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Repr-Digest` response metadata.
  pub fn with_repr_digest(mut self, value: impl AsRef<str>) -> Result<Self, HttpDigestParseError> {
    let digest = HttpDigest::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Repr-Digest"));
    self
      .headers
      .push(HttpHeader::new("Repr-Digest", digest.header_value()));
    Ok(self)
  }

  /// Validates and replaces RFC 9421 `Signature` response metadata without
  /// signing or verifying.
  pub fn with_signature(mut self, value: impl AsRef<str>) -> Result<Self, HttpSignatureParseError> {
    let signature = HttpSignature::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Signature"));
    self
      .headers
      .push(HttpHeader::new("Signature", signature.header_value()));
    Ok(self)
  }

  /// Validates and replaces RFC 9421 `Signature-Input` response metadata
  /// without signing, verifying, or applying cryptographic policy.
  pub fn with_signature_input(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpSignatureInputParseError> {
    let signature_input = HttpSignatureInput::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Signature-Input"));
    self.headers.push(HttpHeader::new(
      "Signature-Input",
      signature_input.header_value(),
    ));
    Ok(self)
  }

  /// Validates and replaces RFC 9209 `Proxy-Status` response metadata
  /// without generating origin proxy status or applying health policy.
  pub fn with_proxy_status(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpProxyStatusParseError> {
    let proxy_status = HttpProxyStatus::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Proxy-Status"));
    self
      .headers
      .push(HttpHeader::new("Proxy-Status", proxy_status.header_value()));
    Ok(self)
  }

  /// Validates and replaces HTTP `Priority` response metadata.
  pub fn with_priority(mut self, value: impl AsRef<str>) -> Result<Self, HttpPriorityParseError> {
    let priority = HttpPriority::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Priority"));
    self
      .headers
      .push(HttpHeader::new("Priority", priority.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Server-Timing` response metadata.
  pub fn with_server_timing(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpServerTimingParseError> {
    let timing = HttpServerTiming::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Server-Timing"));
    self
      .headers
      .push(HttpHeader::new("Server-Timing", timing.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Alt-Svc` response metadata without selecting an endpoint.
  pub fn with_alt_svc(mut self, value: impl AsRef<str>) -> Result<Self, HttpAltSvcParseError> {
    let alt_svc = HttpAltSvc::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Alt-Svc"));
    self
      .headers
      .push(HttpHeader::new("Alt-Svc", alt_svc.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Alt-Used` response metadata without selecting an
  /// alternative service or changing connection policy.
  pub fn with_alt_used(mut self, value: impl AsRef<str>) -> Result<Self, HttpAltUsedParseError> {
    let alt_used = HttpAltUsed::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Alt-Used"));
    self
      .headers
      .push(HttpHeader::new("Alt-Used", alt_used.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Keep-Alive` response metadata without changing
  /// connection lifetime.
  pub fn with_keep_alive(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpKeepAliveParseError> {
    let keep_alive = HttpKeepAlive::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Keep-Alive"));
    self
      .headers
      .push(HttpHeader::new("Keep-Alive", keep_alive.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Pragma` response metadata without applying cache
  /// or intermediary policy.
  pub fn with_pragma(mut self, value: impl AsRef<str>) -> Result<Self, HttpPragmaParseError> {
    let pragma = HttpPragma::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Pragma"));
    self
      .headers
      .push(HttpHeader::new("Pragma", pragma.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Clear-Site-Data` metadata without clearing server state.
  pub fn with_clear_site_data(
    mut self,
    value: impl AsRef<str>,
  ) -> Result<Self, HttpClearSiteDataParseError> {
    let clear_site_data = HttpClearSiteData::parse(value)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Clear-Site-Data"));
    self.headers.push(HttpHeader::new(
      "Clear-Site-Data",
      clear_site_data.header_value(),
    ));
    Ok(self)
  }

  pub fn with_content_location<V: AsRef<str>>(
    mut self,
    value: V,
  ) -> Result<Self, HttpContentLocationParseError> {
    let content_location = HttpContentLocation::parse(value.as_ref())?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-Location"));
    self.headers.push(HttpHeader::new(
      "Content-Location",
      content_location.header_value(),
    ));
    Ok(self)
  }

  pub fn with_content_dpr<V: AsRef<str>>(
    mut self,
    value: V,
  ) -> Result<Self, HttpContentDprParseError> {
    let content_dpr = HttpContentDpr::parse(value.as_ref())?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-DPR"));
    self
      .headers
      .push(HttpHeader::new("Content-DPR", content_dpr.header_value()));
    Ok(self)
  }

  pub fn with_etag(mut self, entity_tag: HttpEntityTag) -> Self {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("ETag"));
    self
      .headers
      .push(HttpHeader::new("ETag", entity_tag.header_value()));
    self
  }

  pub fn with_content_disposition<D>(
    mut self,
    disposition: D,
  ) -> Result<Self, HttpContentDispositionParseError>
  where
    D: IntoHttpContentDisposition,
  {
    let disposition = disposition.into_content_disposition()?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-Disposition"));
    self.headers.push(HttpHeader::new(
      "Content-Disposition",
      disposition.header_value(),
    ));
    Ok(self)
  }

  pub fn with_content_type<T>(mut self, content_type: T) -> Result<Self, HttpContentTypeParseError>
  where
    T: IntoHttpContentType,
  {
    let content_type = content_type.into_content_type()?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Content-Type"));
    self
      .headers
      .push(HttpHeader::new("Content-Type", content_type.header_value()));
    Ok(self)
  }

  pub fn with_accept_patch<I, M>(
    mut self,
    media_types: I,
  ) -> Result<Self, HttpAcceptPatchParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let accept_patch = HttpAcceptPatch::from_media_types(media_types)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Accept-Patch"));
    self
      .headers
      .push(HttpHeader::new("Accept-Patch", accept_patch.header_value()));
    Ok(self)
  }

  pub fn with_accept_post<I, M>(mut self, media_types: I) -> Result<Self, HttpAcceptPostParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let accept_post = HttpAcceptPost::from_media_types(media_types)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Accept-Post"));
    self
      .headers
      .push(HttpHeader::new("Accept-Post", accept_post.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Accept-CH` metadata without applying Client Hints policy.
  pub fn with_accept_ch<I, H>(mut self, client_hints: I) -> Result<Self, HttpAcceptChParseError>
  where
    I: IntoIterator<Item = H>,
    H: AsRef<str>,
  {
    let client_hints: Vec<String> = client_hints
      .into_iter()
      .map(|hint| hint.as_ref().to_owned())
      .collect();
    let accept_ch = HttpAcceptCh::parse_values(client_hints.iter().map(String::as_str))?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Accept-CH"));
    self
      .headers
      .push(HttpHeader::new("Accept-CH", accept_ch.header_value()));
    Ok(self)
  }

  /// Validates and replaces `Critical-CH` metadata without requiring clients to retry.
  pub fn with_critical_ch<I, H>(mut self, client_hints: I) -> Result<Self, HttpCriticalChParseError>
  where
    I: IntoIterator<Item = H>,
    H: AsRef<str>,
  {
    let client_hints: Vec<String> = client_hints
      .into_iter()
      .map(|hint| hint.as_ref().to_owned())
      .collect();
    let critical_ch = HttpCriticalCh::parse_values(client_hints.iter().map(String::as_str))?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Critical-CH"));
    self
      .headers
      .push(HttpHeader::new("Critical-CH", critical_ch.header_value()));
    Ok(self)
  }

  pub fn with_inline_content_disposition(self) -> Result<Self, HttpContentDispositionParseError> {
    self.with_content_disposition(HttpContentDisposition::inline())
  }

  pub fn with_attachment_content_disposition(
    self,
  ) -> Result<Self, HttpContentDispositionParseError> {
    self.with_content_disposition(HttpContentDisposition::attachment())
  }

  pub fn with_attachment_filename<V: AsRef<str>>(
    self,
    filename: V,
  ) -> Result<Self, HttpContentDispositionParseError> {
    self.with_content_disposition(
      HttpContentDisposition::attachment().with_parameter("filename", filename)?,
    )
  }

  pub fn with_accept_ranges<I, U>(mut self, units: I) -> Result<Self, HttpAcceptRangesParseError>
  where
    I: IntoIterator<Item = U>,
    U: AsRef<str>,
  {
    let accept_ranges = HttpAcceptRanges::from_units(units)?;
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Accept-Ranges"));
    self.headers.push(HttpHeader::new(
      "Accept-Ranges",
      accept_ranges.header_value(),
    ));
    Ok(self)
  }

  pub fn with_accept_ranges_none(mut self) -> Self {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Accept-Ranges"));
    self.headers.push(HttpHeader::new(
      "Accept-Ranges",
      HttpAcceptRanges::none().header_value(),
    ));
    self
  }

  pub fn with_age(mut self, delta_seconds: u64) -> Self {
    self
      .headers
      .push(HttpHeader::new("Age", delta_seconds.to_string()));
    self
  }

  pub fn with_expires(mut self, http_date: SystemTime) -> Self {
    self.headers.push(HttpHeader::new(
      "Expires",
      httpdate::fmt_http_date(http_date),
    ));
    self
  }

  pub fn with_sunset(mut self, http_date: SystemTime) -> Self {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Sunset"));
    self.headers.push(HttpHeader::new(
      "Sunset",
      httpdate::fmt_http_date(http_date),
    ));
    self
  }

  pub fn with_memento_datetime(mut self, http_date: SystemTime) -> Self {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Memento-Datetime"));
    self.headers.push(HttpHeader::new(
      "Memento-Datetime",
      HttpMementoDatetime::new(http_date).header_value(),
    ));
    self
  }

  pub fn with_deprecation(mut self, deprecation: HttpDeprecation) -> Self {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case("Deprecation"));
    self
      .headers
      .push(HttpHeader::new("Deprecation", deprecation.header_value()));
    self
  }

  pub fn with_retry_after_delta(mut self, delta_seconds: u64) -> Self {
    self
      .headers
      .push(HttpHeader::new("Retry-After", delta_seconds.to_string()));
    self
  }

  pub fn with_retry_after_date(mut self, http_date: SystemTime) -> Self {
    self.headers.push(HttpHeader::new(
      "Retry-After",
      httpdate::fmt_http_date(http_date),
    ));
    self
  }

  pub fn trailers(&self) -> &[HttpHeader] {
    &self.trailers
  }

  pub fn trailer_value<S: AsRef<str>>(&self, name: S) -> Option<&str> {
    self
      .trailers
      .iter()
      .find(|trailer| trailer.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|trailer| trailer.value.as_str())
  }

  pub fn cache_control(
    &self,
  ) -> Result<Option<HttpResponseCacheControl>, HttpCacheControlParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Cache-Control"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpResponseCacheControl::parse_values(values).map(Some)
  }

  /// Parses attached `CDN-Cache-Control` response metadata without applying CDN cache policy.
  pub fn cdn_cache_control(
    &self,
  ) -> Result<Option<HttpCdnCacheControl>, HttpCdnCacheControlParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("CDN-Cache-Control"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCdnCacheControl::parse_values(values).map(Some)
  }

  /// Parses attached `Cache-Status` response metadata without applying cache policy.
  pub fn cache_status(&self) -> Result<Option<HttpCacheStatus>, HttpCacheStatusParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Cache-Status"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCacheStatus::parse_values(values).map(Some)
  }

  pub fn vary(&self) -> Result<Option<HttpVary>, HttpVaryParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Vary"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpVary::parse_values(values).map(Some)
  }

  /// Parses attached `No-Vary-Search` metadata without changing raw headers,
  /// cache keys, URLs, or response selection policy.
  pub fn no_vary_search(&self) -> Result<Option<HttpNoVarySearch>, HttpNoVarySearchParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("No-Vary-Search"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpNoVarySearch::parse_values(values).map(Some)
  }

  /// Parses attached `Upgrade` response metadata without changing socket
  /// handoff behavior or interpreting the upgraded protocol.
  pub fn upgrade(&self) -> Result<Option<HttpUpgrade>, HttpUpgradeParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Upgrade"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpUpgrade::parse_values(values).map(Some)
  }

  /// Parses attached `Pragma` response metadata without applying cache or
  /// intermediary policy.
  pub fn pragma(&self) -> Result<Option<HttpPragma>, HttpPragmaParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Pragma"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpPragma::parse_values(values).map(Some)
  }

  /// Parses `Link` response metadata without enabling preload, redirects,
  /// caching, or fetch scheduling.
  pub fn links(&self) -> Result<Option<HttpLinkValues>, HttpLinkParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Link"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpLinkValues::parse_values(values).map(Some)
  }

  pub fn allow(&self) -> Result<Option<HttpAllowedMethods>, HttpAllowParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Allow"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAllowedMethods::parse_values(values).map(Some)
  }

  /// Parses attached HTTP/1 `Connection` header metadata without changing
  /// keep-alive, hop-by-hop stripping, or HTTP/2 rejection.
  pub fn connection(&self) -> Result<Option<HttpConnection>, HttpConnectionParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Connection"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpConnection::parse_values(values).map(Some)
  }

  pub fn content_language(
    &self,
  ) -> Result<Option<HttpContentLanguages>, HttpContentLanguageParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Language"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentLanguages::parse_values(values).map(Some)
  }

  /// Parses attached `Access-Control-Allow-Methods` response metadata without
  /// applying CORS policy.
  pub fn access_control_allow_methods(
    &self,
  ) -> Result<Option<HttpAccessControlAllowMethods>, HttpAccessControlAllowMethodsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Allow-Methods")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlAllowMethods::parse_values(values).map(Some)
  }

  /// Parses attached `Access-Control-Allow-Origin` response metadata without
  /// applying CORS policy.
  pub fn access_control_allow_origin(
    &self,
  ) -> Result<Option<HttpAccessControlAllowOrigin>, HttpAccessControlAllowOriginParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Allow-Origin")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlAllowOrigin::parse_values(values).map(Some)
  }

  /// Parses attached `Access-Control-Allow-Credentials` response metadata without
  /// granting credentials automatically.
  pub fn access_control_allow_credentials(
    &self,
  ) -> Result<Option<HttpAccessControlAllowCredentials>, HttpAccessControlAllowCredentialsParseError>
  {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Allow-Credentials")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlAllowCredentials::parse_values(values).map(Some)
  }

  /// Parses attached `Access-Control-Allow-Headers` response metadata without
  /// applying CORS policy.
  pub fn access_control_allow_headers(
    &self,
  ) -> Result<Option<HttpAccessControlAllowHeaders>, HttpAccessControlAllowHeadersParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Allow-Headers")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlAllowHeaders::parse_values(values).map(Some)
  }

  /// Parses attached `Cross-Origin-Resource-Policy` response metadata without
  /// enforcing resource isolation policy.
  pub fn cross_origin_resource_policy(
    &self,
  ) -> Result<Option<HttpCrossOriginResourcePolicy>, HttpCrossOriginResourcePolicyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Resource-Policy")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCrossOriginResourcePolicy::parse_values(values).map(Some)
  }

  /// Parses attached `Cross-Origin-Embedder-Policy` response metadata without
  /// enforcing embedder policy.
  pub fn cross_origin_embedder_policy(
    &self,
  ) -> Result<Option<HttpCrossOriginEmbedderPolicy>, HttpCrossOriginEmbedderPolicyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Embedder-Policy")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCrossOriginEmbedderPolicy::parse_values(values).map(Some)
  }

  /// Parses attached `Cross-Origin-Embedder-Policy-Report-Only` response
  /// metadata without enforcing embedder policy or scheduling reports.
  pub fn cross_origin_embedder_policy_report_only(
    &self,
  ) -> Result<
    Option<HttpCrossOriginEmbedderPolicyReportOnly>,
    HttpCrossOriginEmbedderPolicyReportOnlyParseError,
  > {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Embedder-Policy-Report-Only")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCrossOriginEmbedderPolicyReportOnly::parse_values(values).map(Some)
  }

  /// Parses attached `Cross-Origin-Opener-Policy` response metadata without
  /// enforcing opener policy.
  pub fn cross_origin_opener_policy(
    &self,
  ) -> Result<Option<HttpCrossOriginOpenerPolicy>, HttpCrossOriginOpenerPolicyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Cross-Origin-Opener-Policy")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCrossOriginOpenerPolicy::parse_values(values).map(Some)
  }

  /// Parses attached `Strict-Transport-Security` response metadata without
  /// applying HTTPS-only policy.
  pub fn strict_transport_security(
    &self,
  ) -> Result<Option<HttpStrictTransportSecurity>, HttpStrictTransportSecurityParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Strict-Transport-Security")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpStrictTransportSecurity::parse_values(values).map(Some)
  }

  /// Parses attached `X-Content-Type-Options` response metadata without
  /// applying MIME-sniffing protection.
  pub fn x_content_type_options(
    &self,
  ) -> Result<Option<HttpXContentTypeOptions>, HttpXContentTypeOptionsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("X-Content-Type-Options"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXContentTypeOptions::parse_values(values).map(Some)
  }

  /// Parses attached `X-Frame-Options` response metadata without
  /// applying clickjacking protection.
  pub fn x_frame_options(&self) -> Result<Option<HttpXFrameOptions>, HttpXFrameOptionsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("X-Frame-Options"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXFrameOptions::parse_values(values).map(Some)
  }

  /// Parses bounded `Reporting-Endpoints` response metadata without scheduling reports.
  pub fn reporting_endpoints(
    &self,
  ) -> Result<Option<HttpReportingEndpoints>, HttpReportingEndpointsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Reporting-Endpoints"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpReportingEndpoints::parse_values(values).map(Some)
  }

  /// Returns attached `Content-Security-Policy` metadata without enforcing it.
  pub fn content_security_policy(
    &self,
  ) -> Result<Option<HttpContentSecurityPolicy>, HttpBrowserPolicyParseError> {
    self.browser_policy_value("Content-Security-Policy", |value| {
      HttpContentSecurityPolicy::parse(value)
    })
  }

  /// Returns attached `Permissions-Policy` metadata without enforcing it.
  pub fn permissions_policy(
    &self,
  ) -> Result<Option<HttpPermissionsPolicy>, HttpBrowserPolicyParseError> {
    self.browser_policy_value("Permissions-Policy", |value| {
      HttpPermissionsPolicy::parse(value)
    })
  }

  /// Returns attached `Referrer-Policy` metadata without altering requests.
  pub fn referrer_policy(&self) -> Result<Option<HttpReferrerPolicy>, HttpBrowserPolicyParseError> {
    self.browser_policy_value("Referrer-Policy", |value| HttpReferrerPolicy::parse(value))
  }

  pub fn content_encoding(
    &self,
  ) -> Result<Option<HttpResponseContentEncodings>, HttpContentEncodingParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Encoding"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpResponseContentEncodings::parse_values(values).map(Some)
  }

  /// Parses attached `WWW-Authenticate` response metadata without changing raw headers.
  pub fn www_authenticate(
    &self,
  ) -> Result<Option<HttpWwwAuthenticate>, HttpWwwAuthenticateParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("WWW-Authenticate"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpWwwAuthenticate::parse_values(values).map(Some)
  }

  /// Parses attached `Authentication-Info` response metadata without changing
  /// raw headers.
  pub fn authentication_info(
    &self,
  ) -> Result<Option<HttpAuthenticationInfo>, HttpAuthenticationInfoParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Authentication-Info"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAuthenticationInfo::parse_values(values).map(Some)
  }

  /// Parses attached `Proxy-Authentication-Info` response metadata without
  /// changing raw headers.
  pub fn proxy_authentication_info(
    &self,
  ) -> Result<Option<HttpProxyAuthenticationInfo>, HttpProxyAuthenticationInfoParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Proxy-Authentication-Info")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpProxyAuthenticationInfo::parse_values(values).map(Some)
  }

  /// Parses attached `Content-Digest` metadata without changing raw headers.
  pub fn digest(&self) -> Result<Option<HttpDigest>, HttpDigestParseError> {
    self.digest_field("Content-Digest")
  }

  /// Parses attached `Repr-Digest` metadata without changing raw headers.
  pub fn repr_digest(&self) -> Result<Option<HttpReprDigest>, HttpDigestParseError> {
    self.digest_field("Repr-Digest")
  }

  /// Parses attached RFC 9421 `Signature` metadata without changing raw
  /// headers or verifying signatures.
  pub fn signature(&self) -> Result<Option<HttpSignature>, HttpSignatureParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Signature"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSignature::parse_values(values).map(Some)
  }

  /// Parses attached RFC 9421 `Signature-Input` metadata without changing raw
  /// headers or applying cryptographic policy.
  pub fn signature_input(
    &self,
  ) -> Result<Option<HttpSignatureInput>, HttpSignatureInputParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Signature-Input"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSignatureInput::parse_values(values).map(Some)
  }

  fn digest_field(&self, name: &str) -> Result<Option<HttpDigest>, HttpDigestParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpDigest::parse_values(values).map(Some)
  }

  fn set_browser_policy_header(&mut self, name: &str, value: &str) {
    self
      .headers
      .retain(|header| !header.name.eq_ignore_ascii_case(name));
    self.headers.push(HttpHeader::new(name, value));
  }

  fn browser_policy_value<P>(
    &self,
    name: &str,
    parse: impl FnOnce(&str) -> Result<P, HttpBrowserPolicyParseError>,
  ) -> Result<Option<P>, HttpBrowserPolicyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value.as_str())
      .collect();
    match values.as_slice() {
      [] => Ok(None),
      [value] => parse(value).map(Some),
      _ => Err(HttpBrowserPolicyParseError::new(format!(
        "multiple {name} headers"
      ))),
    }
  }

  /// Parses attached `Proxy-Status` metadata without changing raw headers
  /// or applying proxy health, retry, trailer, or origin-generation policy.
  pub fn proxy_status(&self) -> Result<Option<HttpProxyStatus>, HttpProxyStatusParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Proxy-Status"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpProxyStatus::parse_values(values).map(Some)
  }

  /// Parses attached HTTP `Priority` metadata without changing raw headers.
  pub fn priority(&self) -> Result<Option<HttpPriority>, HttpPriorityParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Priority"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpPriority::parse_values(values).map(Some)
  }

  /// Parses attached `Server-Timing` response metadata without changing raw headers.
  pub fn server_timing(&self) -> Result<Option<HttpServerTiming>, HttpServerTimingParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Server-Timing"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpServerTiming::parse_values(values).map(Some)
  }

  /// Parses attached `Alt-Svc` metadata without changing raw headers or connections.
  pub fn alt_svc(&self) -> Result<Option<HttpAltSvc>, HttpAltSvcParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Alt-Svc"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAltSvc::parse_values(values).map(Some)
  }

  /// Parses attached `Alt-Used` metadata without changing raw headers,
  /// alternative service selection, origins, or connections.
  pub fn alt_used(&self) -> Result<Option<HttpAltUsed>, HttpAltUsedParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Alt-Used"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAltUsed::parse_values(values).map(Some)
  }

  /// Parses attached `NEL` metadata without sending reports or persisting policy.
  pub fn nel(&self) -> Result<Option<HttpNel>, HttpNelParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("NEL"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpNel::parse_values(values).map(Some)
  }

  /// Parses attached `Keep-Alive` metadata without changing connection lifetime.
  pub fn keep_alive(&self) -> Result<Option<HttpKeepAlive>, HttpKeepAliveParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Keep-Alive"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpKeepAlive::parse_values(values).map(Some)
  }

  /// Parses attached `Clear-Site-Data` metadata without changing server state.
  pub fn clear_site_data(&self) -> Result<Option<HttpClearSiteData>, HttpClearSiteDataParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Clear-Site-Data"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpClearSiteData::parse_values(values).map(Some)
  }

  pub fn content_location(
    &self,
  ) -> Result<Option<HttpContentLocation>, HttpContentLocationParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Location"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentLocation::parse_values(values).map(Some)
  }

  pub fn content_dpr(&self) -> Result<Option<HttpContentDpr>, HttpContentDprParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-DPR"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentDpr::parse_values(values).map(Some)
  }

  pub fn etag(&self) -> Result<Option<HttpEntityTag>, HttpEntityTagParseError> {
    let Some(value) = self.single_header_value(
      "ETag",
      HttpEntityTagParseError::new("multiple ETag headers"),
    )?
    else {
      return Ok(None);
    };
    HttpEntityTag::parse(value).map(Some)
  }

  pub fn content_disposition(
    &self,
  ) -> Result<Option<HttpContentDisposition>, HttpContentDispositionParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Disposition"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentDisposition::parse_values(values).map(Some)
  }

  pub fn content_type(&self) -> Result<Option<HttpContentType>, HttpContentTypeParseError> {
    let Some(value) = self.single_header_value(
      "Content-Type",
      HttpContentTypeParseError::new("multiple Content-Type headers"),
    )?
    else {
      return Ok(None);
    };
    HttpContentType::parse(value).map(Some)
  }

  pub fn accept_patch(&self) -> Result<Option<HttpAcceptPatch>, HttpAcceptPatchParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Patch"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptPatch::parse_values(values).map(Some)
  }

  pub fn accept_post(&self) -> Result<Option<HttpAcceptPost>, HttpAcceptPostParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Post"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptPost::parse_values(values).map(Some)
  }

  /// Parses attached `Accept-CH` metadata without applying Client Hints policy.
  pub fn accept_ch(&self) -> Result<Option<HttpAcceptCh>, HttpAcceptChParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-CH"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptCh::parse_values(values).map(Some)
  }

  /// Parses attached `Critical-CH` metadata without requiring clients to retry.
  pub fn critical_ch(&self) -> Result<Option<HttpCriticalCh>, HttpCriticalChParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Critical-CH"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCriticalCh::parse_values(values).map(Some)
  }

  pub fn accept_ranges(&self) -> Result<Option<HttpAcceptRanges>, HttpAcceptRangesParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Ranges"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptRanges::parse_values(values).map(Some)
  }

  pub fn content_range(&self) -> Result<Option<HttpContentRange>, HttpContentRangeParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Range"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentRange::parse_values(values).map(Some)
  }

  pub fn age(&self) -> Result<Option<u64>, HttpAgeParseError> {
    let Some(value) =
      self.single_header_value("Age", HttpAgeParseError::new("multiple Age headers"))?
    else {
      return Ok(None);
    };
    parse_http_age(value).map(Some)
  }

  pub fn expires(&self) -> Result<Option<SystemTime>, HttpExpiresParseError> {
    let Some(value) = self.single_header_value(
      "Expires",
      HttpExpiresParseError::new("multiple Expires headers"),
    )?
    else {
      return Ok(None);
    };
    httpdate::parse_http_date(value)
      .map(Some)
      .map_err(|_| HttpExpiresParseError::new("invalid Expires HTTP-date"))
  }

  pub fn sunset(&self) -> Result<Option<SystemTime>, HttpSunsetParseError> {
    rttp_protocol::sunset::parse_sunset_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Sunset"))
        .map(|header| header.value.as_str()),
    )
  }

  pub fn memento_datetime(
    &self,
  ) -> Result<Option<HttpMementoDatetime>, HttpMementoDatetimeParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Memento-Datetime"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpMementoDatetime::parse_values(values).map(Some)
  }

  pub fn deprecation(&self) -> Result<Option<HttpDeprecation>, HttpDeprecationParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Deprecation"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpDeprecation::parse_values(values).map(Some)
  }

  pub fn retry_after(&self) -> Result<Option<HttpRetryAfter>, HttpRetryAfterParseError> {
    let Some(value) = self.single_header_value(
      "Retry-After",
      HttpRetryAfterParseError::new("multiple Retry-After headers"),
    )?
    else {
      return Ok(None);
    };
    parse_http_retry_after(value).map(Some)
  }

  pub fn body<B: AsRef<[u8]>>(mut self, body: B) -> Self {
    self.body = body.as_ref().to_vec();
    self
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    self
      .write_head_to(&mut bytes, DefaultConnectionHeader::Omit)
      .expect("write to Vec cannot fail");
    if self.allows_body() {
      self
        .write_body_to(&mut bytes)
        .expect("write to Vec cannot fail");
    }
    bytes
  }

  pub fn write_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    self.write_to_with_default_connection(writer, DefaultConnectionHeader::Close)
  }

  pub(crate) fn write_to_with_default_connection<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
  ) -> io::Result<()>
  where
    W: Write,
  {
    self.write_to_with_default_connection_and_body(writer, default_connection, true)
  }

  pub(crate) fn write_to_with_default_connection_and_body<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
    write_body: bool,
  ) -> io::Result<()>
  where
    W: Write,
  {
    self.write_head_to(writer, default_connection)?;
    if write_body && self.allows_body() {
      self.write_body_to(writer)?;
    }
    writer.flush()
  }

  pub(crate) fn write_head_to<W>(
    &self,
    writer: &mut W,
    default_connection: DefaultConnectionHeader,
  ) -> io::Result<()>
  where
    W: Write,
  {
    write!(
      writer,
      "{} {} {}\r\n",
      self.version, self.status_code, self.reason
    )?;

    let connection_header_index = self.connection_header_index();
    for (index, header) in self.headers.iter().enumerate() {
      if self.should_write_head_header(header, index, connection_header_index, default_connection) {
        write!(writer, "{}: {}\r\n", header.name, header.value)?;
      }
    }

    if self.allows_body() {
      if self.uses_chunked_transfer_encoding() {
        self.write_http11_trailer_declaration(writer)?;
      } else {
        write!(writer, "Content-Length: {}\r\n", self.body.len())?;
      }
    }
    if default_connection == DefaultConnectionHeader::ForceClose
      || connection_header_index.is_none()
    {
      match default_connection {
        DefaultConnectionHeader::Close | DefaultConnectionHeader::ForceClose => {
          writer.write_all(b"Connection: close\r\n")?
        }
        DefaultConnectionHeader::KeepAlive => writer.write_all(b"Connection: keep-alive\r\n")?,
        DefaultConnectionHeader::Omit => {}
      }
    }

    writer.write_all(b"\r\n")
  }

  pub(crate) fn write_http11_trailer_declaration<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    if self.trailers.is_empty() {
      return Ok(());
    }

    writer.write_all(b"Trailer: ")?;
    for (index, trailer) in self.trailers.iter().enumerate() {
      if index > 0 {
        writer.write_all(b", ")?;
      }
      writer.write_all(trailer.name.as_bytes())?;
    }
    writer.write_all(b"\r\n")
  }

  pub(crate) fn write_handoff_head_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    write!(
      writer,
      "{} {} {}\r\n",
      self.version, self.status_code, self.reason
    )?;

    let connection_header_index = self.connection_header_index();
    for (index, header) in self.headers.iter().enumerate() {
      if !header.name.eq_ignore_ascii_case("Content-Length")
        && (!header.name.eq_ignore_ascii_case("Connection")
          || Some(index) == connection_header_index)
      {
        write!(writer, "{}: {}\r\n", header.name, header.value)?;
      }
    }

    writer.write_all(b"\r\n")?;
    writer.flush()
  }

  pub(crate) fn write_body_to<W>(&self, writer: &mut W) -> io::Result<()>
  where
    W: Write,
  {
    if self.uses_chunked_transfer_encoding() {
      write!(writer, "{:x}\r\n", self.body.len())?;
      writer.write_all(&self.body)?;
      writer.write_all(b"\r\n0\r\n")?;
      for trailer in &self.trailers {
        write!(writer, "{}: {}\r\n", trailer.name, trailer.value)?;
      }
      writer.write_all(b"\r\n")
    } else {
      writer.write_all(&self.body)
    }
  }

  pub(crate) fn allows_body(&self) -> bool {
    response_status_allows_body(self.status_code)
  }

  pub(crate) fn uses_chunked_transfer_encoding(&self) -> bool {
    self.headers.iter().any(|header| {
      header.name.eq_ignore_ascii_case("Transfer-Encoding")
        && header
          .value
          .split(',')
          .any(|token| token.trim().eq_ignore_ascii_case("chunked"))
    })
  }

  pub(crate) fn should_write_head_header(
    &self,
    header: &HttpHeader,
    index: usize,
    connection_header_index: Option<usize>,
    default_connection: DefaultConnectionHeader,
  ) -> bool {
    if header.name.eq_ignore_ascii_case("Content-Length") {
      return false;
    }
    if !self.allows_body() && header.name.eq_ignore_ascii_case("Transfer-Encoding") {
      return false;
    }
    if self.allows_body()
      && self.uses_chunked_transfer_encoding()
      && !self.trailers.is_empty()
      && header.name.eq_ignore_ascii_case("Trailer")
    {
      return false;
    }
    if !header.name.eq_ignore_ascii_case("Connection") {
      return true;
    }

    default_connection != DefaultConnectionHeader::ForceClose
      && Some(index) == connection_header_index
  }

  pub(crate) fn connection_header_index(&self) -> Option<usize> {
    self
      .headers
      .iter()
      .rposition(|header| header.name.eq_ignore_ascii_case("Connection"))
  }

  pub(crate) fn single_header_value<E>(
    &self,
    name: &str,
    multiple_error: E,
  ) -> Result<Option<&str>, E> {
    let mut values = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value.as_str());
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(multiple_error);
    }
    Ok(Some(value))
  }

  pub(crate) fn closes_connection(&self) -> bool {
    self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Connection"))
      .any(|header| connection_header_has_token(Some(header.value.as_str()), "close"))
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAgeParseError {
  pub(crate) message: String,
}

impl HttpAgeParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpAgeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAgeParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpExpiresParseError {
  pub(crate) message: String,
}

impl HttpExpiresParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpExpiresParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpExpiresParseError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpRetryAfter {
  DeltaSeconds(u64),
  HttpDate(SystemTime),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRetryAfterParseError {
  pub(crate) message: String,
}

impl HttpRetryAfterParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpRetryAfterParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpRetryAfterParseError {}

pub(crate) fn parse_http_age(value: &str) -> Result<u64, HttpAgeParseError> {
  let value = value.trim();
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(HttpAgeParseError::new("invalid Age delta-seconds"));
  }
  value
    .parse::<u64>()
    .map_err(|_| HttpAgeParseError::new("invalid Age delta-seconds"))
}

pub(crate) fn parse_http_retry_after(
  value: &str,
) -> Result<HttpRetryAfter, HttpRetryAfterParseError> {
  if value.len() > MAX_RETRY_AFTER_VALUE_BYTES {
    return Err(HttpRetryAfterParseError::new(
      "Retry-After value is too large",
    ));
  }

  let value = value.trim();
  if value.is_empty() {
    return Err(HttpRetryAfterParseError::new("invalid Retry-After value"));
  }

  if value.bytes().all(|byte| byte.is_ascii_digit()) {
    return value
      .parse::<u64>()
      .map(HttpRetryAfter::DeltaSeconds)
      .map_err(|_| HttpRetryAfterParseError::new("invalid Retry-After delta-seconds"));
  }

  httpdate::parse_http_date(value)
    .map(HttpRetryAfter::HttpDate)
    .map_err(|_| HttpRetryAfterParseError::new("invalid Retry-After HTTP-date"))
}

pub(crate) fn parse_suffix_byte_range(
  suffix_length: &str,
  entity_length: usize,
) -> Result<HttpByteRange, HttpByteRangeError> {
  let suffix_length = parse_byte_range_position(suffix_length)?;
  if suffix_length == 0 {
    return Err(HttpByteRangeError::InvalidRange);
  }
  let end = entity_length
    .checked_sub(1)
    .ok_or(HttpByteRangeError::UnsatisfiedRange)?;
  let start = entity_length.saturating_sub(suffix_length);

  Ok(HttpByteRange { start, end })
}

pub(crate) fn parse_byte_range_position(value: &str) -> Result<usize, HttpByteRangeError> {
  if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(HttpByteRangeError::InvalidRange);
  }
  value
    .parse::<usize>()
    .map_err(|_| HttpByteRangeError::InvalidRange)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpVary {
  pub(crate) wildcard: bool,
  pub(crate) fields: Vec<String>,
}

impl HttpVary {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpVaryParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpVaryParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut fields = Vec::new();
    let mut wildcard = false;
    let mut field_count = 0usize;

    for value in values {
      if value.len() > MAX_VARY_VALUE_BYTES {
        return Err(HttpVaryParseError::new("Vary header value is too large"));
      }

      for field in value.split(',') {
        let field = field.trim();
        if field.is_empty() {
          return Err(HttpVaryParseError::new("invalid Vary field name"));
        }
        if field == "*" {
          wildcard = true;
          continue;
        }
        if !is_http_token(field) {
          return Err(HttpVaryParseError::new("invalid Vary field name"));
        }

        field_count += 1;
        if field_count > MAX_VARY_FIELDS {
          return Err(HttpVaryParseError::new("too many Vary field names"));
        }

        let normalized = field.to_ascii_lowercase();
        if !fields.iter().any(|known| known == &normalized) {
          fields.push(normalized);
        }
      }
    }

    if wildcard && !fields.is_empty() {
      return Err(HttpVaryParseError::new(
        "Vary wildcard cannot be combined with field names",
      ));
    }
    if wildcard {
      return Ok(Self::wildcard());
    }
    if fields.is_empty() {
      return Err(HttpVaryParseError::new("invalid Vary field name"));
    }

    Ok(Self {
      wildcard: false,
      fields,
    })
  }

  pub fn wildcard() -> Self {
    Self {
      wildcard: true,
      fields: Vec::new(),
    }
  }

  pub fn is_wildcard(&self) -> bool {
    self.wildcard
  }

  pub fn field_names(&self) -> Vec<&str> {
    self.fields.iter().map(String::as_str).collect()
  }

  pub fn header_value(&self) -> String {
    if self.wildcard {
      "*".to_string()
    } else {
      self.fields.join(", ")
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpVarySelection {
  pub(crate) wildcard: bool,
  pub(crate) fields: Vec<HttpVarySelectedField>,
}

impl HttpVarySelection {
  pub(crate) fn wildcard() -> Self {
    Self {
      wildcard: true,
      fields: Vec::new(),
    }
  }

  pub(crate) fn from_fields<I>(fields: I) -> Self
  where
    I: IntoIterator<Item = HttpVarySelectedField>,
  {
    Self {
      wildcard: false,
      fields: fields.into_iter().collect(),
    }
  }

  pub fn is_wildcard(&self) -> bool {
    self.wildcard
  }

  pub fn fields(&self) -> &[HttpVarySelectedField] {
    &self.fields
  }

  pub fn field_names(&self) -> Vec<&str> {
    self.fields.iter().map(|field| field.name()).collect()
  }

  pub fn values<S: AsRef<str>>(&self, name: S) -> Vec<&str> {
    self
      .fields
      .iter()
      .find(|field| field.name.eq_ignore_ascii_case(name.as_ref()))
      .map(|field| field.values())
      .unwrap_or_default()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpVarySelectedField {
  pub(crate) name: String,
  pub(crate) values: Vec<String>,
}

impl HttpVarySelectedField {
  pub(crate) fn new<S: AsRef<str>>(name: S, values: Vec<String>) -> Self {
    Self {
      name: name.as_ref().to_ascii_lowercase(),
      values,
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn values(&self) -> Vec<&str> {
    self.values.iter().map(String::as_str).collect()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpVaryParseError {
  pub(crate) message: String,
}

impl HttpVaryParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpVaryParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpVaryParseError {}

/// Server `Content-Disposition` response metadata backed by the shared
/// protocol parser, preserving the server facade accessor surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpContentDisposition {
  inner: rttp_protocol::content_disposition::ContentDisposition,
}

impl HttpContentDisposition {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpContentDispositionParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpContentDispositionParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      inner: rttp_protocol::content_disposition::ContentDisposition::parse_values(values)?,
    })
  }

  pub fn new(disposition_type: impl AsRef<str>) -> Result<Self, HttpContentDispositionParseError> {
    Ok(Self {
      inner: rttp_protocol::content_disposition::ContentDisposition::new(disposition_type)?,
    })
  }

  pub fn inline() -> Self {
    Self {
      inner: rttp_protocol::content_disposition::ContentDisposition::inline(),
    }
  }

  pub fn attachment() -> Self {
    Self {
      inner: rttp_protocol::content_disposition::ContentDisposition::attachment(),
    }
  }

  pub fn with_parameter<N, V>(
    self,
    name: N,
    value: V,
  ) -> Result<Self, HttpContentDispositionParseError>
  where
    N: AsRef<str>,
    V: AsRef<str>,
  {
    Ok(Self {
      inner: self.inner.with_parameter(name, value)?,
    })
  }

  pub fn disposition_type(&self) -> &str {
    self.inner.disposition_type()
  }

  pub fn parameter<S: AsRef<str>>(&self, name: S) -> Option<&str> {
    self
      .inner
      .parameter(name)
      .map(rttp_protocol::content_disposition::ContentDispositionParameter::value)
  }

  pub fn parameters(&self) -> Vec<(&str, &str)> {
    self
      .inner
      .parameters()
      .iter()
      .map(|parameter| (parameter.name(), parameter.value()))
      .collect()
  }

  pub fn header_value(&self) -> String {
    self.inner.header_value()
  }
}

pub trait IntoHttpContentDisposition {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError>;
}

impl IntoHttpContentDisposition for HttpContentDisposition {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError> {
    Ok(self)
  }
}

impl IntoHttpContentDisposition for &HttpContentDisposition {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError> {
    Ok(self.clone())
  }
}

impl IntoHttpContentDisposition for &str {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError> {
    HttpContentDisposition::parse(self)
  }
}

impl IntoHttpContentDisposition for String {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError> {
    HttpContentDisposition::parse(self)
  }
}

impl IntoHttpContentDisposition for &String {
  fn into_content_disposition(
    self,
  ) -> Result<HttpContentDisposition, HttpContentDispositionParseError> {
    HttpContentDisposition::parse(self)
  }
}

pub trait IntoHttpContentType {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError>;
}

impl IntoHttpContentType for HttpContentType {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError> {
    Ok(self)
  }
}

impl IntoHttpContentType for &HttpContentType {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError> {
    Ok(self.clone())
  }
}

impl IntoHttpContentType for &str {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError> {
    HttpContentType::parse(self)
  }
}

impl IntoHttpContentType for String {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError> {
    HttpContentType::parse(self)
  }
}

impl IntoHttpContentType for &String {
  fn into_content_type(self) -> Result<HttpContentType, HttpContentTypeParseError> {
    HttpContentType::parse(self)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpAcceptedMediaTypes {
  media_types: Vec<HttpContentType>,
}

impl HttpAcceptedMediaTypes {
  fn parse_values<'a, I>(
    values: I,
    header_name: &str,
    max_value_bytes: usize,
    max_media_types: usize,
  ) -> Result<Self, String>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut media_types = Vec::new();
    for value in values {
      if value.len() > max_value_bytes {
        return Err(format!("{header_name} header value is too large"));
      }
      for member in split_accept_capability_members(value)? {
        if media_types.len() >= max_media_types {
          return Err(format!("too many {header_name} media types"));
        }
        media_types.push(
          HttpContentType::parse(member)
            .map_err(|_| format!("invalid {header_name} media type"))?,
        );
      }
    }
    if media_types.is_empty() {
      return Err(format!("invalid {header_name} media type"));
    }
    Ok(Self { media_types })
  }

  fn from_media_types<I, M>(
    media_types: I,
    header_name: &str,
    max_value_bytes: usize,
    max_media_types: usize,
  ) -> Result<Self, String>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    let mut value = String::new();
    for (index, media_type) in media_types.into_iter().enumerate() {
      if index > 0 {
        value.push_str(", ");
      }
      value.push_str(media_type.as_ref());
      if value.len() > max_value_bytes {
        return Err(format!("{header_name} header value is too large"));
      }
    }
    Self::parse_values(
      [value.as_str()],
      header_name,
      max_value_bytes,
      max_media_types,
    )
  }

  fn media_types(&self) -> &[HttpContentType] {
    &self.media_types
  }

  fn header_value(&self) -> String {
    self
      .media_types
      .iter()
      .map(HttpContentType::header_value)
      .collect::<Vec<_>>()
      .join(", ")
  }
}

fn split_accept_capability_members(value: &str) -> Result<Vec<&str>, String> {
  let mut members = Vec::new();
  let mut start = 0usize;
  let mut quoted = false;
  let mut escaped = false;
  for (index, byte) in value.bytes().enumerate() {
    if escaped {
      escaped = false;
      continue;
    }
    match byte {
      b'\\' if quoted => escaped = true,
      b'"' => quoted = !quoted,
      b',' if !quoted => {
        let member = value[start..index].trim();
        if member.is_empty() {
          return Err("empty media type".to_string());
        }
        members.push(member);
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped {
    return Err("unterminated media type parameter".to_string());
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err("empty media type".to_string());
  }
  members.push(member);
  Ok(members)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptPatch(HttpAcceptedMediaTypes);

impl HttpAcceptPatch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpAcceptPatchParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpAcceptPatchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    HttpAcceptedMediaTypes::parse_values(
      values,
      "Accept-Patch",
      MAX_ACCEPT_PATCH_VALUE_BYTES,
      MAX_ACCEPT_PATCH_MEDIA_TYPES,
    )
    .map(Self)
    .map_err(HttpAcceptPatchParseError::new)
  }

  pub fn from_media_types<I, M>(media_types: I) -> Result<Self, HttpAcceptPatchParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    HttpAcceptedMediaTypes::from_media_types(
      media_types,
      "Accept-Patch",
      MAX_ACCEPT_PATCH_VALUE_BYTES,
      MAX_ACCEPT_PATCH_MEDIA_TYPES,
    )
    .map(Self)
    .map_err(HttpAcceptPatchParseError::new)
  }

  pub fn media_types(&self) -> &[HttpContentType] {
    self.0.media_types()
  }

  pub fn header_value(&self) -> String {
    self.0.header_value()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptPatchParseError {
  message: String,
}

impl HttpAcceptPatchParseError {
  fn new(message: String) -> Self {
    Self { message }
  }
}

impl fmt::Display for HttpAcceptPatchParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAcceptPatchParseError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptPost(HttpAcceptedMediaTypes);

impl HttpAcceptPost {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpAcceptPostParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpAcceptPostParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    HttpAcceptedMediaTypes::parse_values(
      values,
      "Accept-Post",
      MAX_ACCEPT_POST_VALUE_BYTES,
      MAX_ACCEPT_POST_MEDIA_TYPES,
    )
    .map(Self)
    .map_err(HttpAcceptPostParseError::new)
  }

  pub fn from_media_types<I, M>(media_types: I) -> Result<Self, HttpAcceptPostParseError>
  where
    I: IntoIterator<Item = M>,
    M: AsRef<str>,
  {
    HttpAcceptedMediaTypes::from_media_types(
      media_types,
      "Accept-Post",
      MAX_ACCEPT_POST_VALUE_BYTES,
      MAX_ACCEPT_POST_MEDIA_TYPES,
    )
    .map(Self)
    .map_err(HttpAcceptPostParseError::new)
  }

  pub fn media_types(&self) -> &[HttpContentType] {
    self.0.media_types()
  }

  pub fn header_value(&self) -> String {
    self.0.header_value()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAcceptPostParseError {
  message: String,
}

impl HttpAcceptPostParseError {
  fn new(message: String) -> Self {
    Self { message }
  }
}

impl fmt::Display for HttpAcceptPostParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpAcceptPostParseError {}

pub(crate) fn parse_content_type_parameter_value(
  value: &str,
) -> Result<String, HttpContentTypeParseError> {
  if value.is_empty() {
    return Err(HttpContentTypeParseError::new(
      "invalid Content-Type parameter value",
    ));
  }
  if value.starts_with('"') {
    parse_content_type_quoted_string(value)
  } else if value.contains('"') || !is_http_token(value) {
    Err(HttpContentTypeParseError::new(
      "invalid Content-Type parameter value",
    ))
  } else {
    Ok(value.to_string())
  }
}

pub(crate) fn parse_content_type_quoted_string(
  value: &str,
) -> Result<String, HttpContentTypeParseError> {
  if !value.ends_with('"') || value.len() < 2 {
    return Err(HttpContentTypeParseError::new(
      "invalid Content-Type quoted-string",
    ));
  }

  let inner = &value[1..value.len() - 1];
  let mut parsed = String::new();
  let mut escaped = false;
  for byte in inner.bytes() {
    if escaped {
      if !is_content_type_quoted_pair_byte(byte) {
        return Err(HttpContentTypeParseError::new(
          "invalid Content-Type quoted-string",
        ));
      }
      parsed.push(byte as char);
      escaped = false;
    } else if byte == b'\\' {
      escaped = true;
    } else if byte == b'"' || !is_content_type_quoted_text_byte(byte) {
      return Err(HttpContentTypeParseError::new(
        "invalid Content-Type quoted-string",
      ));
    } else {
      parsed.push(byte as char);
    }
  }

  if escaped || parsed.is_empty() {
    return Err(HttpContentTypeParseError::new(
      "invalid Content-Type quoted-string",
    ));
  }

  Ok(parsed)
}

pub(crate) fn is_content_type_quoted_text_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e)
}

pub(crate) fn is_content_type_quoted_pair_byte(byte: u8) -> bool {
  byte == b'\t' || matches!(byte, 0x20..=0x7e)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HttpRequestCacheControl {
  pub(crate) no_cache: bool,
  pub(crate) no_store: bool,
  pub(crate) max_age: Option<u64>,
  pub(crate) max_stale: Option<Option<u64>>,
  pub(crate) min_fresh: Option<u64>,
  pub(crate) no_transform: bool,
  pub(crate) only_if_cached: bool,
  pub(crate) extensions: Vec<HttpCacheControlExtension>,
}

impl HttpRequestCacheControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCacheControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpCacheControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut cache_control = Self::default();
    let mut directive_count = 0usize;
    for value in values {
      for directive in split_cache_control_directives(value)? {
        directive_count += 1;
        if directive_count > MAX_CACHE_CONTROL_DIRECTIVES {
          return Err(HttpCacheControlParseError::new(
            "too many Cache-Control directives",
          ));
        }
        cache_control.apply_directive(&directive)?;
      }
    }
    Ok(cache_control)
  }

  pub(crate) fn apply_directive(
    &mut self,
    directive: &str,
  ) -> Result<(), HttpCacheControlParseError> {
    let parsed = parse_cache_control_directive(directive)?;

    match parsed.name.to_ascii_lowercase().as_str() {
      "no-cache" => self.no_cache = true,
      "no-store" => self.no_store = true,
      "max-age" => {
        self.max_age = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      "max-stale" => {
        self.max_stale = Some(match parsed.value {
          Some(value) => Some(parse_cache_control_delta_seconds(
            parsed.name,
            Some(&value),
            parsed.value_was_quoted,
          )?),
          None => None,
        })
      }
      "min-fresh" => {
        self.min_fresh = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      "no-transform" => self.no_transform = true,
      "only-if-cached" => self.only_if_cached = true,
      _ => self.extensions.push(HttpCacheControlExtension::new(
        parsed.name,
        parsed.value.as_deref(),
      )),
    }

    Ok(())
  }

  pub fn no_cache(&self) -> bool {
    self.no_cache
  }

  pub fn no_store(&self) -> bool {
    self.no_store
  }

  pub fn max_age(&self) -> Option<u64> {
    self.max_age
  }

  pub fn max_stale(&self) -> Option<Option<u64>> {
    self.max_stale
  }

  pub fn min_fresh(&self) -> Option<u64> {
    self.min_fresh
  }

  pub fn no_transform(&self) -> bool {
    self.no_transform
  }

  pub fn only_if_cached(&self) -> bool {
    self.only_if_cached
  }

  pub fn extensions(&self) -> &[HttpCacheControlExtension] {
    &self.extensions
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HttpResponseCacheControl {
  pub(crate) no_cache: bool,
  pub(crate) no_cache_fields: Vec<String>,
  pub(crate) no_store: bool,
  pub(crate) max_age: Option<u64>,
  pub(crate) s_maxage: Option<u64>,
  pub(crate) private: bool,
  pub(crate) private_fields: Vec<String>,
  pub(crate) public: bool,
  pub(crate) must_revalidate: bool,
  pub(crate) proxy_revalidate: bool,
  pub(crate) immutable: bool,
  pub(crate) stale_while_revalidate: Option<u64>,
  pub(crate) stale_if_error: Option<u64>,
  pub(crate) extensions: Vec<HttpCacheControlExtension>,
}

impl HttpResponseCacheControl {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpCacheControlParseError> {
    Self::parse_values([value.as_ref()])
  }

  pub fn parse_values<'a, I>(values: I) -> Result<Self, HttpCacheControlParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut cache_control = Self::default();
    let mut directive_count = 0usize;
    for value in values {
      for directive in split_cache_control_directives(value)? {
        directive_count += 1;
        if directive_count > MAX_CACHE_CONTROL_DIRECTIVES {
          return Err(HttpCacheControlParseError::new(
            "too many Cache-Control directives",
          ));
        }
        cache_control.apply_directive(&directive)?;
      }
    }
    Ok(cache_control)
  }

  pub(crate) fn apply_directive(
    &mut self,
    directive: &str,
  ) -> Result<(), HttpCacheControlParseError> {
    let parsed = parse_cache_control_directive(directive)?;

    match parsed.name.to_ascii_lowercase().as_str() {
      "no-cache" => {
        self.no_cache = true;
        if let Some(value) = parsed.value {
          self.no_cache_fields = split_cache_control_field_names(&value);
        }
      }
      "no-store" => self.no_store = true,
      "max-age" => {
        self.max_age = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      "s-maxage" => {
        self.s_maxage = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      "private" => {
        self.private = true;
        if let Some(value) = parsed.value {
          self.private_fields = split_cache_control_field_names(&value);
        }
      }
      "public" => self.public = true,
      "must-revalidate" => self.must_revalidate = true,
      "proxy-revalidate" => self.proxy_revalidate = true,
      "immutable" => self.immutable = true,
      "stale-while-revalidate" => {
        self.stale_while_revalidate = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      "stale-if-error" => {
        self.stale_if_error = Some(parse_cache_control_delta_seconds(
          parsed.name,
          parsed.value.as_deref(),
          parsed.value_was_quoted,
        )?)
      }
      _ => self.extensions.push(HttpCacheControlExtension::new(
        parsed.name,
        parsed.value.as_deref(),
      )),
    }

    Ok(())
  }

  pub fn no_cache(&self) -> bool {
    self.no_cache
  }

  pub fn no_cache_fields(&self) -> Vec<&str> {
    self.no_cache_fields.iter().map(String::as_str).collect()
  }

  pub fn no_store(&self) -> bool {
    self.no_store
  }

  pub fn max_age(&self) -> Option<u64> {
    self.max_age
  }

  pub fn s_maxage(&self) -> Option<u64> {
    self.s_maxage
  }

  pub fn private(&self) -> bool {
    self.private
  }

  pub fn private_fields(&self) -> Vec<&str> {
    self.private_fields.iter().map(String::as_str).collect()
  }

  pub fn public(&self) -> bool {
    self.public
  }

  pub fn must_revalidate(&self) -> bool {
    self.must_revalidate
  }

  pub fn proxy_revalidate(&self) -> bool {
    self.proxy_revalidate
  }

  pub fn immutable(&self) -> bool {
    self.immutable
  }

  pub fn stale_while_revalidate(&self) -> Option<u64> {
    self.stale_while_revalidate
  }

  pub fn stale_if_error(&self) -> Option<u64> {
    self.stale_if_error
  }

  pub fn extensions(&self) -> &[HttpCacheControlExtension] {
    &self.extensions
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpCacheControlExtension {
  pub(crate) name: String,
  pub(crate) value: Option<String>,
}

impl HttpCacheControlExtension {
  pub(crate) fn new(name: &str, value: Option<&str>) -> Self {
    Self {
      name: name.to_string(),
      value: value.map(ToString::to_string),
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> Option<&str> {
    self.value.as_deref()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpCacheControlParseError {
  pub(crate) message: String,
}

impl HttpCacheControlParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }
}

impl fmt::Display for HttpCacheControlParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpCacheControlParseError {}

pub(crate) struct ParsedCacheControlDirective<'a> {
  pub(crate) name: &'a str,
  pub(crate) value: Option<String>,
  pub(crate) value_was_quoted: bool,
}

pub(crate) fn split_cache_control_directives(
  value: &str,
) -> Result<Vec<String>, HttpCacheControlParseError> {
  if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
    return Err(HttpCacheControlParseError::new(
      "Cache-Control header value is too large",
    ));
  }

  let mut directives = Vec::new();
  let mut current = String::new();
  let mut in_quote = false;
  let mut escaped = false;

  for ch in value.chars() {
    if escaped {
      current.push(ch);
      escaped = false;
      continue;
    }

    match ch {
      '\\' if in_quote => {
        current.push(ch);
        escaped = true;
      }
      '"' => {
        current.push(ch);
        in_quote = !in_quote;
      }
      ',' if !in_quote => {
        push_cache_control_directive(&mut directives, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(HttpCacheControlParseError::new(
      "malformed Cache-Control quoted-string",
    ));
  }
  push_cache_control_directive(&mut directives, &current)?;
  Ok(directives)
}

pub(crate) fn push_cache_control_directive(
  directives: &mut Vec<String>,
  directive: &str,
) -> Result<(), HttpCacheControlParseError> {
  let directive = directive.trim();
  if directive.is_empty() {
    return Err(HttpCacheControlParseError::new(
      "invalid Cache-Control directive",
    ));
  }
  if directives.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
    return Err(HttpCacheControlParseError::new(
      "too many Cache-Control directives",
    ));
  }
  directives.push(directive.to_string());
  Ok(())
}

pub(crate) fn parse_cache_control_directive(
  directive: &str,
) -> Result<ParsedCacheControlDirective<'_>, HttpCacheControlParseError> {
  let (name, value, value_was_quoted) = match directive.split_once('=') {
    Some((name, value)) => {
      let value = value.trim();
      (
        name.trim(),
        Some(parse_cache_control_directive_value(value)?),
        value.starts_with('"'),
      )
    }
    None => (directive.trim(), None, false),
  };
  if !is_http_token(name) {
    return Err(HttpCacheControlParseError::new(
      "invalid Cache-Control directive",
    ));
  }

  Ok(ParsedCacheControlDirective {
    name,
    value,
    value_was_quoted,
  })
}

pub(crate) fn parse_cache_control_directive_value(
  value: &str,
) -> Result<String, HttpCacheControlParseError> {
  if let Some(value) = value.strip_prefix('"') {
    return parse_cache_control_quoted_string(value);
  }
  if value.contains('"') || value.is_empty() {
    return Err(HttpCacheControlParseError::new(
      "invalid Cache-Control directive value",
    ));
  }
  Ok(value.to_string())
}

pub(crate) fn parse_cache_control_quoted_string(
  value: &str,
) -> Result<String, HttpCacheControlParseError> {
  let mut chars = value.bytes();
  let mut parsed = Vec::new();
  let mut closed = false;

  while let Some(byte) = chars.next() {
    match byte {
      b'"' => {
        closed = true;
        break;
      }
      b'\\' => {
        let Some(escaped) = chars.next() else {
          return Err(HttpCacheControlParseError::new(
            "malformed Cache-Control quoted-string",
          ));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(HttpCacheControlParseError::new(
            "malformed Cache-Control quoted-string",
          ));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(byte) => parsed.push(byte),
      _ => {
        return Err(HttpCacheControlParseError::new(
          "malformed Cache-Control quoted-string",
        ))
      }
    }
  }

  if !closed || chars.any(|byte| !byte.is_ascii_whitespace()) {
    return Err(HttpCacheControlParseError::new(
      "malformed Cache-Control quoted-string",
    ));
  }

  String::from_utf8(parsed)
    .map_err(|_| HttpCacheControlParseError::new("malformed Cache-Control quoted-string"))
}

pub(crate) fn parse_cache_control_delta_seconds(
  name: &str,
  value: Option<&str>,
  value_was_quoted: bool,
) -> Result<u64, HttpCacheControlParseError> {
  let Some(value) = value else {
    return Err(HttpCacheControlParseError::new(format!(
      "missing Cache-Control {name} delta-seconds"
    )));
  };
  if value_was_quoted || value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(HttpCacheControlParseError::new(format!(
      "invalid Cache-Control {name} delta-seconds"
    )));
  }
  value.parse::<u64>().map_err(|_| {
    HttpCacheControlParseError::new(format!("invalid Cache-Control {name} delta-seconds"))
  })
}

pub(crate) fn split_cache_control_field_names(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|field| !field.is_empty())
    .map(ToString::to_string)
    .collect()
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpHeader {
  pub(crate) name: String,
  pub(crate) value: String,
}

impl HttpHeader {
  pub fn new<N: AsRef<str>, V: AsRef<str>>(name: N, value: V) -> Self {
    Self {
      name: name.as_ref().to_string(),
      value: value.as_ref().to_string(),
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

impl fmt::Debug for HttpHeader {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpHeader")
      .field("name", &self.name)
      .field("value", &debug_header_value(&self.name, &self.value))
      .finish()
  }
}

fn debug_header_value<'a>(name: &str, value: &'a str) -> DebugHeaderValue<'a> {
  if is_sensitive_debug_header(name) {
    DebugHeaderValue::Redacted
  } else {
    DebugHeaderValue::Visible(value)
  }
}

enum DebugHeaderValue<'a> {
  Redacted,
  Visible(&'a str),
}

impl fmt::Debug for DebugHeaderValue<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Redacted => formatter.write_str("\"[REDACTED]\""),
      Self::Visible(value) => fmt::Debug::fmt(value, formatter),
    }
  }
}

fn is_sensitive_debug_header(name: &str) -> bool {
  name.eq_ignore_ascii_case("authorization")
    || name.eq_ignore_ascii_case("cookie")
    || name.eq_ignore_ascii_case("idempotency-key")
    || name.eq_ignore_ascii_case("proxy-authorization")
    || name.eq_ignore_ascii_case("set-cookie")
    || name.eq_ignore_ascii_case("traceparent")
    || name.eq_ignore_ascii_case("tracestate")
    || name.eq_ignore_ascii_case("baggage")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpParseError {
  pub(crate) message: String,
}

impl HttpParseError {
  pub(crate) fn new<S: AsRef<str>>(message: S) -> Self {
    Self {
      message: message.as_ref().to_string(),
    }
  }

  pub(crate) fn from_io_error(error: io::Error) -> Self {
    Self::new(error.to_string())
  }
}

impl fmt::Display for HttpParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpParseError {}
