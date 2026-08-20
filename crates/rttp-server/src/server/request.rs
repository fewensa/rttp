use super::*;

pub use rttp_protocol::accept_charset::{
  AcceptCharset as HttpRequestAcceptCharsets,
  AcceptCharsetParseError as HttpAcceptCharsetParseError, AcceptCharsetRange as HttpAcceptCharset,
};
pub use rttp_protocol::accept_encoding::{
  AcceptEncoding as HttpRequestAcceptEncodings, AcceptEncodingCoding as HttpAcceptEncoding,
  AcceptEncodingParseError as HttpAcceptEncodingParseError,
};
pub use rttp_protocol::accept_language::{
  AcceptLanguage as HttpAcceptLanguages, AcceptLanguageParseError as HttpAcceptLanguageParseError,
};
pub use rttp_protocol::access_control_request_headers::{
  AccessControlRequestHeaders as HttpAccessControlRequestHeaders,
  AccessControlRequestHeadersParseError as HttpAccessControlRequestHeadersParseError,
};
pub use rttp_protocol::access_control_request_method::{
  AccessControlRequestMethod as HttpAccessControlRequestMethod,
  AccessControlRequestMethodParseError as HttpAccessControlRequestMethodParseError,
};
pub use rttp_protocol::access_control_request_private_network::{
  AccessControlRequestPrivateNetwork as HttpAccessControlRequestPrivateNetwork,
  AccessControlRequestPrivateNetworkParseError as HttpAccessControlRequestPrivateNetworkParseError,
};
pub use rttp_protocol::authorization::{
  Authorization as HttpAuthorization, AuthorizationParseError as HttpAuthorizationParseError,
  ProxyAuthorization as HttpProxyAuthorization,
};
pub use rttp_protocol::baggage::{
  Baggage as HttpBaggage, BaggageMember as HttpBaggageMember,
  BaggageParseError as HttpBaggageParseError, BaggageProperty as HttpBaggageProperty,
};
pub use rttp_protocol::cdn_loop::{
  CdnLoop as HttpCdnLoop, CdnLoopMember as HttpCdnLoopMember,
  CdnLoopParameter as HttpCdnLoopParameter, CdnLoopParseError as HttpCdnLoopParseError,
};
pub use rttp_protocol::connection::{
  Connection as HttpConnection, ConnectionParseError as HttpConnectionParseError,
};
pub use rttp_protocol::content_length::HttpContentLength;
pub use rttp_protocol::cookie::{HttpCookiePair, HttpCookieParseError, HttpCookies};
pub use rttp_protocol::depth::{Depth as HttpDepth, DepthParseError as HttpDepthParseError};
pub use rttp_protocol::entity_tag::{
  EntityTag as HttpEntityTag, EntityTagParseError as HttpEntityTagParseError,
};
pub use rttp_protocol::expect::{
  Expect as HttpExpectations, ExpectParseError as HttpExpectParseError,
};
pub use rttp_protocol::fetch_metadata::{
  FetchMetadataParseError as HttpFetchMetadataParseError, SecFetchDest, SecFetchMode, SecFetchSite,
  SecFetchUser, SecPurpose,
};
pub use rttp_protocol::forwarded::{
  Forwarded as HttpForwarded, ForwardedElement as HttpForwardedElement,
  ForwardedParameter as HttpForwardedParameter, ForwardedParseError as HttpForwardedParseError,
};
pub use rttp_protocol::host::{Host as HttpHost, HostParseError as HttpHostParseError};
pub use rttp_protocol::idempotency_key::{
  IdempotencyKey as HttpIdempotencyKey, IdempotencyKeyParseError as HttpIdempotencyKeyParseError,
};
pub use rttp_protocol::if_modified_since::{
  IfModifiedSince as HttpIfModifiedSince,
  IfModifiedSinceParseError as HttpIfModifiedSinceParseError,
};
pub use rttp_protocol::if_unmodified_since::{
  IfUnmodifiedSince as HttpIfUnmodifiedSince,
  IfUnmodifiedSinceParseError as HttpIfUnmodifiedSinceParseError,
};
pub use rttp_protocol::max_forwards::{
  MaxForwards as HttpMaxForwards, MaxForwardsParseError as HttpMaxForwardsParseError,
};
pub use rttp_protocol::pragma::{
  Pragma as HttpPragma, PragmaDirective as HttpPragmaDirective,
  PragmaParseError as HttpPragmaParseError,
};
pub use rttp_protocol::prefer::{
  Prefer as HttpRequestPreferences, PreferParseError as HttpPreferParseError,
  Preference as HttpPreference, PreferenceKind as HttpPreferenceKind,
  PreferenceParameter as HttpPreferenceParameter,
};
pub use rttp_protocol::save_data::{
  SaveData as HttpSaveData, SaveDataParseError as HttpSaveDataParseError,
};
pub use rttp_protocol::sec_gpc::{SecGpc as HttpSecGpc, SecGpcParseError as HttpSecGpcParseError};
pub use rttp_protocol::sec_websocket_key::{
  SecWebSocketKey as HttpSecWebSocketKey,
  SecWebSocketKeyParseError as HttpSecWebSocketKeyParseError,
};
pub use rttp_protocol::signature::{
  Signature as HttpSignature, SignatureEntry as HttpSignatureEntry,
  SignatureParseError as HttpSignatureParseError,
};
pub use rttp_protocol::signature_input::{
  SignatureCoveredComponent as HttpSignatureCoveredComponent,
  SignatureDecimal as HttpSignatureDecimal, SignatureInput as HttpSignatureInput,
  SignatureInputMember as HttpSignatureInputMember,
  SignatureInputParseError as HttpSignatureInputParseError,
  SignatureParameter as HttpSignatureParameter,
  SignatureParameterValue as HttpSignatureParameterValue,
};
pub use rttp_protocol::signature_input::{
  SignatureInputBareItem as HttpSignatureInputBareItem,
  SignatureInputComponent as HttpSignatureInputComponent,
  SignatureInputEntry as HttpSignatureInputEntry,
  SignatureInputParameter as HttpSignatureInputParameter,
};
pub use rttp_protocol::te::{
  Te as HttpRequestTe, TeCoding as HttpTe, TeParseError as HttpTeParseError,
};
pub use rttp_protocol::trace_context::{
  TraceParent as HttpTraceParent, TraceParentParseError as HttpTraceParentParseError,
  TraceState as HttpTraceState, TraceStateMember as HttpTraceStateMember,
  TraceStateParseError as HttpTraceStateParseError,
};
pub use rttp_protocol::trailer::{
  Trailer as HttpTrailer, TrailerParseError as HttpTrailerParseError,
};
pub use rttp_protocol::transfer_encoding::{
  TransferEncoding as HttpTransferEncoding,
  TransferEncodingParseError as HttpTransferEncodingParseError,
};
pub use rttp_protocol::upgrade::{
  Upgrade as HttpUpgrade, UpgradeParseError as HttpUpgradeParseError,
};
pub use rttp_protocol::upgrade_insecure_requests::{
  UpgradeInsecureRequests as HttpUpgradeInsecureRequests,
  UpgradeInsecureRequestsParseError as HttpUpgradeInsecureRequestsParseError,
};
pub use rttp_protocol::want_content_digest::{
  WantContentDigest as HttpWantContentDigest, WantContentDigestEntry as HttpWantContentDigestEntry,
  WantContentDigestParseError as HttpWantContentDigestParseError,
};
pub use rttp_protocol::want_repr_digest::{
  WantReprDigest as HttpWantReprDigest, WantReprDigestEntry as HttpWantReprDigestEntry,
  WantReprDigestParseError as HttpWantReprDigestParseError,
};
pub use rttp_protocol::x_forwarded_for::{
  XForwardedFor as HttpXForwardedFor, XForwardedForNode as HttpXForwardedForNode,
  XForwardedForNodeKind as HttpXForwardedForNodeKind,
  XForwardedForParseError as HttpXForwardedForParseError,
};
pub use rttp_protocol::x_forwarded_host::{
  XForwardedHost as HttpXForwardedHost, XForwardedHostParseError as HttpXForwardedHostParseError,
};
pub use rttp_protocol::x_forwarded_proto::{
  XForwardedProto as HttpXForwardedProto,
  XForwardedProtoParseError as HttpXForwardedProtoParseError,
};

pub(crate) const MAX_REQUEST_HEAD_BYTES: usize = 64 * 1024;
pub(crate) const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_ACCEPT_VALUE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_ACCEPT_MEDIA_RANGES: usize = 256;
const MAX_CONDITIONAL_VALUE_BYTES: usize = 64 * 1024;
const MAX_IF_NONE_MATCH_TAGS: usize = 32;

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
  pub(crate) method: String,
  pub(crate) target: String,
  pub(crate) version: String,
  pub(crate) headers: Vec<(String, String)>,
  pub(crate) trailers: Vec<(String, String)>,
  pub(crate) body: Vec<u8>,
  pub(crate) content_length: Option<HttpContentLength>,
  pub(crate) extended_connect_protocol: Option<String>,
}

impl fmt::Debug for Request {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Request")
      .field("method", &self.method)
      .field("target", &self.target)
      .field("version", &self.version)
      .field("headers", &DebugHeaderPairs(&self.headers))
      .field("trailers", &DebugHeaderPairs(&self.trailers))
      .field("body", &self.body)
      .field("content_length", &self.content_length)
      .field("extended_connect_protocol", &self.extended_connect_protocol)
      .finish()
  }
}

struct DebugHeaderPairs<'a>(&'a [(String, String)]);

impl fmt::Debug for DebugHeaderPairs<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_list()
      .entries(self.0.iter().map(|(name, value)| DebugHeaderPair {
        name: name.as_str(),
        value: value.as_str(),
      }))
      .finish()
  }
}

struct DebugHeaderPair<'a> {
  name: &'a str,
  value: &'a str,
}

impl fmt::Debug for DebugHeaderPair<'_> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_tuple("")
      .field(&self.name)
      .field(&debug_header_value(self.name, self.value))
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
    || name.eq_ignore_ascii_case("sec-websocket-accept")
    || name.eq_ignore_ascii_case("sec-websocket-key")
    || name.eq_ignore_ascii_case("set-cookie")
    || name.eq_ignore_ascii_case("traceparent")
    || name.eq_ignore_ascii_case("tracestate")
    || name.eq_ignore_ascii_case("baggage")
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

  pub fn content_length(&self) -> Option<HttpContentLength> {
    self.content_length
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

  /// Parses one bounded `Range` field against an entity length without
  /// selecting or serving a representation.
  pub fn range(&self, entity_length: usize) -> Result<Option<HttpByteRange>, HttpByteRangeError> {
    parse_range_values(self.headers_named("Range"), entity_length)
  }

  /// Parses one strong entity-tag or HTTP-date `If-Range` validator.
  pub fn if_range(&self) -> Result<Option<HttpIfRange>, HttpIfRangeParseError> {
    HttpIfRange::parse_optional_values(self.headers_named("If-Range"))
  }

  /// Parses bounded `If-None-Match` validators without evaluating them.
  pub fn if_none_match(&self) -> Result<Option<HttpIfNoneMatch>, HttpIfNoneMatchParseError> {
    HttpIfNoneMatch::parse_optional_values(self.headers_named("If-None-Match"))
  }

  /// Parses one HTTP-date `If-Modified-Since` validator without evaluating it.
  pub fn if_modified_since(
    &self,
  ) -> Result<Option<HttpIfModifiedSince>, HttpIfModifiedSinceParseError> {
    let values: Vec<&str> = self.headers_named("If-Modified-Since").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIfModifiedSince::parse_values(values).map(Some)
  }

  /// Parses one HTTP-date `If-Unmodified-Since` validator without evaluating it.
  pub fn if_unmodified_since(
    &self,
  ) -> Result<Option<HttpIfUnmodifiedSince>, HttpIfUnmodifiedSinceParseError> {
    let values: Vec<&str> = self.headers_named("If-Unmodified-Since").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIfUnmodifiedSince::parse_values(values).map(Some)
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

  /// Parses received `Sec-Fetch-Site` metadata without enforcing browser policy.
  pub fn sec_fetch_site(&self) -> Result<Option<SecFetchSite>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self.headers_named("Sec-Fetch-Site"),
      "Sec-Fetch-Site",
    )
  }

  /// Parses received `Sec-Fetch-Mode` metadata without enforcing browser policy.
  pub fn sec_fetch_mode(&self) -> Result<Option<SecFetchMode>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self.headers_named("Sec-Fetch-Mode"),
      "Sec-Fetch-Mode",
    )
  }

  /// Parses received `Sec-Fetch-Dest` metadata without enforcing browser policy.
  pub fn sec_fetch_dest(&self) -> Result<Option<SecFetchDest>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self.headers_named("Sec-Fetch-Dest"),
      "Sec-Fetch-Dest",
    )
  }

  /// Parses received `Sec-Fetch-User` metadata without enforcing browser policy.
  pub fn sec_fetch_user(&self) -> Result<Option<SecFetchUser>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self.headers_named("Sec-Fetch-User"),
      "Sec-Fetch-User",
    )
  }

  /// Parses received `Sec-Purpose` metadata without enforcing browser policy,
  /// starting prefetches, or changing cache behavior.
  pub fn sec_purpose(&self) -> Result<Option<SecPurpose>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self.headers_named("Sec-Purpose"),
      "Sec-Purpose",
    )
  }

  /// Parses received `Access-Control-Request-Method` preflight metadata
  /// without applying CORS policy.
  pub fn access_control_request_method(
    &self,
  ) -> Result<Option<HttpAccessControlRequestMethod>, HttpAccessControlRequestMethodParseError> {
    let values: Vec<&str> = self
      .headers_named("Access-Control-Request-Method")
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestMethod::parse_values(values).map(Some)
  }

  /// Parses received `Access-Control-Request-Headers` preflight metadata
  /// without applying CORS policy.
  pub fn access_control_request_headers(
    &self,
  ) -> Result<Option<HttpAccessControlRequestHeaders>, HttpAccessControlRequestHeadersParseError>
  {
    let values: Vec<&str> = self
      .headers_named("Access-Control-Request-Headers")
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestHeaders::parse_values(values).map(Some)
  }

  /// Parses received `Access-Control-Request-Private-Network` preflight
  /// metadata without applying Private Network Access or CORS policy.
  pub fn access_control_request_private_network(
    &self,
  ) -> Result<
    Option<HttpAccessControlRequestPrivateNetwork>,
    HttpAccessControlRequestPrivateNetworkParseError,
  > {
    let values: Vec<&str> = self
      .headers_named("Access-Control-Request-Private-Network")
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestPrivateNetwork::parse_values(values).map(Some)
  }

  /// Parses received `Save-Data` request metadata without applying
  /// reduced-data serving or content-adaptation policy.
  pub fn save_data(&self) -> Result<Option<HttpSaveData>, HttpSaveDataParseError> {
    let values: Vec<&str> = self.headers_named("Save-Data").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSaveData::parse_values(values).map(Some)
  }

  /// Parses received `Sec-GPC` request metadata without applying consent,
  /// tracking, legal, or serving policy.
  pub fn sec_gpc(&self) -> Result<Option<HttpSecGpc>, HttpSecGpcParseError> {
    let values: Vec<&str> = self.headers_named("Sec-GPC").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSecGpc::parse_values(values).map(Some)
  }

  /// Parses received `Upgrade-Insecure-Requests` request metadata without
  /// rewriting URLs, redirecting, or enforcing Content-Security-Policy.
  pub fn upgrade_insecure_requests(
    &self,
  ) -> Result<Option<HttpUpgradeInsecureRequests>, HttpUpgradeInsecureRequestsParseError> {
    let values: Vec<&str> = self.headers_named("Upgrade-Insecure-Requests").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpUpgradeInsecureRequests::parse_values(values).map(Some)
  }

  /// Parses received `Host` request authority without applying virtual-host
  /// routing or scheme defaults.
  pub fn host(&self) -> Result<Option<HttpHost>, HttpHostParseError> {
    parse_host_values(self.headers_named("Host"))
  }

  /// Parses exactly one bounded `Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn authorization(&self) -> Result<Option<HttpAuthorization>, HttpAuthorizationParseError> {
    let values: Vec<&str> = self.headers_named("Authorization").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAuthorization::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Proxy-Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn proxy_authorization(
    &self,
  ) -> Result<Option<HttpProxyAuthorization>, HttpAuthorizationParseError> {
    let values: Vec<&str> = self.headers_named("Proxy-Authorization").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpProxyAuthorization::parse_values(values).map(Some)
  }

  /// Parses request `Cookie` pairs as bounded opaque metadata without applying
  /// cookie storage, matching, or authorization policy.
  pub fn cookies(&self) -> Result<Option<HttpCookies>, HttpCookieParseError> {
    let values: Vec<&str> = self.headers_named("Cookie").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCookies::parse_values(values).map(Some)
  }

  /// Parses received `Max-Forwards` request metadata without automatically
  /// decrementing or forwarding the request.
  pub fn max_forwards(&self) -> Result<Option<HttpMaxForwards>, HttpMaxForwardsParseError> {
    let values: Vec<&str> = self.headers_named("Max-Forwards").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpMaxForwards::parse_values(values).map(Some)
  }

  /// Parses received WebDAV `Depth` request metadata without traversing
  /// resources or enforcing method policy.
  pub fn depth(&self) -> Result<Option<HttpDepth>, HttpDepthParseError> {
    let values: Vec<&str> = self.headers_named("Depth").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpDepth::parse_values(values).map(Some)
  }

  /// Parses exactly one opaque, bounded `Idempotency-Key` field as typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous keys. This
  /// accessor does not retry requests, store keys, or apply idempotency
  /// policy.
  pub fn idempotency_key(
    &self,
  ) -> Result<Option<HttpIdempotencyKey>, HttpIdempotencyKeyParseError> {
    let values: Vec<&str> = self.headers_named("Idempotency-Key").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIdempotencyKey::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Sec-WebSocket-Key` field as typed nonce
  /// metadata. Duplicate fields are rejected to avoid ambiguous nonces. This
  /// accessor does not perform an HTTP upgrade, compute
  /// `Sec-WebSocket-Accept`, or implement WebSocket frames.
  pub fn sec_websocket_key(
    &self,
  ) -> Result<Option<HttpSecWebSocketKey>, HttpSecWebSocketKeyParseError> {
    let values: Vec<&str> = self.headers_named("Sec-WebSocket-Key").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSecWebSocketKey::parse_values(values).map(Some)
  }

  /// Parses received W3C `traceparent` request metadata without creating
  /// identifiers, deciding sampling, or configuring propagation.
  pub fn traceparent(&self) -> Result<Option<HttpTraceParent>, HttpTraceParentParseError> {
    let values: Vec<&str> = self.headers_named("traceparent").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTraceParent::parse_values(values).map(Some)
  }

  /// Parses received W3C `tracestate` request metadata without invoking a
  /// tracing backend or automatically propagating metadata.
  pub fn tracestate(&self) -> Result<Option<HttpTraceState>, HttpTraceStateParseError> {
    let values: Vec<&str> = self.headers_named("tracestate").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTraceState::parse_values(values).map(Some)
  }

  /// Parses received W3C `baggage` request metadata without interpreting
  /// application values or automatically propagating metadata.
  pub fn baggage(&self) -> Result<Option<HttpBaggage>, HttpBaggageParseError> {
    let values: Vec<&str> = self.headers_named("baggage").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpBaggage::parse_values(values).map(Some)
  }

  /// Parses received `Prefer` request metadata without applying preferences.
  pub fn prefer(&self) -> Result<Option<HttpRequestPreferences>, HttpPreferParseError> {
    parse_prefer_values(self.headers_named("Prefer"))
  }
  /// Parses received `Accept-Charset` request metadata without negotiating,
  /// transcoding, or selecting a response charset.
  pub fn accept_charset(
    &self,
  ) -> Result<Option<HttpRequestAcceptCharsets>, HttpAcceptCharsetParseError> {
    let values: Vec<&str> = self.headers_named("Accept-Charset").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestAcceptCharsets::parse_values(values).map(Some)
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

  /// Parses received `Want-Content-Digest` request metadata without selecting
  /// an algorithm or computing a content digest.
  pub fn want_content_digest(
    &self,
  ) -> Result<Option<HttpWantContentDigest>, HttpWantContentDigestParseError> {
    parse_want_content_digest_values(self.headers_named("Want-Content-Digest"))
  }

  /// Parses received `Want-Repr-Digest` request metadata without selecting an
  /// algorithm or computing a representation digest.
  pub fn want_repr_digest(
    &self,
  ) -> Result<Option<HttpWantReprDigest>, HttpWantReprDigestParseError> {
    parse_want_repr_digest_values(self.headers_named("Want-Repr-Digest"))
  }

  /// Parses received RFC 9421 `Signature` request metadata without verifying
  /// signatures or looking up keys.
  pub fn signature(&self) -> Result<Option<HttpSignature>, HttpSignatureParseError> {
    parse_signature_values(self.headers_named("Signature"))
  }

  /// Parses received RFC 9421 `Signature-Input` request metadata without
  /// verifying signatures, looking up keys, or applying cryptographic policy.
  pub fn signature_input(
    &self,
  ) -> Result<Option<HttpSignatureInput>, HttpSignatureInputParseError> {
    parse_signature_input_values(self.headers_named("Signature-Input"))
  }

  /// Parses received `Upgrade` protocol metadata without changing handoff
  /// validation or interpreting the upgraded protocol.
  pub fn upgrade(&self) -> Result<Option<HttpUpgrade>, HttpUpgradeParseError> {
    let values: Vec<&str> = self.headers_named("Upgrade").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpUpgrade::parse_values(values).map(Some)
  }

  /// Parses received `Pragma` request metadata without applying cache,
  /// intermediary, or HTTP/1.0 compatibility policy.
  pub fn pragma(&self) -> Result<Option<HttpPragma>, HttpPragmaParseError> {
    let values: Vec<&str> = self.headers_named("Pragma").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpPragma::parse_values(values).map(Some)
  }

  /// Parses received `Content-Type` representation metadata without sniffing
  /// MIME types or interpreting the body.
  pub fn content_type(&self) -> Result<Option<HttpContentType>, HttpContentTypeParseError> {
    let mut values = self.headers_named("Content-Type");
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(HttpContentTypeParseError::new(
        "multiple Content-Type headers",
      ));
    }
    HttpContentType::parse(value).map(Some)
  }

  /// Parses received `Content-Encoding` representation metadata without
  /// decoding or negotiating content codings.
  pub fn content_encoding(
    &self,
  ) -> Result<Option<HttpResponseContentEncodings>, HttpContentEncodingParseError> {
    let values: Vec<&str> = self.headers_named("Content-Encoding").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpResponseContentEncodings::parse_values(values).map(Some)
  }

  /// Parses received `Content-Language` representation metadata without
  /// selecting a locale or negotiating a variant.
  pub fn content_language(
    &self,
  ) -> Result<Option<HttpContentLanguages>, HttpContentLanguageParseError> {
    let values: Vec<&str> = self.headers_named("Content-Language").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpContentLanguages::parse_values(values).map(Some)
  }

  /// Parses received `Expect` request metadata without automatically sending
  /// an interim response or rejecting unsupported expectations.
  pub fn expectations(&self) -> Result<Option<HttpExpectations>, HttpExpectParseError> {
    let values: Vec<&str> = self.headers_named("Expect").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpExpectations::parse_values(values).map(Some)
  }

  /// Parses received `TE` request metadata without enabling transfer coding,
  /// trailer negotiation, compression, or proxy behavior.
  pub fn te(&self) -> Result<Option<HttpRequestTe>, HttpTeParseError> {
    let values: Vec<&str> = self.headers_named("TE").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestTe::parse_values(values).map(Some)
  }

  /// Parses retained HTTP/1 `Connection` header metadata without changing
  /// keep-alive, hop-by-hop stripping, or HTTP/2 rejection.
  pub fn connection(&self) -> Result<Option<HttpConnection>, HttpConnectionParseError> {
    let values: Vec<&str> = self.headers_named("Connection").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpConnection::parse_values(values).map(Some)
  }

  /// Parses retained `Transfer-Encoding` framing metadata without changing
  /// request body framing or HTTP/2 decode.
  pub fn transfer_encoding(
    &self,
  ) -> Result<Option<HttpTransferEncoding>, HttpTransferEncodingParseError> {
    let values: Vec<&str> = self.headers_named("Transfer-Encoding").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTransferEncoding::parse_values(values).map(Some)
  }

  /// Parses announced trailer field names without waiting for or exposing a
  /// trailer block.
  pub fn trailer_header(&self) -> Result<Option<HttpTrailer>, HttpTrailerParseError> {
    let values: Vec<&str> = self.headers_named("Trailer").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTrailer::parse_values(values).map(Some)
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

  /// Parses bounded RFC 8586 `CDN-Loop` request metadata without detecting
  /// loops, rejecting the request, or inserting a local CDN identifier.
  pub fn cdn_loop(&self) -> Result<Option<HttpCdnLoop>, HttpCdnLoopParseError> {
    let values: Vec<&str> = self.headers_named("CDN-Loop").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCdnLoop::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-For` request metadata without applying a
  /// trusted-proxy policy, deriving identity, or rewriting request addresses.
  pub fn x_forwarded_for(&self) -> Result<Option<HttpXForwardedFor>, HttpXForwardedForParseError> {
    let values: Vec<&str> = self.headers_named("X-Forwarded-For").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedFor::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-Host` request metadata without applying a
  /// trusted-proxy policy, changing routing, or rewriting request authority.
  pub fn x_forwarded_host(
    &self,
  ) -> Result<Option<HttpXForwardedHost>, HttpXForwardedHostParseError> {
    let values: Vec<&str> = self.headers_named("X-Forwarded-Host").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedHost::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-Proto` request metadata without applying a
  /// trusted-proxy policy, upgrading, redirecting, or rewriting schemes.
  pub fn x_forwarded_proto(
    &self,
  ) -> Result<Option<HttpXForwardedProto>, HttpXForwardedProtoParseError> {
    let values: Vec<&str> = self.headers_named("X-Forwarded-Proto").collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedProto::parse_values(values).map(Some)
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
    max_request_body_bytes: usize,
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
          return Ok(Some(Self::from_raw_frame_with_body_kind(
            &raw,
            RequestBodyKind::ContentLength(content_length),
          )?));
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
          match parsed_body_kind {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_body_kind_and_trailers(
                head,
                Vec::new(),
                parsed_body_kind,
                Vec::new(),
              )));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length, max_request_body_bytes)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader, max_request_body_bytes)?;
              return Ok(Some(Self::from_head_body_kind_and_trailers(
                head,
                chunked.body,
                parsed_body_kind,
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
    max_request_body_bytes: usize,
  ) -> io::Result<Option<(Self, RequestBodyKind)>>
  where
    S: Read + Write,
  {
    Self::read_next_head_and_body_kind_from_with_continue(reader, max_request_body_bytes)?.map_or(
      Ok(None),
      |(head, kind)| {
        Ok(Some((
          Self::from_head_body_kind_and_trailers(head, Vec::new(), kind, Vec::new()),
          kind,
        )))
      },
    )
  }

  pub(crate) fn read_next_head_and_body_kind_from_with_continue<S>(
    reader: &mut BufReader<S>,
    max_request_body_bytes: usize,
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
              reject_oversized_request_body(content_length, max_request_body_bytes)?;
            }
            RequestBodyKind::Chunked => {}
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
          return Ok(Some(Self::from_raw_frame_with_body_kind(
            &raw,
            RequestBodyKind::ContentLength(content_length),
          )?));
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
          match parsed_body_kind {
            RequestBodyKind::ContentLength(0) => {
              return Ok(Some(Self::from_head_body_kind_and_trailers(
                head,
                Vec::new(),
                parsed_body_kind,
                Vec::new(),
              )));
            }
            RequestBodyKind::ContentLength(content_length) => {
              reject_oversized_request_body(content_length, MAX_REQUEST_BODY_BYTES)?;
              body_kind = Some(RequestBodyKind::ContentLength(content_length));
            }
            RequestBodyKind::Chunked => {
              let chunked = read_chunked_request_body(reader, MAX_REQUEST_BODY_BYTES)?;
              return Ok(Some(Self::from_head_body_kind_and_trailers(
                head,
                chunked.body,
                parsed_body_kind,
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

  #[cfg(test)]
  pub(crate) fn from_raw_frame(raw: &[u8]) -> io::Result<Self> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))?;
    reject_oversized_request_head(header_end + 4)?;
    let head = parse_request_head(&raw[..header_end])?;
    let body_kind = request_body_kind(&head.headers)?;
    Self::from_raw_frame_with_head_and_body_kind(raw, header_end, head, body_kind)
  }

  fn from_raw_frame_with_body_kind(raw: &[u8], body_kind: RequestBodyKind) -> io::Result<Self> {
    let header_end = find_header_end(raw)
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete HTTP request"))?;
    reject_oversized_request_head(header_end + 4)?;
    let head = parse_request_head(&raw[..header_end])?;
    Self::from_raw_frame_with_head_and_body_kind(raw, header_end, head, body_kind)
  }

  fn from_raw_frame_with_head_and_body_kind(
    raw: &[u8],
    header_end: usize,
    head: RequestHead,
    body_kind: RequestBodyKind,
  ) -> io::Result<Self> {
    let body_start = header_end + 4;
    let body = match body_kind {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length, MAX_REQUEST_BODY_BYTES)?;
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

    let content_length =
      content_length_from_request_body_kind_and_headers(body_kind, &head.headers);

    Ok(Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers: Vec::new(),
      body,
      content_length,
      extended_connect_protocol: None,
    })
  }

  pub(crate) fn from_head_body_kind_and_trailers(
    head: RequestHead,
    body: Vec<u8>,
    body_kind: RequestBodyKind,
    trailers: Vec<(String, String)>,
  ) -> Self {
    let content_length =
      content_length_from_request_body_kind_and_headers(body_kind, &head.headers);

    Self {
      method: head.method,
      target: head.target,
      version: head.version,
      headers: head.headers,
      trailers,
      body,
      content_length,
      extended_connect_protocol: None,
    }
  }
}

fn content_length_from_request_body_kind_and_headers(
  kind: RequestBodyKind,
  headers: &[(String, String)],
) -> Option<HttpContentLength> {
  match kind {
    RequestBodyKind::ContentLength(length)
      if headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Content-Length")) =>
    {
      Some(HttpContentLength::new(length))
    }
    RequestBodyKind::Chunked => None,
    RequestBodyKind::ContentLength(_) => None,
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

fn parse_prefer_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpRequestPreferences>, HttpPreferParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpRequestPreferences::parse_values(values).map(Some)
}

fn parse_want_content_digest_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpWantContentDigest>, HttpWantContentDigestParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpWantContentDigest::parse_values(values).map(Some)
}

fn parse_want_repr_digest_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpWantReprDigest>, HttpWantReprDigestParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpWantReprDigest::parse_values(values).map(Some)
}

fn parse_signature_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpSignature>, HttpSignatureParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpSignature::parse_values(values).map(Some)
}

fn parse_signature_input_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpSignatureInput>, HttpSignatureInputParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpSignatureInput::parse_values(values).map(Some)
}

fn parse_host_values<'a>(
  values: impl IntoIterator<Item = &'a str>,
) -> Result<Option<HttpHost>, HttpHostParseError> {
  let values: Vec<&str> = values.into_iter().collect();
  if values.is_empty() {
    return Ok(None);
  }
  HttpHost::parse_values(values).map(Some)
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

/// A parsed `If-Range` validator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpIfRange {
  EntityTag(HttpEntityTag),
  Date(SystemTime),
}

impl HttpIfRange {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpIfRangeParseError> {
    Self::parse_value(value.as_ref())
  }

  fn parse_optional_values<'a, I>(values: I) -> Result<Option<Self>, HttpIfRangeParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut values = values.into_iter();
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(HttpIfRangeParseError::new("duplicate If-Range headers"));
    }
    Self::parse_value(value).map(Some)
  }

  fn parse_value(value: &str) -> Result<Self, HttpIfRangeParseError> {
    if value.len() > MAX_CONDITIONAL_VALUE_BYTES {
      return Err(HttpIfRangeParseError::new(
        "If-Range header value is too large",
      ));
    }
    let value = value.trim();
    if let Ok(entity_tag) = HttpEntityTag::parse(value) {
      if entity_tag.is_weak() {
        return Err(HttpIfRangeParseError::new(
          "If-Range requires a strong entity tag or HTTP-date",
        ));
      }
      return Ok(Self::EntityTag(entity_tag));
    }
    httpdate::parse_http_date(value)
      .map(Self::Date)
      .map_err(|_| HttpIfRangeParseError::new("invalid If-Range validator"))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpIfRangeParseError {
  message: String,
}

impl HttpIfRangeParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpIfRangeParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpIfRangeParseError {}

/// Parsed `If-None-Match` validators.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpIfNoneMatch {
  Any,
  Tags(Vec<HttpEntityTag>),
}

impl HttpIfNoneMatch {
  pub fn parse(value: impl AsRef<str>) -> Result<Self, HttpIfNoneMatchParseError> {
    Self::parse_optional_values([value.as_ref()])?
      .ok_or_else(|| HttpIfNoneMatchParseError::new("invalid If-None-Match validator"))
  }

  fn parse_optional_values<'a, I>(values: I) -> Result<Option<Self>, HttpIfNoneMatchParseError>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tags = Vec::new();
    let mut saw_value = false;
    let mut wildcard = false;
    for value in values {
      saw_value = true;
      if value.len() > MAX_CONDITIONAL_VALUE_BYTES {
        return Err(HttpIfNoneMatchParseError::new(
          "If-None-Match header value is too large",
        ));
      }
      for member in value.split(',') {
        let member = member.trim();
        if member == "*" {
          if wildcard || !tags.is_empty() {
            return Err(HttpIfNoneMatchParseError::new(
              "If-None-Match wildcard cannot be combined with entity tags",
            ));
          }
          wildcard = true;
          continue;
        }
        if wildcard {
          return Err(HttpIfNoneMatchParseError::new(
            "If-None-Match wildcard cannot be combined with entity tags",
          ));
        }
        if tags.len() >= MAX_IF_NONE_MATCH_TAGS {
          return Err(HttpIfNoneMatchParseError::new(
            "too many If-None-Match validators",
          ));
        }
        let tag = HttpEntityTag::parse(member)
          .map_err(|_| HttpIfNoneMatchParseError::new("invalid If-None-Match validator"))?;
        if tags.iter().any(|known: &HttpEntityTag| {
          known.is_weak() == tag.is_weak() && known.opaque_tag() == tag.opaque_tag()
        }) {
          return Err(HttpIfNoneMatchParseError::new(
            "duplicate If-None-Match validator",
          ));
        }
        tags.push(tag);
      }
    }
    if !saw_value {
      Ok(None)
    } else if wildcard {
      Ok(Some(Self::Any))
    } else if tags.is_empty() {
      Err(HttpIfNoneMatchParseError::new(
        "invalid If-None-Match validator",
      ))
    } else {
      Ok(Some(Self::Tags(tags)))
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpIfNoneMatchParseError {
  message: String,
}

impl HttpIfNoneMatchParseError {
  fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for HttpIfNoneMatchParseError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for HttpIfNoneMatchParseError {}

fn parse_range_values<'a, I>(
  values: I,
  entity_length: usize,
) -> Result<Option<HttpByteRange>, HttpByteRangeError>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut values = values.into_iter();
  let Some(value) = values.next() else {
    return Ok(None);
  };
  if values.next().is_some() {
    return Err(HttpByteRangeError::MultipleRanges);
  }
  HttpByteRange::parse(value, entity_length).map(Some)
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
      validators.push(EntityTagValidator::Tag(
        HttpEntityTag::parse(part.trim()).ok()?,
      ));
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

#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
  pub(crate) method: String,
  pub(crate) path: String,
  pub(crate) query: Option<String>,
  pub(crate) version: String,
  pub(crate) headers: Vec<HttpHeader>,
  pub(crate) body: Vec<u8>,
  pub(crate) content_length: Option<HttpContentLength>,
}

impl fmt::Debug for HttpRequest {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("HttpRequest")
      .field("method", &self.method)
      .field("path", &self.path)
      .field("query", &self.query)
      .field("version", &self.version)
      .field("headers", &self.headers)
      .field("body", &self.body)
      .field("content_length", &self.content_length)
      .finish()
  }
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

    let body_kind = request_body_kind(&head.headers).map_err(HttpParseError::from_io_error)?;
    let content_length =
      content_length_from_request_body_kind_and_headers(body_kind, &head.headers);
    let body = match body_kind {
      RequestBodyKind::ContentLength(content_length) => {
        reject_oversized_request_body(content_length, MAX_REQUEST_BODY_BYTES)
          .map_err(HttpParseError::from_io_error)?;
        if body_bytes.len() != content_length {
          return Err(HttpParseError::new(
            "request body length does not match Content-Length",
          ));
        }
        body_bytes.to_vec()
      }
      RequestBodyKind::Chunked => {
        let mut reader = Cursor::new(body_bytes);
        let chunked = read_chunked_request_body(&mut reader, MAX_REQUEST_BODY_BYTES)
          .map_err(HttpParseError::from_io_error)?;
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
      content_length,
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

  pub fn content_length(&self) -> Option<HttpContentLength> {
    self.content_length
  }

  /// Parses one bounded `Range` field against an entity length without
  /// selecting or serving a representation.
  pub fn range(&self, entity_length: usize) -> Result<Option<HttpByteRange>, HttpByteRangeError> {
    parse_range_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Range"))
        .map(|header| header.value.as_str()),
      entity_length,
    )
  }

  /// Parses one strong entity-tag or HTTP-date `If-Range` validator.
  pub fn if_range(&self) -> Result<Option<HttpIfRange>, HttpIfRangeParseError> {
    HttpIfRange::parse_optional_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("If-Range"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses bounded `If-None-Match` validators without evaluating them.
  pub fn if_none_match(&self) -> Result<Option<HttpIfNoneMatch>, HttpIfNoneMatchParseError> {
    HttpIfNoneMatch::parse_optional_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("If-None-Match"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses one HTTP-date `If-Modified-Since` validator without evaluating it.
  pub fn if_modified_since(
    &self,
  ) -> Result<Option<HttpIfModifiedSince>, HttpIfModifiedSinceParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("If-Modified-Since"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIfModifiedSince::parse_values(values).map(Some)
  }

  /// Parses one HTTP-date `If-Unmodified-Since` validator without evaluating it.
  pub fn if_unmodified_since(
    &self,
  ) -> Result<Option<HttpIfUnmodifiedSince>, HttpIfUnmodifiedSinceParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("If-Unmodified-Since"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIfUnmodifiedSince::parse_values(values).map(Some)
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

  /// Parses received `Sec-Purpose` metadata without enforcing browser policy,
  /// starting prefetches, or changing cache behavior.
  pub fn sec_purpose(&self) -> Result<Option<SecPurpose>, HttpFetchMetadataParseError> {
    rttp_protocol::fetch_metadata::parse_optional_value(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Sec-Purpose"))
        .map(|header| header.value.as_str()),
      "Sec-Purpose",
    )
  }

  /// Parses received `Host` request authority without applying virtual-host
  /// routing or scheme defaults.
  pub fn host(&self) -> Result<Option<HttpHost>, HttpHostParseError> {
    parse_host_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Host"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses exactly one bounded `Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn authorization(&self) -> Result<Option<HttpAuthorization>, HttpAuthorizationParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Authorization"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAuthorization::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Proxy-Authorization` field as opaque typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous credentials.
  pub fn proxy_authorization(
    &self,
  ) -> Result<Option<HttpProxyAuthorization>, HttpAuthorizationParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Proxy-Authorization"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpProxyAuthorization::parse_values(values).map(Some)
  }

  /// Parses request `Cookie` pairs as bounded opaque metadata without applying
  /// cookie storage, matching, or authorization policy.
  pub fn cookies(&self) -> Result<Option<HttpCookies>, HttpCookieParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Cookie"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCookies::parse_values(values).map(Some)
  }

  /// Parses received `Max-Forwards` request metadata without automatically
  /// decrementing or forwarding the request.
  pub fn max_forwards(&self) -> Result<Option<HttpMaxForwards>, HttpMaxForwardsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Max-Forwards"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpMaxForwards::parse_values(values).map(Some)
  }

  /// Parses received WebDAV `Depth` request metadata without traversing
  /// resources or enforcing method policy.
  pub fn depth(&self) -> Result<Option<HttpDepth>, HttpDepthParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Depth"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpDepth::parse_values(values).map(Some)
  }

  /// Parses exactly one opaque, bounded `Idempotency-Key` field as typed
  /// metadata. Duplicate fields are rejected to avoid ambiguous keys. This
  /// accessor does not retry requests, store keys, or apply idempotency
  /// policy.
  pub fn idempotency_key(
    &self,
  ) -> Result<Option<HttpIdempotencyKey>, HttpIdempotencyKeyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Idempotency-Key"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpIdempotencyKey::parse_values(values).map(Some)
  }

  /// Parses exactly one bounded `Sec-WebSocket-Key` field as typed nonce
  /// metadata. Duplicate fields are rejected to avoid ambiguous nonces. This
  /// accessor does not perform an HTTP upgrade, compute
  /// `Sec-WebSocket-Accept`, or implement WebSocket frames.
  pub fn sec_websocket_key(
    &self,
  ) -> Result<Option<HttpSecWebSocketKey>, HttpSecWebSocketKeyParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Sec-WebSocket-Key"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSecWebSocketKey::parse_values(values).map(Some)
  }

  /// Parses received W3C `traceparent` request metadata without creating
  /// identifiers, deciding sampling, or configuring propagation.
  pub fn traceparent(&self) -> Result<Option<HttpTraceParent>, HttpTraceParentParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("traceparent"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTraceParent::parse_values(values).map(Some)
  }

  /// Parses received W3C `tracestate` request metadata without invoking a
  /// tracing backend or automatically propagating metadata.
  pub fn tracestate(&self) -> Result<Option<HttpTraceState>, HttpTraceStateParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("tracestate"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTraceState::parse_values(values).map(Some)
  }

  /// Parses received W3C `baggage` request metadata without interpreting
  /// application values or automatically propagating metadata.
  pub fn baggage(&self) -> Result<Option<HttpBaggage>, HttpBaggageParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("baggage"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpBaggage::parse_values(values).map(Some)
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

  /// Parses received `Access-Control-Request-Method` preflight metadata
  /// without applying CORS policy.
  pub fn access_control_request_method(
    &self,
  ) -> Result<Option<HttpAccessControlRequestMethod>, HttpAccessControlRequestMethodParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Request-Method")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestMethod::parse_values(values).map(Some)
  }

  /// Parses received `Access-Control-Request-Headers` preflight metadata
  /// without applying CORS policy.
  pub fn access_control_request_headers(
    &self,
  ) -> Result<Option<HttpAccessControlRequestHeaders>, HttpAccessControlRequestHeadersParseError>
  {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Request-Headers")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestHeaders::parse_values(values).map(Some)
  }

  /// Parses received `Access-Control-Request-Private-Network` preflight
  /// metadata without applying Private Network Access or CORS policy.
  pub fn access_control_request_private_network(
    &self,
  ) -> Result<
    Option<HttpAccessControlRequestPrivateNetwork>,
    HttpAccessControlRequestPrivateNetworkParseError,
  > {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Access-Control-Request-Private-Network")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAccessControlRequestPrivateNetwork::parse_values(values).map(Some)
  }

  /// Parses received `Save-Data` request metadata without applying
  /// reduced-data serving or content-adaptation policy.
  pub fn save_data(&self) -> Result<Option<HttpSaveData>, HttpSaveDataParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Save-Data"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSaveData::parse_values(values).map(Some)
  }

  /// Parses received `Accept-Charset` request metadata without negotiating,
  /// transcoding, or selecting a response charset.
  pub fn accept_charset(
    &self,
  ) -> Result<Option<HttpRequestAcceptCharsets>, HttpAcceptCharsetParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Charset"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestAcceptCharsets::parse_values(values).map(Some)
  }

  /// Parses received `Sec-GPC` request metadata without applying consent,
  /// tracking, legal, or serving policy.
  pub fn sec_gpc(&self) -> Result<Option<HttpSecGpc>, HttpSecGpcParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Sec-GPC"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpSecGpc::parse_values(values).map(Some)
  }

  /// Parses received `Upgrade-Insecure-Requests` request metadata without
  /// rewriting URLs, redirecting, or enforcing Content-Security-Policy.
  pub fn upgrade_insecure_requests(
    &self,
  ) -> Result<Option<HttpUpgradeInsecureRequests>, HttpUpgradeInsecureRequestsParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| {
        header
          .name
          .eq_ignore_ascii_case("Upgrade-Insecure-Requests")
      })
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpUpgradeInsecureRequests::parse_values(values).map(Some)
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

  /// Parses received `Want-Content-Digest` request metadata without selecting
  /// an algorithm or computing a content digest.
  pub fn want_content_digest(
    &self,
  ) -> Result<Option<HttpWantContentDigest>, HttpWantContentDigestParseError> {
    parse_want_content_digest_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Want-Content-Digest"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received `Want-Repr-Digest` request metadata without selecting an
  /// algorithm or computing a representation digest.
  pub fn want_repr_digest(
    &self,
  ) -> Result<Option<HttpWantReprDigest>, HttpWantReprDigestParseError> {
    parse_want_repr_digest_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Want-Repr-Digest"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received RFC 9421 `Signature` request metadata without verifying
  /// signatures or looking up keys.
  pub fn signature(&self) -> Result<Option<HttpSignature>, HttpSignatureParseError> {
    parse_signature_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Signature"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received RFC 9421 `Signature-Input` request metadata without
  /// verifying signatures, looking up keys, or applying cryptographic policy.
  pub fn signature_input(
    &self,
  ) -> Result<Option<HttpSignatureInput>, HttpSignatureInputParseError> {
    parse_signature_input_values(
      self
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("Signature-Input"))
        .map(|header| header.value.as_str()),
    )
  }

  /// Parses received `Upgrade` protocol metadata without changing handoff
  /// validation or interpreting the upgraded protocol.
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

  /// Parses received `Pragma` request metadata without applying cache,
  /// intermediary, or HTTP/1.0 compatibility policy.
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

  /// Parses received `Content-Type` representation metadata without sniffing
  /// MIME types or interpreting the body.
  pub fn content_type(&self) -> Result<Option<HttpContentType>, HttpContentTypeParseError> {
    let mut values = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Content-Type"))
      .map(|header| header.value.as_str());
    let Some(value) = values.next() else {
      return Ok(None);
    };
    if values.next().is_some() {
      return Err(HttpContentTypeParseError::new(
        "multiple Content-Type headers",
      ));
    }
    HttpContentType::parse(value).map(Some)
  }

  /// Parses received `Content-Encoding` representation metadata without
  /// decoding or negotiating content codings.
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

  /// Parses received `Content-Language` representation metadata without
  /// selecting a locale or negotiating a variant.
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

  /// Parses received `Expect` request metadata without automatically sending
  /// an interim response or rejecting unsupported expectations.
  pub fn expectations(&self) -> Result<Option<HttpExpectations>, HttpExpectParseError> {
    let values = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Expect"))
      .map(|header| header.value.as_str());
    let values: Vec<&str> = values.collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpExpectations::parse_values(values).map(Some)
  }

  /// Parses received `TE` request metadata without enabling transfer coding,
  /// trailer negotiation, compression, or proxy behavior.
  pub fn te(&self) -> Result<Option<HttpRequestTe>, HttpTeParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("TE"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpRequestTe::parse_values(values).map(Some)
  }

  /// Parses retained HTTP/1 `Connection` header metadata without changing
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

  /// Parses retained `Transfer-Encoding` framing metadata without changing
  /// request body framing or HTTP/2 decode.
  pub fn transfer_encoding(
    &self,
  ) -> Result<Option<HttpTransferEncoding>, HttpTransferEncodingParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Transfer-Encoding"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTransferEncoding::parse_values(values).map(Some)
  }

  /// Parses announced trailer field names without waiting for or exposing a
  /// trailer block.
  pub fn trailer_header(&self) -> Result<Option<HttpTrailer>, HttpTrailerParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Trailer"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpTrailer::parse_values(values).map(Some)
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
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("Accept-Language"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpAcceptLanguages::parse_values(values).map(Some)
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

  /// Parses bounded RFC 8586 `CDN-Loop` request metadata without detecting
  /// loops, rejecting the request, or inserting a local CDN identifier.
  pub fn cdn_loop(&self) -> Result<Option<HttpCdnLoop>, HttpCdnLoopParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("CDN-Loop"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpCdnLoop::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-For` request metadata without applying a
  /// trusted-proxy policy, deriving identity, or rewriting request addresses.
  pub fn x_forwarded_for(&self) -> Result<Option<HttpXForwardedFor>, HttpXForwardedForParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("X-Forwarded-For"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedFor::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-Host` request metadata without applying a
  /// trusted-proxy policy, changing routing, or rewriting request authority.
  pub fn x_forwarded_host(
    &self,
  ) -> Result<Option<HttpXForwardedHost>, HttpXForwardedHostParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("X-Forwarded-Host"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedHost::parse_values(values).map(Some)
  }

  /// Parses bounded `X-Forwarded-Proto` request metadata without applying a
  /// trusted-proxy policy, upgrading, redirecting, or rewriting schemes.
  pub fn x_forwarded_proto(
    &self,
  ) -> Result<Option<HttpXForwardedProto>, HttpXForwardedProtoParseError> {
    let values: Vec<&str> = self
      .headers
      .iter()
      .filter(|header| header.name.eq_ignore_ascii_case("X-Forwarded-Proto"))
      .map(|header| header.value.as_str())
      .collect();
    if values.is_empty() {
      return Ok(None);
    }
    HttpXForwardedProto::parse_values(values).map(Some)
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
