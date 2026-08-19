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

## Authentication metadata

`Request::authorization()` / `HttpRequest::authorization()` and
`Request::proxy_authorization()` / `HttpRequest::proxy_authorization()` expose
one bounded opaque credential field as `HttpAuthorization` metadata. Both
helpers return `Ok(None)` when absent and reject malformed, oversized (over 64
KiB), or duplicate fields while leaving the raw request headers available to
handler code.

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
