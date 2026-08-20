# rttp-server

`rttp-server` provides the blocking HTTP server implementation re-exported by
the `rttp` compatibility facade.

Typed request and response helpers in this crate are metadata-only unless a
section explicitly says otherwise. They parse, validate, normalize, or expose
bounded HTTP metadata; RTTP only parses or declares that metadata and never
applies policy itself. Handler code owns security, caching, authentication and
authorization, and browser-policy decisions, and remains responsible for
retries, representation selection, and body transformation.

## Request Cache-Control metadata

Handlers can call `Request::cache_control()` to obtain typed request cache
directives. The helper combines all case-insensitive `Cache-Control` header
fields, preserves the request for handler-defined error policy, and returns an
error for malformed values, values larger than 64 KiB, or more than 256
directives. It only parses metadata; it does not apply caching behavior.

## Response Cache-Status metadata

`HttpResponse::cache_status()` parses attached `Cache-Status` response fields
into `HttpCacheStatus`. The helper combines repeated fields in wire order as
an RFC 9211 / RFC 8941 list of cache identifiers and parameters, including
typed `hit`, `fwd`, `fwd-status`, `ttl`, `stored`, `collapsed`, `key`, and
`detail` values plus well-formed extension parameters. It applies these
bounds: 64 KiB per field value, at most 256 members, at most 256 parameters
per member, and 64 KiB per parameter value.

Malformed Cache-Status metadata returns `HttpCacheStatusParseError` while
leaving the raw response headers in place. An absent header returns
`Ok(None)`. The helper only exposes metadata for handler-owned policy; it
does not store cache entries, compute freshness, revalidate, select
endpoints, retry, or choose status behavior.

## Response CDN-Cache-Control metadata

`HttpResponse::cdn_cache_control()` parses attached `CDN-Cache-Control`
response fields into `HttpCdnCacheControl`. The helper preserves CDN-specific
extension directives in order with each directive token name and optional
parsed value. It applies the shared cache-directive bounds: 64 KiB per field
value, at most 256 directives, valid HTTP tokens for directive names and
unquoted values, and well-formed quoted strings.

Malformed CDN metadata returns `HttpCdnCacheControlParseError` while leaving
the raw response headers in place. The helper only exposes metadata for
handler-owned policy; it does not create or manage a CDN cache, compute
freshness, evaluate surrogate keys, revalidate automatically, enforce
shared-cache policy, retry, replay, redirect, or choose status behavior.

## Response Content-Security-Policy-Report-Only metadata

`HttpResponse::with_content_security_policy_report_only(value)` validates and
replaces attached `Content-Security-Policy-Report-Only` response metadata with
one bounded field. `HttpResponse::content_security_policy_report_only()` parses
attached raw fields into `HttpContentSecurityPolicyReportOnly`, preserving
repeated fields in wire order and returning parser errors without removing raw
headers.

The helper shares CSP policy field bounds with `Content-Security-Policy`: 64
KiB per field value and at most 256 fields. The report-only type and parse
error remain distinct. These helpers only declare and parse metadata; RTTP does
not evaluate directives, enforce CSP, send reports, or create browser policy
state.

## Authentication metadata

`Request::authorization()` / `HttpRequest::authorization()` and
`Request::proxy_authorization()` / `HttpRequest::proxy_authorization()` expose
one bounded opaque credential field as shared `rttp-protocol` metadata.
`Authorization` is exposed as `HttpAuthorization`, and `Proxy-Authorization`
as `HttpProxyAuthorization`; both expose `scheme()` and `credentials()`. Both
helpers return `Ok(None)` when absent and reject malformed, oversized (over 64
KiB), duplicate, or control-byte-injected fields while leaving the raw request
headers available to handler code. Typed debug output redacts credentials.

`HttpResponse::with_www_authenticate(value)` validates and replaces attached
`WWW-Authenticate` fields with one normalized declaration, and
`HttpResponse::www_authenticate()` parses attached raw challenge fields without
changing them. Invalid typed declarations are rejected before a response is
emitted; raw headers remain intact unless a typed declaration deliberately
replaces them.

These helpers only expose metadata. RTTP does not validate credentials, select
realms, challenge clients automatically, or enforce authentication or
authorization decisions.

## Request representation metadata

Handlers can call `Request::content_type()`, `content_encoding()`, and
`content_language()` (and the matching `HttpRequest` helpers) to observe
bounded typed `Content-Type`, `Content-Encoding`, and `Content-Language`
request metadata. HTTP/1.1 and HTTP/2 share the same `Request` helpers.
`content_type()` treats the field as a singleton; `content_encoding()` and
`content_language()` combine case-insensitive fields in wire order. Absent
fields return `Ok(None)`. Malformed, oversized, or over-limit values return
a parser error while `Request::header()` and `Request::body()` continue to
expose the original request.

These helpers parse request metadata only; they do not sniff, decode,
negotiate, cache, redirect, retry, or select representations.

## Accept-Charset request metadata

Handlers can call `Request::accept_charset()` and
`HttpRequest::accept_charset()` to observe bounded typed `Accept-Charset`
request metadata through the shared `rttp-protocol` primitive. The helpers
combine case-insensitive fields in wire order into
`HttpRequestAcceptCharsets`. Each entry exposes `charset()` and q-value
`quality()` in thousandths (`1000` is the default quality of `1`). The shared
protocol type is the authority for charset-range, wildcard, q-value,
duplicate, member-count, and size validation. Absent metadata returns
`Ok(None)`. Malformed, oversized, duplicate, empty, or over-limit values
return a parse error while `Request::header()` and `Request::body()` continue
to expose the original request.

These helpers parse request metadata only. They do not negotiate, transcode,
decode bodies, sniff MIME types, or select a response charset.

## Accept-Encoding request metadata

Handlers can call `Request::accept_encoding()` and
`HttpRequest::accept_encoding()` to observe bounded typed `Accept-Encoding`
request metadata through the shared `rttp-protocol` primitive. The helpers
combine case-insensitive fields in wire order into
`HttpRequestAcceptEncodings`. Each entry exposes `coding()` and q-value
`quality()` in thousandths (`1000` is the default quality of `1`). The shared
protocol type is the authority for coding-token, wildcard, q-value, duplicate,
member-count, and size validation. Absent metadata returns `Ok(None)`.
Malformed, oversized, duplicate, empty, or over-limit values return a parse
error while `Request::header()` and `Request::body()` continue to expose the
original request.

These helpers parse request metadata only. They do not enable automatic
compression, decompression, or content negotiation.

## Request and response Connection metadata

Handlers can call `Request::connection()`, `HttpRequest::connection()`, and
`HttpResponse::connection()` to observe bounded typed `Connection` header
metadata from already-retained HTTP/1 fields. The helpers combine
case-insensitive fields in wire order into `HttpConnection` and preserve token
spelling, including duplicates. Absent fields return `Ok(None)`. Malformed,
empty, oversized, or over-limit values return a parser error while
`Request::header()` and `HttpResponse` raw headers continue to expose the
original fields. HTTP/2 continues to reject inbound `Connection` at decode
time.

These helpers parse HTTP/1 header metadata only. They do not change
keep-alive, hop-by-hop stripping, upgrade/h2c handoff, or HTTP/2 rejection.

## Request and response Upgrade metadata

Handlers can call `Request::upgrade()`, `HttpRequest::upgrade()`, and
`HttpResponse::upgrade()` to observe bounded typed `Upgrade` metadata from
already-retained HTTP/1 fields. `HttpResponse::with_upgrade()` validates and
replaces attached response `Upgrade` metadata. The helpers combine fields in
wire order into `HttpUpgrade`, preserve protocol spelling, and return
`Ok(None)` when the header is absent.

Each field value is limited to 64 KiB. Parsing accepts at most 32 protocols.
Each protocol must be an HTTP token, optionally followed by `/` and a token
protocol version. Empty members, malformed protocols, control bytes,
oversized values, and too many protocols return a parser error while raw
request or response headers remain available.

These helpers parse or declare HTTP/1 header metadata only. They do not add
`Connection: Upgrade`, select h2c, change CONNECT handling, transfer sockets
to `handoff`, or implement the upgraded protocol.

## Response Keep-Alive metadata

Handlers can call `HttpResponse::keep_alive()` to observe bounded typed
`Keep-Alive` response metadata and `HttpResponse::with_keep_alive(value)` to
validate and replace the `Keep-Alive` response field. The helpers parse
RFC 2068 `Keep-Alive` fields in wire order into `HttpKeepAlive`; the optional
`timeout` delta-seconds and optional `max` `1*DIGIT` values are parsed as
checked unsigned integers, and unrecognized `name=token` parameters are
preserved as bounded `HttpKeepAliveExtension` metadata. Absent fields return
`Ok(None)`. Duplicate recognized parameters, malformed values, overflow,
oversized values, or over-limit values return a parser error while
`HttpResponse` raw headers continue to expose the original fields.

These helpers expose Keep-Alive as metadata only. They do not change
connection lifetime, connection pooling, keep-alive timers, or HTTP/2
behavior.

## Request Transfer-Encoding framing metadata

Handlers can call `Request::transfer_encoding()` and
`HttpRequest::transfer_encoding()` to observe bounded typed
`Transfer-Encoding` framing metadata from already-validated HTTP/1 state.
The helpers combine case-insensitive fields in wire order into
`HttpTransferEncoding` and require a sole `chunked` coding, matching existing
HTTP/1 framing. Absent fields return `Ok(None)`. Malformed, stacked,
duplicate, oversized, or over-limit values return a parser error while
`Request::header()` and `Request::body()` continue to expose the original
request. HTTP/2 continues to reject `Transfer-Encoding` at decode time.

These helpers parse framing metadata only. They do not change
`request_body_kind`, decode a chunked body, negotiate `TE`, or alter
Content-Length handling.

## TE request metadata

Handlers can call `Request::te()` and `HttpRequest::te()` to observe bounded
typed `TE` request metadata through the shared protocol-owned
`rttp-protocol` `Te` type. The helpers combine case-insensitive fields in wire
order into `HttpRequestTe`; each `HttpTe` exposes `coding()`, optional
thousandths `quality()`, and `is_trailers()`. Absent fields return `Ok(None)`.
Malformed codings, `chunked`, q-valued `trailers`, invalid q-values,
case-insensitive duplicates, oversized values, or more than 32 codings return a
parser error while `Request::header()` continues to expose the original raw
field. HTTP/2 continues to reject every `TE` value other than an exact
`TE: trailers` at decode time.

These helpers parse metadata only. They do not enable a transfer-coding
engine, negotiate trailers, apply compression, or alter request framing.

## Fetch Metadata request metadata

Handlers can call `Request::sec_fetch_site()`, `sec_fetch_mode()`,
`sec_fetch_dest()`, `sec_fetch_user()`, and `sec_purpose()` to observe bounded
typed `Sec-Fetch-*` and `Sec-Purpose` request metadata. Malformed values return
a parser error while `Request::header()` continues to expose the original raw
field. RTTP does not enforce browser security policy, block requests, validate
origins, infer navigation policy, start prefetches, or change cache behavior
from these fields.

## Client Hints response metadata

`HttpResponse::with_accept_ch(value)` and
`HttpResponse::with_critical_ch(value)` validate client-hint token lists and
replace any existing `Accept-CH` or `Critical-CH` fields with one normalized
response field. `HttpResponse::accept_ch()` and `HttpResponse::critical_ch()`
parse attached raw fields into `HttpAcceptCh` and `HttpCriticalCh` metadata;
both expose the validated tokens with `client_hints()` and return `Ok(None)`
when the field is absent.

The helpers only declare and inspect metadata. The server does not retain
per-client opt-ins, select hints, alter response policy, or trigger retries.

## Digest response metadata

`HttpResponse::with_digest(value)` and `HttpResponse::with_repr_digest(value)`
validate bounded Structured Fields dictionaries and replace existing
`Content-Digest` or `Repr-Digest` response fields with their normalized values.
`HttpResponse::digest()` and `HttpResponse::repr_digest()` parse attached raw
fields into `HttpDigest` and `HttpReprDigest` metadata, returning parser errors
without changing those raw fields.

These helpers only declare and parse metadata. They do not calculate hashes,
verify bodies, canonicalize representations, sign values, or enforce integrity.

## NEL response metadata

`HttpResponse::with_nel(value)` validates one `NEL` field as bounded W3C
Network Error Logging policy JSON and replaces any existing `NEL` fields with
one normalized value. `HttpResponse::nel()` parses attached raw fields into
`HttpNel` metadata, returning parser errors without changing those raw fields.
The policy exposes its required non-negative `max_age` as `u64`, optional
`report_to` name, `include_subdomains` flag, and `success_fraction`/
`failure_fraction` values as checked members; unknown JSON members are
preserved verbatim without policy semantics. Field values are bounded to
64 KiB, member counts to 256 per object, nesting depth to 64, and each decoded
string to 64 KiB.

These helpers only declare and parse metadata. The server does not send
network error reports, persist policy, or configure Reporting endpoint groups.

## Alt-Used response metadata

`HttpResponse::with_alt_used(value)` validates one bounded `Alt-Used`
authority through the shared protocol `HttpAltUsed` type and replaces any
existing `Alt-Used` fields with one normalized value. `HttpResponse::alt_used()`
parses attached raw fields into `HttpAltUsed` metadata, returning `Ok(None)`
when the header is absent and returning parser errors without changing raw
fields. Valid metadata preserves host spelling, optional port, and bracketed
IPv6 literal form. Malformed authorities, duplicate fields, control bytes, and
values larger than 64 KiB are rejected.

These helpers only declare and parse metadata. The server does not select
alternative services, rewrite origins, migrate sockets, retry, or change
connection policy from `Alt-Used`.

## Origin-Trial response metadata

`HttpResponse::with_origin_trials(values)` validates a bounded collection of
opaque `Origin-Trial` tokens through the shared protocol `HttpOriginTrials`
type, replaces any existing `Origin-Trial` fields, and emits one
`Origin-Trial` header per token. `HttpResponse::origin_trials()` parses
attached raw fields into the same type, returning `Ok(None)` when the header
is absent and returning parser errors without changing raw fields. Each token
is limited to 8 KiB after OWS trim, the collection is limited to 64 tokens,
and the combined token bytes are limited to 64 KiB. Duplicate token strings
are preserved. Token material is redacted from typed debug output and generic
`HttpHeader` debug output.

These helpers only declare and parse metadata. The server does not validate
token signatures, expiration, origin applicability, or activate browser
trials.

## Reporting-Endpoints response metadata

`HttpResponse::with_reporting_endpoints(endpoints)` validates a bounded
`Reporting-Endpoints` dictionary through the shared protocol
`HttpReportingEndpoints` type and replaces any existing
`Reporting-Endpoints` fields with one normalized value.
`HttpResponse::reporting_endpoints()` parses attached raw fields into the
same type, returning parser errors without changing those raw fields.
Each field value is bounded to 64 KiB, the combined raw field-value bytes
are bounded to 64 KiB, and the member count is bounded to 32. Endpoint names
are lowercase tokens that may start with `*`; URLs must be quoted and
unescape only `\\` and `\"`. Invalid names, unquoted URLs, malformed quoted
strings, duplicate names, oversized input, and too many members return a
parser error while `HttpResponse` raw headers continue to expose the
original fields.

These helpers only declare and parse metadata. The server does not schedule,
send, persist, retry, or route reports.

## Cross-Origin-Opener-Policy-Report-Only response metadata

`HttpResponse::with_cross_origin_opener_policy_report_only(value)` validates
a singleton `Cross-Origin-Opener-Policy-Report-Only` structured-field item
through the shared protocol `HttpCrossOriginOpenerPolicyReportOnly` type and
replaces any existing same-name fields with one normalized value.
`HttpResponse::cross_origin_opener_policy_report_only()` parses attached raw
fields into the same type, returning parser errors without changing those raw
fields. The type reuses the canonical COOP directives `unsafe-none`,
`same-origin-allow-popups`, `same-origin`, and `noopener-allow-popups`.
Well-formed parameters are retained as metadata; `report-to` is exposed as a
reporting-endpoint name when present. Each field value is bounded to 64 KiB;
parameter count is bounded to 256, and each parameter value is bounded to
64 KiB. Duplicate fields, duplicate parameter names, unknown directives,
malformed structured fields, and oversized values return a parser error while
`HttpResponse` raw headers continue to expose the original fields.

These helpers only declare and parse metadata. The server does not isolate
browsing contexts, validate `Reporting-Endpoints` members, or send reports.

## Proxy-Status response metadata

`HttpResponse::with_proxy_status(value)` validates RFC 9209 `Proxy-Status` as
a bounded Structured Fields list of Token or String proxy identifiers with
opaque parameters and replaces any existing `Proxy-Status` fields with one
normalized value. `HttpResponse::proxy_status()` parses attached raw fields
into `HttpProxyStatus` metadata, returning parser errors without changing
those raw fields. Absent fields return `Ok(None)`. Empty lists, inner-lists,
malformed syntax, control bytes, oversized values, and duplicate parameters
return a parser error while `HttpResponse` raw headers continue to expose the
original fields.

These helpers expose Proxy-Status as metadata only. They do not interpret
proxy health, retry requests, promote trailers, or generate origin
`Proxy-Status` values.

## Accept-Ranges response metadata

`HttpResponse::with_accept_ranges(units)` declares supported range units with
one bounded comma-separated `Accept-Ranges` response header, while
`HttpResponse::with_accept_ranges_none()` declares the `Accept-Ranges: none`
sentinel. `HttpResponse::accept_ranges()` parses attached raw fields into
`HttpAcceptRanges`, the shared protocol parser also used by the client facade.
Present values expose `units()`, `is_none()`, and `header_value()`; the `none`
sentinel is represented as an empty unit list. Each field value is bounded to
64 KiB and the parsed header set to 256 range units; malformed or empty values,
case-insensitive duplicate units, `none` combined with any unit, and `none`
through the unit declaration helper are rejected. The declaration helper
replaces existing raw `Accept-Ranges` fields, while manually attached fields
remain preserved until the typed parser is requested.

These helpers only declare and inspect metadata. RTTP does not parse request
`Range` fields, generate `Range` requests, create a partial response engine,
serve bytes, slice content, resume downloads, or choose redirect, retry, or
status-policy behavior.

## Content-Disposition response metadata

`HttpResponse::with_content_disposition(value)` validates one
`Content-Disposition` field value with the shared protocol-owned
`HttpContentDisposition` type, removes any existing raw `Content-Disposition`
fields, and adds a single validated `Content-Disposition` header.
`HttpResponse::with_attachment_filename` is a convenience helper for
`attachment; filename=...`. `HttpResponse::content_disposition()` parses
attached raw fields into `HttpContentDisposition`, returns `Ok(None)` when
absent, and preserves invalid raw fields until typed parsing is requested.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB, the parameter list is limited to 256 entries, and each parameter value
is limited to 64 KiB. Disposition type and parameter names are HTTP tokens,
quoted-strings must be well formed, and `filename*` must be an unquoted RFC
5987 ext-value. Duplicate parameters and duplicate fields are rejected because
`Content-Disposition` is singleton response metadata. `filename` and
`filename*` remain independent stored parameters.

These helpers only declare and parse metadata. RTTP does not start automatic
downloads, derive filesystem paths, decode RFC 5987 values, choose a filename
winner, sniff MIME types, negotiate variants, redirect, retry/replay, cache,
or attach status-code policy from `Content-Disposition`.

## Content-Location response metadata

`HttpResponse::with_content_location(value)` validates one
`Content-Location` URI-reference field value with the shared protocol-owned
`HttpContentLocation` type, trims outer whitespace, removes any existing raw
`Content-Location` fields, and adds a single validated `Content-Location`
header. `HttpResponse::content_location()` parses attached raw fields into
`HttpContentLocation`, returns `Ok(None)` when absent, and preserves invalid
raw fields until typed parsing is requested.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty absolute URI or relative URI reference without
control characters, interior whitespace, unsafe field-value characters,
malformed URI syntax, or broken percent-encoding. Duplicate fields are rejected
because `Content-Location` is singleton response metadata. The preserved
trimmed reference is available through `as_str()` and `header_value()`.

These helpers only declare and parse metadata. RTTP does not resolve relative
references against a response URL, follow redirects, select cache variants,
replace representations, generate routes, trigger retries, or alter status
policy from `Content-Location`.

## Service-Worker-Allowed response metadata

`HttpResponse::with_service_worker_allowed(value)` validates one
`Service-Worker-Allowed` origin-relative or absolute path field value with the
shared protocol-owned `HttpServiceWorkerAllowed` type, trims outer whitespace,
removes any existing raw `Service-Worker-Allowed` fields, and adds a single
validated `Service-Worker-Allowed` header.
`HttpResponse::service_worker_allowed()` parses attached raw fields into
`HttpServiceWorkerAllowed`, returns `Ok(None)` when absent, and preserves
invalid raw fields until typed parsing is requested.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty origin-relative or absolute path without
control or non-ASCII characters, interior whitespace, unsafe field-value characters, broken
percent-encoding, absolute URIs, or network-path authority forms. Duplicate
fields are rejected because `Service-Worker-Allowed` is singleton response
metadata. The preserved trimmed path is available through `as_str()` and
`header_value()`.

These helpers only declare and parse metadata. RTTP does not register service
workers, evaluate service-worker scope, resolve the value against a script
URL, or apply application routing policy from `Service-Worker-Allowed`.

## Content-DPR response metadata

`HttpResponse::with_content_dpr(value)` validates one `Content-DPR` field value
with the shared protocol-owned `HttpContentDpr` type, trims outer whitespace,
removes any existing raw `Content-DPR` fields, and adds a single validated
`Content-DPR` header. `HttpResponse::content_dpr()` parses attached raw fields
into `HttpContentDpr`, returns `Ok(None)` when absent, and preserves invalid
raw fields until typed parsing is requested.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must match `1*DIGIT["." 1*DIGIT]` as a finite ratio greater than
zero. Duplicate fields are rejected because `Content-DPR` is singleton response
metadata. The parsed ratio is available through `ratio()`, and the preserved
trimmed decimal is available through `header_value()`.

These helpers only declare and parse metadata. RTTP does not rescale images,
send request DPR, apply Client Hints policy, retry, or change transport from
`Content-DPR`.

## Deprecation response metadata

`HttpResponse::with_deprecation(value)` replaces any existing raw `Deprecation`
fields and adds one canonical Structured Fields boolean (`?0` / `?1`) or date
(`@` followed by signed UNIX seconds) header from `HttpDeprecation`.
`HttpResponse::deprecation()` parses attached raw fields into
`HttpDeprecation`, returns `Ok(None)` when absent, and preserves invalid raw
fields until typed parsing is requested.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB. Empty values, item parameters, inner lists, comma-joined items,
integers without `@`, decimals, strings, tokens including historical `true`,
byte sequences, display strings, IMF-fixdate values, forbidden ASCII control
bytes, and dates that cannot be represented as `SystemTime` are rejected
because `Deprecation` is singleton response metadata.

These helpers only declare and parse metadata. RTTP does not compare `Sunset`,
follow `Link` `rel=deprecation`, decide whether a resource is already
deprecated, retry requests, or select another endpoint.

## No-Vary-Search response metadata

`HttpResponse::with_no_vary_search(value)` validates and replaces attached
`No-Vary-Search` fields with one normalized response declaration.
`HttpResponse::no_vary_search()` parses attached raw fields into
`HttpNoVarySearch` metadata. The typed value exposes recognized `key-order`,
`params`, and `except` members while keeping the behavior metadata-only.

These helpers do not store responses, match cache keys, normalize URLs, replay
requests, apply browser navigation behavior, or enforce shared-cache policy.

## Permissions-Policy response metadata

`HttpResponse::with_permissions_policy(value)` validates one W3C Permissions
Policy Structured Fields dictionary and replaces any existing raw
`Permissions-Policy` fields with one canonical value from the shared protocol
parser. `HttpResponse::permissions_policy()` parses attached raw fields into
`HttpPermissionsPolicy` metadata, returning `Ok(None)` when absent and parser
errors without changing raw fields. Directives expose the feature token and
allowlist: `*` as the whole allowlist, the `self` token, quoted serialized
HTTP(S) origins, and inner lists including the empty `()`. Field values are
bounded to 64 KiB, directives to 256 per header set, and allowlist members to
256 per directive. Duplicate feature keys, duplicate allowlist members, the
HTML-attribute tokens `src` and `'none'`, and unparsable input are rejected; a
well-formed `report-to` parameter is accepted and dropped.

These helpers only declare and parse metadata. RTTP does not grant or deny
browser permissions, compare origins, resolve `self`, or enforce origin
policy, and it does not send reports.

## Document-Policy response metadata

`HttpResponse::with_document_policy(value)` validates one WICG Document Policy
Structured Fields dictionary and replaces any existing raw `Document-Policy`
fields with one canonical value from the shared protocol parser.
`HttpResponse::document_policy()` parses attached raw fields into
`HttpDocumentPolicy` metadata, returning `Ok(None)` when absent and parser
errors without changing raw fields. Directives expose the configuration-point
name, typed value (boolean, integer, decimal, or token), and the retained
`report-to` endpoint name. Directive names are opaque lowercase tokens or `*`
and are not looked up against a browser configuration-point list. Field
values are bounded to 64 KiB, the cumulative raw bytes across all supplied
fields to 64 KiB, and directives to 256 per header set. Empty dictionaries,
duplicate directive names, duplicate parameters, unknown parameters, and
unparsable input are rejected.

These helpers only declare and parse metadata. RTTP does not execute
configuration points, block document loads, compare required policies, echo
`Sec-Required-Document-Policy`, or send reports.

## Supports-Loading-Mode response metadata

`HttpResponse::with_supports_loading_mode(tokens)` validates a declared token
list through the shared protocol parser and replaces any existing raw
`Supports-Loading-Mode` fields with one canonical comma-separated value.
`HttpResponse::supports_loading_mode()` parses attached raw fields into
`HttpSupportsLoadingMode` metadata, returning `Ok(None)` when absent and
parser errors without changing raw fields. The typed value exposes the
ordered tokens with `tokens()`, membership checks with `contains(token)`, and
exact predicates for the defined `fenced-frame`,
`credentialed-prerender`, and `prerender-cross-origin-frames` tokens;
well-formed unknown tokens such as `uncredentialed-prerender` are retained.
Field values are bounded to 64 KiB, the combined raw bytes across fields to
64 KiB, and tokens to 256 per header set. Empty members, strings, integers,
inner lists, parameterized items, duplicate tokens, non-token members, and
oversized values are rejected.

These helpers only declare and parse metadata. RTTP does not prerender
documents, admit fenced frames, change navigation, or alter resource loading.

## Want-Content-Digest request metadata

Handlers can call `Request::want_content_digest()` and
`HttpRequest::want_content_digest()` to observe bounded typed
`Want-Content-Digest` algorithm preferences. The helpers combine
case-insensitive fields in wire order into `HttpWantContentDigest`. Each entry
exposes `algorithm()` and `preference()` (`0` through `10`). Absent metadata
returns `Ok(None)`. Malformed, oversized, duplicate, empty, or over-limit
values return a parse error while `Request::header()` and `Request::body()`
continue to expose the original request.

These helpers parse request metadata only. They do not select an algorithm,
compute or verify content digests, attach `Content-Digest`, or negotiate
content.

## Want-Repr-Digest request metadata

Handlers can call `Request::want_repr_digest()` and
`HttpRequest::want_repr_digest()` to observe bounded typed `Want-Repr-Digest`
algorithm preferences. The helpers combine case-insensitive fields in wire
order into `HttpWantReprDigest`. Each entry exposes `algorithm()` and
`preference()` (`0` through `10`). Absent metadata returns `Ok(None)`.
Malformed, oversized, duplicate, empty, or over-limit values return a parse
error while `Request::header()` and `Request::body()` continue to expose the
original request.

These helpers parse request metadata only. They do not select an algorithm,
compute or verify representation digests, attach `Repr-Digest`, or negotiate a
representation.

## Host request authority

Handlers can call `Request::host()` and `HttpRequest::host()` to observe the
effective `Host` authority as bounded `HttpHost` metadata. The helpers parse
the stored case-insensitive `Host` field as `host[:port]`, including bracketed
IPv6, using the inbound Host grammar. Absent metadata returns `Ok(None)`.
Duplicate or malformed values return a parse error while `Request::header()`
and `Request::body()` continue to expose the original request. HTTP/2
`:authority` remains mapped onto `header("host")`; `host()` then parses that
single mapped value.

These helpers parse request metadata only. They do not select a virtual host,
compare origins, apply scheme defaults, or change HTTP/1 decode or HTTP/2
request-target handling.

## Save-Data request metadata

Handlers can call `Request::save_data()` and `HttpRequest::save_data()` to
observe bounded typed `Save-Data` request metadata. Absent fields return
`Ok(None)`. The recognized value is the case-sensitive `on` token with
optional surrounding SP or HTAB. Malformed, oversized, duplicate, or
control-byte values return a parser error while `Request::header()` and
`HttpRequest::header()` continue to expose the original raw field.

These helpers parse request metadata only. They do not select a
representation, compress a body, advertise Client Hints, or apply browser
data-saver policy.

## Expect request metadata

Handlers can call `Request::expectations()` and `HttpRequest::expectations()`
to observe bounded typed `Expect` request metadata through the shared
protocol type, re-exported as `HttpExpectations`. Absent fields return
`Ok(None)`. `expects_continue()` identifies the standardized `100-continue`
expectation, while `unsupported()` preserves well-formed extension names for
handler policy. Malformed, duplicate, oversized, or excessive values return
`HttpExpectParseError` while `Request::header()` and `HttpRequest::header()`
continue to expose the original raw field.

These helpers parse request metadata only. They do not send `100 Continue`,
wait for an interim response, reject unsupported extensions, or change body
framing.

## Sec-GPC request metadata

Handlers can call `Request::sec_gpc()` and `HttpRequest::sec_gpc()` to observe
bounded typed `Sec-GPC` request metadata through the shared protocol
`HttpSecGpc` representation. Absent fields return `Ok(None)`. The recognized
value is the case-sensitive `1` signal with optional surrounding SP or HTAB.
Malformed, oversized, duplicate, or control-byte values return a parser error
while `Request::header()` and `HttpRequest::header()` continue to expose the
original raw field.

These helpers parse request metadata only. They do not infer or enforce
consent, tracking, legal, or serving policy.

## Upgrade-Insecure-Requests request metadata

Handlers can call `Request::upgrade_insecure_requests()` and
`HttpRequest::upgrade_insecure_requests()` to observe bounded typed
`Upgrade-Insecure-Requests` request metadata. Absent fields return `Ok(None)`.
The recognized value is the case-sensitive `1` token with optional surrounding
SP or HTAB. Malformed, oversized, duplicate, or control-byte values return a
parser error while `Request::header()` and `HttpRequest::header()` continue to
expose the original raw field.

These helpers parse request metadata only. They do not rewrite `http://` URLs
to `https://`, redirect requests, or enforce Content-Security-Policy.

## Max-Forwards request metadata

Handlers can call `Request::max_forwards()` and `HttpRequest::max_forwards()`
to observe bounded typed `Max-Forwards` request metadata. Absent fields return
`Ok(None)`. The recognized value is a singleton `1*DIGIT` hop count that fits
in `u32` (`0` through `4294967295`) with optional surrounding SP or HTAB.
Malformed, overflowing, oversized, duplicate, or control-byte values return a
parser error while `Request::header()` and `HttpRequest::header()` continue to
expose the original raw field.

These helpers parse request metadata only. They do not decrement the hop
count, route a request, select TRACE or OPTIONS, or apply forwarding policy.

## WebDAV Depth request metadata

Handlers can call `Request::depth()` and `HttpRequest::depth()` to observe
bounded typed WebDAV `Depth` request metadata through the shared protocol
`HttpDepth` type. Absent fields return `Ok(None)`. Recognized values are the
singleton depth values `0`, `1`, and `infinity`, with optional surrounding SP
or HTAB and lowercase canonical emission for `infinity`. Malformed,
oversized, duplicate, or control-byte values return a parser error while
`Request::header()` and `HttpRequest::header()` continue to expose the
original raw field.

These helpers parse request metadata only. They do not traverse resources,
select WebDAV methods, or enforce method policy.

## Idempotency-Key request metadata

Handlers can call `Request::idempotency_key()` and
`HttpRequest::idempotency_key()` to observe bounded typed `Idempotency-Key`
request metadata through the shared protocol `HttpIdempotencyKey` type.
Absent fields return `Ok(None)`. A recognized value is a singleton opaque key
of one or more visible ASCII characters with optional surrounding SP or HTAB,
bounded to 64 KiB. `as_str()` returns the stored key and `header_value()`
emits it unchanged. Malformed, oversized, duplicate, or control-byte values
return a parser error while `Request::header()` and `HttpRequest::header()`
continue to expose the original raw field. The key is redacted from typed
`Debug`.

These helpers parse request metadata only. They do not retry requests, store
keys, compare keys across requests, or apply application idempotency policy.

## Sec-WebSocket-Key request metadata

Handlers can call `Request::sec_websocket_key()` and
`HttpRequest::sec_websocket_key()` to observe bounded typed
`Sec-WebSocket-Key` request metadata through the shared protocol
`HttpSecWebSocketKey` type. Absent fields return `Ok(None)`. A recognized
value is a singleton RFC 4648 section 4 base64 encoding of exactly 16 nonce
bytes with optional surrounding SP or HTAB, bounded to 64 KiB. `as_str()`
returns the stored encoded nonce and `header_value()` emits it unchanged.
Malformed, URL-safe or unpadded, wrong-decoded-length, oversized, duplicate,
or control-byte values return a parser error while `Request::header()` and
`HttpRequest::header()` continue to expose the original raw field. The nonce
is redacted from typed `Debug`.

These helpers parse request metadata only. They do not perform an HTTP
upgrade, compute `Sec-WebSocket-Accept`, generate a random nonce, or
implement WebSocket frames.

## Sec-WebSocket-Version request and response metadata

Handlers can call `Request::sec_websocket_version()` and
`HttpRequest::sec_websocket_version()` to observe bounded typed
`Sec-WebSocket-Version` request metadata through the shared protocol
`HttpSecWebSocketVersion` type. Absent fields return `Ok(None)`.
`HttpResponse::with_sec_websocket_version(versions)` declares validated
response metadata that replaces attached same-name fields, and
`HttpResponse::sec_websocket_version()` parses attached response fields.
Recognized values are RFC 6455 version tokens (`0` through `299` without
leading zeros) in numeric descending order, such as `13` or `13, 8, 7`.
Multiple fields are combined in wire order, each field value and the combined
raw or canonical serialized field set is bounded to 64 KiB, and the combined
member count is bounded to 32. Empty members, non-decimal tokens, leading-zero
multi-digit tokens, duplicates, unordered lists, control-byte values, and
bound violations return a parser error while `Request::header()` and
`HttpRequest::header()` continue to expose the original raw fields.

These helpers declare and parse metadata only. They do not perform a
WebSocket handshake, emit `Connection: Upgrade` or `Upgrade: websocket`,
compute `Sec-WebSocket-Accept`, negotiate versions, or switch protocols.

## Pragma request and response metadata

Handlers can call `Request::pragma()` and `HttpRequest::pragma()` to observe
bounded typed `Pragma` request metadata through the shared protocol
`HttpPragma` type, and `HttpResponse::with_pragma(value)` to declare validated
`Pragma` response metadata that replaces attached same-name fields.
`HttpResponse::pragma()` parses attached response `Pragma` fields. Absent
fields return `Ok(None)`. The helpers parse RFC 9111 `pragma-directive`
members: the defined valueless `no-cache` token or an `extension-pragma`
token with an optional token or quoted-string value. Multiple `Pragma` fields
are combined in wire order, directive names are matched case-insensitively,
duplicate names are rejected, each field value is bounded to 64 KiB, combined
field values are bounded to 64 KiB including `", "` separator overhead, each
directive value is bounded to 64 KiB, and the combined directive count is
bounded to 256. Empty members, malformed tokens or quoted-strings, valued
`no-cache` forms, forbidden ASCII control bytes, and bound violations return a
parser error while `Request::header()` and `HttpRequest::header()` continue to
expose the original raw fields.

These helpers declare and parse metadata only. They do not translate `Pragma`
into `Cache-Control`, store cache entries, or apply cache, intermediary, or
HTTP/1.0 compatibility policy.

## W3C Trace Context request metadata

Handlers can call `Request::traceparent()`, `Request::tracestate()`, and the
matching `HttpRequest` helpers to observe bounded W3C Trace Context request
metadata through shared protocol types. Absent fields return `Ok(None)`.
Malformed, oversized, duplicate, unsupported-version, all-zero identifier, or
invalid-member values return parser errors while `Request::header()` and
`HttpRequest::header()` continue to expose the original raw fields.

`HttpTraceParent` exposes the documented version, trace-id, parent-id, flags,
and sampled-bit accessors. `HttpTraceState` preserves ordered members with
key/value accessors. Trace context propagation values are redacted from typed
`Debug`. These helpers parse request metadata only; they do not create trace
identifiers, decide sampling, select a tracing backend, or automatically
propagate context.

## W3C Baggage request metadata

Handlers can call `Request::baggage()` and the matching `HttpRequest` helper
to observe bounded W3C Baggage request metadata through the shared protocol
`HttpBaggage` type. Absent fields return `Ok(None)`. Malformed, oversized,
duplicate-key, or over-limit values return parser errors while
`Request::header()` and `HttpRequest::header()` continue to expose the
original raw fields.

`HttpBaggage` preserves ordered members with key, value, and property
accessors. Member and property values are redacted from typed `Debug`. These
helpers parse request metadata only; they do not interpret application data,
store request context, select a tracing backend, or automatically propagate
baggage.

## CDN-Loop request metadata

Handlers can call `Request::cdn_loop()` and the matching `HttpRequest` helper
to observe bounded RFC 8586 `CDN-Loop` request metadata through the shared
protocol `HttpCdnLoop` type. Absent fields return `Ok(None)`. Malformed,
oversized, duplicate-parameter, or over-limit values return parser errors
while `Request::header()` and `HttpRequest::header()` continue to expose the
original raw fields.

`HttpCdnLoop` preserves ordered members with an opaque CDN identifier
(`uri-host` with optional port or an RFC 7230 token pseudonym) and optional
HTTP parameter accessors. Each field value, the combined raw field set
including `", "` separator overhead, and the combined serialized value are
bounded to 64 KiB, the combined member count is bounded to 256, and each
member is bounded to 32 parameters. Repeated `CDN-Loop` fields are combined in
wire order, and repeated CDN identifiers are valid loop-visible metadata.
These helpers expose loop metadata only; they do not detect or break loops,
reject requests because an identifier is already present, or forward the
field automatically.

## Conditional HTTP-date request metadata

Handlers can call `Request::if_modified_since()`,
`HttpRequest::if_modified_since()`, and the matching `if_unmodified_since()`
accessors to observe bounded typed HTTP-date validators through the shared
protocol `HttpIfModifiedSince` and `HttpIfUnmodifiedSince` types. Absent
fields return `Ok(None)`. A recognized value is one HTTP-date instant with
optional surrounding SP or HTAB; `datetime()` exposes the instant and
`header_value()` formats it as IMF-fixdate. The accessors previously returned
`SystemTime` directly; they now return the typed protocol value, so callers
call `datetime()` to obtain the instant. Malformed, oversized, duplicate, or
control-byte values return a parser error while `Request::header()` and
`HttpRequest::header()` continue to expose the original raw field.

These helpers parse request metadata only. They do not compare
`Last-Modified`, evaluate conditional precedence, serve or reject a
representation, or apply cache policy. `Request::evaluate_conditional()` and
`evaluate_conditional_request()` keep their existing RFC 9110 precedence and
second-level date comparison behavior.

## HTTP message signature metadata

`Request::signature()` / `signature_input()` and the same methods on
`HttpRequest` parse received RFC 9421 `Signature` and `Signature-Input`
fields into `HttpSignature` and `HttpSignatureInput`. Absent field sets
return `Ok(None)`. Present malformed fields return a parse error while
`Request::header()` continues to expose the original values. The two
fields are parsed independently.

`HttpResponse::with_signature()` and `with_signature_input()` validate and
replace existing same-name fields with one canonical value.
`HttpResponse::signature()` and `signature_input()` parse attached raw
fields without changing them.

These helpers only declare and parse metadata. They do not sign, verify,
look up keys, canonicalize covered components, or apply cryptographic
policy.
