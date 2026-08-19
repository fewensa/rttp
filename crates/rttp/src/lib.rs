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
  ContentSecurityPolicyParseError, ContentType, CriticalCh, CrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyParseError, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginEmbedderPolicyReportOnlyParseError, CrossOriginOpenerPolicy,
  CrossOriginOpenerPolicyParseError, Deprecation, DeprecationParseError, EntityTag,
  EntityTagParseError, HttpContentLength, Location, LocationParseError, MementoDatetime,
  MementoDatetimeParseError, Nel, NelParseError, NelUnknownMember, NoVarySearch,
  NoVarySearchParams, NoVarySearchParseError, Pragma, PragmaParseError, ReprDigest,
  StrictTransportSecurity, StrictTransportSecurityParseError, Upgrade, UpgradeParseError,
  WantContentDigest, WantReprDigest, WwwAuthenticate, WwwAuthenticateChallenge,
  WwwAuthenticateParameter, WwwAuthenticateParseError, XContentTypeOptions,
  XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
};
#[cfg(feature = "client")]
pub use rttp_client::{SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser, SecPurpose};

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
