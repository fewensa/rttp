//! Compatibility facade for the RTTP client and server crates.

pub struct Http {}

pub use rttp_server::server;

#[cfg(feature = "client")]
pub use rttp_client::response::{
  AcceptCh, AcceptPatch, AcceptPost, AcceptRanges, AltSvc, AltSvcAlternative, AltSvcParameter,
  AltSvcParseError, AuthenticationInfo, AuthenticationInfoParameter, AuthenticationInfoParseError,
  CdnCacheControl, CdnCacheControlParseError, ContentLocation, ContentLocationParseError,
  ContentRange, ContentRangeParseError, ContentSecurityPolicy, ContentSecurityPolicyParseError,
  CriticalCh, CrossOriginEmbedderPolicy, CrossOriginEmbedderPolicyParseError,
  CrossOriginEmbedderPolicyReportOnly, CrossOriginEmbedderPolicyReportOnlyParseError,
  CrossOriginOpenerPolicy, CrossOriginOpenerPolicyParseError, HttpContentLength, Location,
  LocationParseError, Nel, NelParseError, NelUnknownMember, NoVarySearch, NoVarySearchParams,
  NoVarySearchParseError, StrictTransportSecurity, StrictTransportSecurityParseError,
  XContentTypeOptions, XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
};
#[cfg(feature = "client")]
pub use rttp_client::{SecFetchDest, SecFetchMode, SecFetchSite, SecFetchUser};

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
