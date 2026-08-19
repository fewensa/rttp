//! Response wrappers: raw response capture and typed response helpers.
//!
//! The response surface is split into two layers:
//!
//! - `raw_response` owns `RawResponse`, the raw capture layer. It parses the
//!   response wire bytes into the status line, headers, cookies, and body, and
//!   holds trailers parsed upstream by the connection layer. It applies bounded
//!   body handling (single gzip decoding and the configured body-size limit) and
//!   rejects malformed responses. It retains the original binary for
//!   `binary_get()` and `Response::binary()`; `string()` re-renders from the
//!   parsed fields.
//! - `response` owns `Response`, the typed helper layer. It wraps `RawResponse`
//!   and adds typed interpretation: status predicates, header, trailer, and
//!   cookie lookups, typed header parsers such as `etag()`, `cache_control()`,
//!   and `set_cookies()`, and informational-response handling.
//!
//! `RawResponse` is the single source of parsed wire data and stays internal;
//! typed helpers read through it and never re-parse the raw response bytes.

#![allow(clippy::module_inception)]

pub use self::response::*;

pub use rttp_protocol::access_control_allow_headers::{
  AccessControlAllowHeaders, AccessControlAllowHeadersParseError,
};
pub use rttp_protocol::access_control_allow_methods::{
  AccessControlAllowMethods, AccessControlAllowMethodsParseError,
};
pub use rttp_protocol::access_control_allow_origin::{
  AccessControlAllowOrigin, AccessControlAllowOriginParseError,
};
pub use rttp_protocol::access_control_expose_headers::{
  AccessControlExposeHeaders, AccessControlExposeHeadersParseError,
};
pub use rttp_protocol::access_control_max_age::{
  AccessControlMaxAge, AccessControlMaxAgeParseError,
};
pub use rttp_protocol::clear_site_data::{
  ClearSiteData as HttpClearSiteData, ClearSiteDataDirective as HttpClearSiteDataDirective,
  ClearSiteDataParseError as HttpClearSiteDataParseError,
};
pub use rttp_protocol::client_hints::{
  AcceptCh, AcceptChParseError, CriticalCh, CriticalChParseError,
};
pub use rttp_protocol::cookie::{
  HttpCookieParseError, HttpSetCookie, HttpSetCookieAttribute, HttpSetCookies,
};
pub use rttp_protocol::cross_origin_embedder_policy::{
  CrossOriginEmbedderPolicy, CrossOriginEmbedderPolicyParseError,
};
pub use rttp_protocol::cross_origin_embedder_policy_report_only::{
  CrossOriginEmbedderPolicyReportOnly, CrossOriginEmbedderPolicyReportOnlyParseError,
};
pub use rttp_protocol::cross_origin_opener_policy::{
  CrossOriginOpenerPolicy, CrossOriginOpenerPolicyParseError,
};
pub use rttp_protocol::cross_origin_resource_policy::{
  CrossOriginResourcePolicy, CrossOriginResourcePolicyParseError,
};

mod raw_response;
mod response;

pub use rttp_protocol::alt_svc::{AltSvc, AltSvcAlternative, AltSvcParameter, AltSvcParseError};
pub use rttp_protocol::connection::{Connection, ConnectionParseError};
pub use rttp_protocol::digest::{
  ContentDigest, ContentDigestEntry, Digest, DigestEntry, DigestParseError, ReprDigest,
  ReprDigestEntry,
};
pub use rttp_protocol::no_vary_search::{
  NoVarySearch, NoVarySearchExtension, NoVarySearchParams, NoVarySearchParseError,
};
pub use rttp_protocol::prefer::{
  PreferParseError, Preference, PreferenceApplied, PreferenceAppliedParseError, PreferenceKind,
  PreferenceParameter,
};
pub use rttp_protocol::priority::{Priority, PriorityExtension, PriorityParseError};
pub use rttp_protocol::proxy_authentication_info::{
  ProxyAuthenticationInfo, ProxyAuthenticationInfoParameter, ProxyAuthenticationInfoParseError,
};
pub use rttp_protocol::referrer_policy::{
  ReferrerPolicy, ReferrerPolicyParseError, ReferrerPolicyToken,
};
pub use rttp_protocol::server_timing::{
  ServerTiming, ServerTimingMetric, ServerTimingParameter, ServerTimingParseError,
};
pub use rttp_protocol::signature::{Signature, SignatureEntry, SignatureParseError};
pub use rttp_protocol::signature_input::{
  SignatureInput, SignatureInputBareItem, SignatureInputComponent, SignatureInputEntry,
  SignatureInputParameter, SignatureInputParseError,
};
pub use rttp_protocol::strict_transport_security::{
  StrictTransportSecurity, StrictTransportSecurityParseError,
};
pub use rttp_protocol::timing_allow_origin::{TimingAllowOrigin, TimingAllowOriginParseError};
pub use rttp_protocol::trailer::{Trailer, TrailerParseError};
pub use rttp_protocol::transfer_encoding::{TransferEncoding, TransferEncodingParseError};
pub use rttp_protocol::want_content_digest::{
  WantContentDigest, WantContentDigestEntry, WantContentDigestParseError,
};
pub use rttp_protocol::want_repr_digest::{
  WantReprDigest, WantReprDigestEntry, WantReprDigestParseError,
};
pub use rttp_protocol::warning::{Warning, WarningParseError, WarningValue};
pub use rttp_protocol::www_authenticate::{
  WwwAuthenticate, WwwAuthenticateChallenge, WwwAuthenticateParameter, WwwAuthenticateParseError,
};
pub use rttp_protocol::x_content_type_options::{
  XContentTypeOptions, XContentTypeOptionsParseError,
};
pub use rttp_protocol::x_frame_options::{XFrameOptions, XFrameOptionsParseError};
