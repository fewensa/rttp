//! Compatibility facade for the RTTP client and server crates.

pub struct Http {}

pub use rttp_server::server;

#[cfg(feature = "client")]
pub use rttp_client::response::{
  AcceptCh, AcceptCharset, AcceptEncoding, AcceptPatch, AcceptPost, AcceptRanges,
  AccessControlAllowCredentials, AccessControlAllowCredentialsParseError, AltSvc,
  AltSvcAlternative, AltSvcParameter, AltSvcParseError, AuthenticationInfo,
  AuthenticationInfoParameter, AuthenticationInfoParseError, CacheStatus, CacheStatusIdentifier,
  CacheStatusMember, CacheStatusParameter, CacheStatusParseError, CdnCacheControl,
  CdnCacheControlParseError, ContentDigest, ContentDisposition, ContentDispositionParseError,
  ContentDpr, ContentDprParseError, ContentEncoding, ContentLanguage, ContentLocation,
  ContentLocationParseError, ContentRange, ContentRangeParseError, ContentSecurityPolicy,
  ContentSecurityPolicyParseError, ContentSecurityPolicyReportOnly,
  ContentSecurityPolicyReportOnlyParseError, ContentType, CriticalCh, CrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyParseError, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginEmbedderPolicyReportOnlyParseError, CrossOriginOpenerPolicy,
  CrossOriginOpenerPolicyParseError, Deprecation, DeprecationParseError, DocumentPolicy,
  DocumentPolicyDirective, DocumentPolicyParseError, DocumentPolicyValue, EntityTag,
  EntityTagParseError, HttpContentLength, Location, LocationParseError, MementoDatetime,
  MementoDatetimeParseError, Nel, NelParseError, NelUnknownMember, NoVarySearch,
  NoVarySearchParams, NoVarySearchParseError, PermissionsPolicy, PermissionsPolicyAllowlist,
  PermissionsPolicyAllowlistMember, PermissionsPolicyDirective, PermissionsPolicyParseError,
  Pragma, PragmaParseError, ReprDigest, StrictTransportSecurity, StrictTransportSecurityParseError,
  Upgrade, UpgradeParseError, WantContentDigest, WantReprDigest, WwwAuthenticate,
  WwwAuthenticateChallenge, WwwAuthenticateParameter, WwwAuthenticateParseError,
  XContentTypeOptions, XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
};
#[cfg(feature = "client")]
pub use rttp_client::{
  Baggage, BaggageMember, BaggageParseError, BaggageProperty, SecFetchDest, SecFetchMode,
  SecFetchSite, SecFetchUser, SecPurpose, TraceParent, TraceParentParseError, TraceState,
  TraceStateMember, TraceStateParseError,
};

impl Http {
  #[cfg(feature = "client")]
  pub fn client() -> rttp_client::HttpClient {
    rttp_client::HttpClient::new()
  }

  pub fn server<A>(addr: A) -> std::io::Result<server::HttpServer>
  where
    A: std::net::ToSocketAddrs,
  {
    server::HttpServer::bind(addr)
  }
}
