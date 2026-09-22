//! Compatibility facade for the RTTP client and server crates.

pub struct Http {}

pub use rttp_server::server;

#[cfg(feature = "client")]
pub use rttp_client::response::{
  AcceptCh, AcceptChParseError, AcceptCharset, AcceptCharsetParseError, AcceptCharsetRange,
  AcceptEncoding, AcceptEncodingCoding, AcceptEncodingParseError, AcceptPatch,
  AcceptPatchParseError, AcceptPost, AcceptPostParseError, AcceptRanges, AcceptRangesParseError,
  AcceptSignature, AcceptSignatureBareItem, AcceptSignatureComponent,
  AcceptSignatureComponentParameter, AcceptSignatureCoveredComponent, AcceptSignatureDecimal,
  AcceptSignatureEntry, AcceptSignatureMember, AcceptSignatureParameter,
  AcceptSignatureParameterValue, AcceptSignatureParseError, AccessControlAllowCredentials,
  AccessControlAllowCredentialsParseError, AccessControlAllowHeaders,
  AccessControlAllowHeadersParseError, AccessControlAllowMethods,
  AccessControlAllowMethodsParseError, AccessControlAllowOrigin,
  AccessControlAllowOriginParseError, AccessControlAllowPrivateNetwork,
  AccessControlAllowPrivateNetworkParseError, AccessControlExposeHeaders,
  AccessControlExposeHeadersParseError, AccessControlMaxAge, AccessControlMaxAgeParseError, Age,
  AgeParseError, AltSvc, AltSvcAlternative, AltSvcParameter, AltSvcParseError, AltUsed,
  AltUsedParseError, AlternateAttribute, AlternateVariant, Alternates, AlternatesParseError,
  AuthenticationInfo, AuthenticationInfoParameter, AuthenticationInfoParseError, CacheControl,
  CacheControlExtension, CacheStatus, CacheStatusIdentifier, CacheStatusMember,
  CacheStatusParameter, CacheStatusParseError, CdnCacheControl, CdnCacheControlParseError,
  Connection, ConnectionParseError, ContentDigest, ContentDigestEntry, ContentDisposition,
  ContentDispositionParameter, ContentDispositionParseError, ContentDpr, ContentDprParseError,
  ContentEncoding, ContentLanguage, ContentLocation, ContentLocationParseError, ContentRange,
  ContentRangeParseError, ContentSecurityPolicy, ContentSecurityPolicyParseError,
  ContentSecurityPolicyReportOnly, ContentSecurityPolicyReportOnlyParseError, ContentType,
  ContentTypeParameter, CriticalCh, CriticalChParseError, CrossOriginEmbedderPolicy,
  CrossOriginEmbedderPolicyParseError, CrossOriginEmbedderPolicyReportOnly,
  CrossOriginEmbedderPolicyReportOnlyParseError, CrossOriginOpenerPolicy,
  CrossOriginOpenerPolicyParseError, CrossOriginOpenerPolicyReportOnly,
  CrossOriginOpenerPolicyReportOnlyBareItem, CrossOriginOpenerPolicyReportOnlyParameter,
  CrossOriginOpenerPolicyReportOnlyParseError, Dav, DavClass, DavParseError, DeltaBase,
  DeltaBaseParseError, Deprecation, DeprecationParseError, Digest, DigestEntry, DigestParseError,
  DocumentPolicy, DocumentPolicyDirective, DocumentPolicyParseError, DocumentPolicyReportOnly,
  DocumentPolicyReportOnlyDirective, DocumentPolicyReportOnlyParseError,
  DocumentPolicyReportOnlyValue, DocumentPolicyValue, EntityTag, EntityTagParseError,
  HttpClearSiteData, HttpClearSiteDataDirective, HttpClearSiteDataParseError, HttpContentLength,
  HttpCookieParseError, HttpSameSite, HttpSetCookie, HttpSetCookieAttribute,
  HttpSetCookieAttributeKind, HttpSetCookies, Im, ImMember, ImParameter, ImParseError, KeepAlive,
  KeepAliveExtension, KeepAliveParseError, Location, LocationParseError, MediaType,
  MediaTypeParameter, MementoDatetime, MementoDatetimeParseError, Nel, NelParseError,
  NelUnknownMember, NoVarySearch, NoVarySearchExtension, NoVarySearchParams,
  NoVarySearchParseError, OriginAgentCluster, OriginAgentClusterParseError, OriginTrialParseError,
  OriginTrials, PermissionsPolicy, PermissionsPolicyAllowlist, PermissionsPolicyAllowlistMember,
  PermissionsPolicyDirective, PermissionsPolicyParseError, PermissionsPolicyReportOnly,
  PermissionsPolicyReportOnlyAllowlist, PermissionsPolicyReportOnlyAllowlistMember,
  PermissionsPolicyReportOnlyDirective, PermissionsPolicyReportOnlyParseError, Pragma,
  PragmaDirective, PragmaParseError, ProxyAuthenticate, ProxyAuthenticateChallenge,
  ProxyAuthenticateParameter, ProxyAuthenticateParseError, ProxyAuthenticationInfo,
  ProxyAuthenticationInfoParameter, ProxyAuthenticationInfoParseError, ProxyStatus,
  ProxyStatusBareItem, ProxyStatusIdentifier, ProxyStatusMember, ProxyStatusParameter,
  ProxyStatusParseError, RateLimitLimit, RateLimitLimitItem, RateLimitLimitParseError,
  RateLimitParseError, RateLimitRemaining, RateLimitRemainingParseError, RateLimitReset,
  RateLimitResetParseError, ReferrerPolicy, ReferrerPolicyParseError, ReferrerPolicyToken,
  ReportingEndpoints, ReportingEndpointsParseError, ReprDigest, ReprDigestEntry, ResponseDate,
  ResponseDateParseError, ResponseExpires, ResponseExpiresParseError, ResponseLastModified,
  ResponseLastModifiedParseError, RetryAfter, RetryAfterParseError, ScheduleTag,
  ScheduleTagParseError, SecWebSocketAccept, SecWebSocketAcceptParseError, SecWebSocketExtension,
  SecWebSocketExtensionParameter, SecWebSocketExtensionParameterValue, SecWebSocketExtensions,
  SecWebSocketExtensionsParseError, SecWebSocketProtocol, SecWebSocketProtocolParseError,
  SecWebSocketVersion, SecWebSocketVersionParseError, ServerTiming, ServerTimingMetric,
  ServerTimingParameter, ServerTimingParseError, ServiceWorkerAllowed,
  ServiceWorkerAllowedParseError, Signature, SignatureCoveredComponent, SignatureDecimal,
  SignatureEntry, SignatureInput, SignatureInputBareItem, SignatureInputComponent,
  SignatureInputEntry, SignatureInputMember, SignatureInputParameter, SignatureInputParseError,
  SignatureParameter, SignatureParameterValue, SignatureParseError, SpeculationRules,
  SpeculationRulesParseError, StrictTransportSecurity, StrictTransportSecurityParseError,
  SupportsLoadingMode, SupportsLoadingModeParseError, SurrogateControl, SurrogateControlParseError,
  Tcn, TcnDirective, TcnParseError, TimingAllowOrigin, TimingAllowOriginParseError, Trailer,
  TrailerParseError, TransferEncoding, TransferEncodingParseError, Upgrade, UpgradeParseError,
  VariantVary, VariantVaryParseError, Vary, VaryParseError, WantContentDigest, WantReprDigest,
  Warning, WarningParseError, WarningValue, WwwAuthenticate, WwwAuthenticateChallenge,
  WwwAuthenticateParameter, WwwAuthenticateParseError, XContentTypeOptions,
  XContentTypeOptionsParseError, XFrameOptions, XFrameOptionsParseError,
};
#[cfg(feature = "client")]
pub use rttp_client::response::{
  LinkParameter, LinkParseError, LinkValue, LinkValues, PreferParseError, Preference,
  PreferenceApplied, PreferenceAppliedParseError, PreferenceKind, PreferenceParameter, Priority,
  PriorityExtension, PriorityParseError,
};
#[cfg(feature = "client")]
pub use rttp_client::{
  AIm, AImMember, AImParameter, AImParseError, AcceptDatetime, AcceptDatetimeParseError, Baggage,
  BaggageMember, BaggageParseError, BaggageProperty, Depth, DepthParseError, Destination,
  DestinationParseError, DeviceMemory, DeviceMemoryParseError, Dnt, DntParseError, Downlink,
  DownlinkParseError, Dpr, DprParseError, EarlyData, EarlyDataParseError, Ect, EctParseError,
  Expect, ExpectParseError, From, FromParseError, If, IfCondition, IfList, IfParseError,
  IfPredicate, IfResourceTag, IfScheduleTagMatch, IfScheduleTagMatchParseError, IfStateToken,
  LockToken, LockTokenParseError, Negotiate, NegotiateDirective, NegotiateParseError, Overwrite,
  OverwriteParseError, PrefersColorScheme, PrefersColorSchemeParseError, PrefersContrast,
  PrefersContrastParseError, PrefersReducedData, PrefersReducedDataParseError,
  PrefersReducedMotion, PrefersReducedMotionParseError, PrefersReducedTransparency,
  PrefersReducedTransparencyParseError, Referer, RefererParseError, Rtt, RttParseError, SecChDpr,
  SecChDprParseError, SecChUa, SecChUaArch, SecChUaArchParseError, SecChUaBitness,
  SecChUaBitnessParseError, SecChUaEntry, SecChUaFormFactors, SecChUaFormFactorsParseError,
  SecChUaFullVersion, SecChUaFullVersionList, SecChUaFullVersionListEntry,
  SecChUaFullVersionListParseError, SecChUaFullVersionParseError, SecChUaMobile,
  SecChUaMobileParseError, SecChUaModel, SecChUaModelParseError, SecChUaParseError,
  SecChUaPlatform, SecChUaPlatformParseError, SecChUaPlatformVersion,
  SecChUaPlatformVersionParseError, SecChUaWow64, SecChUaWow64ParseError, SecChViewportHeight,
  SecChViewportHeightParseError, SecChViewportWidth, SecChViewportWidthParseError, SecFetchDest,
  SecFetchMode, SecFetchSite, SecFetchUser, SecGpc, SecGpcParseError, SecPurpose,
  SecRequiredDocumentPolicy, SecRequiredDocumentPolicyDirective,
  SecRequiredDocumentPolicyParseError, SecRequiredDocumentPolicyValue, SecWebSocketKey,
  SecWebSocketKeyParseError, Timeout, TimeoutParseError, TimeoutType, TraceParent,
  TraceParentParseError, TraceState, TraceStateMember, TraceStateParseError,
  UpgradeInsecureRequests, UpgradeInsecureRequestsParseError, UserAgent, UserAgentMember,
  UserAgentParseError, Via, ViaMember, ViaParseError, ViewportWidth, ViewportWidthParseError,
  Width, WidthParseError, XForwardedFor, XForwardedForNode, XForwardedForNodeKind,
  XForwardedForParseError, XForwardedHost, XForwardedHostParseError, XForwardedProto,
  XForwardedProtoParseError,
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
