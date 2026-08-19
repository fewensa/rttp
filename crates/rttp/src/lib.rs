//! Compatibility facade for the RTTP client and server crates.

pub struct Http {}

pub use rttp_server::server;

#[cfg(feature = "client")]
pub use rttp_client::response::{
  AcceptCh, AcceptCharset, AcceptEncoding, AcceptPatch, AcceptPost, AcceptRanges,
  AccessControlAllowCredentials, AccessControlAllowCredentialsParseError, AltSvc,
  AltSvcAlternative, AltSvcParameter, AltSvcParseError, AltUsed, AltUsedParseError,
  AuthenticationInfo, AuthenticationInfoParameter, AuthenticationInfoParseError, CacheStatus,
  CacheStatusIdentifier, CacheStatusMember, CacheStatusParameter, CacheStatusParseError,
  CdnCacheControl, CdnCacheControlParseError, ContentDigest, ContentDisposition,
  ContentDispositionParseError, ContentDpr, ContentDprParseError, ContentEncoding, ContentLanguage,
  ContentLocation, ContentLocationParseError, ContentRange, ContentRangeParseError,
  ContentSecurityPolicy, ContentSecurityPolicyParseError, ContentSecurityPolicyReportOnly,
  ContentSecurityPolicyReportOnlyParseError, ContentType, CriticalCh, CrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyParseError, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginEmbedderPolicyReportOnlyParseError, CrossOriginOpenerPolicy,
  CrossOriginOpenerPolicyParseError, CrossOriginOpenerPolicyReportOnly,
  CrossOriginOpenerPolicyReportOnlyBareItem, CrossOriginOpenerPolicyReportOnlyParameter,
  CrossOriginOpenerPolicyReportOnlyParseError, Deprecation, DeprecationParseError, DocumentPolicy,
  DocumentPolicyDirective, DocumentPolicyParseError, DocumentPolicyValue, EntityTag,
  EntityTagParseError, HttpContentLength, Location, LocationParseError, MementoDatetime,
  MementoDatetimeParseError, Nel, NelParseError, NelUnknownMember, NoVarySearch,
  NoVarySearchParams, NoVarySearchParseError, OriginTrialParseError, OriginTrials,
  PermissionsPolicy, PermissionsPolicyAllowlist, PermissionsPolicyAllowlistMember,
  PermissionsPolicyDirective, PermissionsPolicyParseError, Pragma, PragmaParseError, ReprDigest,
  ServiceWorkerAllowed, ServiceWorkerAllowedParseError, SpeculationRules,
  SpeculationRulesParseError, StrictTransportSecurity, StrictTransportSecurityParseError,
  SupportsLoadingMode, SupportsLoadingModeParseError, Upgrade, UpgradeParseError,
  WantContentDigest, WantReprDigest, WwwAuthenticate, WwwAuthenticateChallenge,
  WwwAuthenticateParameter, WwwAuthenticateParseError, XContentTypeOptions,
  XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
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
