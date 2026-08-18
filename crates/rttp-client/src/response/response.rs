use std::collections::HashSet;
use std::fmt;
use std::time::SystemTime;

use httpdate::parse_http_date;
use url::Url;

use crate::error;
use crate::response::raw_response::RawResponse;
use crate::response::AltSvc;
use crate::response::ContentDigest;
use crate::response::Digest;
use crate::response::Priority;
use crate::response::ProxyAuthenticationInfo;
use crate::response::ReprDigest;
use crate::response::ServerTiming;
use crate::response::Trailer;
use crate::response::Warning;
use crate::response::WwwAuthenticate;
use crate::types::{Cookie, Header, RoUrl, StatusCode};
use rttp_protocol::access_control_allow_headers::AccessControlAllowHeaders;
use rttp_protocol::access_control_allow_methods::AccessControlAllowMethods;
use rttp_protocol::access_control_allow_origin::AccessControlAllowOrigin;
use rttp_protocol::access_control_expose_headers::AccessControlExposeHeaders;
use rttp_protocol::access_control_max_age::AccessControlMaxAge;
use rttp_protocol::allow as protocol_allow;
use rttp_protocol::clear_site_data::ClearSiteData;
use rttp_protocol::client_hints::{AcceptCh, CriticalCh};
use rttp_protocol::cookie::HttpSetCookies;
use rttp_protocol::cross_origin_embedder_policy::CrossOriginEmbedderPolicy;
use rttp_protocol::cross_origin_embedder_policy_report_only::CrossOriginEmbedderPolicyReportOnly;
use rttp_protocol::cross_origin_opener_policy::CrossOriginOpenerPolicy;
use rttp_protocol::cross_origin_resource_policy::CrossOriginResourcePolicy;
use rttp_protocol::prefer::PreferenceApplied;
use rttp_protocol::referrer_policy::ReferrerPolicy;
use rttp_protocol::strict_transport_security::StrictTransportSecurity;
use rttp_protocol::sunset::parse_sunset_values;
use rttp_protocol::timing_allow_origin::TimingAllowOrigin;

const MAX_CACHE_CONTROL_VALUE_BYTES: usize = 64 * 1024;
const MAX_CACHE_CONTROL_DIRECTIVES: usize = 256;
const MAX_ACCEPT_RANGES_VALUE_BYTES: usize = 64 * 1024;
const MAX_ACCEPT_RANGE_UNITS: usize = 256;
const MAX_ACCEPT_MEDIA_TYPES: usize = 256;
const MAX_DATE_VALUE_BYTES: usize = 64 * 1024;
const MAX_RETRY_AFTER_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_TYPE_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_TYPE_PARAMETERS: usize = 256;
const MAX_CONTENT_LOCATION_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_DISPOSITION_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_DISPOSITION_PARAMETERS: usize = 256;
const MAX_CONTENT_ENCODING_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_ENCODINGS: usize = 256;
const MAX_CONTENT_LANGUAGE_VALUE_BYTES: usize = 64 * 1024;
const MAX_CONTENT_LANGUAGE_TAGS: usize = 256;
const MAX_VARY_VALUE_BYTES: usize = 64 * 1024;
const MAX_VARY_FIELD_NAMES: usize = 256;
const MAX_LINK_VALUE_BYTES: usize = 64 * 1024;
const MAX_LINK_VALUES: usize = 256;
const MAX_LINK_PARAMETERS: usize = 256;
const MAX_LINK_PARAMETER_VALUE_BYTES: usize = 64 * 1024;
const MAX_REPORTING_ENDPOINTS_VALUE_BYTES: usize = 64 * 1024;
const MAX_REPORTING_ENDPOINTS: usize = 256;

#[derive(Clone)]
pub struct Response {
  raw: RawResponse,
  informational_responses: Vec<InformationalResponse>,
}

impl Response {
  pub fn new(url: RoUrl, binary: Vec<u8>) -> error::Result<Self> {
    Ok(Self {
      raw: RawResponse::new(url, binary)?,
      informational_responses: Vec::new(),
    })
  }

  pub(crate) fn with_trailers(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
  ) -> error::Result<Self> {
    Self::with_trailers_and_informational(url, binary, trailers, Vec::new())
  }

  pub(crate) fn with_trailers_and_limit(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
    max_body_bytes: usize,
  ) -> error::Result<Self> {
    Self::with_trailers_and_informational_and_limit(
      url,
      binary,
      trailers,
      Vec::new(),
      max_body_bytes,
    )
  }

  pub(crate) fn with_trailers_and_informational(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
    informational_responses: Vec<InformationalResponse>,
  ) -> error::Result<Self> {
    Self::with_trailers_and_informational_and_limit(
      url,
      binary,
      trailers,
      informational_responses,
      crate::config::DEFAULT_MAX_BUFFERED_RESPONSE_BODY_BYTES,
    )
  }

  pub(crate) fn with_trailers_and_informational_and_limit(
    url: RoUrl,
    binary: Vec<u8>,
    trailers: Vec<Header>,
    informational_responses: Vec<InformationalResponse>,
    max_body_bytes: usize,
  ) -> error::Result<Self> {
    Ok(Self {
      raw: RawResponse::with_trailers_and_limit(url, binary, trailers, max_body_bytes)?,
      informational_responses,
    })
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InformationalResponse {
  code: u16,
  reason: String,
  headers: Vec<Header>,
}

impl InformationalResponse {
  pub(crate) fn new(code: u16, reason: String, headers: Vec<Header>) -> Self {
    Self {
      code,
      reason,
      headers,
    }
  }

  pub fn code(&self) -> u16 {
    self.code
  }
  /// Returns the status code; panics if the stored code is not a valid HTTP status code.
  pub fn status(&self) -> StatusCode {
    StatusCode::from_u16(self.code()).expect("response code is validated")
  }

  pub fn reason(&self) -> &String {
    &self.reason
  }

  pub fn headers(&self) -> &Vec<Header> {
    &self.headers
  }

  pub fn headers_of_name<S: AsRef<str>>(&self, name: S) -> Vec<&Header> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .collect()
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .headers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn header_values<S: AsRef<str>>(&self, name: S) -> Vec<&String> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value())
      .collect()
  }

  pub fn header_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.header(name).map(|header| header.value())
  }
}

impl Response {
  fn classified_status(&self) -> Option<StatusCode> {
    u16::try_from(self.code())
      .ok()
      .and_then(|code| StatusCode::from_u16(code).ok())
  }

  pub fn ok(&self) -> bool {
    self.code() == 200
  }

  pub fn is_success(&self) -> bool {
    self
      .classified_status()
      .is_some_and(|code| code.is_success())
  }

  pub fn is_client_error(&self) -> bool {
    self
      .classified_status()
      .is_some_and(|code| code.is_client_error())
  }

  pub fn is_server_error(&self) -> bool {
    self
      .classified_status()
      .is_some_and(|code| code.is_server_error())
  }

  pub fn is_informational(&self) -> bool {
    self
      .classified_status()
      .is_some_and(|code| code.is_informational())
  }

  pub fn is_redirection(&self) -> bool {
    self
      .classified_status()
      .is_some_and(|code| code.is_redirection())
  }

  pub fn is_error(&self) -> bool {
    self.classified_status().is_some_and(|code| code.is_error())
  }

  pub fn is_partial_content(&self) -> bool {
    self.code() == 206
  }

  pub fn is_range_not_satisfiable(&self) -> bool {
    self.code() == 416
  }

  pub fn is_too_early(&self) -> bool {
    self.code() == 425
  }

  pub fn is_not_extended(&self) -> bool {
    self.code() == 510
  }

  pub fn is_network_authentication_required(&self) -> bool {
    self.code() == 511
  }

  pub fn is_not_modified(&self) -> bool {
    self.code() == 304
  }

  pub fn is_precondition_failed(&self) -> bool {
    self.code() == 412
  }

  pub fn is_redirect(&self) -> bool {
    matches!(self.code(), 301 | 302 | 303 | 307 | 308)
  }

  pub fn code(&self) -> u32 {
    self.raw.code_get()
  }

  pub fn version(&self) -> &String {
    self.raw.version_get()
  }

  pub fn reason(&self) -> &String {
    self.raw.reason_get()
  }

  fn url(&self) -> &Url {
    self.raw.url_get()
  }

  pub fn host(&self) -> &str {
    self.url().host_str().unwrap_or_default()
  }

  pub fn body(&self) -> &ResponseBody {
    self.raw.body_get()
  }

  pub fn binary(&self) -> &[u8] {
    self.raw.binary_get()
  }

  pub fn location(&self) -> Option<&String> {
    self.header_value("location")
  }

  pub fn etag(&self) -> Option<&String> {
    self.header_value("etag")
  }

  pub fn last_modified(&self) -> Option<&String> {
    self.header_value("last-modified")
  }

  pub fn date(&self) -> error::Result<Option<SystemTime>> {
    let values = self.header_values("date");
    match values.as_slice() {
      [] => Ok(None),
      [value] => {
        if value.len() > MAX_DATE_VALUE_BYTES {
          return Err(error::bad_response("Date header value is too large"));
        }
        parse_http_date(value)
          .map(Some)
          .map_err(|_| error::bad_response("Invalid Date HTTP-date"))
      }
      _ => Err(error::bad_response("Duplicate Date header values")),
    }
  }

  pub fn age(&self) -> error::Result<Option<u64>> {
    self
      .header_value("age")
      .map(|value| parse_age_delta_seconds(value).map(Some))
      .unwrap_or(Ok(None))
  }

  pub fn expires(&self) -> error::Result<Option<SystemTime>> {
    self
      .header_value("expires")
      .map(|value| {
        parse_http_date(value)
          .map(Some)
          .map_err(|_| error::bad_response("Invalid Expires HTTP-date"))
      })
      .unwrap_or(Ok(None))
  }

  pub fn sunset_value(&self) -> Option<&String> {
    self.header_value("sunset")
  }

  pub fn sunset(&self) -> error::Result<Option<SystemTime>> {
    parse_sunset_values(self.header_values("sunset").into_iter().map(String::as_str))
      .map_err(|err| error::bad_response(err.to_string()))
  }

  pub fn retry_after(&self) -> error::Result<Option<RetryAfter>> {
    let values = self.header_values("retry-after");
    match values.as_slice() {
      [] => Ok(None),
      [value] => RetryAfter::parse(value).map(Some),
      _ => Err(error::bad_response("Duplicate Retry-After header values")),
    }
  }

  pub fn allow(&self) -> error::Result<Option<Allow>> {
    let values = self.header_values("allow");
    if values.is_empty() {
      return Ok(None);
    }
    Allow::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  pub fn accept_ranges(&self) -> error::Result<Option<AcceptRanges>> {
    let values = self.header_values("accept-ranges");
    if values.is_empty() {
      return Ok(None);
    }
    AcceptRanges::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses `Accept-Patch` media-type metadata without selecting a request
  /// method or sending a follow-up request.
  pub fn accept_patch(&self) -> error::Result<Option<AcceptPatch>> {
    let values = self.header_values("accept-patch");
    if values.is_empty() {
      return Ok(None);
    }
    AcceptPatch::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses `Accept-Post` media-type metadata without choosing an upload or
  /// sending a follow-up request.
  pub fn accept_post(&self) -> error::Result<Option<AcceptPost>> {
    let values = self.header_values("accept-post");
    if values.is_empty() {
      return Ok(None);
    }
    AcceptPost::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses bounded `Accept-CH` response metadata without applying Client Hints policy.
  pub fn accept_ch(&self) -> error::Result<Option<AcceptCh>> {
    let values = self.header_values("accept-ch");
    if values.is_empty() {
      return Ok(None);
    }
    AcceptCh::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Critical-CH` response metadata without retrying requests.
  pub fn critical_ch(&self) -> error::Result<Option<CriticalCh>> {
    let values = self.header_values("critical-ch");
    if values.is_empty() {
      return Ok(None);
    }
    CriticalCh::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Timing-Allow-Origin` response metadata without applying timing policy.
  pub fn timing_allow_origin(&self) -> error::Result<Option<TimingAllowOrigin>> {
    let values = self.header_values("timing-allow-origin");
    if values.is_empty() {
      return Ok(None);
    }
    TimingAllowOrigin::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Access-Control-Expose-Headers` response metadata without
  /// applying CORS exposure policy.
  pub fn access_control_expose_headers(&self) -> error::Result<Option<AccessControlExposeHeaders>> {
    let values = self.header_values("access-control-expose-headers");
    if values.is_empty() {
      return Ok(None);
    }
    AccessControlExposeHeaders::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Access-Control-Allow-Headers` response metadata without
  /// applying CORS header policy.
  pub fn access_control_allow_headers(&self) -> error::Result<Option<AccessControlAllowHeaders>> {
    let values = self.header_values("access-control-allow-headers");
    if values.is_empty() {
      return Ok(None);
    }
    AccessControlAllowHeaders::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Access-Control-Allow-Methods` response metadata without
  /// applying CORS method policy.
  pub fn access_control_allow_methods(&self) -> error::Result<Option<AccessControlAllowMethods>> {
    let values = self.header_values("access-control-allow-methods");
    if values.is_empty() {
      return Ok(None);
    }
    AccessControlAllowMethods::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Access-Control-Allow-Origin` response metadata without
  /// applying CORS origin policy.
  pub fn access_control_allow_origin(&self) -> error::Result<Option<AccessControlAllowOrigin>> {
    let values = self.header_values("access-control-allow-origin");
    if values.is_empty() {
      return Ok(None);
    }
    AccessControlAllowOrigin::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Access-Control-Max-Age` response metadata without applying CORS caching.
  pub fn access_control_max_age(&self) -> error::Result<Option<AccessControlMaxAge>> {
    let values = self.header_values("access-control-max-age");
    if values.is_empty() {
      return Ok(None);
    }
    AccessControlMaxAge::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses `Preference-Applied` response metadata without applying preference semantics.
  pub fn preference_applied(&self) -> error::Result<Option<PreferenceApplied>> {
    let values = self.header_values("preference-applied");
    if values.is_empty() {
      return Ok(None);
    }
    PreferenceApplied::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Referrer-Policy` response metadata without changing
  /// outbound Referer behavior or redirect policy.
  pub fn referrer_policy(&self) -> error::Result<Option<ReferrerPolicy>> {
    let values = self.header_values("referrer-policy");
    if values.is_empty() {
      return Ok(None);
    }
    ReferrerPolicy::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses bounded `Strict-Transport-Security` response metadata without applying HSTS policy.
  pub fn strict_transport_security(&self) -> error::Result<Option<StrictTransportSecurity>> {
    let values = self.header_values("strict-transport-security");
    if values.is_empty() {
      return Ok(None);
    }
    StrictTransportSecurity::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  pub fn content_range(&self) -> Option<ContentRange> {
    self
      .header_value("content-range")
      .and_then(ContentRange::parse)
  }

  pub fn content_type(&self) -> error::Result<Option<ContentType>> {
    let values = self.header_values("content-type");
    match values.as_slice() {
      [] => Ok(None),
      [value] => ContentType::parse(value).map(Some),
      _ => Err(error::bad_response("Duplicate Content-Type header values")),
    }
  }

  pub fn content_location(&self) -> error::Result<Option<ContentLocation>> {
    let values = self.header_values("content-location");
    match values.as_slice() {
      [] => Ok(None),
      [value] => ContentLocation::parse(value).map(Some),
      _ => Err(error::bad_response(
        "Duplicate Content-Location header values",
      )),
    }
  }

  pub fn content_disposition(&self) -> error::Result<Option<ContentDisposition>> {
    let values = self.header_values("content-disposition");
    match values.as_slice() {
      [] => Ok(None),
      [value] => ContentDisposition::parse(value).map(Some),
      _ => Err(error::bad_response(
        "Duplicate Content-Disposition header values",
      )),
    }
  }

  pub fn content_language(&self) -> error::Result<Option<ContentLanguage>> {
    let values = self.header_values("content-language");
    if values.is_empty() {
      return Ok(None);
    }
    ContentLanguage::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses bounded `Reporting-Endpoints` response metadata without sending reports.
  pub fn reporting_endpoints(&self) -> error::Result<Option<ReportingEndpoints>> {
    let values = self.header_values("reporting-endpoints");
    if values.is_empty() {
      return Ok(None);
    }
    ReportingEndpoints::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  pub fn content_encoding(&self) -> error::Result<Option<ContentEncoding>> {
    let values = self.header_values("content-encoding");
    if values.is_empty() {
      return Ok(None);
    }
    ContentEncoding::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses all `WWW-Authenticate` fields as bounded authentication challenge metadata.
  pub fn www_authenticate(&self) -> error::Result<Option<WwwAuthenticate>> {
    let values = self.header_values("www-authenticate");
    if values.is_empty() {
      return Ok(None);
    }
    WwwAuthenticate::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Proxy-Authentication-Info` fields as bounded auth-param
  /// metadata without verifying `rspauth` or generating `Proxy-Authorization`.
  pub fn proxy_authentication_info(&self) -> error::Result<Option<ProxyAuthenticationInfo>> {
    let values = self.header_values("proxy-authentication-info");
    if values.is_empty() {
      return Ok(None);
    }
    ProxyAuthenticationInfo::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Content-Digest` fields as bounded response metadata.
  pub fn digest(&self) -> error::Result<Option<Digest>> {
    self.digest_field("content-digest")
  }

  /// Parses all `Content-Digest` fields as bounded response metadata without
  /// verifying hashes or selecting an algorithm.
  pub fn content_digest(&self) -> error::Result<Option<ContentDigest>> {
    self.digest_field("content-digest")
  }

  /// Parses all `Repr-Digest` fields as bounded response metadata.
  pub fn repr_digest(&self) -> error::Result<Option<ReprDigest>> {
    self.digest_field("repr-digest")
  }

  fn digest_field(&self, name: &str) -> error::Result<Option<Digest>> {
    let values = self.header_values(name);
    if values.is_empty() {
      return Ok(None);
    }
    Digest::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Priority` fields as bounded RFC 9218 metadata.
  pub fn priority(&self) -> error::Result<Option<Priority>> {
    let values = self.header_values("priority");
    if values.is_empty() {
      return Ok(None);
    }
    Priority::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Server-Timing` fields as bounded response timing metadata.
  pub fn server_timing(&self) -> error::Result<Option<ServerTiming>> {
    let values = self.header_values("server-timing");
    if values.is_empty() {
      return Ok(None);
    }
    ServerTiming::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Warning` fields as bounded RFC 7234 warning-value metadata.
  /// This does not change cache freshness, stale-response handling, or
  /// response-acceptance policy.
  pub fn warning(&self) -> error::Result<Option<Warning>> {
    let values = self.header_values("warning");
    if values.is_empty() {
      return Ok(None);
    }
    Warning::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses all `Alt-Svc` fields as bounded alternative-service metadata.
  /// This does not migrate connections or select an alternative endpoint.
  pub fn alt_svc(&self) -> error::Result<Option<AltSvc>> {
    let values = self.header_values("alt-svc");
    if values.is_empty() {
      return Ok(None);
    }
    AltSvc::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  pub fn cache_control(&self) -> error::Result<Option<CacheControl>> {
    let values = self.header_values("cache-control");
    if values.is_empty() {
      return Ok(None);
    }
    CacheControl::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses `Clear-Site-Data` response metadata without clearing any client state.
  pub fn clear_site_data(&self) -> error::Result<Option<ClearSiteData>> {
    let values = self.header_values("clear-site-data");
    if values.is_empty() {
      return Ok(None);
    }
    ClearSiteData::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses `Cross-Origin-Resource-Policy` response metadata without enforcing resource isolation.
  pub fn cross_origin_resource_policy(&self) -> error::Result<Option<CrossOriginResourcePolicy>> {
    let values = self.header_values("cross-origin-resource-policy");
    if values.is_empty() {
      return Ok(None);
    }
    CrossOriginResourcePolicy::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses `Cross-Origin-Embedder-Policy` response metadata without enforcing embedder policy.
  pub fn cross_origin_embedder_policy(&self) -> error::Result<Option<CrossOriginEmbedderPolicy>> {
    let values = self.header_values("cross-origin-embedder-policy");
    if values.is_empty() {
      return Ok(None);
    }
    CrossOriginEmbedderPolicy::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses `Cross-Origin-Embedder-Policy-Report-Only` response metadata without
  /// enforcing embedder policy or scheduling reports.
  pub fn cross_origin_embedder_policy_report_only(
    &self,
  ) -> error::Result<Option<CrossOriginEmbedderPolicyReportOnly>> {
    let values = self.header_values("cross-origin-embedder-policy-report-only");
    if values.is_empty() {
      return Ok(None);
    }
    CrossOriginEmbedderPolicyReportOnly::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  /// Parses `Cross-Origin-Opener-Policy` response metadata without enforcing opener policy.
  pub fn cross_origin_opener_policy(&self) -> error::Result<Option<CrossOriginOpenerPolicy>> {
    let values = self.header_values("cross-origin-opener-policy");
    if values.is_empty() {
      return Ok(None);
    }
    CrossOriginOpenerPolicy::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  pub fn vary(&self) -> error::Result<Option<Vary>> {
    let values = self.header_values("vary");
    if values.is_empty() {
      return Ok(None);
    }
    Vary::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses announced trailer field names without waiting for or exposing a
  /// trailer block.
  pub fn trailer_header(&self) -> error::Result<Option<Trailer>> {
    let values = self.header_values("trailer");
    if values.is_empty() {
      return Ok(None);
    }
    Trailer::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|error| error::bad_response(error.to_string()))
  }

  /// Parses `Link` response metadata without enabling preload, redirects,
  /// caching, or fetch scheduling.
  pub fn links(&self) -> error::Result<Option<LinkValues>> {
    let values = self.header_values("link");
    if values.is_empty() {
      return Ok(None);
    }
    LinkValues::parse_values(values.into_iter().map(String::as_str)).map(Some)
  }

  /// Parses response `Set-Cookie` fields as bounded opaque metadata without
  /// creating a cookie jar or applying storage and matching policy.
  pub fn set_cookies(&self) -> error::Result<Option<HttpSetCookies>> {
    let values = self.header_values("set-cookie");
    if values.is_empty() {
      return Ok(None);
    }
    HttpSetCookies::parse_values(values.into_iter().map(String::as_str))
      .map(Some)
      .map_err(|parse_error| error::bad_response(parse_error.to_string()))
  }

  pub fn headers(&self) -> &Vec<Header> {
    self.raw.headers_get()
  }

  pub fn trailers(&self) -> &Vec<Header> {
    self.raw.trailers_get()
  }

  pub fn informational_responses(&self) -> &[InformationalResponse] {
    &self.informational_responses
  }

  pub fn early_hints(&self) -> Vec<&InformationalResponse> {
    self
      .informational_responses()
      .iter()
      .filter(|response| response.code() == 103)
      .collect()
  }

  pub fn headers_of_name<S: AsRef<str>>(&self, name: S) -> Vec<&Header> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .collect()
  }

  pub fn header<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .headers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn header_values<S: AsRef<str>>(&self, name: S) -> Vec<&String> {
    self
      .headers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value())
      .collect()
  }

  pub fn header_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.header(name).map(|header| header.value())
  }

  pub fn trailers_of_name<S: AsRef<str>>(&self, name: S) -> Vec<&Header> {
    self
      .trailers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .collect()
  }

  pub fn trailer<S: AsRef<str>>(&self, name: S) -> Option<&Header> {
    self
      .trailers()
      .iter()
      .find(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn trailer_values<S: AsRef<str>>(&self, name: S) -> Vec<&String> {
    self
      .trailers()
      .iter()
      .filter(|header| header.name().eq_ignore_ascii_case(name.as_ref()))
      .map(|header| header.value())
      .collect()
  }

  pub fn trailer_value<S: AsRef<str>>(&self, name: S) -> Option<&String> {
    self.trailer(name).map(|header| header.value())
  }

  pub fn cookies(&self) -> &Vec<Cookie> {
    self.raw.cookies_get()
  }

  pub fn cookie<S: AsRef<str>>(&self, name: S) -> Option<&Cookie> {
    self
      .cookies()
      .iter()
      .find(|cookie| cookie.name().eq_ignore_ascii_case(name.as_ref()))
  }
}

impl fmt::Debug for Response {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&self.raw, formatter)
  }
}

impl fmt::Display for Response {
  #[inline]
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    fmt::Display::fmt(&self.raw, formatter)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allow {
  inner: protocol_allow::Allow,
}

impl Allow {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    protocol_allow::Allow::parse_values(values)
      .map(|inner| Self { inner })
      .map_err(|err| error::bad_response(err.to_string()))
  }

  pub fn methods(&self) -> Vec<&str> {
    self.inner.methods()
  }

  pub fn contains_method(&self, method: impl AsRef<str>) -> bool {
    self.inner.contains_method(method)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptRanges {
  units: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPatch {
  media_types: Vec<ContentType>,
}

impl AcceptPatch {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      media_types: parse_accepted_media_types(values, "Accept-Patch")?,
    })
  }

  pub fn media_types(&self) -> &[ContentType] {
    &self.media_types
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptPost {
  media_types: Vec<ContentType>,
}

impl AcceptPost {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    Ok(Self {
      media_types: parse_accepted_media_types(values, "Accept-Post")?,
    })
  }

  pub fn media_types(&self) -> &[ContentType] {
    &self.media_types
  }
}

fn parse_accepted_media_types<'a, I>(values: I, header: &str) -> error::Result<Vec<ContentType>>
where
  I: IntoIterator<Item = &'a str>,
{
  let mut media_types = Vec::new();

  for value in values {
    if value.len() > MAX_CONTENT_TYPE_VALUE_BYTES {
      return Err(error::bad_response(format!(
        "{header} header value is too large"
      )));
    }
    for member in split_accepted_media_type_members(value)? {
      if media_types.len() >= MAX_ACCEPT_MEDIA_TYPES {
        return Err(error::bad_response(format!(
          "Too many {header} media types"
        )));
      }
      media_types.push(ContentType::parse(member)?);
    }
  }

  if media_types.is_empty() {
    return Err(error::bad_response(format!("Invalid {header} media type")));
  }
  Ok(media_types)
}

impl AcceptRanges {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut units = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
      if value.len() > MAX_ACCEPT_RANGES_VALUE_BYTES {
        return Err(error::bad_response(
          "Accept-Ranges header value is too large",
        ));
      }

      for unit in value.split(',') {
        let unit = unit.trim();
        if unit.is_empty() || !is_token(unit) {
          return Err(error::bad_response("Invalid Accept-Ranges range-unit"));
        }
        if units.len() >= MAX_ACCEPT_RANGE_UNITS {
          return Err(error::bad_response("Too many Accept-Ranges range-units"));
        }

        let normalized = unit.to_ascii_lowercase();
        if !seen.insert(normalized.clone()) {
          return Err(error::bad_response("Duplicate Accept-Ranges range-unit"));
        }
        units.push(normalized);
      }
    }

    if units.is_empty() {
      return Err(error::bad_response("Invalid Accept-Ranges range-unit"));
    }
    if units.iter().any(|unit| unit == "none") && units.len() != 1 {
      return Err(error::bad_response(
        "Accept-Ranges none cannot be combined with range-units",
      ));
    }

    Ok(Self { units })
  }

  pub fn units(&self) -> Vec<&str> {
    self.units.iter().map(String::as_str).collect()
  }

  pub fn is_none(&self) -> bool {
    self.units.as_slice() == ["none"]
  }

  pub fn accepts_bytes(&self) -> bool {
    self.units.iter().any(|unit| unit == "bytes")
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetryAfter {
  DeltaSeconds(u64),
  HttpDate(SystemTime),
}

impl RetryAfter {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    let value = value.as_ref();
    if value.len() > MAX_RETRY_AFTER_VALUE_BYTES {
      return Err(error::bad_response("Retry-After header value is too large"));
    }
    let value = value.trim();
    if value.is_empty() {
      return Err(error::bad_response("Invalid Retry-After value"));
    }
    if value.chars().all(|ch| ch.is_ascii_digit()) {
      return value
        .parse::<u64>()
        .map(Self::DeltaSeconds)
        .map_err(|_| error::bad_response("Invalid Retry-After delta-seconds"));
    }
    parse_http_date(value)
      .map(Self::HttpDate)
      .map_err(|_| error::bad_response("Invalid Retry-After value"))
  }

  pub fn delta_seconds(&self) -> Option<u64> {
    match self {
      Self::DeltaSeconds(delta_seconds) => Some(*delta_seconds),
      Self::HttpDate(_) => None,
    }
  }

  pub fn http_date(&self) -> Option<SystemTime> {
    match self {
      Self::DeltaSeconds(_) => None,
      Self::HttpDate(http_date) => Some(*http_date),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentRange {
  unit: String,
  start: Option<u64>,
  end: Option<u64>,
  complete_length: Option<u64>,
}

impl ContentRange {
  pub fn parse(value: impl AsRef<str>) -> Option<Self> {
    let value = value.as_ref().trim();
    let (unit, range_and_length) = value.split_once(' ')?;
    if unit.is_empty() {
      return None;
    }

    let (range, complete_length) = range_and_length.split_once('/')?;
    let complete_length = parse_complete_length(complete_length)?;
    if range == "*" {
      return Some(Self {
        unit: unit.to_string(),
        start: None,
        end: None,
        complete_length,
      });
    }

    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    if start > end {
      return None;
    }

    Some(Self {
      unit: unit.to_string(),
      start: Some(start),
      end: Some(end),
      complete_length,
    })
  }

  pub fn unit(&self) -> &str {
    &self.unit
  }

  pub fn start(&self) -> Option<u64> {
    self.start
  }

  pub fn end(&self) -> Option<u64> {
    self.end
  }

  pub fn complete_length(&self) -> Option<u64> {
    self.complete_length
  }

  pub fn is_unsatisfied(&self) -> bool {
    self.start.is_none() && self.end.is_none()
  }
}

fn parse_complete_length(value: &str) -> Option<Option<u64>> {
  if value == "*" {
    return Some(None);
  }
  value.parse::<u64>().ok().map(Some)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentType {
  type_: String,
  subtype: String,
  parameters: Vec<ContentTypeParameter>,
}

impl ContentType {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    let value = value.as_ref();
    if value.len() > MAX_CONTENT_TYPE_VALUE_BYTES {
      return Err(error::bad_response(
        "Content-Type header value is too large",
      ));
    }
    if value.contains(['\r', '\n']) {
      return Err(error::bad_response("Invalid Content-Type value"));
    }

    let members = split_content_type_members(value)?;
    let Some(media_type) = members.first().map(|member| member.trim()) else {
      return Err(error::bad_response("Invalid Content-Type media type"));
    };
    let (type_, subtype) = media_type
      .split_once('/')
      .ok_or_else(|| error::bad_response("Invalid Content-Type media type"))?;
    let type_ = type_.trim();
    let subtype = subtype.trim();
    if !is_token(type_) || !is_token(subtype) {
      return Err(error::bad_response("Invalid Content-Type media type"));
    }

    let mut parameters = Vec::new();
    let mut seen = HashSet::new();
    for member in members.iter().skip(1) {
      if parameters.len() >= MAX_CONTENT_TYPE_PARAMETERS {
        return Err(error::bad_response("Too many Content-Type parameters"));
      }

      let parameter = ContentTypeParameter::parse(member)?;
      let normalized = parameter.name.to_ascii_lowercase();
      if !seen.insert(normalized) {
        return Err(error::bad_response("Duplicate Content-Type parameter"));
      }
      parameters.push(parameter);
    }

    Ok(Self {
      type_: type_.to_ascii_lowercase(),
      subtype: subtype.to_ascii_lowercase(),
      parameters,
    })
  }

  pub fn type_(&self) -> &str {
    &self.type_
  }

  pub fn subtype(&self) -> &str {
    &self.subtype
  }

  pub fn essence(&self) -> String {
    format!("{}/{}", self.type_, self.subtype)
  }

  pub fn is(&self, type_: impl AsRef<str>, subtype: impl AsRef<str>) -> bool {
    self.type_.eq_ignore_ascii_case(type_.as_ref())
      && self.subtype.eq_ignore_ascii_case(subtype.as_ref())
  }

  pub fn parameters(&self) -> &[ContentTypeParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(ContentTypeParameter::value)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentTypeParameter {
  name: String,
  value: String,
}

impl ContentTypeParameter {
  fn parse(value: &str) -> error::Result<Self> {
    let (name, raw_value) = value
      .split_once('=')
      .ok_or_else(|| error::bad_response("Invalid Content-Type parameter"))?;
    let name = name.trim();
    let raw_value = raw_value.trim();
    if !is_token(name) {
      return Err(error::bad_response("Invalid Content-Type parameter name"));
    }

    let parsed_value = parse_content_type_parameter_value(raw_value)?;
    Ok(Self {
      name: name.to_ascii_lowercase(),
      value: parsed_value,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

fn split_content_type_members(value: &str) -> error::Result<Vec<String>> {
  let mut members = Vec::new();
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
      ';' if !in_quote => {
        push_content_type_member(&mut members, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(error::bad_response("Malformed Content-Type quoted-string"));
  }
  push_content_type_member(&mut members, &current)?;
  Ok(members)
}

fn split_accepted_media_type_members(value: &str) -> error::Result<Vec<String>> {
  let mut members = Vec::new();
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
        push_accepted_media_type_member(&mut members, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(error::bad_response("Invalid accepted media type"));
  }
  push_accepted_media_type_member(&mut members, &current)?;
  Ok(members)
}

fn push_accepted_media_type_member(members: &mut Vec<String>, current: &str) -> error::Result<()> {
  let member = current.trim();
  if member.is_empty() {
    return Err(error::bad_response("Invalid accepted media type"));
  }
  members.push(member.to_string());
  Ok(())
}

fn push_content_type_member(members: &mut Vec<String>, member: &str) -> error::Result<()> {
  let member = member.trim();
  if member.is_empty() {
    return Err(error::bad_response("Invalid Content-Type member"));
  }
  members.push(member.to_string());
  Ok(())
}

fn parse_content_type_parameter_value(value: &str) -> error::Result<String> {
  if value.is_empty() {
    return Err(error::bad_response("Invalid Content-Type parameter value"));
  }
  if let Some(value) = value.strip_prefix('"') {
    return parse_content_type_quoted_string(value);
  }
  if value.contains('"') || !is_token(value) {
    return Err(error::bad_response("Invalid Content-Type parameter value"));
  }
  Ok(value.to_string())
}

fn parse_content_type_quoted_string(value: &str) -> error::Result<String> {
  let mut chars = value.chars();
  let mut parsed = String::new();
  let mut closed = false;

  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(error::bad_response("Malformed Content-Type quoted-string"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(error::bad_response("Malformed Content-Type quoted-string"));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => return Err(error::bad_response("Malformed Content-Type quoted-string")),
    }
  }

  if !closed || chars.any(|ch| !ch.is_ascii_whitespace()) {
    return Err(error::bad_response("Malformed Content-Type quoted-string"));
  }
  Ok(parsed)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLocation {
  value: String,
}

impl ContentLocation {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    let value = value.as_ref();
    if value.len() > MAX_CONTENT_LOCATION_VALUE_BYTES {
      return Err(error::bad_response(
        "Content-Location header value is too large",
      ));
    }

    let value = trim_http_optional_whitespace(value);
    if value.is_empty() {
      return Err(error::bad_response("Invalid Content-Location value"));
    }
    if !is_content_location_field_value(value) {
      return Err(error::bad_response("Invalid Content-Location value"));
    }

    if Url::parse(value).is_ok() {
      return Ok(Self {
        value: value.to_string(),
      });
    }

    let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
    if !is_relative_uri_reference_field_value(value) {
      return Err(error::bad_response("Invalid Content-Location value"));
    }
    Url::options()
      .base_url(Some(&base))
      .parse(value)
      .map_err(|_| error::bad_response("Invalid Content-Location value"))?;

    Ok(Self {
      value: value.to_string(),
    })
  }

  pub fn as_str(&self) -> &str {
    &self.value
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDisposition {
  disposition_type: String,
  parameters: Vec<ContentDispositionParameter>,
}

impl ContentDisposition {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    let value = value.as_ref();
    if value.len() > MAX_CONTENT_DISPOSITION_VALUE_BYTES {
      return Err(error::bad_response(
        "Content-Disposition header value is too large",
      ));
    }
    if value.contains(['\r', '\n']) {
      return Err(error::bad_response("Invalid Content-Disposition value"));
    }

    let members = split_content_disposition_members(value)?;
    let Some(disposition_type) = members.first().map(|member| member.trim()) else {
      return Err(error::bad_response(
        "Invalid Content-Disposition disposition type",
      ));
    };
    if !is_token(disposition_type) {
      return Err(error::bad_response(
        "Invalid Content-Disposition disposition type",
      ));
    }

    let mut parameters = Vec::new();
    let mut seen = HashSet::new();
    for member in members.iter().skip(1) {
      if parameters.len() >= MAX_CONTENT_DISPOSITION_PARAMETERS {
        return Err(error::bad_response(
          "Too many Content-Disposition parameters",
        ));
      }

      let parameter = ContentDispositionParameter::parse(member)?;
      let normalized = parameter.name.to_ascii_lowercase();
      if !seen.insert(normalized) {
        return Err(error::bad_response(
          "Duplicate Content-Disposition parameter",
        ));
      }
      parameters.push(parameter);
    }

    Ok(Self {
      disposition_type: disposition_type.to_string(),
      parameters,
    })
  }

  pub fn disposition_type(&self) -> &str {
    &self.disposition_type
  }

  pub fn parameters(&self) -> &[ContentDispositionParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&ContentDispositionParameter> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
  }

  pub fn filename(&self) -> Option<&str> {
    self
      .parameter("filename")
      .map(ContentDispositionParameter::value)
  }

  pub fn filename_ext(&self) -> Option<&str> {
    self
      .parameter("filename*")
      .map(ContentDispositionParameter::value)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDispositionParameter {
  name: String,
  value: String,
}

/// Bounded `Link` response metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkValues {
  values: Vec<LinkValue>,
}

impl LinkValues {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut parsed = Vec::new();
    for value in values {
      if value.len() > MAX_LINK_VALUE_BYTES {
        return Err(error::bad_response("Link header value is too large"));
      }
      for member in split_link_values(value)? {
        if parsed.len() >= MAX_LINK_VALUES {
          return Err(error::bad_response("Too many Link values"));
        }
        parsed.push(LinkValue::parse_member(&member)?);
      }
    }
    if parsed.is_empty() {
      return Err(error::bad_response("Invalid Link value"));
    }
    Ok(Self { values: parsed })
  }

  pub fn values(&self) -> &[LinkValue] {
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
pub struct LinkValue {
  target: String,
  parameters: Vec<LinkParameter>,
}

impl LinkValue {
  fn parse_member(member: &str) -> error::Result<Self> {
    let member = member.trim();
    let Some(target_and_tail) = member.strip_prefix('<') else {
      return Err(error::bad_response("Invalid Link target"));
    };
    let Some(target_end) = target_and_tail.find('>') else {
      return Err(error::bad_response("Invalid Link target"));
    };
    let target = &target_and_tail[..target_end];
    validate_link_target(target)?;

    let mut parameters = Vec::new();
    let tail = target_and_tail[target_end + 1..].trim();
    if !tail.is_empty() {
      if !tail.starts_with(';') {
        return Err(error::bad_response("Invalid Link parameter"));
      }
      for parameter in split_link_parameters(&tail[1..])? {
        if parameters.len() >= MAX_LINK_PARAMETERS {
          return Err(error::bad_response("Too many Link parameters"));
        }
        let parameter = LinkParameter::parse(&parameter)?;
        if parameters
          .iter()
          .any(|known: &LinkParameter| known.name.eq_ignore_ascii_case(&parameter.name))
        {
          return Err(error::bad_response("Duplicate Link parameter"));
        }
        parameters.push(parameter);
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

  pub fn parameters(&self) -> &[LinkParameter] {
    &self.parameters
  }

  pub fn parameter(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .parameters
      .iter()
      .find(|parameter| parameter.name.eq_ignore_ascii_case(name.as_ref()))
      .map(LinkParameter::value)
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkParameter {
  name: String,
  value: String,
}

impl LinkParameter {
  fn parse(value: &str) -> error::Result<Self> {
    let (name, value) = value.split_once('=').unwrap_or((value, ""));
    let name = name.trim();
    let value = value.trim();
    if !is_token(name) {
      return Err(error::bad_response("Invalid Link parameter name"));
    }
    if value.len() > MAX_LINK_PARAMETER_VALUE_BYTES {
      return Err(error::bad_response("Link parameter value is too large"));
    }
    let value = if value.is_empty() {
      String::new()
    } else {
      parse_link_parameter_value(value)?
    };
    Ok(Self {
      name: name.to_ascii_lowercase(),
      value,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

fn validate_link_target(target: &str) -> error::Result<()> {
  if target.is_empty()
    || target
      .bytes()
      .any(|byte| byte.is_ascii_control() || byte == b'<' || byte == b'>')
  {
    return Err(error::bad_response("Invalid Link target"));
  }
  let base = Url::parse("http://example.invalid/").expect("valid internal base URL");
  Url::options()
    .base_url(Some(&base))
    .parse(target)
    .map_err(|_| error::bad_response("Invalid Link target"))?;
  Ok(())
}

fn split_link_values(value: &str) -> error::Result<Vec<String>> {
  split_link_members(value, b',', "Invalid Link value")
}

fn split_link_parameters(value: &str) -> error::Result<Vec<String>> {
  split_link_members(value, b';', "Invalid Link parameter")
}

fn split_link_members(value: &str, delimiter: u8, message: &str) -> error::Result<Vec<String>> {
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
          return Err(error::bad_response(message));
        }
        members.push(member.to_string());
        start = index + 1;
      }
      _ => {}
    }
  }
  if quoted || escaped || in_target {
    return Err(error::bad_response(message));
  }
  let member = value[start..].trim();
  if member.is_empty() {
    return Err(error::bad_response(message));
  }
  members.push(member.to_string());
  Ok(members)
}

fn parse_link_parameter_value(value: &str) -> error::Result<String> {
  if value.is_empty() {
    return Err(error::bad_response("Invalid Link parameter value"));
  }
  if let Some(value) = value.strip_prefix('"') {
    return parse_content_disposition_quoted_string(value)
      .map_err(|_| error::bad_response("Malformed Link quoted-string"));
  }
  if value.contains('"') || !is_token(value) {
    return Err(error::bad_response("Invalid Link parameter value"));
  }
  Ok(value.to_string())
}

impl ContentDispositionParameter {
  fn parse(value: &str) -> error::Result<Self> {
    let (name, raw_value) = value
      .split_once('=')
      .ok_or_else(|| error::bad_response("Invalid Content-Disposition parameter"))?;
    let name = name.trim();
    let raw_value = raw_value.trim();
    if !is_token(name) {
      return Err(error::bad_response(
        "Invalid Content-Disposition parameter name",
      ));
    }
    if raw_value.len() > MAX_CONTENT_DISPOSITION_PARAMETER_VALUE_BYTES {
      return Err(error::bad_response(
        "Content-Disposition parameter value is too large",
      ));
    }

    let (parsed_value, value_was_quoted) = parse_content_disposition_parameter_value(raw_value)?;
    if name.eq_ignore_ascii_case("filename*")
      && (value_was_quoted || !is_content_disposition_ext_value(&parsed_value))
    {
      return Err(error::bad_response(
        "Invalid Content-Disposition filename* parameter",
      ));
    }

    Ok(Self {
      name: name.to_string(),
      value: parsed_value,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn value(&self) -> &str {
    &self.value
  }
}

fn split_content_disposition_members(value: &str) -> error::Result<Vec<String>> {
  let mut members = Vec::new();
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
      ';' if !in_quote => {
        push_content_disposition_member(&mut members, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(error::bad_response(
      "Malformed Content-Disposition quoted-string",
    ));
  }
  push_content_disposition_member(&mut members, &current)?;
  Ok(members)
}

fn push_content_disposition_member(members: &mut Vec<String>, member: &str) -> error::Result<()> {
  let member = member.trim();
  if member.is_empty() {
    return Err(error::bad_response("Invalid Content-Disposition member"));
  }
  members.push(member.to_string());
  Ok(())
}

fn parse_content_disposition_parameter_value(value: &str) -> error::Result<(String, bool)> {
  if value.is_empty() {
    return Err(error::bad_response(
      "Invalid Content-Disposition parameter value",
    ));
  }
  if let Some(value) = value.strip_prefix('"') {
    return parse_content_disposition_quoted_string(value).map(|value| (value, true));
  }
  if value.contains('"') || !is_token(value) {
    return Err(error::bad_response(
      "Invalid Content-Disposition parameter value",
    ));
  }
  Ok((value.to_string(), false))
}

fn parse_content_disposition_quoted_string(value: &str) -> error::Result<String> {
  let mut chars = value.chars();
  let mut parsed = String::new();
  let mut closed = false;

  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(error::bad_response(
            "Malformed Content-Disposition quoted-string",
          ));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(error::bad_response(
            "Malformed Content-Disposition quoted-string",
          ));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => {
        return Err(error::bad_response(
          "Malformed Content-Disposition quoted-string",
        ))
      }
    }
  }

  if !closed || chars.any(|ch| !ch.is_ascii_whitespace()) {
    return Err(error::bad_response(
      "Malformed Content-Disposition quoted-string",
    ));
  }
  Ok(parsed)
}

fn is_content_disposition_ext_value(value: &str) -> bool {
  let mut parts = value.splitn(3, '\'');
  let Some(charset) = parts.next() else {
    return false;
  };
  let Some(language) = parts.next() else {
    return false;
  };
  let Some(encoded_value) = parts.next() else {
    return false;
  };

  !charset.is_empty()
    && is_token(charset)
    && language.bytes().all(is_content_disposition_language_byte)
    && !encoded_value.is_empty()
    && is_content_disposition_ext_value_chars(encoded_value)
}

fn is_content_disposition_ext_value_chars(value: &str) -> bool {
  let mut bytes = value.bytes().peekable();
  while let Some(byte) = bytes.next() {
    if byte == b'%' {
      let Some(first) = bytes.next() else {
        return false;
      };
      let Some(second) = bytes.next() else {
        return false;
      };
      if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
        return false;
      }
    } else if !is_content_disposition_attr_char(byte) {
      return false;
    }
  }
  true
}

fn is_content_disposition_attr_char(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
    )
}

fn is_content_disposition_language_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.')
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentLanguage {
  tags: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingEndpoints {
  endpoints: Vec<(String, String)>,
}

impl ReportingEndpoints {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut endpoints = Vec::new();
    for value in values {
      if value.len() > MAX_REPORTING_ENDPOINTS_VALUE_BYTES {
        return Err(error::bad_response(
          "Reporting-Endpoints header value is too large",
        ));
      }
      parse_reporting_endpoints_value(value, &mut endpoints)?;
    }
    if endpoints.is_empty() {
      return Err(error::bad_response(
        "Invalid Reporting-Endpoints dictionary",
      ));
    }
    Ok(Self { endpoints })
  }

  pub fn endpoints(&self) -> Vec<(&str, &str)> {
    self
      .endpoints
      .iter()
      .map(|(name, url)| (name.as_str(), url.as_str()))
      .collect()
  }

  pub fn endpoint(&self, name: impl AsRef<str>) -> Option<&str> {
    self
      .endpoints
      .iter()
      .find(|(known, _)| known == name.as_ref())
      .map(|(_, url)| url.as_str())
  }
}

fn parse_reporting_endpoints_value(
  value: &str,
  endpoints: &mut Vec<(String, String)>,
) -> error::Result<()> {
  let bytes = value.as_bytes();
  let mut position = 0;
  while position < bytes.len() {
    while position < bytes.len() && bytes[position].is_ascii_whitespace() {
      position += 1;
    }
    let name_start = position;
    while position < bytes.len()
      && is_reporting_endpoint_key_byte(bytes[position], position == name_start)
    {
      position += 1;
    }
    if position == name_start {
      return Err(error::bad_response(
        "Invalid Reporting-Endpoints endpoint name",
      ));
    }
    let name = &value[name_start..position];
    if position >= bytes.len() || bytes[position] != b'=' {
      return Err(error::bad_response(
        "Invalid Reporting-Endpoints dictionary",
      ));
    }
    position += 1;
    if position >= bytes.len() || bytes[position] != b'\"' {
      return Err(error::bad_response(
        "Reporting-Endpoints URL must be a quoted string",
      ));
    }
    position += 1;
    let mut url = String::new();
    loop {
      let Some(&byte) = bytes.get(position) else {
        return Err(error::bad_response(
          "Malformed Reporting-Endpoints quoted string",
        ));
      };
      position += 1;
      match byte {
        b'\"' => break,
        b'\\' => {
          let Some(&escaped) = bytes.get(position) else {
            return Err(error::bad_response(
              "Malformed Reporting-Endpoints quoted string",
            ));
          };
          if !matches!(escaped, b'\"' | b'\\') {
            return Err(error::bad_response(
              "Malformed Reporting-Endpoints quoted string",
            ));
          }
          position += 1;
          url.push(escaped as char);
        }
        0..=31 | 127..=u8::MAX => {
          return Err(error::bad_response(
            "Malformed Reporting-Endpoints quoted string",
          ))
        }
        _ => url.push(byte as char),
      }
    }
    if endpoints.iter().any(|(known, _)| known == name) {
      return Err(error::bad_response(
        "Duplicate Reporting-Endpoints endpoint name",
      ));
    }
    if endpoints.len() >= MAX_REPORTING_ENDPOINTS {
      return Err(error::bad_response(
        "Too many Reporting-Endpoints endpoints",
      ));
    }
    endpoints.push((name.to_string(), url));
    while position < bytes.len() && bytes[position].is_ascii_whitespace() {
      position += 1;
    }
    if position == bytes.len() {
      break;
    }
    if bytes[position] != b',' {
      return Err(error::bad_response(
        "Invalid Reporting-Endpoints dictionary",
      ));
    }
    position += 1;
    if position == bytes.len() {
      return Err(error::bad_response(
        "Invalid Reporting-Endpoints dictionary",
      ));
    }
  }
  Ok(())
}

fn is_reporting_endpoint_key_byte(byte: u8, first: bool) -> bool {
  if first {
    byte.is_ascii_lowercase() || byte == b'*'
  } else {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.' | b'*')
  }
}

impl ContentLanguage {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut tags = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
      if value.len() > MAX_CONTENT_LANGUAGE_VALUE_BYTES {
        return Err(error::bad_response(
          "Content-Language header value is too large",
        ));
      }

      for tag in value.split(',') {
        let tag = tag.trim();
        if tag.is_empty() || !is_language_range(tag) {
          return Err(error::bad_response("Invalid Content-Language tag"));
        }
        if tags.len() >= MAX_CONTENT_LANGUAGE_TAGS {
          return Err(error::bad_response("Too many Content-Language tags"));
        }

        let normalized = tag.to_ascii_lowercase();
        if !seen.insert(normalized) {
          return Err(error::bad_response("Duplicate Content-Language tag"));
        }
        tags.push(tag.to_string());
      }
    }

    if tags.is_empty() {
      return Err(error::bad_response("Invalid Content-Language tag"));
    }

    Ok(Self { tags })
  }

  pub fn tags(&self) -> Vec<&str> {
    self.tags.iter().map(String::as_str).collect()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEncoding {
  codings: Vec<String>,
}

impl ContentEncoding {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut codings = Vec::new();
    let mut seen = HashSet::new();

    for value in values {
      if value.len() > MAX_CONTENT_ENCODING_VALUE_BYTES {
        return Err(error::bad_response(
          "Content-Encoding header value is too large",
        ));
      }

      for coding in value.split(',') {
        let coding = coding.trim();
        if coding.is_empty() || !is_token(coding) {
          return Err(error::bad_response("Invalid Content-Encoding coding"));
        }
        if codings.len() >= MAX_CONTENT_ENCODINGS {
          return Err(error::bad_response("Too many Content-Encoding codings"));
        }

        let normalized = coding.to_ascii_lowercase();
        if !seen.insert(normalized) {
          return Err(error::bad_response("Duplicate Content-Encoding coding"));
        }
        codings.push(coding.to_string());
      }
    }

    if codings.is_empty() {
      return Err(error::bad_response("Invalid Content-Encoding coding"));
    }

    Ok(Self { codings })
  }

  pub fn codings(&self) -> Vec<&str> {
    self.codings.iter().map(String::as_str).collect()
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vary {
  any: bool,
  field_names: Vec<String>,
}

impl Vary {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut any = false;
    let mut field_names = Vec::new();
    let mut seen = HashSet::new();
    let mut field_name_count = 0usize;

    for value in values {
      if value.len() > MAX_VARY_VALUE_BYTES {
        return Err(error::bad_response("Vary header value is too large"));
      }

      for member in value.split(',') {
        let member = member.trim();
        if member.is_empty() {
          return Err(error::bad_response("Invalid Vary field name"));
        }

        if member == "*" {
          if any || field_names.is_empty() {
            any = true;
            continue;
          }
          return Err(error::bad_response(
            "Vary wildcard cannot be combined with field names",
          ));
        }

        if any {
          return Err(error::bad_response(
            "Vary wildcard cannot be combined with field names",
          ));
        }
        if !is_token(member) {
          return Err(error::bad_response("Invalid Vary field name"));
        }

        field_name_count += 1;
        if field_name_count > MAX_VARY_FIELD_NAMES {
          return Err(error::bad_response("Too many Vary field names"));
        }

        let normalized = member.to_ascii_lowercase();
        if seen.insert(normalized.clone()) {
          field_names.push(normalized);
        }
      }
    }

    if !any && field_names.is_empty() {
      return Err(error::bad_response("Invalid Vary field name"));
    }

    Ok(Self { any, field_names })
  }

  pub fn is_any(&self) -> bool {
    self.any
  }

  pub fn field_names(&self) -> Vec<&str> {
    self.field_names.iter().map(String::as_str).collect()
  }

  pub fn contains_field_name(&self, field_name: impl AsRef<str>) -> bool {
    let field_name = field_name.as_ref();
    if !is_token(field_name) {
      return false;
    }
    let field_name = field_name.to_ascii_lowercase();
    self.field_names.iter().any(|name| name == &field_name)
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheControl {
  no_cache: bool,
  no_cache_fields: Vec<String>,
  no_store: bool,
  max_age: Option<u64>,
  s_maxage: Option<u64>,
  private: bool,
  private_fields: Vec<String>,
  public: bool,
  must_revalidate: bool,
  proxy_revalidate: bool,
  immutable: bool,
  stale_while_revalidate: Option<u64>,
  stale_if_error: Option<u64>,
  extensions: Vec<CacheControlExtension>,
}

impl CacheControl {
  pub fn parse(value: impl AsRef<str>) -> error::Result<Self> {
    Self::parse_values([value.as_ref()])
  }

  fn parse_values<'a, I>(values: I) -> error::Result<Self>
  where
    I: IntoIterator<Item = &'a str>,
  {
    let mut cache_control = Self::default();
    let mut directive_count = 0usize;
    for value in values {
      for directive in split_cache_control_directives(value)? {
        directive_count += 1;
        if directive_count > MAX_CACHE_CONTROL_DIRECTIVES {
          return Err(error::bad_response("Too many Cache-Control directives"));
        }
        cache_control.apply_directive(&directive)?;
      }
    }
    Ok(cache_control)
  }

  fn apply_directive(&mut self, directive: &str) -> error::Result<()> {
    let (name, value, value_was_quoted) = match directive.split_once('=') {
      Some((name, value)) => {
        let value = value.trim();
        (
          name.trim(),
          Some(parse_directive_value(value)?),
          value.starts_with('"'),
        )
      }
      None => (directive.trim(), None, false),
    };
    if !is_token(name) {
      return Err(error::bad_response("Invalid Cache-Control directive"));
    }

    match name.to_ascii_lowercase().as_str() {
      "no-cache" => {
        self.no_cache = true;
        if let Some(value) = value {
          self.no_cache_fields = split_field_names(&value);
        }
      }
      "no-store" => self.no_store = true,
      "max-age" => {
        self.max_age = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "s-maxage" => {
        self.s_maxage = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "private" => {
        self.private = true;
        if let Some(value) = value {
          self.private_fields = split_field_names(&value);
        }
      }
      "public" => self.public = true,
      "must-revalidate" => self.must_revalidate = true,
      "proxy-revalidate" => self.proxy_revalidate = true,
      "immutable" => self.immutable = true,
      "stale-while-revalidate" => {
        self.stale_while_revalidate = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      "stale-if-error" => {
        self.stale_if_error = Some(parse_delta_seconds(
          name,
          value.as_deref(),
          value_was_quoted,
        )?)
      }
      _ => self
        .extensions
        .push(CacheControlExtension::new(name, value.as_deref())),
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

  pub fn extensions(&self) -> &[CacheControlExtension] {
    &self.extensions
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheControlExtension {
  name: String,
  value: Option<String>,
}

impl CacheControlExtension {
  fn new(name: &str, value: Option<&str>) -> Self {
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

fn split_cache_control_directives(value: &str) -> error::Result<Vec<String>> {
  if value.len() > MAX_CACHE_CONTROL_VALUE_BYTES {
    return Err(error::bad_response(
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
        push_directive(&mut directives, &current)?;
        current.clear();
      }
      _ => current.push(ch),
    }
  }

  if in_quote || escaped {
    return Err(error::bad_response("Malformed Cache-Control quoted-string"));
  }
  push_directive(&mut directives, &current)?;
  Ok(directives)
}

fn push_directive(directives: &mut Vec<String>, directive: &str) -> error::Result<()> {
  let directive = directive.trim();
  if directive.is_empty() {
    return Err(error::bad_response("Invalid Cache-Control directive"));
  }
  if directives.len() >= MAX_CACHE_CONTROL_DIRECTIVES {
    return Err(error::bad_response("Too many Cache-Control directives"));
  }
  directives.push(directive.to_string());
  Ok(())
}

fn parse_directive_value(value: &str) -> error::Result<String> {
  if let Some(value) = value.strip_prefix('"') {
    return parse_quoted_string(value);
  }
  if value.contains('"') || value.is_empty() {
    return Err(error::bad_response("Invalid Cache-Control directive value"));
  }
  Ok(value.to_string())
}

fn parse_quoted_string(value: &str) -> error::Result<String> {
  let mut chars = value.chars();
  let mut parsed = String::new();
  let mut closed = false;

  while let Some(ch) = chars.next() {
    match ch {
      '"' => {
        closed = true;
        break;
      }
      '\\' => {
        let Some(escaped) = chars.next() else {
          return Err(error::bad_response("Malformed Cache-Control quoted-string"));
        };
        if !is_quoted_pair_char(escaped) {
          return Err(error::bad_response("Malformed Cache-Control quoted-string"));
        }
        parsed.push(escaped);
      }
      _ if is_qdtext(ch) => parsed.push(ch),
      _ => return Err(error::bad_response("Malformed Cache-Control quoted-string")),
    }
  }

  if !closed || chars.any(|ch| !ch.is_ascii_whitespace()) {
    return Err(error::bad_response("Malformed Cache-Control quoted-string"));
  }
  Ok(parsed)
}

fn parse_delta_seconds(
  name: &str,
  value: Option<&str>,
  value_was_quoted: bool,
) -> error::Result<u64> {
  let Some(value) = value else {
    return Err(error::bad_response(format!(
      "Missing Cache-Control {name} delta-seconds"
    )));
  };
  if value_was_quoted || value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
    return Err(error::bad_response(format!(
      "Invalid Cache-Control {name} delta-seconds"
    )));
  }
  value
    .parse::<u64>()
    .map_err(|_| error::bad_response(format!("Invalid Cache-Control {name} delta-seconds")))
}

fn parse_age_delta_seconds(value: &str) -> error::Result<u64> {
  if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
    return Err(error::bad_response("Invalid Age delta-seconds"));
  }
  value
    .parse::<u64>()
    .map_err(|_| error::bad_response("Invalid Age delta-seconds"))
}

fn split_field_names(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|field| !field.is_empty())
    .map(ToString::to_string)
    .collect()
}

fn trim_http_optional_whitespace(value: &str) -> &str {
  value.trim_matches(|ch| matches!(ch, ' ' | '\t'))
}

fn is_content_location_field_value(value: &str) -> bool {
  value.bytes().all(|byte| {
    byte.is_ascii_graphic() && byte != b'"' && byte != b'<' && byte != b'>' && byte != b'\\'
  })
}

fn is_relative_uri_reference_field_value(value: &str) -> bool {
  let mut fragment_seen = false;
  let mut query_seen = false;
  let mut bytes = value.bytes().peekable();

  while let Some(byte) = bytes.next() {
    match byte {
      b'%' => {
        let Some(first) = bytes.next() else {
          return false;
        };
        let Some(second) = bytes.next() else {
          return false;
        };
        if !first.is_ascii_hexdigit() || !second.is_ascii_hexdigit() {
          return false;
        }
      }
      b'#' => {
        if fragment_seen {
          return false;
        }
        fragment_seen = true;
      }
      b'?' => {
        if !fragment_seen {
          query_seen = true;
        }
      }
      _ => {
        if fragment_seen {
          if !is_fragment_char(byte) {
            return false;
          }
        } else if query_seen {
          if !is_query_char(byte) {
            return false;
          }
        } else if !is_uri_path_char(byte) {
          return false;
        }
      }
    }
  }

  true
}

fn is_uri_path_char(byte: u8) -> bool {
  is_uri_pchar(byte) || byte == b'/'
}

fn is_query_char(byte: u8) -> bool {
  is_uri_pchar(byte) || matches!(byte, b'/' | b'?')
}

fn is_fragment_char(byte: u8) -> bool {
  is_query_char(byte)
}

fn is_uri_pchar(byte: u8) -> bool {
  byte.is_ascii_alphanumeric()
    || matches!(
      byte,
      b'-'
        | b'.'
        | b'_'
        | b'~'
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
        | b':'
        | b'@'
    )
}

fn is_language_range(value: &str) -> bool {
  if value == "*" {
    return true;
  }

  let mut subtags = value.split('-');
  let Some(primary) = subtags.next() else {
    return false;
  };
  if !is_language_primary_subtag(primary) {
    return false;
  }

  subtags.all(is_language_subtag)
}

fn is_language_primary_subtag(value: &str) -> bool {
  (1..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_language_subtag(value: &str) -> bool {
  (1..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && value.bytes().all(is_token_byte)
}

fn is_token_byte(byte: u8) -> bool {
  matches!(
    byte,
    b'!' | b'#'
      | b'$'
      | b'%'
      | b'&'
      | b'\''
      | b'*'
      | b'+'
      | b'-'
      | b'.'
      | b'^'
      | b'_'
      | b'`'
      | b'|'
      | b'~'
      | b'0'..=b'9'
      | b'A'..=b'Z'
      | b'a'..=b'z'
  )
}

fn is_qdtext(ch: char) -> bool {
  matches!(ch, '\t' | ' ' | '!' | '#'..='[' | ']'..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

fn is_quoted_pair_char(ch: char) -> bool {
  matches!(ch, '\t' | ' '..='~') || ('\u{80}'..='\u{ff}').contains(&ch)
}

#[derive(Clone)]
pub struct ResponseBody {
  binary: Vec<u8>,
}

impl ResponseBody {
  pub fn new(binary: Vec<u8>) -> Self {
    Self { binary }
  }

  pub fn binary(&self) -> &[u8] {
    self.binary.as_slice()
  }

  pub fn string(&self) -> error::Result<String> {
    String::from_utf8(self.binary.clone()).map_err(error::body)
  }
}

impl fmt::Debug for ResponseBody {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match self.string() {
      Ok(text) => fmt::Debug::fmt(&text, formatter),
      Err(e) => fmt::Debug::fmt(&e, formatter),
    }
  }
}

impl fmt::Display for ResponseBody {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    match self.string() {
      Ok(text) => fmt::Display::fmt(&text, formatter),
      Err(e) => fmt::Display::fmt(&e, formatter),
    }
  }
}
