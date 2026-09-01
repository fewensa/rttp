rttp
====

`rttp` is a compatibility facade over the `rttp_client` and `rttp-server`
APIs. It re-exports the blocking server through `rttp::server` and
`rttp::Http::server` for local tests and simple embedded use, and behind the
optional `client` feature exposes `rttp::Http::client` plus selected client
metadata types. It is not a separate runtime or policy layer: the facade owns
no server loop, event loop, connection pooling, cache, retry, or status-policy
engine of its own, and all bounded behavior and metadata-only helpers remain in
the underlying `rttp_client` and `rttp-server` crates.

Applications that want one dependency for both client and server entry points
can use this compatibility facade.
The compatibility facade keeps client and server entry points available from a single crate.

## Server

Create a listener with `rttp::Http::server` or call `HttpServer::bind` directly.
The server listener is built with `socket2`.

```rust,no_run
use std::time::Duration;

use rttp::server::HttpResponse;

fn main() -> std::io::Result<()> {
  let server = rttp::Http::server("127.0.0.1:0")?
    .with_read_timeout(Some(Duration::from_secs(5)))
    .with_write_timeout(Some(Duration::from_secs(5)));
  println!("listening on {}", server.local_addr()?);

  server.accept_one(|request| {
    println!("{} {}", request.method(), request.target());
    HttpResponse::ok("hello")
      .header("Transfer-Encoding", "chunked")
      .header("Trailer", "X-Trace, X-Signature")
      .trailer("X-Trace", "abc")
      .trailer("X-Signature", "signed")
  })
}
```

`HttpServer::local_addr` returns the bound address, which is useful when binding
to port `0` in tests. `HttpServer::accept_one` serves one connection.
`HttpServer::serve_requests` serves a fixed number of sequential connections on
the same listener. `HttpServer::with_read_timeout` and
`HttpServer::with_write_timeout` apply socket-level timeouts to each accepted
connection; pass `None` to leave the corresponding socket timeout unset.

Add `Transfer-Encoding: chunked` to an `HttpResponse` to write the complete
response body with HTTP/1.x chunked transfer framing instead of an automatic
`Content-Length` when the response status permits a message body. Response
trailers added with `HttpResponse::trailer` are written after the terminating
zero-size chunk, and can be inspected before serialization with
`HttpResponse::trailers` or `HttpResponse::trailer_value`. Add a `Trailer`
response header when advertising which trailer fields will follow.

`HttpResponse::with_content_security_policy_report_only(value)` and
`content_security_policy_report_only()` expose bounded
`Content-Security-Policy-Report-Only` response metadata through the server
facade. The client facade re-exports the matching
`ContentSecurityPolicyReportOnly` type and `Response` accessor. Values remain
opaque, repeated fields preserve wire order, and raw headers remain observable
on parse failures; the facade does not enforce CSP or send reports.

`HttpResponse::with_dav(value)` / `dav()` and client `Response::dav()` expose
bounded WebDAV `DAV` response metadata through the shared protocol `Dav` type.
The parser preserves ordered classes, accepts `1`, `2`, `3`, extension tokens,
and `<absolute-URI>` Coded-URLs, and rejects malformed, duplicate, oversized,
aggregate-oversized, or over-32-member values. The facade does not infer,
negotiate, or enforce WebDAV feature support from this header.

## Bounded HTTP/1.1 byte ranges

The server exposes byte-range primitives, not an automatic static-file server.
Applications that want range support should read the request `Range` header,
decide whether it applies to the selected representation, and call
`HttpByteRange::parse(range_header, entity_length)`.

`HttpByteRange::parse` supports one `bytes` range at a time:
`bytes=start-end`, `bytes=start-`, and `bytes=-suffix`. Closed ranges must have
`start <= end`; open-ended ranges are clipped to the entity length; suffix
ranges must request at least one byte. Unsupported units return
`UnsupportedUnit`, comma-separated ranges return `MultipleRanges`, malformed
or inverted ranges return `InvalidRange`, and ranges outside the entity return
`UnsatisfiedRange`.

Use `HttpResponse::partial_content(body, range)` for a satisfiable range. It
returns `206 Partial Content`, writes `Content-Range: bytes start-end/length`,
and sends only the selected body bytes. Use
`HttpResponse::range_not_satisfiable(entity_length)` for an unsatisfied range;
it returns `416 Range Not Satisfiable` with
`Content-Range: bytes */length` and an empty body.
`HttpResponse::content_range()` parses an attached `Content-Range` field into
the shared checked protocol `HttpContentRange` metadata type and rejects
invalid or duplicate fields only when the typed helper is requested.

Use `HttpResponse::with_accept_ranges(units)` to declare supported range units
with one bounded comma-separated `Accept-Ranges` header, or
`HttpResponse::with_accept_ranges_none()` to declare `Accept-Ranges: none`.
`HttpResponse::accept_ranges()` parses attached `Accept-Ranges` headers into
`HttpAcceptRanges`. Present values expose `units()`, `is_none()`, and
`header_value()` for serialization. The declaration helper replaces any
existing raw `Accept-Ranges` fields with one validated field, while manually
attached `HttpResponse::header("Accept-Ranges", ...)` fields remain preserved
until the typed parser is requested.

The helper is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed header set is limited to 256 range units, range
units must be valid HTTP tokens, malformed or empty values are rejected,
duplicates are rejected case-insensitively across all parsed header fields, and
the `none` sentinel is represented as an empty unit list through
`with_accept_ranges_none()` or a parsed raw `Accept-Ranges: none` field. The
underlying parser is shared with the client facade through `rttp-protocol`.
These helpers interoperate with adjacent response metadata helpers such as
`HttpResponse::cache_control()`, `HttpResponse::vary()`,
`HttpResponse::allow()`, and `HttpResponse::content_language()` by preserving
raw headers and parsing only when requested.

For conditional range requests, build `HttpConditionalMetadata` from the
selected representation and call `Request::evaluate_if_range(&metadata,
entity_length)` or `HttpRequest::evaluate_if_range(&metadata, entity_length)`.
The helper returns `PartialContent(HttpByteRange)` when the request can use the
parsed single byte range, `RangeNotSatisfiable` when the guarded range is
outside the representation, or `FullResponse` when there is no `Range` header
or an `If-Range` validator is absent, invalid, weak, stale, or missing from the
caller-provided metadata. Strong ETags use strong comparison, and HTTP-date
validators require an exact `Last-Modified` match at HTTP-date second
precision.

Multipart byte ranges are intentionally not serialized: RTTP does not generate
`Range` requests, `multipart/byteranges` responses, partial response engines,
byte serving, content slicing, download resume behavior, cache policy,
automatic retry/replay, redirect behavior, or status-policy decisions. There
is no built-in filesystem serving, path normalization, MIME selection, ETag or
Last-Modified generation, authorization, directory-index, or dotfile policy.
Those remain application decisions before choosing `200`, `206`, or `416`.

## Bounded HTTP/1.1 conditional requests

The server exposes validator evaluation helpers, not a static-file or cache
policy engine. Build representation metadata with
`HttpConditionalMetadata::new().entity_tag(HttpEntityTag::strong("tag"))` or
`HttpEntityTag::weak("tag")`, optionally add `last_modified(SystemTime)`, and
evaluate a request with `Request::evaluate_conditional(&metadata)` or
`evaluate_conditional_request(&request, &metadata)`.

Evaluation returns `HttpConditionalRequestOutcome::Proceed`,
`NotModified`, or `PreconditionFailed`. `If-Match` uses strong ETag comparison
and takes precedence over `If-Unmodified-Since`; a failed match returns
`PreconditionFailed`. `If-None-Match` uses weak ETag comparison and takes
precedence over `If-Modified-Since`; a match returns `NotModified` for `GET`
or `HEAD` and `PreconditionFailed` for other methods. `If-Modified-Since` and
`If-Unmodified-Since` are evaluated only when their HTTP-date values parse
successfully, and last-modified comparisons are performed at HTTP-date second
precision.

Handlers can also parse the raw validators without evaluation:
`Request::if_modified_since()`, `HttpRequest::if_modified_since()`, and the
matching `if_unmodified_since()` accessors expose one HTTP-date instant through
the shared protocol `HttpIfModifiedSince` and `HttpIfUnmodifiedSince` types.
Absent fields return `Ok(None)`; malformed, oversized, duplicate, or
control-byte values return a parse error while the raw field stays available
through `header()`. These accessors only parse metadata and do not change
evaluator precedence or comparison behavior.

Use `HttpResponse::not_modified(&metadata)` for `304 Not Modified`; it adds
available `ETag` and `Last-Modified` validators and serializes without a
message body. `HttpResponse::with_etag(HttpEntityTag::strong("tag"))` and
`HttpResponse::with_etag(HttpEntityTag::weak("tag"))` declare one bounded
response `ETag`, and `HttpResponse::etag()` parses attached singleton `ETag`
metadata with the same protocol entity-tag authority while preserving raw
headers when typed parsing fails. Use `HttpResponse::precondition_failed()` for
`412 Precondition Failed`; it returns an empty response unless application code
adds its own headers or body. Applications remain responsible for the normal
successful `200` response when evaluation returns `Proceed`.

`HttpResponse::with_schedule_tag(HttpScheduleTag::parse("\"sched-17\"")?)`
declares one bounded `Schedule-Tag` response field, and
`HttpResponse::schedule_tag()` parses attached singleton `Schedule-Tag`
metadata through the shared protocol `HttpScheduleTag` type backed by
`EntityTag`. The client facade re-exports the matching `ScheduleTag` type and
`Response::schedule_tag()` accessor. These helpers only declare and expose
response metadata; RTTP does not generate calendar versions, compare schedule
validators, inspect calendars, or apply scheduling policy.

ETag comparison is deliberately scoped to the supplied representation metadata.
The helpers do not pick entity tags, read files, check authorization, choose a
static-file policy, store cached responses, perform automatic revalidation, or
implement a full cache-control engine. Invalid conditional headers that cannot
be parsed as the bounded helper syntax are ignored by the evaluation helper
rather than rejected before handler code.

## Bounded HTTP/1.1 informational responses and Early Hints

`HttpResponse::early_hints(links)` constructs a bodyless `103 Early Hints`
response with one `Link` header per supplied value.
`HttpResponse::early_hints_with_headers(links, metadata)` adds validated
metadata headers alongside those links for applications that want to send
adjacent response metadata before a final response. Serialize the returned
model with the same response-writing path used for other `HttpResponse`
values, then write the final response separately.

The constructors are bounded and validation-oriented. They require at least
one non-empty `Link` value, bound each Early Hints field value to 64 KiB,
reject invalid field-value bytes, reject invalid metadata field names, and
keep `Link` values in the dedicated links argument. Metadata fields that would
affect connection state or body framing are rejected, including `Connection`,
`Content-Length`, `Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`,
`Transfer-Encoding`, and `Upgrade`. A body assigned to the `103` model is not
serialized, and `Content-Length` is not generated for it.

`103 Early Hints` is separate from `101 Switching Protocols`. `101` responses
remain bodyless terminal handoff responses for `HttpHandoff::upgrade` and
other caller-owned protocol transitions; they are not serialized as skipped
informational metadata. Raw headers attached through ordinary
`HttpResponse::header` calls remain preserved until a typed helper validates,
parses, or replaces the relevant field.

Early Hints support is metadata-only. The server does not execute preloads,
choose cache policy, redirect, retry, replay requests, generate routes, expose
a streaming early-write API, alter TLS/ALPN behavior, or decide final response
status from `103` metadata.

## Bounded HTTP/1.1 Cache-Control behavior

Server `Cache-Control` helpers parse directive metadata for application policy;
they do not enforce cache behavior. `Request::cache_control()` and
`HttpRequest::cache_control()` parse request directives into
`HttpRequestCacheControl`, including `no-cache`, `no-store`, `max-age`,
`max-stale` with or without a value, `min-fresh`, `no-transform`,
`only-if-cached`, and extension directives. `HttpResponse::cache_control()`
parses response headers already attached to an `HttpResponse` into
`HttpResponseCacheControl`, including `no-cache`, `no-store`, `max-age`,
`s-maxage`, `private`, `public`, `must-revalidate`, `proxy-revalidate`,
`immutable`, `stale-while-revalidate`, `stale-if-error`, quoted field-name
lists, and extension directives.

Unknown extension directives are preserved rather than discarded. Each
`HttpCacheControlExtension` exposes the directive token name and optional
parsed value, with quoted-string escaping removed when a quoted value is used.
The helpers do not interpret extension semantics or negotiate extension
behavior.

Parsing is bounded and validation-oriented. Each `Cache-Control` field value is
limited to 64 KiB, the parsed set is limited to 256 directives across all
values passed to the helper, directive names and unquoted values must be valid
HTTP tokens, quoted strings must be well formed, and delta-seconds values must
be unquoted non-negative decimal integers that fit in `u64`. Invalid
`Cache-Control` syntax returns `HttpCacheControlParseError` from the helper;
it does not by itself reject the request before handler code or remove the
original header from the response model.

These helpers are separate from conditional validator evaluation.
`Request::evaluate_conditional()` and `Request::evaluate_if_range()` use only
caller-supplied `HttpConditionalMetadata` and the conditional request headers.
They do not consult `Cache-Control` directives to decide whether a response is
fresh, whether it must be revalidated, or whether validators should be emitted
on a later request.

RTTP does not provide cache storage, automatic revalidation, freshness
calculation against wall-clock time, `Vary` matching, shared-cache policy
enforcement, or automatic conditional requests. Directives such as `max-age`,
`s-maxage`, `no-cache`, `only-if-cached`, `must-revalidate`, and extension
directives are exposed as parsed metadata for application-owned policy.

`HttpResponse::cache_status()` parses attached `Cache-Status` response fields
into `HttpCacheStatus` as a bounded RFC 9211 / RFC 8941 list of cache
identifiers and parameters. Repeated fields are combined in wire order. Each
field is limited to 64 KiB, the member count is limited to 256, each member is
limited to 256 parameters, and each parameter value is limited to 64 KiB.
Parse errors return `HttpCacheStatusParseError` and do not remove the original
raw response headers from the response model. An absent header returns
`Ok(None)`. The helper does not store cache entries, compute freshness,
revalidate, select endpoints, retry, or choose status behavior.

`HttpResponse::cdn_cache_control()` parses attached `CDN-Cache-Control`
response fields into `HttpCdnCacheControl` with the same bounded directive
model as response `Cache-Control`: 64 KiB per field, at most 256 directives,
valid HTTP tokens for directive names and unquoted values, and well-formed
quoted strings. CDN-specific extension directives are preserved in order with
their optional parsed values. Parse errors return
`HttpCdnCacheControlParseError` and do not remove the original raw response
headers from the response model.

CDN cache metadata remains metadata-only. RTTP does not create or manage a CDN
cache, compute freshness, evaluate surrogate keys, revalidate automatically,
enforce shared-cache policy, retry, replay, redirect, or choose status behavior
from `CDN-Cache-Control`.

Server `Date`, `Age`, `Expires`, and `Last-Modified` helpers expose adjacent
response metadata without adding cache policy.
`HttpResponse::with_age(delta_seconds)` adds an `Age` header from a
non-negative `u64` delta-seconds value, and

`HttpResponse::with_surrogate_control()` validates and replaces
`Surrogate-Control` response metadata, `HttpResponse::surrogate_control()`
parses attached raw response fields, and client `Response::surrogate_control()`
parses received fields through the same shared protocol type. Validation
accepts directives with optional token or quoted values, rejects duplicates
case-insensitively across fields, bounds each value and the aggregate header set
to 64 KiB, and limits parsed directives to 256. Surrogate metadata remains
metadata-only: RTTP does not create or manage a CDN cache, compute freshness,
evaluate surrogate keys, translate directives into `Cache-Control`, enforce
shared-cache policy, retry, replay, redirect, or choose status behavior from
`Surrogate-Control`.

`HttpResponse::age()` parses an attached single `Age` header back into `u64`.
The accepted bound is exactly the `u64` delta-seconds range: `0` through
`u64::MAX`. Empty, signed, fractional, non-numeric, comma-list, duplicated, and
overflowing values return `HttpAgeParseError`.

`HttpResponse::with_date(time)`, `with_expires(time)`, and
`with_last_modified(time)` serialize canonical IMF-fixdate `Date`, `Expires`,
and `Last-Modified` headers from `SystemTime`. `HttpResponse::date()`,
`expires()`, and `last_modified_date()` parse attached singleton HTTP-date
fields through the shared protocol primitives. IMF-fixdate, obsolete RFC 850
dates, and asctime dates are supported; malformed, duplicated, control-byte,
unsupported-time-zone, oversized, or non-date values return typed parse errors.

`HttpResponse::with_retry_after_delta(delta_seconds)` serializes a
delta-seconds `Retry-After` header from a non-negative `u64` through the shared
protocol `RetryAfter` formatter, while `HttpResponse::with_retry_after_date(time)`
serializes an HTTP-date `Retry-After` header from `SystemTime` through the same
primitive. `HttpResponse::retry_after()` parses an attached single
`Retry-After` header into the protocol-backed `HttpRetryAfter::DeltaSeconds` or
`HttpRetryAfter::HttpDate`. Empty, signed, fractional, non-numeric non-date,
comma-list, duplicated, overflowing, oversized, or malformed HTTP-date values
return `HttpRetryAfterParseError`.

Malformed typed helper reads return validation errors, while raw
`HttpResponse::header("Date", ...)`, `HttpResponse::header("Age", ...)`,
`HttpResponse::header("Expires", ...)`,
`HttpResponse::header("Last-Modified", ...)`, and
`HttpResponse::header("Retry-After", ...)` values remain preserved exactly as
response headers. These helpers do not calculate freshness, correct clock skew,
validate cache state against wall-clock time, store responses, match stored
responses, revalidate responses, enforce shared-cache policy, attach behavior
to status codes, throttle requests, sleep, retry, replay requests, apply
backoff, integrate with a scheduler, or issue automatic conditional requests.

Server `Memento-Datetime` helpers expose archival datetime metadata without
adding time negotiation. `HttpResponse::with_memento_datetime(time)` replaces
one `Memento-Datetime` header from `SystemTime`, and
`HttpResponse::memento_datetime()` parses an attached singleton field into
`HttpMementoDatetime`. Empty, malformed, control-byte, duplicate, and oversize
values return `HttpMementoDatetimeParseError`. Raw
`HttpResponse::header("Memento-Datetime", ...)` values remain preserved.
These helpers do not select an archival representation, negotiate
`Accept-Datetime`, implement TimeGate behavior, retry, or change transport
handling.

Server `Accept-Datetime` helpers expose the matching request metadata without
adding time negotiation. `Request::accept_datetime()` and
`HttpRequest::accept_datetime()` parse a singleton `Accept-Datetime` field
into `HttpAcceptDatetime`; absent fields return `Ok(None)`, and malformed,
control-byte, duplicate, and oversize values return
`HttpAcceptDatetimeParseError` while raw request headers remain preserved.
The client facade re-exports the matching `AcceptDatetime` type and
`HttpClient::accept_datetime()` builder. These helpers do not select an
archived representation, implement TimeGate behavior, add `Vary`, alter cache
policy, or feed conditional evaluation. A parsed `Accept-Datetime` instant
matches `HttpMementoDatetime` for the same HTTP-date, but the two helpers do
not negotiate with each other.

## Bounded HTTP/1.1 Vary behavior

Server-side `Vary` helpers expose response declaration and request-selection
metadata without implementing a cache. `HttpResponse::with_vary(value)` parses
and normalizes a response `Vary` value before adding the header, while
`HttpResponse::vary()` parses any `Vary` headers already attached to a
response. `HttpVary::parse(value)` accepts either `*` or a comma-separated
list of field names. Named fields are normalized to lowercase, deduplicated,
and exposed through `HttpVary::field_names()`.

`Request::vary_selection(&vary)` and `HttpRequest::vary_selection(&vary)`
collect the current request header values named by a parsed `HttpVary`, using
case-insensitive header-name matching. Wildcard `Vary` produces a wildcard
`HttpVarySelection` and does not read specific request headers. Named
selection values are metadata for application-owned policy; RTTP does not
compare them against stored cache keys.

Parsing is bounded and validation-oriented. Each `Vary` field value is limited
to 64 KiB, the parsed field-name list is limited to 256 entries, and each
named member must be a valid HTTP token. Empty members, malformed field names,
mixed wildcard/named values, oversized values, and too many field names return
`HttpVaryParseError` from the helper. A parse error does not by itself reject a
request before handler code or remove existing response headers.

These helpers do not add cache storage, a stored-response matching engine,
cache key persistence, automatic request replay, shared-cache policy
enforcement, or automatic conditional requests. Applications that build a
cache must persist any selected request metadata and enforce their own cache
policy around these helpers.

## Bounded No-Vary-Search metadata

`NoVarySearch` and `HttpNoVarySearch` expose bounded Structured Fields
response metadata for `No-Vary-Search`. Client responses can call
`Response::no_vary_search()`, while server responses can call
`HttpResponse::with_no_vary_search(value)` and
`HttpResponse::no_vary_search()`. The typed value exposes recognized
`key-order`, `params`, and `except` members and keeps extension dictionary
members as metadata.

These helpers do not create cache storage, match cache keys, normalize URLs,
replay requests, apply browser navigation behavior, or enforce shared-cache
policy.

## Bounded authentication metadata

`Request::authorization()` and `HttpRequest::authorization()` parse a single
`Authorization` field into `HttpAuthorization` metadata with `scheme()` and
`credentials()` accessors. They return `Ok(None)` when the field is absent and
reject invalid, oversized (over 64 KiB), or duplicate fields so credentials are
not ambiguous. `Request::proxy_authorization()` and
`HttpRequest::proxy_authorization()` expose `Proxy-Authorization` as
`HttpProxyAuthorization` with the same bounded opaque representation,
control-byte injection checks, duplicate handling, and redacted debug output.
Raw request headers remain available when either typed parser reports an error.

`HttpResponse::with_www_authenticate(value)` validates bounded challenges from
`rttp_protocol::www_authenticate`, replaces existing raw `WWW-Authenticate`
fields with one normalized declaration, and rejects malformed declarations
before emission. `HttpResponse::www_authenticate()` parses attached raw
challenge fields while preserving them on errors.

Credential interpretation remains application-owned. RTTP does not validate
credentials, select realms, challenge clients automatically, enforce
authentication or authorization decisions, verify schemes, store or refresh
credentials, retry, or forward credentials across redirects.

## Bounded Idempotency-Key request metadata

`HttpClient::idempotency_key(value)` validates and emits one opaque
`Idempotency-Key` request field through the shared `rttp_protocol`
`IdempotencyKey` type, replacing any existing same-name field before a socket
is opened. `Request::idempotency_key()` and `HttpRequest::idempotency_key()`
parse received fields into the same `HttpIdempotencyKey` representation,
returning `Ok(None)` when absent. A recognized value is a singleton key of one
or more visible ASCII characters bounded to 64 KiB with optional surrounding
SP or HTAB; empty, space-containing, control-byte (including CR/LF/NUL and
obs-text), duplicate, and oversized values are rejected. The key is redacted
from typed `Debug`, and raw request headers remain available when the typed
parser reports an error.

These helpers declare and observe request metadata only. RTTP does not retry
requests, store or compare keys across requests, deduplicate requests, or
apply application idempotency policy.

## Bounded WebDAV Depth request metadata

`HttpClient::depth(value)` validates and emits one WebDAV `Depth` request
field through the shared `rttp_protocol` `Depth` type, replacing any existing
same-name field before a socket is opened. `Request::depth()` and
`HttpRequest::depth()` parse received fields into the same `HttpDepth`
representation, returning `Ok(None)` when absent. Recognized values are the
singleton depth values `0`, `1`, and `infinity`, bounded to 64 KiB with
optional surrounding SP or HTAB; malformed, duplicate, oversized, and
control-byte values are rejected while raw request headers remain available
when the typed parser reports an error.

`HttpClient::timeout(value)` validates and emits one WebDAV `Timeout`
request field through the shared `rttp_protocol` `Timeout` type, replacing
any existing same-name field before a socket is opened. `Request::timeout()`
and `HttpRequest::timeout()` parse received fields into the same
`HttpTimeout` representation, returning `Ok(None)` when absent. Recognized
values are ordered `Second-n` and `Infinite` alternatives, bounded to 64 KiB
per field and 64 KiB aggregate with at most 32 members; malformed,
overflowing, duplicate, oversized, too-many-member, and control-byte values
are rejected while raw request headers remain available when the typed parser
reports an error. RTTP does not create locks, refresh locks, or select an
application timeout.

`HttpClient::overwrite(value)` validates and emits one WebDAV `Overwrite`
request field through the shared `rttp_protocol` `Overwrite` type, replacing
any existing same-name field before a socket is opened. `Request::overwrite()`
and `HttpRequest::overwrite()` parse received fields into the same
`HttpOverwrite` representation, returning `Ok(None)` when absent. Recognized
values are the singleton tokens `T` and `F`, bounded to 64 KiB with optional
surrounding SP or HTAB and case-sensitive matching; malformed, duplicate,
oversized, and control-byte values are rejected while raw request headers
remain available when the typed parser reports an error. RTTP does not
overwrite destination resources, apply the RFC 4918 default `T` when the
field is absent, or enforce WebDAV policy.

These helpers declare and observe request metadata only. RTTP does not
traverse resources, select WebDAV methods, or enforce method policy.

## Bounded WebDAV Lock-Token metadata

`HttpClient::lock_token(value)` validates and emits one WebDAV `Lock-Token`
request field through the shared `rttp_protocol` `LockToken` type, replacing
any existing same-name field before a socket is opened.
`Request::lock_token()` and `HttpRequest::lock_token()` parse received fields
into the same `HttpLockToken` representation, returning `Ok(None)` when
absent. `HttpResponse::with_lock_token(value)` validates and replaces one
response field, and `HttpResponse::lock_token()` plus `Response::lock_token()`
parse retained response headers. A recognized value is exactly one
angle-bracketed absolute URI bounded to 64 KiB with optional surrounding SP
or HTAB; empty, unbracketed, relative, comma-list, extra-bracket, duplicate,
oversized, and control-byte values are rejected while raw headers remain
available when the typed parser reports an error. The token is redacted from
typed `Debug`.

These helpers declare and observe metadata only. RTTP does not create,
refresh, release, persist, compare ownership of, or enforce WebDAV locks.

## Bounded WebDAV Destination request metadata

`HttpClient::destination(value)` validates and emits one WebDAV
`Destination` request field through the shared `rttp_protocol`
`Destination` type, replacing any existing same-name field before a socket
is opened. `Request::destination()` and `HttpRequest::destination()` parse
received fields into the same `HttpDestination` representation, returning
`Ok(None)` when absent. A recognized value is one absolute URI, bounded to
64 KiB with optional surrounding SP or HTAB; the trimmed URI string is
preserved without resolution or normalization. Malformed, relative,
duplicate, oversized, and control-byte values are rejected while raw request
headers remain available when the typed parser reports an error.

These helpers declare and observe request metadata only. RTTP does not
resolve the destination, authorize the target URI, select WebDAV methods, or
copy, move, or delete application resources.

## Bounded WebDAV If request metadata

`HttpClient::if_header(value)` validates and emits one RFC 4918 section 10.4
WebDAV `If` request field through the shared `rttp_protocol` `If` type,
replacing any existing same-name field before a socket is opened.
`Request::if_header()` and `HttpRequest::if_header()` parse received fields
into the same `HttpIf` representation, returning `Ok(None)` when absent. A
recognized value is either entirely untagged
(`(<opaquelocktoken:...>) (Not <DAV:no-lock>)`) or entirely tagged
(`<http://example.test/src> (<opaquelocktoken:...>) </dst> (Not <DAV:no-lock>)`);
mixed productions are rejected. Each field value and the aggregate input are
bounded to 64 KiB, the combined value is bounded to 32 lists and 256
conditions. Resource tags accept an RFC 3986 `Simple-ref` (absolute-URI or
path-absolute with optional query); state tokens are absolute coded URLs,
including `<DAV:no-lock>`; conditions accept the case-sensitive `Not` token
only when it is followed by SP or HTAB and then a state token or a bracketed
entity tag (`[W/"etag"]`). List
order, repeated lists, and resource tags are preserved, and
`header_value()` re-emits the canonical field text. Empty, unterminated,
mixed, malformed, duplicate, oversized, too-many-list, too-many-condition,
and control-byte values are rejected while raw request headers remain
available when the typed parser reports an error. State tokens are redacted
from typed `Debug` and raw-header debug output.

These helpers declare and observe request metadata only. RTTP does not
evaluate locks, entity tags, or other resource state, and it does not
generate precondition outcomes such as 412 Precondition Failed.

## Bounded If-Schedule-Tag-Match request metadata

`HttpClient::if_schedule_tag_match(value)` validates and emits one
`If-Schedule-Tag-Match` request field through the shared `rttp_protocol`
`IfScheduleTagMatch` type, which reuses the shared `EntityTag`
representation, replacing any existing same-name field before a socket is
opened. `Request::if_schedule_tag_match()` and
`HttpRequest::if_schedule_tag_match()` parse received fields into the same
`HttpIfScheduleTagMatch` representation, returning `Ok(None)` when absent. A
recognized value is one entity-tag-shaped schedule validator such as
`"sched-17"` or `W/"sched-17"`, bounded to 64 KiB with optional surrounding
SP or HTAB and weak syntax preserved; malformed, wildcard, comma-list,
duplicate, oversized, and control-byte values are rejected while raw request
headers remain available when the typed parser reports an error.

These helpers declare and observe request metadata only. RTTP does not
compare the validator to stored calendar state, inspect calendars, select
scheduling behavior, or apply 412 or scheduling policy.

## Bounded Sec-WebSocket-Key request metadata

`HttpClient::sec_websocket_key(value)` validates and emits one
`Sec-WebSocket-Key` request field through the shared `rttp_protocol`
`SecWebSocketKey` type, replacing any existing same-name field before a socket
is opened. `Request::sec_websocket_key()` and `HttpRequest::sec_websocket_key()`
parse received fields into the same `HttpSecWebSocketKey` representation,
returning `Ok(None)` when absent. A recognized value is a singleton RFC 4648
section 4 base64 encoding of exactly 16 nonce bytes bounded to 64 KiB with
optional surrounding SP or HTAB; empty, interior-whitespace, non-base64,
URL-safe or unpadded, wrong-decoded-length, control-byte (including CR/LF/NUL
and obs-text), duplicate, and oversized values are rejected. The nonce is
redacted from typed `Debug`, and raw request headers remain available when the
typed parser reports an error.

`HttpResponse::with_sec_websocket_accept_for_key()` derives the RFC 6455
`Sec-WebSocket-Accept` value from a validated key, and
`Response::sec_websocket_accept()` plus `verify_sec_websocket_accept()` parse
and verify response metadata against that same GUID plus SHA-1/base64
transform. The RFC example key `dGhlIHNhbXBsZSBub25jZQ==` maps to
`s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.

These helpers declare and observe handshake metadata only. RTTP does not
perform an HTTP upgrade, generate a random nonce, or implement WebSocket
frames.

## Bounded Sec-WebSocket-Version request and response metadata

`HttpClient::sec_websocket_version(value)` validates and emits
`Sec-WebSocket-Version` request metadata through the shared `rttp_protocol`
`SecWebSocketVersion` type, replacing any existing same-name field before a
socket is opened. `Request::sec_websocket_version()` and
`HttpRequest::sec_websocket_version()` parse received fields into the same
`HttpSecWebSocketVersion` representation, returning `Ok(None)` when absent.
`HttpResponse::with_sec_websocket_version(versions)` declares validated
rejection-response metadata without adding `Connection` or `Upgrade`, and
`HttpResponse::sec_websocket_version()` plus client `Response::sec_websocket_version()`
parse attached or received fields. Recognized values are RFC 6455 version
tokens (`0` through `299` without leading zeros) in numeric descending order,
such as `13` or `13, 8, 7`. Empty members, non-decimal tokens, leading-zero
multi-digit tokens, duplicates, unordered lists, control-byte, over-limit, and
oversized values are rejected while raw headers remain available.

These helpers declare and observe metadata only. RTTP does not perform a
WebSocket handshake, emit `Connection: Upgrade`, compute
`Sec-WebSocket-Accept`, negotiate versions, or switch protocols.

## Bounded Sec-WebSocket-Protocol request and response metadata

`HttpClient::sec_websocket_protocol(value)` validates and emits
`Sec-WebSocket-Protocol` request metadata as offers in preference order
through the shared `rttp_protocol` `SecWebSocketProtocol` type, replacing any
existing same-name field before a socket is opened. `Request::sec_websocket_protocol()`
and `HttpRequest::sec_websocket_protocol()` parse received fields into the
same `HttpSecWebSocketProtocol` representation, returning `Ok(None)` when
absent. `HttpResponse::with_sec_websocket_protocol(token)` declares validated
response metadata for one selected token without adding `Connection` or
`Upgrade`, and `HttpResponse::sec_websocket_protocol()` plus client
`Response::sec_websocket_protocol()` parse attached or received fields as a
selection singleton. Recognized members are RFC 6455 section 11.3.4 `token`
values such as `chat`, `superchat`, or `graphql-transport-ws`, compared
case-sensitively. Empty members, malformed tokens, parameters, slashes,
duplicates, control-byte, over-limit, and oversized values are rejected while
raw headers remain available; a multi-token response value fails the
singleton selection parse.

These helpers declare and observe metadata only. RTTP does not perform a
WebSocket handshake, emit `Connection: Upgrade`, choose an application
subprotocol, or switch protocols. Applications own the selection decision;
RTTP never picks a token from the offer list.

## Bounded Sec-WebSocket-Extensions request and response metadata

`HttpClient::sec_websocket_extensions(value)` validates and emits
`Sec-WebSocket-Extensions` request metadata as ordered extension offers
through the shared `rttp_protocol` `SecWebSocketExtensions` type, replacing
any existing same-name field before a socket is opened. `Request::sec_websocket_extensions()`
and `HttpRequest::sec_websocket_extensions()` parse received fields into the
same `HttpSecWebSocketExtensions` representation, returning `Ok(None)` when
absent. `HttpResponse::with_sec_websocket_extensions(value)` declares
validated response metadata for one selected extension without adding
`Connection` or `Upgrade`, and `HttpResponse::sec_websocket_extensions()`
plus client `Response::sec_websocket_extensions()` parse attached or received
fields as a selection singleton. The canonical representation validates
extension tokens, ordered parameters, token and quoted-string parameter
values, duplicates, member count, and total size while preserving raw headers
on parse errors.

These helpers declare and observe metadata only. RTTP does not perform a
WebSocket handshake, emit `Connection: Upgrade`, activate compression,
negotiate extensions, or switch protocols. Applications own all extension
behavior.

## Bounded W3C Trace Context request metadata

`HttpClient::traceparent()` and `HttpClient::tracestate()` validate and emit
bounded W3C Trace Context request metadata through shared `rttp_protocol`
`TraceParent` and `TraceState` types, replacing existing same-name fields
before a socket is opened. `Request::traceparent()` / `HttpRequest::traceparent()`
and `Request::tracestate()` / `HttpRequest::tracestate()` parse received fields,
returning `Ok(None)` when absent and preserving raw headers on parse errors.

Traceparent validation checks version `00`, rejects version `ff` and
unsupported versions, malformed or uppercase identifiers, malformed flags,
duplicates, and all-zero trace or parent identifiers. Tracestate validation
preserves member order while bounding total size, member count, key/value size,
member grammar, and duplicate keys. Typed `Debug` redacts propagation values.
These helpers declare and observe request metadata only: RTTP does not create
trace identifiers, decide sampling, select a tracing backend, or automatically
propagate context.

## Bounded W3C Baggage request metadata

`HttpClient::baggage()` validates and emits bounded W3C Baggage request
metadata through the shared `rttp_protocol` `Baggage` type, replacing any
existing `baggage` field before a socket is opened. `Request::baggage()` /
`HttpRequest::baggage()` parse received fields, returning `Ok(None)` when
absent and preserving raw headers on parse errors.

Baggage validation preserves member order while bounding total size, member
count, per-member size, key/value/property grammar, and duplicate keys. Typed
`Debug` redacts member and property values. These helpers declare and observe
request metadata only: RTTP does not interpret application baggage data,
store request context, select a tracing backend, or automatically propagate
baggage.

## Bounded CDN-Loop request metadata

`HttpClient::cdn_loop()` validates and emits bounded RFC 8586 `CDN-Loop`
request metadata through the shared `rttp_protocol` `CdnLoop` type, combining
any existing `CDN-Loop` field with the new member in wire order before a
socket is opened. `Request::cdn_loop()` / `HttpRequest::cdn_loop()` parse
received fields into `HttpCdnLoop`, returning `Ok(None)` when absent and
preserving raw headers on parse errors.

`CdnLoop` validates opaque CDN identifiers (`uri-host` with optional port or
an RFC 7230 token pseudonym), optional HTTP parameters, and bounds: 64 KiB per
field value, per combined raw field set including `", "` separator overhead,
and per combined serialized value, 256 combined members, and 32
parameters per member. Duplicate parameter names are rejected; repeated CDN
identifiers are valid loop-visible metadata. These helpers expose loop
metadata only: RTTP does not detect or break loops, reject requests because an
identifier is already present, insert a local CDN identifier, forward the
field automatically, or treat `CDN-Loop` as hop-by-hop.

## Bounded Via request and response metadata

`HttpClient::via()` validates and emits bounded HTTP `Via` request metadata
through the shared `rttp_protocol` `Via` type, combining any existing `Via`
field with the new hops in wire order before a socket is opened.
`Request::via()` / `HttpRequest::via()` parse received fields into `HttpVia`,
returning `Ok(None)` when absent and preserving raw headers on parse errors.
`Response::via()` parses the same representation from client responses.
`HttpResponse::with_via()` replaces raw response fields with one validated
caller-supplied chain, and `HttpResponse::via()` parses attached raw fields.

`Via` validates received-protocol, received-by, comments, and bounds: 64 KiB
per field value, per combined raw field set including `", "` separator
overhead, per combined serialized value, 256 combined members, and 128
comment nesting levels. Duplicate hops are preserved in wire order. These
helpers expose hop metadata only: RTTP does not append or remove hops, infer
trusted proxies, or change HTTP/1.1 or HTTP/2 proxy policy.

## Bounded X-Forwarded compatibility metadata

`HttpClient::x_forwarded_for()`, `x_forwarded_host()`, and
`x_forwarded_proto()` validate and emit bounded `X-Forwarded-For`,
`X-Forwarded-Host`, and `X-Forwarded-Proto` request metadata through the
shared protocol types. `Request` and `HttpRequest` parse the same
representations, return `Ok(None)` when absent, and preserve raw headers on
parse errors.

`XForwardedFor` preserves ordered IP node values and `unknown`;
`XForwardedHost` preserves ordered host authorities; and `XForwardedProto`
preserves ordered URI scheme tokens. Each field family is bounded to 64 KiB
per field value, 64 KiB for the combined raw field set including `", "`
separator overhead, 64 KiB for serialized output, and 256 members. Repeated
fields are combined in wire order.

These helpers expose compatibility metadata only. RTTP does not trust,
rewrite, or enforce forwarded identity, select a client address, change
routing, redirect, upgrade, or choose a trusted proxy set. Applications that
use these fields must choose and enforce their own trusted proxies.

## Bounded HTTP/1.1 Allow behavior

Server-side `Allow` helpers expose response declaration and method-list parsing
metadata without implementing method negotiation or automatic status handling.
`HttpResponse::with_allow(methods)` validates an explicit method list and adds
one comma-separated `Allow` header, while `HttpResponse::allow()` parses any
`Allow` headers already attached to a response into `HttpAllowedMethods`.
`HttpAllowedMethods::parse(value)` accepts a comma-separated list of HTTP
method tokens and preserves the declared token spelling and order.

Parsing is bounded and validation-oriented. Each `Allow` field value is
limited to 64 KiB, the parsed method list is limited to 32 entries, and each
member must be a valid HTTP token. Empty members, malformed method tokens,
duplicates across one or more helper-parsed header fields, oversized values,
and too many methods return `HttpAllowParseError` from the helper. Raw
`HttpResponse::header("Allow", ...)` values remain preserved exactly as
ordinary response headers; helper parse errors do not remove existing headers.

These helpers are metadata-only. RTTP does not choose fallback methods, retry
or replay requests, implement `OPTIONS` policy, generate `405` responses,
dispatch routes, or provide a status-code policy engine from `Allow`.

## Bounded HTTP/1.1 Content-Language behavior

Server-side `Content-Language` helpers expose request and response metadata
without implementing language negotiation, locale fallback, or variant
matching.
`Request::content_language()` and `HttpRequest::content_language()` parse
received `Content-Language` fields in wire order into `HttpContentLanguages`
and return `Ok(None)` when the header is absent. HTTP/1.1 and HTTP/2 share
the same `Request` helpers.
`HttpResponse::with_content_language(languages)` validates an explicit language
tag list and adds one comma-separated `Content-Language` header, while
`HttpResponse::content_language()` parses any `Content-Language` headers
already attached to a response into `HttpContentLanguages`.
`HttpContentLanguages::parse(value)` accepts comma-separated language tags and
preserves the declared spelling and order; the parsed tags are exposed by both
`HttpContentLanguages::languages()` and `HttpContentLanguages::tags()`.

Parsing is bounded and validation-oriented. Each `Content-Language` field value
is limited to 64 KiB, the parsed language list is limited to 256 entries, and
each tag must match the BCP 47-shaped grammar enforced by the shared protocol
primitive: language, optional extlang, script, region, variant, extension, and
private-use subtags, plus registered grandfathered tags. Empty members,
malformed tags, duplicates across one or more helper-parsed header fields,
oversized values, and too many tags return `HttpContentLanguageParseError` from
the helper. Raw
`Request::header("Content-Language", ...)` and
`HttpResponse::header("Content-Language", ...)` values remain preserved exactly
as ordinary headers; helper parse errors do not remove existing headers.

These helpers parse request metadata only and interoperate with adjacent
helpers such as `Request::accept_language()`, `HttpResponse::cache_control()`,
`HttpResponse::allow()`, and `HttpResponse::vary()` by preserving raw headers
and parsing only when requested. They do not sniff, decode, negotiate, cache,
redirect, retry, or select representations from `Content-Language`.

## Bounded Accept request metadata

`HttpClient::accept()` and `accept_with_q()` validate helper-built request
media ranges through the shared `rttp-protocol` Accept primitive while keeping
the client facade's existing 32-helper-range limit and raw
`header(("Accept", value))` escape hatch. Helper q-value arguments preserve the
existing facade boundary by accepting legacy empty fractional forms such as
`0.` and `1.` while rejecting surrounding whitespace. `Request::accept()` and
`HttpRequest::accept()` expose the same protocol representation as
`HttpAccept`/`HttpMediaRange`, returning `Ok(None)` when absent and preserving
raw headers when malformed, duplicate, oversized, or excessive values fail typed
parsing.

These helpers declare and parse metadata only. They do not select response
representations, retry, redirect, cache, or apply server negotiation policy.

## Bounded Accept-Charset request metadata

Server-side `Accept-Charset` helpers expose request metadata through the
shared `rttp-protocol` primitive. `Request::accept_charset()` and
`HttpRequest::accept_charset()` parse all received `Accept-Charset` fields
in wire order into `HttpRequestAcceptCharsets` and return `Ok(None)` when
the header is absent. HTTP/1.1 and HTTP/2 share the same `Request` helpers.
Each entry provides its `charset()` and q-value `quality()` in thousandths
(`1000` is the default quality of `1`). The shared protocol type is the
authority for charset-range, wildcard, q-value, duplicate, member-count, and
size validation.

Parsing is bounded and validation-oriented. Each `Accept-Charset` field value
is limited to 64 KiB, the combined list is limited to 32 members, and each
range must be an RFC 9110 token, including `*`. Empty members, malformed
tokens or q-values, duplicates across one or more helper-parsed header
fields, oversized values, and too many members return
`HttpAcceptCharsetParseError` from the helper. Raw
`Request::header("Accept-Charset", ...)` values remain preserved exactly as
ordinary headers; helper parse errors do not remove existing headers.

These helpers parse request metadata only. They do not negotiate, transcode,
decode bodies, sniff MIME types, or select a response charset.

## Bounded A-IM request metadata

`HttpClient::a_im()`, `a_im_with_q()`, and `a_im_value()` emit bounded `A-IM`
request metadata through the shared `rttp-protocol` primitive. Server-side
`Request::a_im()` and `HttpRequest::a_im()` parse all received `A-IM` fields
in wire order into `HttpAIm` and return `Ok(None)` when the header is absent.
Each entry provides its `token()`, q-value `quality()` in thousandths
(`1000` is the default quality of `1`), and ordered `parameters()`. The
shared protocol type is the authority for token, q-value, parameter,
duplicate, member-count, and size validation.

Parsing is bounded and validation-oriented. Each `A-IM` field value and the
combined raw field set are limited to 64 KiB, the combined list is limited to
32 members, and each member is limited to 16 parameters. Empty members,
malformed tokens, q-values, or parameters, duplicates across one or more
helper-parsed header fields, oversized values, and too many members return
`HttpAImParseError` from the helper. Raw `Request::header("A-IM", ...)`
values remain preserved exactly as ordinary headers; helper parse errors do
not remove existing headers.

These helpers declare and parse request metadata only. They do not select a
preferred instance manipulation or apply delta encodings.

## Bounded IM response metadata

`HttpResponse::with_im(members)` validates an ordered list of
instance-manipulation tokens, replaces any existing raw `IM` fields with one
validated `IM` header, and returns a parse error for invalid, duplicate,
empty, oversized, or over-limit members. `HttpResponse::im()` and client
`Response::im()` parse attached `IM` fields in wire order into the shared
`rttp-protocol` `Im` type (`HttpIm` on the server), and return `Ok(None)`
when the header is absent. Present values expose `members()` with `token()`
and `parameters()` per member. Each field value and the combined raw field
set are limited to 64 KiB, the combined list is limited to 32 members, and
each member is limited to 16 parameters. Malformed, duplicate, or over-limit values
return an error while raw `IM` fields remain observable as ordinary headers.

These helpers only declare and inspect response metadata. They do not decode
or apply instance manipulations and do not require or synthesize the
`226 IM Used` status.

## Bounded Negotiate request metadata

`HttpClient::negotiate(value)` emits one bounded RFC 2295 `Negotiate` request
field through the shared `rttp-protocol` primitive, replacing any existing
same-name field before a socket is opened. Server-side `Request::negotiate()`
and `HttpRequest::negotiate()` parse all received `Negotiate` fields in wire
order into `HttpNegotiate` and return `Ok(None)` when the header is absent.
Each `HttpNegotiateDirective` is a `trans`, `vlist`, `guess-small`, or `*`
flag, a `major.minor` remote variant selection algorithm version, or a
`token[=token]` extension. The shared protocol type is the authority for
token, version, feature value, duplicate, member-count, and size validation.

Parsing is bounded and validation-oriented. Each `Negotiate` field value and
the combined raw field set are limited to 64 KiB, and the combined list is
limited to 32 members. Flags match case-insensitively and emit lowercase,
versions are normalized to `major.minor`, and duplicate directives are
rejected: at most one of each flag and `*`, one occurrence of each version
pair, and one extension per case-insensitive name. Empty members, malformed
tokens, valued flags or versions, duplicates across one or more
helper-parsed header fields, oversized values, and too many members return
`HttpNegotiateParseError` from the helper. Raw
`Request::header("Negotiate", ...)` values remain preserved exactly as
ordinary headers; helper parse errors do not remove existing headers.

These helpers declare and parse request metadata only. They do not select a
variant, run transparent content negotiation, or change cache selection.

## Bounded Accept-Encoding request metadata

Server-side `Accept-Encoding` helpers expose request metadata through the
shared `rttp-protocol` primitive. `Request::accept_encoding()` and
`HttpRequest::accept_encoding()` parse all received `Accept-Encoding` fields
in wire order into `HttpRequestAcceptEncodings` and return `Ok(None)` when
the header is absent. HTTP/1.1 and HTTP/2 share the same `Request` helpers.
Each entry provides its `coding()` and q-value `quality()` in thousandths
(`1000` is the default quality of `1`). The shared protocol type is the
authority for coding-token, wildcard, q-value, duplicate, member-count, and
size validation.

Parsing is bounded and validation-oriented. Each `Accept-Encoding` field value
is limited to 64 KiB, the combined list is limited to 32 members, and each
coding must be an RFC 9110 token, including `identity` and `*`. Empty members,
malformed tokens or q-values, duplicates across one or more helper-parsed
header fields, oversized values, and too many members return
`HttpAcceptEncodingParseError` from the helper. Raw
`Request::header("Accept-Encoding", ...)` values remain preserved exactly as
ordinary headers; helper parse errors do not remove existing headers.

These helpers parse request metadata only. They do not enable automatic
compression, decompression, or content negotiation.

## Bounded Sec-GPC request metadata

`HttpClient::sec_gpc()` emits `Sec-GPC: 1` through the shared protocol
representation. Server-side `Request::sec_gpc()` and
`HttpRequest::sec_gpc()` parse the same bounded singleton `1` signal and
return `Ok(None)` when the field is absent. Malformed, oversized, duplicate,
or control-byte values return a parser error while raw request headers remain
available.

These helpers only declare or parse request metadata. RTTP does not infer or
enforce consent, tracking, legal, or serving policy.

## Bounded Pragma metadata

`HttpClient::pragma(value)` and `HttpClient::pragma_no_cache()` emit bounded
RFC 9111 `Pragma` request metadata through the shared protocol `Pragma` type,
combining and replacing already-attached same-name fields. Server-side
`Request::pragma()`, `HttpRequest::pragma()`, and `HttpResponse::pragma()`
parse the same representation into `HttpPragma`, and
`HttpResponse::with_pragma(value)` declares validated response metadata that
replaces attached same-name fields. Absent fields return `Ok(None)`. Multiple
fields are combined in wire order, directive names are matched
case-insensitively, duplicate names are rejected, each field value is bounded
to 64 KiB, combined field values are bounded to 64 KiB including `", "`
separator overhead, each directive value is bounded to 64 KiB, and the combined
directive count is bounded to 256. Malformed tokens or quoted-strings, valued
`no-cache` forms, empty members, and bound violations return a parser error
while raw request headers remain available.

These helpers only declare or parse metadata. RTTP does not translate `Pragma`
into `Cache-Control`, store cache entries, or apply cache, intermediary, or
HTTP/1.0 compatibility policy.

## Bounded HTTP/1.1 Content-Location behavior

Server-side `Content-Location` helpers expose response metadata declaration and
parsing through the shared protocol-owned `HttpContentLocation` type without
implementing redirect handling, cache selection, or route policy.
`HttpResponse::with_content_location(value)` validates one `Content-Location`
URI-reference field value, trims outer whitespace, removes any existing raw
`Content-Location` fields, and adds a single validated `Content-Location`
header. `HttpResponse::content_location()` parses any attached
`Content-Location` header into `HttpContentLocation` and returns `Ok(None)`
when the header is absent.

Parsing is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty absolute URI or relative URI reference without
control characters, interior whitespace, unsafe field-value characters,
malformed URI syntax, or broken percent-encoding. Duplicate
`Content-Location` fields are rejected because the helper treats the header as
singleton response metadata. Malformed values, duplicated singleton fields, and
oversized values return `HttpContentLocationParseError` from the helper. Raw
`HttpResponse::header("Content-Location", ...)` values remain preserved exactly
as ordinary response headers until a typed declaration helper replaces them or
the typed parser is requested.

These helpers interoperate with adjacent response metadata helpers such as
`HttpResponse::cache_control()`, `HttpResponse::allow()`,
`HttpResponse::content_language()`, `HttpResponse::vary()`,
`HttpResponse::retry_after()`, `HttpResponse::age()`,
`HttpResponse::expires()`, and `HttpResponse::accept_ranges()` by preserving
raw headers and parsing only when requested. They are metadata-only: RTTP does
not treat `Content-Location` as redirect behavior, cache variant selection,
representation replacement, retry/replay behavior, route generation, or
status-policy behavior.

## Bounded HTTP/1.1 Service-Worker-Allowed behavior

Server-side `Service-Worker-Allowed` helpers expose response metadata
declaration and parsing through the shared protocol-owned
`HttpServiceWorkerAllowed` type without registering service workers or
resolving application routing policy.
`HttpResponse::with_service_worker_allowed(value)` validates one
`Service-Worker-Allowed` origin-relative or absolute path field value, trims
outer whitespace, removes any existing raw `Service-Worker-Allowed` fields,
and adds a single validated `Service-Worker-Allowed` header.
`HttpResponse::service_worker_allowed()` parses any attached
`Service-Worker-Allowed` header into `HttpServiceWorkerAllowed` and returns
`Ok(None)` when the header is absent.

Parsing is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty origin-relative or absolute path without
control or non-ASCII characters, interior whitespace, unsafe field-value characters, broken
percent-encoding, absolute URIs, or network-path authority forms. Duplicate
`Service-Worker-Allowed` fields are rejected because the helper treats the
header as singleton response metadata. Malformed values, duplicated singleton
fields, and oversized values return `HttpServiceWorkerAllowedParseError` from
the helper. Raw `HttpResponse::header("Service-Worker-Allowed", ...)` values
remain preserved exactly as ordinary response headers until a typed
declaration helper replaces them or the typed parser is requested.

These helpers are metadata-only: RTTP does not register service workers,
evaluate service-worker scope, resolve the value against a script URL, or
apply application routing policy from `Service-Worker-Allowed`.

## Bounded HTTP/1.1 Content-DPR behavior

Server-side `Content-DPR` helpers expose response metadata declaration and
parsing through the shared protocol-owned `HttpContentDpr` type without
rescaling images or applying Client Hints policy.
`HttpResponse::with_content_dpr(value)` validates one `Content-DPR` field
value, trims outer whitespace, removes any existing raw `Content-DPR` fields,
and adds a single validated `Content-DPR` header. `HttpResponse::content_dpr()`
parses any attached `Content-DPR` header into `HttpContentDpr` and returns
`Ok(None)` when the header is absent.

Parsing is bounded and validation-oriented. The field value is limited to
64 KiB and must match `1*DIGIT["." 1*DIGIT]` as a finite ratio greater than
zero. Duplicate `Content-DPR` fields are rejected because the helper treats the
header as singleton response metadata. Malformed values, duplicated singleton
fields, and oversized values return `HttpContentDprParseError` from the helper.
Raw `HttpResponse::header("Content-DPR", ...)` values remain preserved exactly
as ordinary response headers until a typed declaration helper replaces them or
the typed parser is requested.

These helpers are observation-only. RTTP does not rescale images, send request
DPR, apply Client Hints policy, retry, replay, redirect, or change transport
from `Content-DPR`.

## Bounded HTTP/1.1 Content-Disposition behavior

Server-side `Content-Disposition` helpers expose response metadata declaration
and parsing without implementing download policy, filesystem handling, MIME
sniffing, cache behavior, redirect handling, retry, or negotiation.
`HttpContentDisposition` is backed by the shared protocol primitive.
`HttpContentDisposition::parse(value)` validates one field value, including a
token disposition type and bounded parameters. `HttpContentDisposition::inline()`
and `HttpContentDisposition::attachment()` construct common dispositions, and
`with_parameter(name, value)` adds safely serialized parameters.

`HttpResponse::with_content_disposition(value)` validates the provided model or
field value, removes any existing raw `Content-Disposition` fields, and adds one
validated `Content-Disposition` header. `HttpResponse::with_attachment_filename`
is a convenience helper for `attachment; filename=...`.
`HttpResponse::content_disposition()` parses an attached singleton header and
returns `Ok(None)` when the header is absent.

Parsing is bounded and validation-oriented. The field value is limited to
64 KiB, parameter count is limited to 256, each parameter value is limited to
64 KiB, token positions must be valid HTTP tokens, quoted-string input must be
well formed, `filename*` must be an unquoted RFC 5987 ext-value, and CR/LF or
other control characters other than HTAB are rejected. Duplicate parameters
and duplicate `Content-Disposition` fields are rejected by the typed parser.
Raw `HttpResponse::header("Content-Disposition", ...)` values remain preserved
exactly as ordinary response headers until a typed declaration helper replaces
them or the typed parser is requested.

`filename` and `filename*` are preserved as separate parameters. The helper
serializes the parameter values it is given, parses those values back as
metadata, and does not decode RFC 5987 extended values or choose between
`filename` and `filename*`. Applications remain responsible for any filename
precedence, display, storage, or download policy. These helpers are
metadata-only: RTTP does not start automatic downloads, derive filesystem
paths, sniff MIME types, negotiate response variants, redirect, retry/replay,
cache, or attach status-code policy from `Content-Disposition`.

## Bounded NEL response metadata

Server-side NEL helpers expose response declaration and parsing without
implementing network error reporting. `HttpResponse::with_nel(value)` validates
one `NEL` field as bounded W3C Network Error Logging policy JSON, removes any
existing raw `NEL` fields, and adds one normalized value.
`HttpResponse::nel()` parses an attached raw field and returns `Ok(None)` when
the header is absent.

On the client, `Response::nel()` parses the `NEL` response field into the same
typed `Nel` policy metadata, returning `Ok(None)` when the header is absent
and preserving raw headers on parse failures. The policy exposes its required
non-negative `max_age` as `u64`, optional `report_to` name,
`include_subdomains` flag, and `success_fraction`/`failure_fraction` values as
checked members; unknown JSON members are preserved verbatim without policy
semantics. Field values are bounded to 64 KiB, member counts to 256 per
object, nesting depth to 64, and each decoded string to 64 KiB.

These helpers are metadata-only: RTTP does not send network error reports,
persist policy, or configure Reporting endpoint groups.

## Bounded Alt-Used response metadata

`AltUsed` and `HttpAltUsed` share the protocol authority parser for bounded
`Alt-Used` response metadata. Client responses can call `Response::alt_used()`
to parse one attached field, and server responses can call
`HttpResponse::with_alt_used(value)` or `HttpResponse::alt_used()` to declare
or observe the same metadata. Valid metadata preserves host spelling,
optional port, and bracketed IPv6 literal form. Malformed authorities,
duplicate fields, and values larger than 64 KiB are rejected while raw headers
remain available on parse failures.

These helpers are metadata-only: RTTP does not select alternative services,
rewrite origins, migrate sockets, retry, or change connection policy from
`Alt-Used`.

## Bounded Alternates response metadata

`Alternates` and `HttpAlternates` share the protocol parser for bounded
RFC 2295-style `Alternates` response metadata. Client responses can call
`Response::alternates()` to parse attached fields, and server responses can
call `HttpResponse::with_alternates(value)` or `HttpResponse::alternates()`
to declare or observe the same metadata. Valid metadata preserves variant
URI-references, the original accepted source-quality text, and attributes
such as `type`, `language`, `encoding`, and `length`. Each field value is
limited to 64 KiB, the combined field bytes are limited to 64 KiB, the
variant count is limited to 256, and each variant holds at most 256
attributes. Malformed members, invalid URIs, invalid qvalues, duplicate
attributes or variants, and oversized values are rejected while raw headers
remain available on parse failures.

These helpers are metadata-only: RTTP does not select a variant, fetch a
variant URI, replay requests, resolve URIs against the response URL, apply
`Vary` matching, or change representation policy from `Alternates`.

## Bounded TCN response metadata

`Tcn` and `HttpTcn` share the protocol parser for bounded RFC 2295 `TCN`
response metadata. Client responses can call `Response::tcn()` to parse
attached fields, and server responses can call `HttpResponse::with_tcn(value)`
or `HttpResponse::tcn()` to declare or observe the same metadata. Valid
metadata is a singleton response field containing `list`, `choice`, `adhoc`,
`re-choose`, or `keep` tokens, normalized to lowercase in wire order.
Duplicate fields, duplicate tokens, malformed or unknown tokens, empty
members, oversized values, and control-byte injection are rejected while raw
headers remain available on accessor failures.

These helpers are metadata-only: RTTP does not select a variant, synthesize
`Alternates` or `Vary`, apply transparent content negotiation, or change cache
behavior from `TCN`.

## Bounded Set-Cookie response metadata

`HttpSetCookie` and `HttpSetCookies` are the shared protocol representation
for bounded response `Set-Cookie` metadata. Client responses can call
`Response::set_cookies()` to parse attached fields, and server responses can
call `HttpResponse::with_set_cookie(cookie)` or `HttpResponse::set_cookies()`
to declare or observe the same metadata. Valid metadata preserves multiple
field lines and raw headers, retains quoted cookie-value state, exposes
standard and extension attributes, and rejects duplicate attributes
case-insensitively. Per-value, per-field, count, and aggregate-size bounds are
enforced by the protocol type. Typed debug and error output redacts cookie
values. Raw `header("Set-Cookie", value)` remains an escape hatch.

These helpers are metadata-only: RTTP does not implement a cookie jar,
persistence, domain/path matching, expiry enforcement, SameSite or
partitioning policy, redirect cookie handling, or automatic request `Cookie`
emission.

## Bounded Origin-Trial response metadata

`OriginTrials` and `HttpOriginTrials` share the protocol parser for bounded
opaque `Origin-Trial` response metadata. Client responses can call
`Response::origin_trials()` to parse attached fields, and server responses
can call `HttpResponse::with_origin_trials(values)` or
`HttpResponse::origin_trials()` to declare or observe the same metadata.
Valid metadata preserves multiple tokens and duplicates in wire order. Each
token is limited to 8 KiB, the collection is limited to 64 tokens, and the
combined token bytes are limited to 64 KiB. Injected controls, obs-text,
empty values, and oversized collections are rejected while raw headers remain
available on parse failures. Token material is redacted from debug output.

These helpers are metadata-only: RTTP does not validate token signatures,
expiration, origin applicability, or activate browser trials.

## Bounded Reporting-Endpoints response metadata

Server-side Reporting-Endpoints helpers expose response declaration and
parsing without scheduling reports. `HttpResponse::with_reporting_endpoints(endpoints)`
validates a bounded endpoint-name to quoted-URL dictionary, removes any
existing raw `Reporting-Endpoints` fields, and adds one normalized value.
`HttpResponse::reporting_endpoints()` parses attached raw fields and returns
`Ok(None)` when the header is absent.

On the client, `Response::reporting_endpoints()` parses the same dictionary
through the shared protocol type, returning `Ok(None)` when the header is
absent and preserving raw headers on parse failures. Each field value is
bounded to 64 KiB, the combined raw field-value bytes are bounded to 64 KiB,
and the member count is bounded to 32.

These helpers are metadata-only: RTTP does not schedule, send, persist,
retry, or route reports.

## Bounded Cross-Origin-Opener-Policy-Report-Only response metadata

Server-side COOP Report-Only helpers expose response declaration and parsing
without isolating browsing contexts or sending reports.
`HttpResponse::with_cross_origin_opener_policy_report_only(value)` validates
a singleton structured-field item through the shared protocol type, removes
any existing raw `Cross-Origin-Opener-Policy-Report-Only` fields, and adds
one normalized value. `HttpResponse::cross_origin_opener_policy_report_only()`
parses attached raw fields and returns `Ok(None)` when the header is absent.

On the client, `Response::cross_origin_opener_policy_report_only()` parses
the same metadata, returning `Ok(None)` when the header is absent and
preserving raw headers on parse failures. The type reuses the canonical COOP
directives `unsafe-none`, `same-origin-allow-popups`, `same-origin`, and
`noopener-allow-popups`. Well-formed parameters are retained; `report-to` is
exposed as a reporting-endpoint name when present. Each field value is
bounded to 64 KiB; parameter count is bounded to 256, and each parameter
value is bounded to 64 KiB.

These helpers are metadata-only: RTTP does not isolate browsing contexts,
validate `Reporting-Endpoints` members, deliver reports, or schedule report
delivery.

## Bounded HTTP/1.1 representation metadata behavior

Server-side representation metadata helpers expose request parsing and
response declaration without changing payload bytes.
`Request::content_type()` and `HttpRequest::content_type()` parse a singleton
received `Content-Type` field into `HttpContentType` and return `Ok(None)`
when absent. Duplicate `Content-Type` fields are a helper error.
`Request::content_encoding()` and `HttpRequest::content_encoding()` parse
received `Content-Encoding` fields in wire order into
`HttpResponseContentEncodings`. HTTP/1.1 and HTTP/2 share the same `Request`
helpers. `HttpContentType::parse(value)`
validates a `Content-Type` field, normalizes the media type and parameter
names to lowercase, preserves parameter values, and exposes
`media_type()`, `type_()`, `subtype()`, `parameter(name)`, `parameters()`,
and `header_value()`.
`HttpContentType::new(type_name, subtype)` constructs a normalized media type,
and `with_parameter(name, value)` appends safely serialized parameters with
normalized names and preserved values.
`HttpResponse::with_content_type(value)` accepts any `IntoHttpContentType`,
removes existing raw `Content-Type` fields, and adds one validated
`Content-Type` header. `HttpResponse::content_type()` parses an attached
singleton header and returns `Ok(None)` when the header is absent.

`HttpResponseContentEncodings::parse(value)` validates comma-separated
`Content-Encoding` codings, while
`HttpResponseContentEncodings::from_codings(codings)` validates an explicit
coding list for declarations. `HttpResponse::with_content_encoding(codings)`
removes existing raw `Content-Encoding` fields and adds one validated
comma-separated header. `HttpResponse::content_encoding()` parses all attached
`Content-Encoding` fields in wire order and returns `Ok(None)` when absent.
Coding spelling and order are preserved, including repeated codings across
one or more header fields.

Parsing and declaration are bounded. Each `Content-Type` and
`Content-Encoding` field value is limited to 64 KiB. Server `Content-Type`
helpers accept at most 256 parameters and reject malformed media types,
malformed parameter syntax, malformed quoted strings, duplicate parameters,
duplicate singleton fields, CR/LF or other control bytes, oversized values,
and too many parameters. Server `Content-Encoding` helpers accept at most 256
codings and reject empty members, malformed tokens, oversized values, and too
many codings. Raw `Request::header(...)` and
`HttpResponse::header(...)` values remain preserved exactly as ordinary
headers until a typed declaration helper replaces them or the typed parser is
requested; parser errors do not remove existing headers or change the request
body.

```rust
let content_type = request.content_type()?.expect("Content-Type");
if content_type.media_type() == "application/json" {
  let charset = content_type.parameter("charset");
}

let encodings = request.content_encoding()?.expect("Content-Encoding");
assert_eq!(vec!["gzip"], encodings.codings());

let declared = HttpContentType::new("application", "json")?
  .with_parameter("charset", "utf-8")?;

let response = HttpResponse::ok("{}")
  .with_content_type(declared)?
  .with_content_encoding(["gzip", "br"])?;

let codings = response.content_encoding()?.expect("Content-Encoding");
assert_eq!(vec!["gzip", "br"], codings.codings());
```

These helpers parse request metadata only; they do not sniff, decode,
negotiate, cache, redirect, retry, or select representations from
`Content-Type` or `Content-Encoding`.

## Bounded Connection metadata

`Request::connection()`, `HttpRequest::connection()`, and
`HttpResponse::connection()` parse retained HTTP/1 `Connection` fields into
`HttpConnection`. They return `Ok(None)` when the header is absent. Present
values combine case-insensitive fields in wire order and preserve token
spelling, including duplicates. Parse errors leave raw headers unchanged.
HTTP/2 continues to reject inbound `Connection` at decode time. These helpers
do not change keep-alive, hop-by-hop stripping, upgrade/h2c, or HTTP/2
rejection.

## Bounded Upgrade metadata

`HttpClient::upgrade_protocols()` validates and replaces request `Upgrade`
metadata without changing request method, socket handoff, or `Connection`
handling. `Response::upgrade()`, `Request::upgrade()`,
`HttpRequest::upgrade()`, and `HttpResponse::upgrade()` parse retained HTTP/1
`Upgrade` fields into `Upgrade`/`HttpUpgrade` metadata. They return
`Ok(None)` when the header is absent. `HttpResponse::with_upgrade()` validates
and replaces attached response `Upgrade` metadata.

Each field value is limited to 64 KiB. Parsing accepts at most 32 protocols.
Each protocol must be an HTTP token, optionally followed by `/` and a token
protocol version. Empty members, malformed protocols, control bytes,
oversized values, and too many protocols return a parser error while raw
headers remain available.

These helpers expose HTTP/1 header metadata only. They do not add
`Connection: Upgrade`, select h2c, perform client `upgrade()` handoff, change
server `HttpHandoff::upgrade` socket handoff, or implement the upgraded
protocol.

## Bounded Transfer-Encoding framing metadata

`Request::transfer_encoding()` and `HttpRequest::transfer_encoding()` parse
retained HTTP/1 `Transfer-Encoding` fields into `HttpTransferEncoding`. They
return `Ok(None)` when the header is absent. Present values combine
case-insensitive fields in wire order and must yield a sole `chunked` coding,
matching existing HTTP/1 framing. Parse errors leave raw headers and the
request body unchanged. HTTP/2 continues to reject `Transfer-Encoding` at
decode time. These helpers do not change `request_body_kind`, `TE`,
Content-Length, or HTTP/2 decode.

## Bounded trailer behavior

HTTP/1.1 server trailer support remains chunked-scope only. Chunked request
trailers are preserved on `Request` and can be read with `Request::trailers`
or `Request::trailer`; fixed-length request bodies do not carry a trailer
section. HTTP/1.1 response trailers are serialized only for chunked
`HttpResponse` bodies when the response status permits a body.

On the same listener, prior-knowledge h2c and valid `Upgrade: h2c` requests
use HTTP/2 trailing HEADERS instead of HTTP/1.1 chunk trailers. Inbound
application trailers such as `X-Trace`, `X-Upload-Status`, and
`X-Upload-Checksum` are exposed on `Request` after bounded header-list and
HPACK decoding. Outbound h2c response trailers come from
`HttpResponse::trailer` and are emitted as trailing HEADERS after response
DATA, split to the active peer frame-size limit. HTTP/2 pseudo-headers in
trailers are rejected before handler dispatch, and trailer names reserved for
connection state, routing, authentication/cookies, transfer framing, or
payload processing are rejected.

The h2c Upgrade path is only the bounded HTTP/2 path selected by a valid
`Upgrade: h2c` request. Ordinary HTTP/1.1 `CONNECT` authority-form requests and
non-h2c `HttpHandoff::upgrade` handoffs remain separate caller-owned protocol
paths; they are not trailer-parsed as HTTP/2 streams.

## Bounded HTTP/2 CONTINUATION behavior

The server supports large HTTP/2 header blocks on its bounded h2c paths.
Inbound request HEADERS and trailing HEADERS may arrive as an initial HEADERS
frame followed by CONTINUATION frames, and RTTP reassembles the complete HPACK
block before decoding, header-list-size enforcement, trailer validation, and
handler dispatch. Outbound response HEADERS and trailing HEADERS are split into
HEADERS plus CONTINUATION frames when their encoded HPACK block exceeds the
active peer `SETTINGS_MAX_FRAME_SIZE`.

`SETTINGS_MAX_FRAME_SIZE` controls per-frame payload boundaries, not the total
decoded metadata allowance. The server advertises the default 16,384-byte
frame size, accepts only legal peer frame-size settings from 16,384 through
16,777,215 bytes, rejects inbound frames above the active local limit, and uses
the active peer limit to fragment response HEADERS, DATA, and trailing HEADERS.
Decoded request metadata remains bounded by the advertised
`SETTINGS_MAX_HEADER_LIST_SIZE`; HPACK dynamic table size controls compression
state only.

CONTINUATION ordering is enforced before application code sees a request. Once
a request HEADERS frame starts a block without `END_HEADERS`, only
CONTINUATION frames on that same stream may arrive until `END_HEADERS` closes
the block. CONTINUATION on stream 0, orphan CONTINUATION frames,
wrong-stream CONTINUATION frames, interleaved DATA or control frames before
`END_HEADERS`, and EOF before the block is closed are rejected without handler
dispatch.

This behavior is shared by HTTP/2 prior-knowledge preface detection and the
valid `Upgrade: h2c` server path after `101 Switching Protocols`. It does not
apply to ordinary HTTP/1.1 `CONNECT`, non-h2c `HttpHandoff::upgrade`
handoffs, TLS ALPN, proxy h2, h2c tunnel handoff, server push, extension
callbacks, persistent sessions, or unbounded multiplexing.

The server currently parses blocking HTTP/1.x requests for local tests and
simple embedded use. It supports fixed `Content-Length` and chunked request
bodies, preserves chunked request trailers on `Request`, bounds request
head/body parsing, handles `HEAD` without writing a response body, honors
`Connection` close/keep-alive semantics across a bounded `serve_requests` loop,
writes response body framing and response trailers consistently, and accepts
`Expect: 100-continue`. On the same socket2 listener, the accept path detects
either the HTTP/2 client preface or an HTTP/1.1 `Upgrade: h2c` request and
dispatches the resulting h2c connection to the same minimal bounded handler,
including bodyless DELETE, OPTIONS, and TRACE requests. HTTP/1.1 h2c Upgrade
is opt-in on both sides: the request must be `HTTP/1.1`, include
`Connection: Upgrade, HTTP2-Settings`, `Upgrade: h2c`, exactly one
`HTTP2-Settings` field with a valid unpadded base64url SETTINGS payload, and
no request body; malformed h2c upgrade attempts receive `400 Bad Request`
before handler dispatch. When the upgrade is valid, the server writes
`101 Switching Protocols`, consumes the client's HTTP/2 preface on the same
socket, applies the advertised SETTINGS as the initial peer SETTINGS, and uses
the HTTP/2 stream id sequence reserved for an HTTP/1.1 upgrade. The server
advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the active request allowance
for that bounded accept path and rejects new h2c streams once the open-stream
count plus completed requests reaches that allowance. The h2c path handles
`HEAD` without writing response DATA frames.
The server validates peer `SETTINGS_ENABLE_PUSH` values as only `0` or `1`;
any other value rejects the bounded h2c handshake. It also validates
`SETTINGS_ENABLE_CONNECT_PROTOCOL` values as only `0` or `1`; a received value
of `1`, whether in the initial peer SETTINGS or a later SETTINGS update,
enables bounded RFC 8441 extended CONNECT request dispatch for subsequent
HEADERS on that connection. Without that negotiated setting, any `:protocol`
pseudo-header is rejected before handler dispatch. The server also advertises
and enforces a conservative
`SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata; request HEADERS
and trailing HEADERS can span CONTINUATION frames, but decoded metadata remains
bounded before handler dispatch. It advertises the default 16,384-byte
`SETTINGS_MAX_FRAME_SIZE`, rejects peer SETTINGS values outside the legal
HTTP/2 range of 16,384 through 16,777,215 bytes, rejects inbound frames larger
than the active local limit, and splits outbound response HEADERS, DATA, and
trailing HEADERS to the active peer frame-size limit.
Incoming padded HEADERS, DATA, and trailer frames are accepted without exposing
padding bytes to handlers, and application trailers such as `X-Trace`,
`X-Upload-Status`, and `X-Upload-Checksum` are preserved on `Request`.
Trailing HEADERS that contain HTTP/2 pseudo-headers are rejected before handler
dispatch. Trailer field names that affect connection state, routing,
authentication/cookies, framing, or payload processing are also rejected,
including `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`,
`Transfer-Encoding`, `Upgrade`, `Host`, `Content-Length`, `Cache-Control`,
`Content-Encoding`, `Content-Range`, `Content-Type`, `Max-Forwards`,
`Authorization`, `Proxy-Authenticate`, `Proxy-Authorization`, `Cookie`,
`Set-Cookie`, and `WWW-Authenticate`. HPACK static Huffman strings, request
dynamic table entries, and bounded large header blocks are carried with
CONTINUATION frames. Peer `SETTINGS_HEADER_TABLE_SIZE` values bound response
dynamic indexing: the server uses the peer's latest advertised table size when
encoding response HEADERS and applies later updates before response trailers.
A peer value of zero evicts response dynamic entries and keeps response HEADERS
and trailers literal encoded. Inbound request and request-trailer decoding is
bounded to the server's fixed 4,096-byte HPACK dynamic table limit; incoming
dynamic table size updates may shrink that table, including to zero, but
updates above 4,096 bytes are rejected. These HPACK limits affect compression
state only and do not change `SETTINGS_MAX_HEADER_LIST_SIZE`, trailer
validation, DATA flow control, handler dispatch, or multiplex scheduling. Valid
prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, and `Upgrade`; `TE` is accepted only as `te: trailers`
and other `TE` values are rejected. When serializing h2c responses, the server
strips HTTP/1.x connection-specific response fields and generated HTTP/2
framing fields from HEADERS: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`TE`, `Trailer`, `Transfer-Encoding`, `Upgrade`, and `Content-Length`. H2c
response trailers skip the existing forbidden trailer set, including invalid
pseudo-header-like names, connection-specific, transfer/framing, routing,
authentication, and cookie fields. Valid
standalone PRIORITY frames and HEADERS priority fields are validated and ignored
as metadata; malformed priority metadata is rejected, and request or response
ordering does not use priority scheduling. Multiple prior-knowledge h2c request
streams may be open on one connection only up to the advertised bounded
active-stream allowance from `SETTINGS_MAX_CONCURRENT_STREAMS`; this is not
general multiplex scheduling or full persistent HTTP/2 session management.
Inbound PING without ACK is acknowledged only when it arrives on stream 0 with
exactly 8 octets of opaque data; the PING ACK carries that same opaque data.
Inbound PING ACK is ignored for this bounded path. PING with a non-zero stream
id or payload length other than 8 is malformed and rejected. This
acknowledgement path does not add keepalive timers, automatic client- or
server-initiated PING policy, retry/replay, a full session manager, or a full
multiplex scheduler.
Unknown frame types, including extension frames, are ignored only after the
HTTP/2 preface is accepted in this bounded h2c server path where HTTP/2
permits that behavior; RTTP does not expose an extension callback API or
negotiate extensions. Reserved stream identifier high bits are masked when
frames are parsed or written, which normalizes frame identifiers without
adding unbounded multiplexing, session management, or external h2-stack
support.
Server push is outside this bounded server path: inbound `PUSH_PROMISE` frames
are rejected deterministically before handler dispatch instead of attempting
push state management, and RTTP does not implement server-side push state even
when a peer sends `SETTINGS_ENABLE_PUSH = 1`.
Responses are still written synchronously as requests complete. The bounded
h2c path supports conservative DATA flow-control for prior-knowledge use. It
uses `GOAWAY` as a bounded shutdown signal when the loop ends, reporting the
last completed stream id so clients can apply a deterministic stream boundary.
If the bounded request allowance is exhausted while additional streams are
already open, the server first sends a graceful `GOAWAY` boundary and lets
streams within that boundary finish; new streams outside the boundary are
refused with `REFUSED_STREAM` and are not dispatched to the handler. If the
peer closes the TCP connection, a read/write timeout fires, or the socket is
reset before `GOAWAY` can be written, that is transport termination rather
than an HTTP/2 graceful shutdown signal and no additional stream boundary is
implied.
Within that same prior-knowledge h2c server path, inbound `RST_STREAM` is a
bounded reset/cancellation signal for the affected stream: reset request
streams are not dispatched to handlers, and reset response streams stop within
the bounded write path. RTTP does not expose a public cancellation callback API,
retry work automatically, keep persistent HTTP/2 sessions, or model a full
HTTP/2 stream state machine around those resets.
The h2c handler does not share the HTTP/1.1 `CONNECT` or non-h2c `Upgrade`
handoff paths. Ordinary h2c `CONNECT` without `:protocol` remains unsupported
proxy tunneling and is rejected before handler dispatch. Negotiated extended
CONNECT is exposed to handlers as a normal `Request` with method `CONNECT`,
version `HTTP/2`, origin-form target from `:path`, `host` derived from
`:authority`, and `Request::extended_connect_protocol()` returning the
`:protocol` value. The handler returns a normal `HttpResponse`; RTTP does not
switch the stream to caller-owned tunnel bytes. HTTP/1.1 `CONNECT`
authority-form requests and `HttpHandoff::upgrade` for non-h2c protocols
remain separate handoff paths for caller-owned protocols, and the h2c Upgrade
detection preserves those existing handoffs when `Upgrade` is not `h2c`.

```rust,no_run
use rttp::server::HttpResponse;

fn main() -> std::io::Result<()> {
  let server = rttp::Http::server("127.0.0.1:8080")?;

  server.accept_one(|request| {
    if request.method() == "CONNECT"
      && request.version() == "HTTP/2"
      && request.extended_connect_protocol() == Some("websocket")
    {
      return HttpResponse::ok("accepted extended CONNECT metadata");
    }

    HttpResponse::new(400, "Bad Request")
  })
}
```

The server is intentionally not a full RFC-covering web server and still does
not implement server TLS, TLS ALPN, extension callback APIs, full extension
negotiation, external h2 integration, full WebSocket-over-h2, proxy h2, h2c
tunnel handoff, connection pooling, persistent multiplex sessions, persistent
HTTP/2 session management, automatic retry, public cancellation callbacks,
dynamic policy APIs, full RFC 8441 support, full stream state machines, full
HTTP/2 features such as unbounded multiplexing, unbounded multiplex scheduling,
general multiplexing, general tunnel scheduling, server push, and priority
scheduling, or async accept loops.

## Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, protocol-owned `Expect` metadata including `100-continue`, and obsolete line folding rejection | Expect metadata does not send `100 Continue` or reject unsupported extensions; intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Byte ranges | `HttpByteRange` parses one `bytes` range, `Request::evaluate_if_range` and `HttpRequest::evaluate_if_range` gate it with caller-provided strong ETag or exact HTTP-date metadata, `HttpResponse::partial_content`/`range_not_satisfiable` serialize `206`/`416` with the shared checked `HttpContentRange` formatter, `HttpResponse::content_range` parses attached `Content-Range` metadata, and `HttpAcceptRanges` plus `HttpResponse::with_accept_ranges`/`with_accept_ranges_none`/`accept_ranges` declare and parse bounded `Accept-Ranges` metadata while preserving raw headers | No Range request generation, multipart range serialization, partial response engine, automatic retry/replay, redirect behavior, cache storage or policy, filesystem serving, automatic cache validation, static-file policy, automatic byte serving, content slicing, download resume, or status-policy behavior |
| Conditional requests | `Request::evaluate_conditional`, `evaluate_conditional_request`, `HttpConditionalMetadata`, and `HttpEntityTag` evaluate bounded HTTP/1.1 validators; `HttpResponse::not_modified`, `precondition_failed`, `with_etag`, and typed bounded `etag` serialize or expose `304`/`412` metadata while preserving raw headers; `with_delta_base`/`delta_base` declare and parse bounded singleton `Delta-Base` metadata through the shared entity-tag primitive; `with_schedule_tag` and `schedule_tag` declare and parse bounded `Schedule-Tag` response metadata through the shared protocol type | No cache storage, static-file serving policy, automatic revalidation, cached-entity lookup, delta application, calendar-version generation, schedule-tag comparison, calendar inspection, scheduling policy, or cache-control engine |
| Informational responses and Early Hints | `HttpResponse::early_hints` and `early_hints_with_headers` construct validated bodyless `103 Early Hints` response metadata with bounded `Link` and safe metadata headers | `101 Switching Protocols` remains a separate terminal handoff response; no automatic preload execution, cache policy, redirect/retry/replay, route generation, streaming early-write API, TLS/ALPN behavior, or status-policy behavior |
| Cache-Control, CDN-Cache-Control, and Cache-Status | `Request::cache_control`, `HttpRequest::cache_control`, and `HttpResponse::cache_control` parse bounded request/response directives, numeric freshness fields, quoted field-name lists, and extension directives; `HttpResponse::cdn_cache_control` parses bounded response `CDN-Cache-Control` directives and CDN extension metadata while preserving raw response headers on parse errors; `HttpResponse::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw response headers on parse errors; `HttpResponse::with_age`/`age`, `with_expires`/`expires`, and protocol-backed `with_retry_after_delta`/`with_retry_after_date`/`retry_after` declare and parse response `Age`, `Expires`, and `Retry-After` metadata | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, automatic conditional requests, directive-based validator evaluation, automatic sleep, retry, replay, backoff, scheduler integration, or status-code policy engine |
| Cache-Control, CDN-Cache-Control, Surrogate-Control, and Cache-Status | `Request::cache_control`, `HttpRequest::cache_control`, and `HttpResponse::cache_control` parse bounded request/response directives, numeric freshness fields, quoted field-name lists, and extension directives; `HttpResponse::cdn_cache_control` parses bounded response `CDN-Cache-Control` directives and CDN extension metadata while preserving raw response headers on parse errors; `HttpResponse::with_surrogate_control`/`surrogate_control` and client `Response::surrogate_control` parse or declare bounded `Surrogate-Control` directives through the shared protocol type, rejecting duplicates and oversized aggregate values while preserving raw headers on parse errors; `HttpResponse::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw response headers on parse errors; `HttpResponse::with_date`/`date`, `with_age`/`age`, `with_expires`/`expires`, `with_last_modified`/`last_modified_date`, and `with_retry_after_delta`/`with_retry_after_date`/`retry_after` declare and parse response `Date`, `Age`, `Expires`, `Last-Modified`, and `Retry-After` metadata | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, `Surrogate-Control` to `Cache-Control` translation, automatic conditional requests, directive-based validator evaluation, automatic sleep, retry, replay, backoff, scheduler integration, or status-code policy engine |
| Alt-Used | `AltUsed`, `HttpAltUsed`, `Response::alt_used`, `HttpResponse::with_alt_used`, and `HttpResponse::alt_used` parse or declare bounded singleton response authority metadata while preserving raw headers on parse failures and replacing raw response duplicates on typed declaration | No alternative service selection, origin rewriting, socket migration, retry, or connection-policy behavior |
| Alternates | `Alternates`, `HttpAlternates`, `Response::alternates`, `HttpResponse::with_alternates`, and `HttpResponse::alternates` parse or declare bounded RFC 2295-style variant metadata, validating URIs, qvalues, attributes, duplicates, member counts, and size bounds while preserving raw headers on parse failures and replacing raw response fields on typed declaration | No transparent content negotiation, variant selection, automatic fetch, request replay, URI resolution, cache storage, `Vary` matching, or quality ranking |
| Origin-Trial | `OriginTrials`, `HttpOriginTrials`, `Response::origin_trials`, `HttpResponse::with_origin_trials`, and `HttpResponse::origin_trials` parse or declare bounded opaque `Origin-Trial` tokens in wire order, preserve duplicates, redact token material from debug output, and replace raw response fields on typed declaration | No token signature validation, expiration checks, origin applicability, feature activation, or browser trial policy |
| Memento-Datetime | `HttpResponse::with_memento_datetime`/`memento_datetime` declare and parse bounded singleton `Memento-Datetime` IMF-fixdate metadata while preserving raw headers on parse errors | No archival selection, `Accept-Datetime` negotiation, TimeGate behavior, retry, or transport changes |
| Accept-Datetime | `HttpClient::accept_datetime` validates and emits bounded singleton `Accept-Datetime` request metadata through the protocol `AcceptDatetime` type, and `Request::accept_datetime`/`HttpRequest::accept_datetime` parse received fields into `HttpAcceptDatetime`, accepting obsolete HTTP-date forms and preserving raw headers on parse errors | No archival selection, TimeGate behavior, `Vary` injection, cache-policy changes, or conditional evaluation |
| Vary | `HttpVary`, `HttpResponse::with_vary`, `HttpResponse::vary`, `Request::vary_selection`, and `HttpRequest::vary_selection` parse, declare, and select bounded `Vary` metadata with case-insensitive field-name handling | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| Authentication metadata | `Request::authorization`/`proxy_authorization` and `HttpRequest::authorization`/`proxy_authorization` expose one bounded opaque credential field, `Response::www_authenticate` parses bounded client response challenges, and `HttpResponse::with_www_authenticate`/`www_authenticate` declare and parse bounded `WWW-Authenticate` challenges | No credential validation, realm selection, automatic client challenge, retry, or authentication/authorization enforcement |
| Idempotency-Key | `HttpClient::idempotency_key` validates and emits one bounded opaque `Idempotency-Key` request field through the shared protocol type, and `Request::idempotency_key`/`HttpRequest::idempotency_key` parse received fields into the same representation while preserving raw headers on errors and redacting the key from typed debug output | No retry, replay, key storage or comparison, deduplication store, or application idempotency policy |
| Depth | `HttpClient::depth` validates and emits bounded singleton WebDAV `Depth` request metadata through the shared protocol type, and `Request::depth`/`HttpRequest::depth` parse received fields into the same representation while preserving raw headers on errors | No resource traversal, WebDAV method selection, method-policy enforcement, retry, or forwarding policy |
| Lock-Token | `HttpClient::lock_token` validates and emits one bounded WebDAV `Lock-Token` request field through the shared protocol type, `Request::lock_token`/`HttpRequest::lock_token` parse received request fields, and `HttpResponse::with_lock_token`/`lock_token` plus `Response::lock_token` declare or parse response fields while preserving raw headers on errors and redacting the token from typed debug output | No lock creation, refresh, release, persistence, ownership comparison, or WebDAV lock policy |
| Destination | `HttpClient::destination` validates and emits bounded singleton WebDAV `Destination` request metadata through the shared protocol type, and `Request::destination`/`HttpRequest::destination` parse received fields into the same representation while preserving raw headers on errors | No destination resolution, URI normalization, authorization, COPY/MOVE execution, or application resource policy |
| Timeout | `HttpClient::timeout` validates and emits bounded ordered WebDAV `Timeout` request metadata through the shared protocol type, and `Request::timeout`/`HttpRequest::timeout` parse received fields into the same representation while preserving raw headers on errors | No lock creation, lock refresh, application-timeout selection, retry, or forwarding policy |
| If-Schedule-Tag-Match and Schedule-Tag | `HttpClient::if_schedule_tag_match` validates and emits bounded singleton `If-Schedule-Tag-Match` request metadata, `Request::if_schedule_tag_match`/`HttpRequest::if_schedule_tag_match` parse received fields, and `Response::schedule_tag` parses bounded singleton `Schedule-Tag` response metadata through shared protocol types backed by `EntityTag` | No calendar-version generation, schedule-tag comparison, calendar inspection, scheduling policy, retry, or 412/status-policy behavior |
| Overwrite | `HttpClient::overwrite` validates and emits bounded singleton WebDAV `Overwrite` request metadata through the shared protocol type, accepting only the tokens `T` and `F`, and `Request::overwrite`/`HttpRequest::overwrite` parse received fields into the same representation while preserving raw headers on errors | No destination overwrite, RFC 4918 default-`T` synthesis, resource policy, COPY/MOVE execution, retry, or forwarding policy |
| If | `HttpClient::if_header` validates and emits bounded RFC 4918 WebDAV `If` request metadata through the shared protocol type, accepting untagged or tagged condition lists with `Not`, state tokens, and bracketed entity tags and preserving order, and `Request::if_header`/`HttpRequest::if_header` parse received fields into the same `HttpIf` representation while preserving raw headers on errors and redacting state tokens from typed debug output | No lock, entity-tag, or resource-state evaluation; no 412 or other precondition outcome; no lock creation, refresh, or release; no COPY/MOVE/UNLOCK execution; no retry, or forwarding policy |
| WebSocket handshake metadata | `HttpClient::sec_websocket_key` validates and emits one bounded `Sec-WebSocket-Key` request field through the shared protocol type, and `Request::sec_websocket_key`/`HttpRequest::sec_websocket_key` parse received fields into the same representation while preserving raw headers on errors; server responses can derive `Sec-WebSocket-Accept` with the RFC GUID plus SHA-1/base64 transform, and clients can parse and verify the response against a validated key; typed debug output redacts key and accept material; workspace integration tests cover live HTTP/1.1 and h2c metadata-only roundtrips | No HTTP upgrade, random nonce generation, WebSocket frames, or handshake policy |
| Sec-WebSocket-Version | `HttpClient::sec_websocket_version`, `Request::sec_websocket_version`/`HttpRequest::sec_websocket_version`, `HttpResponse::with_sec_websocket_version`/`sec_websocket_version`, and client `Response::sec_websocket_version` share the bounded protocol version-list representation, requiring canonical descending order and preserving raw headers on errors; workspace integration tests cover live HTTP/1.1 and h2c metadata-only roundtrips | No WebSocket handshake, `Connection: Upgrade` emission, `Sec-WebSocket-Accept` computation, version negotiation, protocol switch, or frames |
| Sec-WebSocket-Protocol | `HttpClient::sec_websocket_protocol`, `Request::sec_websocket_protocol`/`HttpRequest::sec_websocket_protocol`, `HttpResponse::with_sec_websocket_protocol`/`sec_websocket_protocol`, and client `Response::sec_websocket_protocol` share the bounded protocol token representation: request offers preserve preference order while response values are selection singletons, with case-sensitive duplicates and raw headers preserved on errors; workspace integration tests cover live HTTP/1.1 and h2c metadata-only roundtrips | No WebSocket handshake, `Connection: Upgrade` emission, automatic subprotocol choice, protocol switch, or frames |
| Sec-WebSocket-Extensions | `HttpClient::sec_websocket_extensions`, `Request::sec_websocket_extensions`/`HttpRequest::sec_websocket_extensions`, `HttpResponse::with_sec_websocket_extensions`/`sec_websocket_extensions`, and client `Response::sec_websocket_extensions` share the bounded protocol extension-list representation: request offers preserve extension and parameter order while response values are one-extension selection singletons, with duplicates and raw headers preserved on errors; workspace integration tests cover live HTTP/1.1 and h2c metadata-only roundtrips | No WebSocket handshake, `Connection: Upgrade` emission, compression activation, extension negotiation, protocol switch, or frames |
| W3C Trace Context | `HttpClient::traceparent`/`tracestate` validate and emit bounded W3C Trace Context request metadata, and `Request::traceparent`/`tracestate` plus `HttpRequest` helpers parse received fields while preserving raw headers on errors and redacting propagation values from typed debug output | No trace-id creation, sampling decision, tracing backend, span model, or automatic propagation |
| W3C Baggage | `HttpClient::baggage` validates and emits bounded W3C Baggage request metadata, and `Request::baggage` plus `HttpRequest` helpers parse received fields while preserving raw headers on errors and redacting member and property values from typed debug output | No application-data interpretation, request-context storage, tracing backend, span model, or automatic propagation |
| X-Forwarded compatibility metadata | `HttpClient::x_forwarded_for`/`x_forwarded_host`/`x_forwarded_proto` emit bounded compatibility request metadata; `Request` and `HttpRequest` helpers parse ordered node, authority, and scheme values while preserving raw headers on errors | No forwarded identity trust, client address selection, routing rewrite, scheme rewrite, redirect, upgrade, enforcement, or trusted-proxy selection; applications must choose trusted proxies |
| CDN-Loop | `HttpClient::cdn_loop` validates and emits bounded RFC 8586 `CDN-Loop` request metadata, combining an existing same-name field with the new member in wire order, and `Request::cdn_loop` plus `HttpRequest` helpers parse received fields into `HttpCdnLoop` while preserving raw headers on errors | No CDN identifier insertion, loop detection or rejection, automatic forwarding, or hop-by-hop handling |
| Via | `HttpClient::via` validates and emits bounded HTTP `Via` hop metadata, combining an existing same-name field with the new hops in wire order; `Request::via` plus `HttpRequest` helpers and `Response::via` parse received fields into the shared type; `HttpResponse::with_via`/`via` declare or parse caller-supplied chains while preserving raw headers on errors | No automatic hop insertion or removal, trusted-proxy inference, identity rewrite, or HTTP/1.1 or HTTP/2 proxy-policy changes |
| No-Vary-Search | `NoVarySearch`, `HttpNoVarySearch`, `Response::no_vary_search`, `HttpResponse::with_no_vary_search`, and `HttpResponse::no_vary_search` parse and declare bounded Structured Fields response metadata for query-parameter variance declarations | No cache storage, cache-key matching, URL normalization, navigation behavior, request replay, or shared-cache policy enforcement |
| Allow | `HttpAllowedMethods`, `HttpResponse::with_allow`, and `HttpResponse::allow` declare and parse bounded `Allow` method-list metadata | No route dispatch, automatic `405` generation, `OPTIONS` policy, fallback method selection, retry/replay, or status-code policy engine |
| Client Hints | `HttpAcceptCh`/`HttpCriticalCh`, `HttpResponse::with_accept_ch`/`with_critical_ch`, and `accept_ch`/`critical_ch` declare and parse bounded Client Hints opt-in metadata; `Response::accept_ch` and `critical_ch` expose it to clients | No browser opt-in state, request-header generation, automatic retry, persistence, or Client Hints policy |
| Content-Language | `HttpContentLanguages`, `Request::content_language`, `HttpRequest::content_language`, `HttpResponse::with_content_language`, and `HttpResponse::content_language` parse or declare bounded `Content-Language` metadata | No automatic language negotiation, route selection, locale fallback, variant matching, cache policy, retry, replay, redirect, or status-policy behavior |
| Accept | `HttpClient::accept`/`accept_with_q`, `HttpAccept`, `Request::accept`, and `HttpRequest::accept` share the bounded `rttp-protocol` `Accept` request metadata primitive while preserving raw-header escape hatches and raw values on parse errors | No content negotiation, representation selection, MIME sniffing, body decoding, cache `Vary` synthesis, or response choice |
| Accept-Charset | `HttpRequestAcceptCharsets`, `Request::accept_charset`, and `HttpRequest::accept_charset` parse bounded `Accept-Charset` request metadata through the shared `rttp-protocol` type | No content negotiation, charset transcoding, body decoding, MIME sniffing, or response selection |
| A-IM | `HttpClient::a_im`/`a_im_with_q`/`a_im_value` emit bounded `A-IM` request metadata through the shared protocol type, and `Request::a_im`/`HttpRequest::a_im` parse received fields into `HttpAIm` while preserving raw headers on errors | No automatic delta-encoding selection, application, compression, or response transformation |
| IM | `HttpResponse::with_im` declares one validated `IM` header replacing raw duplicates, `HttpResponse::im` and client `Response::im` parse attached `IM` fields into the shared protocol `Im` type while preserving raw headers on errors | No instance-manipulation decoding, inversion, or application, and no `226 IM Used` status policy |
| Delta-Base | `HttpResponse::with_delta_base` declares one validated `Delta-Base` header, and `HttpResponse::delta_base` plus client `Response::delta_base` parse bounded singleton `Delta-Base` response metadata through the shared entity-tag primitive while preserving raw headers on parse errors | No cached-entity lookup, validator comparison, delta application, or cache policy |
| Negotiate | `HttpClient::negotiate` emits bounded RFC 2295 `Negotiate` request metadata through the shared protocol type, and `Request::negotiate`/`HttpRequest::negotiate` parse received fields into `HttpNegotiate` while preserving raw headers on errors | No variant selection, transparent content negotiation, `Alternates`/`TCN` synthesis, or automatic cache selection |
| TCN | `Response::tcn()` and `HttpResponse::with_tcn()`/`tcn()` share bounded RFC 2295 `TCN` response metadata through `Tcn`/`HttpTcn` while preserving raw headers on accessor errors | No variant selection, `Alternates`/`Vary` synthesis, transparent content negotiation, or cache behavior |
| Set-Cookie | `Response::set_cookies()` and `HttpResponse::with_set_cookie()`/`set_cookies()` share bounded protocol `Set-Cookie` response metadata, preserve multiple field lines and raw headers, redact cookie values from typed debug and errors, and reject duplicate attributes | No cookie jar, persistence, domain/path matching, expiry enforcement, SameSite or partitioning policy, or automatic request `Cookie` emission |
| Variant-Vary | `HttpVariantVary`, `HttpResponse::with_variant_vary`, and `HttpResponse::variant_vary` plus client `Response::variant_vary` declare or parse bounded RFC 2295 `Variant-Vary` response metadata through the shared protocol type, replacing raw duplicates on declaration and preserving raw headers on accessor errors | No cache-key construction, variant selection, `Alternates`/`TCN`/`Vary` synthesis, transparent content negotiation, or cache behavior |
| Transparent negotiation metadata matrix | Workspace integration tests cover live HTTP/1.1 and h2c client/server/facade roundtrips for `A-IM`, `IM`, `Delta-Base`, `Alternates`, `Negotiate`, `TCN`, and `Variant-Vary`, including valid values, malformed/duplicate/bounds rejection, raw-header observability, and declared-order q-value recording | No variant selection, `Alternates` URI fetching, delta application, `226 IM Used` synthesis, or cache policy |
| Accept-Encoding | `HttpRequestAcceptEncodings`, `Request::accept_encoding`, and `HttpRequest::accept_encoding` parse bounded `Accept-Encoding` request metadata through the shared `rttp-protocol` type | No compression, decompression, content negotiation, retries, or transport changes |
| Sec-GPC | `HttpClient::sec_gpc`, `Request::sec_gpc`, and `HttpRequest::sec_gpc` share the bounded protocol `Sec-GPC` `1`-signal representation and preserve raw values on errors | No consent inference, tracking-policy enforcement, legal policy, serving policy, retries, or browser state |
| Pragma | `HttpClient::pragma`/`pragma_no_cache`, `Request::pragma`, `HttpRequest::pragma`, `HttpResponse::with_pragma`, and `HttpResponse::pragma` share the bounded protocol `Pragma` representation across client construction, server access, server response declaration, and client response access, combining fields in wire order and preserving raw headers on errors | No translation into `Cache-Control`, cache storage, freshness checks, revalidation, or cache/intermediary policy |
| Content-Location | `HttpResponse::with_content_location` declares one bounded singleton `Content-Location` header, and `HttpResponse::content_location` parses attached singleton response metadata while preserving raw headers | No redirect behavior, cache variant selection, representation replacement, retry/replay, route generation, or status-policy behavior |
| Service-Worker-Allowed | `HttpResponse::with_service_worker_allowed` declares one bounded singleton `Service-Worker-Allowed` header, and `HttpResponse::service_worker_allowed` plus client `Response::service_worker_allowed` parse attached singleton path metadata while preserving raw headers | No service-worker registration, scope evaluation, script-URL resolution, or application routing policy |
| Content-DPR | `HttpResponse::with_content_dpr` declares one bounded singleton `Content-DPR` header, and `HttpResponse::content_dpr` plus client `Response::content_dpr` parse attached singleton decimal-ratio metadata while preserving raw headers | No image rescaling, request DPR emission, Client Hints policy, retry, or transport changes |
| Content-Type and Content-Encoding | `HttpContentType`, `Request::content_type`, `HttpRequest::content_type`, `HttpResponse::with_content_type`, `content_type`, `HttpResponseContentEncodings`, `Request::content_encoding`, `HttpRequest::content_encoding`, `HttpResponse::with_content_encoding`, and `content_encoding` parse or declare bounded representation metadata while preserving raw headers on parse failures and replacing raw response duplicates on typed declaration | No MIME sniffing, body decoding, charset transcoding, compression/decompression, negotiation, cache policy, redirects, retry/replay, or filesystem serving |
| Connection | `HttpConnection`, `Request::connection`, `HttpRequest::connection`, and `HttpResponse::connection` parse bounded HTTP/1 `Connection` tokens, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to hop-by-hop stripping, keep-alive/close, upgrade/h2c, or HTTP/2 rejection |
| Transfer-Encoding | `HttpTransferEncoding`, `Request::transfer_encoding`, and `HttpRequest::transfer_encoding` parse bounded HTTP/1 `Transfer-Encoding` fields that must be sole `chunked`, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to `request_body_kind`, `TE`, Content-Length, or HTTP/2 decode rejection |
| Upgrade metadata | `Upgrade`, `HttpUpgrade`, `HttpClient::upgrade_protocols`, `Response::upgrade`, `Request::upgrade`, `HttpRequest::upgrade`, `HttpResponse::with_upgrade`, and `HttpResponse::upgrade` validate, declare, or parse bounded HTTP/1 `Upgrade` protocol metadata while preserving raw headers on parse failures | No automatic `Connection: Upgrade`, h2c selection, client/server socket handoff, ALPN negotiation, or upgraded protocol implementation |
| Content-Disposition | Protocol-backed `HttpContentDisposition`, `HttpResponse::with_content_disposition`, `with_attachment_filename`, and `content_disposition` declare and parse bounded singleton `Content-Disposition` response metadata, preserve parsed `filename` and `filename*` parameter values, preserve raw headers on parse failures, and replace raw duplicates on typed declaration | No automatic download, filesystem path handling, MIME sniffing, cache behavior, redirect behavior, retry/replay, negotiation, or status-policy behavior |
| NEL | `HttpNel`, `HttpResponse::with_nel`, and `HttpResponse::nel` declare and parse bounded W3C Network Error Logging policy JSON, preserve unknown JSON members as raw metadata, preserve raw headers on parse failures, and replace raw duplicates on typed declaration; the client `Response::nel` parses the same metadata | No network error report sending, policy persistence, Reporting endpoint group configuration, retry, redirect behavior, or status-policy behavior |
| Reporting-Endpoints | `HttpReportingEndpoints`, `HttpResponse::with_reporting_endpoints`, and `HttpResponse::reporting_endpoints` declare and parse bounded endpoint-name to quoted-URL dictionaries through the shared protocol type, preserve raw headers on parse failures, and replace raw duplicates on typed declaration; the client `Response::reporting_endpoints` parses the same metadata | No report scheduling, sending, persistence, retry, routing, or endpoint policy behavior |
| Cross-Origin-Opener-Policy-Report-Only | `HttpCrossOriginOpenerPolicyReportOnly`, `HttpResponse::with_cross_origin_opener_policy_report_only`, and `HttpResponse::cross_origin_opener_policy_report_only` declare and parse bounded singleton COOP Report-Only metadata, reuse the canonical COOP directives, retain reporting parameters including `report-to`, preserve raw headers on parse failures, and replace raw duplicates on typed declaration; the client `Response::cross_origin_opener_policy_report_only` parses the same metadata | No browsing-context isolation, report scheduling, sending, persistence, retry, routing, or `Reporting-Endpoints` validation |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpHandoff::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Application metadata trailers are allowed; trailer names that affect connection state, routing, authentication/cookies, framing, or payload processing are rejected |
| Bounded h2c server | The same `socket2` listener detects the HTTP/2 prior-knowledge preface or a valid HTTP/1.1 `Upgrade: h2c` request with `HTTP2-Settings`, validates SETTINGS including legal `SETTINGS_ENABLE_PUSH` and `SETTINGS_ENABLE_CONNECT_PROTOCOL` values of only `0` or `1` and legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, dispatches RFC 8441 extended CONNECT only after `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` has been negotiated, exposes negotiated extended CONNECT as a normal `Request` with method `CONNECT`, version `HTTP/2`, target from `:path`, `host` from `:authority`, and `Request::extended_connect_protocol()` from `:protocol`, advertises the default 16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects inbound frames above the active local limit, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the bounded active stream allowance, enforces that allowance before dispatching new streams, advertises and enforces a conservative `SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, serves bounded streams including bodyless DELETE, OPTIONS, TRACE, and negotiated extended CONNECT, handles HEAD without response DATA, rejects connection-specific request fields before handler dispatch, strips connection-specific response fields during h2c serialization, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges inbound PING without ACK on stream 0 and exactly 8 octets with matching opaque data, ignores inbound PING ACK, rejects malformed PING frames, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman fields and bounded CONTINUATION header blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | Ordinary `CONNECT`, missing-negotiation `:protocol`, non-CONNECT `:protocol`, malformed h2c Upgrade, request bodies on h2c Upgrade, and `PUSH_PROMISE` are rejected deterministically before handler dispatch; HTTP/1.1 `CONNECT` and non-h2c `Upgrade` remain separate handoff paths; bounded h2c only, with no keepalive timers, no automatic client/server initiated PING policy, no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, full WebSocket-over-h2, proxy h2, tunnel handoff, connection pooling, persistent multiplex sessions, persistent HTTP/2 session management, automatic retry/replay, server push, full RFC 8441 support, full session manager, full stream state machine, full multiplex scheduler, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, general tunnel scheduling, priority scheduling, or full HTTP/2 server feature set |

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`http2`, `tls-native`, `tls-rustls`, or `all` for the corresponding
`rttp_client` capabilities. The client feature includes the bounded HTTP/1.1
Range helpers from `rttp_client`: `range(start, end)`, `range_from(start)`,
and `range_suffix(length)` set single `bytes` ranges; `if_range_etag(etag)`
sets a single strong entity-tag `If-Range` validator; and
`if_range_date(http_date)` sets an HTTP-date `If-Range` validator. `Response`
exposes `is_partial_content()`, `is_range_not_satisfiable()`, and
checked `content_range()` for `206` and `416` responses. Manual `Range` and
`If-Range` headers remain available through the generic header API. These
client helpers do not evaluate `If-Range`, retry requests, store cache entries,
generate multipart range requests, or apply automatic cache validation. The
client response API also includes `Response::cache_control()`,
`Response::cdn_cache_control()`, `Response::surrogate_control()`, and
`Response::cache_status()` from
`rttp_client`, which parse bounded response `Cache-Control` and
`CDN-Cache-Control` and `Surrogate-Control` directives and extensions and RFC
9211 `Cache-Status` members as metadata only, and `Response::allow()`, which parses bounded
ordered HTTP method metadata from `Allow`; the wrapper does not add cache
storage, CDN cache, Cache-Status forwarding or freshness policy, automatic
revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache
policy enforcement, surrogate-key behavior, `Surrogate-Control` to
`Cache-Control` translation, automatic conditional requests,
fallback method selection, retry/replay, or status-code policy behavior. The
`http2` feature exposes the bounded
prior-knowledge h2c client path for GET, HEAD, bodyless DELETE, OPTIONS, or
TRACE, and buffered POST, PUT, or PATCH requests. It opens at most one request
stream, advertises `SETTINGS_ENABLE_PUSH = 0` so peers see server push
disabled on the client side, validates received `SETTINGS_ENABLE_PUSH` values
as only `0` or `1`, and honors the peer's initial
`SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the
peer allows zero streams. The bounded client path also honors peer-advertised
`SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits before sending request
HEADERS or trailing HEADERS. It
validates `SETTINGS_MAX_FRAME_SIZE` on both sides of the bounded h2c handshake:
a configured local `http2_max_frame_size` is advertised only when set, must be
in the legal HTTP/2 range of 16,384 through 16,777,215 bytes, and rejects
inbound frame payloads above that active local limit; peer-advertised values
outside the same range reject the handshake, while legal peer values are used
to split outbound request HEADERS, DATA, and trailing HEADERS. It also
includes rejection of request bodies for GET, HEAD, DELETE,
OPTIONS, and TRACE, HEAD response body suppression, stripping of HTTP/1.x
connection-specific request fields before h2c emission, rejection of
connection-specific peer response fields, HPACK static Huffman strings, request
dynamic entries within the peer's advertised `SETTINGS_HEADER_TABLE_SIZE`,
bounded local response dynamic table decoding, bounded large header blocks via
CONTINUATION frames, padded incoming response frames, and conservative DATA
flow-control for single-stream prior-knowledge use. Any received
`SETTINGS_ENABLE_PUSH` value other than `0` or `1` rejects the bounded h2c
handshake. The explicit `HttpClient::http2_extended_connect(protocol)` request
mode advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` and emits `:method
CONNECT` with `:protocol`, `:scheme`, `:authority`, and `:path` on the same
bounded single-stream path. It returns the peer's HTTP/2 response through the
normal `Response` API; it does not hand an upgraded socket to the caller.
Valid response PRIORITY
frames and HEADERS priority fields are validated and ignored as metadata;
malformed priority metadata is rejected, and no priority scheduling is
performed. Inbound PING without ACK is acknowledged only when it arrives on
stream 0 with exactly 8 octets of opaque data; the PING ACK carries that same
opaque data. Inbound PING ACK is ignored for this bounded path. PING with a
non-zero stream id or payload length other than 8 is malformed and rejected.
This acknowledgement path does not add keepalive timers, automatic client- or
server-initiated PING policy, retry/replay, a full session manager, or a full
multiplex scheduler. Server push is outside this bounded client path.
For client HPACK, the peer's `SETTINGS_HEADER_TABLE_SIZE` bounds outbound
request dynamic indexing for request HEADERS and trailers, and a peer value of
zero disables that request dynamic table. The local response decoder uses the
default 4,096-byte table unless `ConfigBuilder::http2_header_table_size`
configures and advertises another `u32`-sized limit; incoming response
table-size updates may shrink the decoder table, including to zero, but
updates above the advertised local limit are rejected. The wrapper does not add
a public dynamic policy API for changing those limits after the h2c handshake.
Unknown frame types, including extension frames, are ignored only after the
prior-knowledge h2c handshake in this bounded direct-client path where HTTP/2
permits that behavior; RTTP does not expose extension callbacks or perform
full extension negotiation. Reserved stream identifier high bits are masked
when frames are parsed or written, which normalizes wire framing but does not
add broader multiplex scheduling or persistent session management. Incoming
`PUSH_PROMISE` frames are rejected
deterministically instead of creating or tracking push state even when the
peer advertises `SETTINGS_ENABLE_PUSH = 1`. HTTP/1.1
`CONNECT` tunnel handoff remains a separate path; prior-knowledge h2c `GOAWAY`
is treated as a bounded shutdown signal: completed responses remain usable,
active responses continue only when the peer's `last-stream-id` includes the
stream, and lower boundaries reject the response deterministically. A
`GOAWAY` received before stream 1 is opened is treated as request refusal and
no request HEADERS are sent. RTTP returns that refusal to the caller instead
of retrying on a new connection; callers that know a request is safe or
idempotent must choose any retry policy themselves. This protocol shutdown
boundary is distinct from a transport-level disconnect, read timeout, write
timeout, or TCP reset, which is reported through the normal socket/error path
without an HTTP/2 `last-stream-id` boundary.
`RST_STREAM` is likewise bounded to this prior-knowledge h2c client path: a
reset for the active stream is reported as response cancellation, while
malformed reset frames are rejected deterministically. RTTP does not expose a
public cancellation callback API or retry the request automatically. The h2c
client strips `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, `Upgrade`, `TE`, `Trailer`, `Host`, and any field named
by a `Connection` token from emitted request HEADERS. Peer response HEADERS
containing `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`,
`Transfer-Encoding`, or `Upgrade` are rejected. Application request trailers
such as `X-Trace`, `X-Upload-Status`, or `X-Upload-Checksum` are valid in this
bounded h2c path and are encoded as trailing HEADERS after request DATA.
Configured request trailers are rejected before emission when their field name
is invalid or reserved for connection/framing/routing behavior: `Connection`,
`Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, `Content-Length`, `Host`, `Proxy-Authenticate`, or
`Proxy-Authorization`. Peer response trailers use the existing
forbidden-trailer validation for invalid pseudo-header-like names,
connection-specific, routing, authentication/cookie, and framing fields such
as `Authorization`, `Connection`, `Content-Length`, `Cookie`, `Host`,
`Keep-Alive`, `Proxy-Authenticate`, `Proxy-Authorization`,
`Proxy-Connection`, `Set-Cookie`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, and `WWW-Authenticate`. Ordinary `CONNECT`, header-configured RFC
8441 `:protocol` metadata, HTTP/1.1 `Upgrade` handoff requests, proxy
tunneling, extended CONNECT request bodies, and extended CONNECT request
trailers are rejected before a client socket is opened. HTTP/1.1 `CONNECT`
tunnel handoff and `Upgrade` remain separate client handoff paths; this h2c
path does not provide full WebSocket-over-h2, proxy h2, TLS ALPN, tunnel
handoff, persistent multiplex sessions, general tunnel scheduling, or full RFC
8441 support. Extension callback APIs, full extension negotiation, external h2
integration, connection pooling, automatic retry, server push, full stream
state machines, and full HTTP/2 features such as unbounded multiplex
scheduling, general multiplexing, and priority scheduling remain outside that
bounded prior-knowledge path. RTTP does not expose a dynamic policy API for
changing h2c frame-size or metadata limits at
runtime.

```rust,no_run
# #[cfg(feature = "http2")]
# fn example() -> Result<(), Box<dyn std::error::Error>> {
let response = rttp::Http::client()
  .get()
  .url("http://127.0.0.1:8080/chat")
  .http2_extended_connect("websocket")
  .emit_http2_prior_knowledge()?;
# Ok(())
# }
```

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
