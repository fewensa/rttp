rttp
====

A small Rust HTTP workspace. Application code typically depends on the `rttp`
facade crate, which forwards to the public `rttp_client` and `rttp-server`
crates; those share the wire primitives in the internal `rttp-protocol` crate.
The private, unpublished `rttp_test_support` crate owns the reusable local HTTP
and client helpers and shared test fixtures used by the workspace test suites.

## Workspace crates

- **`rttp`** — the compatibility facade. It exposes `rttp::Http::server` and,
  with the `client` feature enabled, `rttp::Http::client`; it re-exports the
  `rttp_server::server` module and forwards selected `rttp_client` response
  types. It wraps the established APIs and does not reimplement client or
  server behavior.
- **`rttp_client`** — the public HTTP client crate. Plain HTTP is available by
  default over direct `socket2` TCP connections; optional `async`, `http2`,
  `tls-native`, and `tls-rustls` features add async request APIs, bounded
  prior-knowledge h2c, and TLS. SOCKS proxy handshakes remain delegated to the
  `socks` crate.
- **`rttp-server`** — the public blocking HTTP server crate. It serves local
  tests and simple embedded use from a `socket2` listener, parses HTTP/1.x
  requests, and detects the bounded h2c paths (the HTTP/2 prior-knowledge
  preface or a valid `Upgrade: h2c` request). It does not implement server
  TLS.
- **`rttp-protocol`** — the internal, transport-independent wire-primitive
  crate (library name `rttp_protocol`), intentionally unpublished. It owns
  protocol syntax and framing validation only, split into typed per-header
  modules shared by the client and server; application policy stays in its
  callers.
- **`rttp_test_support`** — the private test-support crate used only by the
  workspace test suites.

Across the client, server, and protocol crates, typed header and wire helpers
stay metadata-only unless a section explicitly documents runtime behavior. They
parse, validate, normalize, and expose bounded protocol metadata; they do not
imply cache engines, browser policy, authentication decisions, retries,
representation selection, or body transformation.

Typed response metadata includes `Cross-Origin-Embedder-Policy` and
`Cross-Origin-Embedder-Policy-Report-Only`. Both accept the directives
`unsafe-none`, `require-corp`, and `credentialless` as singleton bounded
structured-field items; well-formed parameters such as `report-to` are accepted
as syntax and normalized away by typed builders. RTTP exposes these values for
application use only: it does not enforce browser embedder policy, retain
reporting metadata, deliver reports, or schedule report delivery.

Typed response metadata also includes `Cross-Origin-Opener-Policy` and
`Cross-Origin-Opener-Policy-Report-Only`. Both accept the directives
`unsafe-none`, `same-origin-allow-popups`, `same-origin`, and
`noopener-allow-popups` as singleton bounded structured-field items. Enforcing
COOP accepts well-formed parameters such as `report-to` as syntax and discards
them. Report-only COOP retains reporting parameters, including `report-to`, as
metadata and does not validate those names against `Reporting-Endpoints`. RTTP
exposes these values for application use only: it does not enforce
browsing-context isolation, deliver reports, or schedule report delivery.

## Local verification

Run the same checks as GitHub CI from the repository root before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Client

`rttp_client` supports plain HTTP by default. Optional features add async
request APIs and TLS implementations:

| name | comment |
|------|---------|
| async | Async request APIs |
| http2 | Bounded prior-knowledge h2c over direct `socket2` TCP connections |
| tls-native | HTTPS with `native-tls` |
| tls-rustls | HTTPS with `rustls` |

```toml
[dependencies]
rttp_client = "0.2"
```

Direct TCP client connections are opened with `socket2`. SOCKS proxy handshakes
are still delegated to the `socks` crate.
HTTP/1.x chunked responses are decoded by the client, and response trailers are
available through `Response::trailers`, `Response::trailer`, and
`Response::trailer_value`.

### Bounded HTTP/1.1 byte ranges

`HttpClient` can emit single `bytes` ranges with bounded helpers:
`range(start, end)` writes `Range: bytes=start-end`, `range_from(start)` writes
`Range: bytes=start-`, and `range_suffix(length)` writes
`Range: bytes=-length`. The helpers reject inverted closed ranges and a zero
suffix before opening a socket. Callers can still set a manual `Range` header
with the generic header API for cases outside those helpers.

```rust
client
  .get()
  .url("http://example.test/archive")
  .range(1_024, 2_047)?
  .if_range_etag(r#""revision-42""#)?
  .emit()?;
```

`206 Partial Content` and `416 Range Not Satisfiable` are visible through
`Response::is_partial_content()` and `Response::is_range_not_satisfiable()`.
`Response::content_range()` parses `Content-Range` into `ContentRange`, using
`start` and `end` for satisfiable ranges such as `bytes 10-19/200`, and no
`start` or `end` for unsatisfied ranges such as `bytes */200`.

`If-Range` is available through bounded request helpers that compose with the
range helpers: `if_range_etag(etag)` writes a single strong entity-tag
validator, and `if_range_date(http_date)` writes an HTTP-date validator. The
ETag helper rejects weak tags, `*`, lists, and malformed tag syntax before a
socket is opened; the date helper requires a value that parses as an HTTP-date.
Manual `If-Range` headers remain available through the generic header API for
cases outside the helper validation.

`Response::accept_ranges()` parses response `Accept-Ranges` fields into
`AcceptRanges` metadata. It returns `Ok(None)` when the header is absent;
present values expose `units()`, `is_none()`, and `accepts_bytes()`. Parsing is
bounded to 64 KiB per header field and 256 range units, rejects malformed or
empty values, rejects duplicate units case-insensitively across all parsed
fields while preserving each unit's spelling and order, and represents the
`none` sentinel as an empty unit list. Raw `Accept-Ranges` fields
remain available through the ordinary response header accessors even when the
typed parser rejects a malformed value.

On the client side, these APIs only emit request metadata and expose response
metadata. RTTP does not generate `Range` requests from `Accept-Ranges`,
evaluate `If-Range`, automatically retry or replay a failed or full-response
range request, store cached responses, synthesize multipart range requests,
implement a partial response engine, serve bytes, slice content, resume
downloads, follow redirects because of range metadata, choose status-policy
behavior, or apply automatic cache validation policy.

### Bounded HTTP/1.1 conditional requests

`HttpClient` can emit common conditional validators with
`if_none_match(etag)`, `if_match(etag)`, `if_modified_since(http_date)`, and
`if_unmodified_since(http_date)`. The ETag helpers accept one validator per
call: `*`, a strong tag such as `"abc"`, or a weak tag such as `W/"abc"`.
Comma-separated validator lists remain available through the generic `header`
API. The date helpers validate the supplied value as one HTTP-date through the
shared protocol `IfModifiedSince` and `IfUnmodifiedSince` types and emit the
canonical IMF-fixdate form before a socket is opened; malformed, empty,
oversized (over 64 KiB), or control-byte values are rejected.

```rust
client
  .get()
  .url("http://example.test/manifest")
  .if_none_match(r#""revision-42""#)?
  .if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT")?
  .emit()?;
```

Responses expose conditional metadata with `Response::is_not_modified()` for
`304 Not Modified`, `Response::is_precondition_failed()` for
`412 Precondition Failed`, typed bounded `Response::etag()`, and
`Response::last_modified()`. Malformed, oversized, or duplicate response
`ETag` fields make the typed helper return an error while raw values remain
available through `Response::etag_value()`, `Response::header_value()`, and
`Response::header_values()`. `304` responses are handled as bodyless even if
misleading body framing is present; `412` remains an ordinary response status
for the caller to handle.

These helpers do not add cache storage, automatic revalidation, or a
cache-control engine. `If-Range` is range-scoped and uses the dedicated
`if_range_etag` and `if_range_date` helpers above. RTTP only emits bounded
request headers and exposes response metadata; cache persistence and validation
policy remain application-owned.

### Bounded HTTP/1.1 informational responses and Early Hints

`rttp_client` skips HTTP/1.1 informational response heads before the terminal
response, while preserving the skipped metadata on
`Response::informational_responses()`. Each `InformationalResponse` exposes
its `code()`, `reason()`, raw `headers()`, `headers_of_name()`,
`header_value()`, and `header_values()`, so callers can observe `100 Continue`,
`102 Processing`, `103 Early Hints`, and other skippable `1xx` heads without
turning them into the final response.

The skipped head parser is bounded and validation-oriented. Each informational
head is limited to the normal response-head bound, must be an HTTP/1.1 `1xx`
status line, must contain well-formed header fields, and must not declare
`Content-Length` or `Transfer-Encoding` body framing. Malformed or oversized
informational heads reject the response before the final response bytes are
consumed. `101 Switching Protocols` is not treated as skipped metadata: it
remains the terminal response for upgrade and tunnel handoff paths, and its
socket handoff stays separate from `Response::informational_responses()`.

On the server side, `HttpResponse::early_hints(links)` and
`HttpResponse::early_hints_with_headers(links, metadata)` construct a bodyless
`103 Early Hints` response model. The helpers require at least one validated
`Link` value, bound each header value to 64 KiB, reject malformed field bytes,
and reject metadata fields that would affect connection or body framing such
as `Connection`, `Content-Length`, `TE`, `Trailer`, `Transfer-Encoding`, and
`Upgrade`. Manual raw headers on ordinary responses remain preserved until a
typed helper is requested or a typed constructor replaces them.

Early Hints support is metadata-only. RTTP does not automatically preload
linked resources, apply cache policy, redirect, retry, replay requests,
generate routes, expose a streaming early-write API, or add TLS/ALPN behavior
from `103` metadata.

### Bounded HTTP/1.1 Link response metadata

`Response::links()` parses one or more final-response `Link` fields into
ordered `LinkValues` and `LinkValue` metadata. Each value retains its target
URI/reference and ordered parameters, including unknown parameters such as
extensions alongside `rel`. Parsing is on demand, so malformed or oversized
metadata returns an error without discarding raw response headers. Fields and
parameter values are limited to 64 KiB, with at most 256 link-values and 256
parameters per value.

This shares Early Hints' bounded metadata posture, but it does not preload,
schedule fetches, redirect, apply cache policy, or generate routes.

### Bounded Cache-Control request metadata

`HttpClient::cache_control_no_cache()`, `cache_control_no_store()`, and
`cache_control_max_age(seconds)` append common request directives to one
`Cache-Control` field. `cache_control_extension(name)` and
`cache_control_extension_with_value(name, value)` append valueless and
token-valued extension directives. The helpers reject malformed tokens,
duplicate directives, oversized values, and more than 256 directives before a
connection is opened.

These methods only declare request metadata: they do not create a cache,
compute freshness, or automatically revalidate. Raw
`header(("Cache-Control", value))` remains available for quoted-string
extension values or other syntax outside this bounded API.

### Bounded HTTP/1.1 Cache-Control behavior

`Response::cache_control()` parses one or more response `Cache-Control` header
fields into `CacheControl`. It exposes the common response directives
`no-cache`, `no-store`, `max-age`, `s-maxage`, `private`, `public`,
`must-revalidate`, `proxy-revalidate`, `immutable`,
`stale-while-revalidate`, and `stale-if-error`. Quoted field-name lists on
`no-cache` and `private` are split into field names, and unknown extension
directives are preserved as `CacheControlExtension` values with their token name
and optional parsed value.

The parser is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed header set is limited to 256 directives, directive
names and unquoted values must be valid HTTP tokens, quoted strings must be
well formed, and delta-seconds values must be unquoted non-negative decimal
integers that fit in `u64`. A malformed `Cache-Control` value makes
`Response::cache_control()` return an error; the original response headers and
body remain available through the ordinary response APIs.

`Cache-Control` parsing is intentionally separate from conditional validator
helpers. `Response::etag()` and `Response::last_modified()` expose validators,
and request helpers such as `if_none_match()` and `if_modified_since()` can
emit conditional requests when the application chooses to do so. RTTP does not
combine `Cache-Control` directives with validators to decide freshness, build a
cache entry, or issue a follow-up request.

The client has no cache store and does not perform automatic revalidation,
freshness calculation against wall-clock time, `Vary` matching, shared-cache
policy enforcement, or automatic conditional requests. Directives such as
`max-age`, `s-maxage`, `no-cache`, `must-revalidate`, and extension directives
are exposed as parsed metadata only.

### Bounded Cache-Status response metadata

`Response::cache_status()` parses one or more response `Cache-Status` header
fields into `CacheStatus`. It combines repeated fields in wire order as an
RFC 9211 / RFC 8941 list of cache identifiers (`sf-token` or `sf-string`) plus
parameters, including typed `hit`, `fwd`, `fwd-status`, `ttl`, `stored`,
`collapsed`, `key`, and `detail` values and well-formed extension parameters.

Each field value is limited to 64 KiB, the parsed header set is limited to
256 members, each member is limited to 256 parameters, and each parameter
value is limited to 64 KiB. A malformed `Cache-Status` value makes the helper
return an error while the raw response headers and body remain available
through the ordinary response APIs. An absent header returns `Ok(None)`.

Cache-Status metadata is exposed for application-owned policy only. RTTP does
not store cache entries, compute freshness, revalidate, select endpoints,
retry, or alter response acceptance from `Cache-Status`.

### Bounded CDN-Cache-Control response metadata

`Response::cdn_cache_control()` parses one or more response
`CDN-Cache-Control` header fields into `CdnCacheControl`. It preserves
directives in wire order, including CDN-specific extension directives, and
exposes each directive token name plus its optional parsed value.

The helper uses the same bounded validation model as response
`Cache-Control`: each field value is limited to 64 KiB, the parsed header set
is limited to 256 directives, directive names and unquoted values must be valid
HTTP tokens, and quoted strings must be well formed. A malformed
`CDN-Cache-Control` value makes the helper return an error while the raw
response headers and body remain available through the ordinary response APIs.

CDN cache metadata is exposed for application-owned policy only. RTTP does not
create a CDN cache, compute freshness, revalidate automatically, implement
surrogate-key behavior, apply shared-cache policy, retry, replay, redirect, or
alter response acceptance from `CDN-Cache-Control`.

### Bounded HTTP/1.1 Date, Age, and Expires behavior

`Response::date()` parses the response `Date` header as singleton HTTP-date
metadata. The helper returns `Ok(None)` when the header is absent, returns
`SystemTime` when the value is present and valid, and returns an error for
malformed or duplicate values.

`Response::age()` parses the response `Age` header through the protocol `Age`
type as HTTP/1.1 delta-seconds metadata. The helper returns `Ok(None)` when the
header is absent, returns the non-negative decimal value as `u64` when it is
present and valid, and returns an error for empty, signed, fractional,
non-numeric, comma-list, overflowing, duplicate, or oversize values.
Surrounding SP and HTAB are trimmed as optional whitespace. Each field value
is bounded to 64 KiB, and the accepted numeric bound is the `u64`
delta-seconds range: `0` through `u64::MAX`.

`Response::expires()` parses the response `Expires` header as an HTTP-date using
the same HTTP-date parser used by the client date helpers. It returns
`Ok(None)` when the header is absent, returns `SystemTime` for valid HTTP-date
values including the standard IMF-fixdate and obsolete HTTP-date forms accepted
by the parser, and returns an error for malformed or non-date values.

Malformed helper values do not reject the raw response. The original `Date`,
`Age`, and `Expires` fields remain available through `header_value`,
`header_values`, and the other raw header accessors. These helpers expose
metadata only; RTTP does not calculate freshness, correct clock skew, validate
cache state against wall-clock time, store responses, match stored responses,
revalidate responses, apply shared-cache policy, issue automatic conditional
requests, retry, redirect, schedule work, or choose status policy.

### Bounded Memento-Datetime behavior

`Response::memento_datetime()` parses the response `Memento-Datetime` header
through the protocol `MementoDatetime` type as one singleton IMF-fixdate.
The helper returns `Ok(None)` when the header is absent, returns
`MementoDatetime` when the value is present and valid, and returns an error
for empty, malformed, control-byte, duplicate, or oversize values. Each field
value is bounded to 64 KiB. Surrounding SP and HTAB are trimmed as optional
whitespace.

Malformed helper values do not reject the raw response. The original
`Memento-Datetime` field remains available through `header_value` and
`header_values`. This helper exposes metadata only; RTTP does not select an
archival representation, negotiate `Accept-Datetime`, implement TimeGate
behavior, retry, or change transport handling.

### Bounded HTTP/1.1 Retry-After behavior

`Response::retry_after()` parses a single response `Retry-After` header as
either HTTP-date metadata or non-negative delta-seconds. It returns `Ok(None)`
when the header is absent. Present values are exposed as `RetryAfter`, with
`delta_seconds()` returning `u64` for the delta form and `http_date()`
returning `SystemTime` for the date form.

The helper is bounded and validation-oriented. The header value is limited to
64 KiB, duplicate `Retry-After` header fields are rejected, delta-seconds must
be unsigned decimal digits that fit in `u64`, and malformed dates or other
invalid values return an error. The original response headers remain available
through `Response::header_value()` and `Response::header_values()`.

RTTP does not sleep, retry, replay requests, apply backoff, integrate with a
scheduler, calculate cache freshness, or decide status-code retry policy from
`Retry-After`.

### Bounded HTTP/1.1 Allow behavior

`Response::allow()` parses one or more response `Allow` header fields into
`Allow` metadata. It returns `Ok(None)` when the header is absent. Present
values are parsed as comma-separated HTTP method tokens across all `Allow`
fields in wire order, and the resulting method list is exposed by
`Allow::methods()` and checked with `Allow::contains_method(method)`.

The helper is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed method list is limited to 256 entries, and each
method must be a valid HTTP token. Empty list members, malformed method tokens,
duplicate method names, oversized values, and too many methods make
`Response::allow()` return an error while leaving the original response headers
and body available through the ordinary response APIs.

The helper is metadata-only. `rttp_client` does not choose fallback methods,
retry or replay requests, or attach automatic behavior to `405` or `OPTIONS`
responses based on `Allow`.

### Bounded HTTP/1.1 Content-Language behavior

`Response::content_language()` parses one or more response `Content-Language`
header fields into `ContentLanguage` metadata. It returns `Ok(None)` when the
header is absent. Present values are parsed as comma-separated language ranges
across all `Content-Language` fields in wire order, and the resulting list is
exposed by `ContentLanguage::tags()`. `ContentLanguage::parse(value)` is
available when callers want to validate a single raw field value directly.

The helper is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed tag list is limited to 256 entries, and concrete
tags must contain non-empty ASCII alphanumeric subtags separated by hyphens
with an alphabetic primary subtag. Empty members, malformed values, duplicate
tags across one or more helper-parsed header fields, oversized values, and too
many tags make `Response::content_language()` return an error while leaving
the original response headers and body available through the ordinary response
APIs.

The helper interoperates with the existing response metadata helpers by
preserving raw headers and parsing only when requested. RTTP does not perform
automatic language negotiation, locale fallback, variant matching, cache
policy, retry, replay, redirect, or status-policy behavior from
`Content-Language`.

### Bounded HTTP/1.1 Accept-Language behavior

`HttpClient::accept_language(ranges)` validates and emits one
`Accept-Language` request header from ordered language ranges through the
shared protocol-owned `AcceptLanguage` type. Each range may include an
optional `q` weight, for example `fr-CA; q=0.8`; `*` is also accepted. The
shared primitive limits each supplied value to 64 KiB and the parsed range
list to 32 entries, rejects malformed ranges or q-values and case-insensitive
duplicates, and returns a builder error before opening a connection. Raw
`header(("Accept-Language", value))` remains available when callers need to
preserve unvalidated metadata.

On the server, `Request::accept_language()` and
`HttpRequest::accept_language()` parse received fields in wire order into
`HttpAcceptLanguages` (the server alias for the shared protocol `AcceptLanguage`
type), returning `Ok(None)` when the header is absent.
`HttpAcceptLanguages::ranges()` and `HttpAcceptLanguages::qualities()` expose
the validated ranges and optional q-values. Each received field is bounded to
64 KiB and the combined range list to 32 entries; malformed values, invalid
q-values, and duplicate ranges return `HttpAcceptLanguageParseError` without
changing the raw request headers.

These helpers are metadata-only. RTTP does not perform locale matching,
fallback selection, translation lookup, routing, or automatic response choice
from `Accept-Language`.

### Bounded HTTP/1.1 Content-Location behavior

`Response::content_location()` parses a response `Content-Location` header into
the shared protocol-owned `ContentLocation` metadata type. It returns
`Ok(None)` when the header is absent and rejects duplicate header fields
because `Content-Location` is handled as a singleton response metadata field.
`ContentLocation::parse(value)` is available when callers want to validate one
raw field value directly; it trims outer HTTP optional whitespace and exposes
the preserved reference text with `ContentLocation::as_str()` and
`ContentLocation::header_value()`.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty absolute URI or relative URI reference that can
be parsed without control characters, interior whitespace, unsafe field-value
characters, malformed URI syntax, or broken percent-encoding.
Malformed values, duplicated singleton fields, and oversized values make
`Response::content_location()` return an error while leaving the original
response headers and body available through `Response::header_value()`,
`Response::header_values()`, and the other response metadata helpers.

The helper interoperates with adjacent response metadata helpers such as
`Response::cache_control()`, `Response::allow()`,
`Response::content_language()`, `Response::vary()`, `Response::retry_after()`,
`Response::age()`, `Response::expires()`, and
`Response::accept_ranges()` by preserving raw headers and parsing only when
requested. It is metadata-only: RTTP does not treat `Content-Location` as
redirect behavior, cache variant selection, representation replacement,
retry/replay behavior, route generation, or status-policy behavior.

### Bounded HTTP/1.1 Service-Worker-Allowed behavior

`Response::service_worker_allowed()` parses a response `Service-Worker-Allowed`
header into the shared protocol-owned `ServiceWorkerAllowed` metadata type. It
returns `Ok(None)` when the header is absent and rejects duplicate header
fields because `Service-Worker-Allowed` is handled as a singleton response
metadata field. `ServiceWorkerAllowed::parse(value)` is available when callers
want to validate one raw field value directly; it trims outer HTTP optional
whitespace and exposes the preserved path text with
`ServiceWorkerAllowed::as_str()` and `ServiceWorkerAllowed::header_value()`.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must be a non-empty origin-relative or absolute path without
control or non-ASCII characters, interior whitespace, unsafe field-value characters, broken
percent-encoding, absolute URIs, or network-path authority forms.
Malformed values, duplicated singleton fields, and oversized values make
`Response::service_worker_allowed()` return an error while leaving the original
response headers and body available through `Response::header_value()`,
`Response::header_values()`, and the other response metadata helpers.

The helper interoperates with adjacent response metadata helpers by preserving
raw headers and parsing only when requested. It is metadata-only: RTTP does
not register service workers, evaluate service-worker scope, resolve the
value against a script URL, or apply application routing policy.

### Bounded HTTP/1.1 Content-DPR behavior

`Response::content_dpr()` parses a response `Content-DPR` header into the
shared protocol-owned `ContentDpr` metadata type. It returns `Ok(None)` when
the header is absent and rejects duplicate header fields because `Content-DPR`
is handled as a singleton response metadata field. `ContentDpr::parse(value)`
is available when callers want to validate one raw field value directly; it
trims outer HTTP optional whitespace and exposes the finite positive ratio with
`ContentDpr::ratio()` plus the preserved decimal text with
`ContentDpr::header_value()`.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB and must match `1*DIGIT["." 1*DIGIT]` as a finite ratio greater than
zero. Zero, trailing or leading decimal points, signs, exponent notation,
control bytes, malformed values, duplicated singleton fields, and oversized
values make `Response::content_dpr()` return an error while leaving the
original response headers and body available through `Response::header_value()`,
`Response::header_values()`, and the other response metadata helpers.

The helper is observation-only. RTTP does not rescale images, send request DPR,
apply Client Hints policy, retry, replay, redirect, or change transport from
`Content-DPR`.

### Bounded HTTP/1.1 representation metadata behavior

`Response::content_type()` parses a singleton response `Content-Type` header
into `ContentType` metadata. It returns `Ok(None)` when the header is absent
and rejects duplicate `Content-Type` fields. Present values expose the
normalized media type through `type_()`, `subtype()`, `essence()`, and
`is(type, subtype)`, plus ordered parameters through `parameters()` and
case-insensitive `parameter(name)`. `ContentType::parse(value)` is available
when callers want to validate one raw field value directly.

`Response::content_encoding()` parses one or more response `Content-Encoding`
fields into `ContentEncoding` metadata. It returns `Ok(None)` when the header
is absent. Present values are parsed as comma-separated content codings across
all fields in wire order and exposed through `ContentEncoding::codings()`.
Coding spelling and order are preserved, while duplicate detection is
case-insensitive.

Both helpers are bounded and validation-oriented. Each field value is limited
to 64 KiB. Client `Content-Type` parsing accepts at most 256 parameters, lowers
the media type and parameter names, rejects missing or malformed media types,
malformed parameter syntax, malformed quoted strings, duplicate parameters,
duplicate singleton fields, CR/LF injection, oversized values, and too many
parameters. Client `Content-Encoding` parsing accepts at most 256 codings and
rejects empty members, malformed tokens, duplicate codings, oversized values,
and too many codings. Parse errors do not reject the raw response: original
headers and body remain available through `Response::header_value()`,
`Response::header_values()`, and the ordinary body APIs.

```rust
let content_type = response.content_type()?.expect("Content-Type");
if content_type.is("application", "json") {
  let charset = content_type.parameter("charset");
}

let content_encoding = response.content_encoding()?.expect("Content-Encoding");
assert_eq!(vec!["gzip", "br"], content_encoding.codings());
```

These helpers are representation metadata only. `rttp_client` does not perform
MIME sniffing, body decoding from this metadata, charset transcoding,
compression or decompression policy, negotiation, cache policy, redirects,
retry/replay, or filesystem serving from `Content-Type` or
`Content-Encoding`.

### Bounded Accept-Charset request metadata

`rttp-protocol` owns the shared `Accept-Charset` primitive. Client helpers
format through that type, and server `Request` / `HttpRequest` helpers parse
with the same rules.

`HttpClient::accept_charset()` appends a validated request charset range,
while `accept_charset_with_q()` accepts an HTTP q-value from `0` through `1`
with at most three fractional digits. The helpers emit one comma-separated
`Accept-Charset` field and reject invalid charset tokens, q-values,
duplicates, oversized values, and more than 32 ranges before a connection is
opened.

On the server, `Request::accept_charset()` and
`HttpRequest::accept_charset()` parse all received `Accept-Charset` fields in
wire order into `HttpRequestAcceptCharsets`, an alias of the shared protocol
type. Each entry provides its `charset()` and q-value `quality()` in
thousandths (`1000` is the default quality of `1`). Absent metadata returns
`Ok(None)`; malformed, duplicate, empty, oversized, or excessive entries
return a parse error without changing the request itself.

These helpers declare and parse metadata only. They do not negotiate, transcode,
decode bodies, sniff MIME types, or select a response charset.

### Bounded Accept-Encoding request metadata

`rttp-protocol` owns the shared `Accept-Encoding` primitive. Client helpers
format through that type, and server `Request` / `HttpRequest` helpers parse
with the same rules.

`HttpClient::accept_encoding()` appends a validated request coding, while
`accept_encoding_with_q()` accepts an HTTP q-value from `0` through `1` with
at most three fractional digits. Convenience helpers cover `gzip`, `deflate`,
`br`, and `identity`, including q-value variants. The helpers emit one
comma-separated `Accept-Encoding` field and reject invalid coding tokens,
q-values, duplicates, oversized values, and more than 32 codings before a
connection is opened.

On the server, `Request::accept_encoding()` and
`HttpRequest::accept_encoding()` parse all received `Accept-Encoding` fields
in wire order into `HttpRequestAcceptEncodings`, an alias of the shared
protocol type. Each entry provides its `coding()` and q-value `quality()` in
thousandths (`1000` is the default quality of `1`). Absent metadata returns
`Ok(None)`; malformed, duplicate, empty, oversized, or excessive entries
return a parse error without changing the request itself.

These helpers declare and parse metadata only. They do not enable automatic
compression, decompression, or content negotiation.

### Bounded digest preference request metadata

`HttpClient::want_content_digest()` and `want_content_digest_with_q()` append
validated algorithms to one `Want-Content-Digest` field. `want_repr_digest()`
and `want_repr_digest_with_q()` do the same for `Want-Repr-Digest`. The
`_with_q` variants accept RFC 9530 relative preference values from `0` through
`10`; the default helper value is `10`.

```rust
HttpClient::new()
  .get()
  .url("http://example.test/asset")
  .want_content_digest("sha-256")?
  .want_repr_digest_with_q("sha-512", "8")?;
```

Each helper emits a comma-separated field and rejects malformed algorithm
tokens or q-values, case-insensitive duplicate algorithms, fields over 64 KiB,
and more than 32 algorithms before opening a connection. Raw
`header(("Want-Content-Digest", value))` and
`header(("Want-Repr-Digest", value))` remain available for syntax outside the
bounded helper API.

On the server, `Request::want_content_digest()` and
`HttpRequest::want_content_digest()` parse received `Want-Content-Digest`
fields in wire order into `HttpWantContentDigest`. `Request::want_repr_digest()`
and `HttpRequest::want_repr_digest()` do the same for `Want-Repr-Digest` into
`HttpWantReprDigest`. Each entry exposes `algorithm()` and `preference()` (`0`
through `10`). Absent metadata returns `Ok(None)`; malformed, duplicate, empty,
oversized, or excessive entries return a parse error without changing the
request itself.

These helpers declare and parse preferences only. They do not select an
algorithm, compute digests, verify response body hashes, attach
`Content-Digest` or `Repr-Digest`, retry requests, or sign messages.

### Bounded Accept request metadata

`HttpClient::accept()` appends one validated media range, while
`accept_with_q()` adds a q-value from `0` through `1` with at most three
fractional digits. Convenience helpers cover `*/*`, JSON, HTML, XML, and plain
text, including q-value variants. The client rejects malformed media types or
parameters, duplicate parameters, invalid q-values, oversized fields, and more
than 32 media ranges before opening a connection. Raw
`header(("Accept", value))` remains available for values outside the bounded
helper API.

These helpers only declare request metadata. RTTP does not perform automatic
representation selection, retries, redirects, caching, or server policy from
`Accept`.

### Bounded `Expect: 100-continue` request metadata

`HttpClient::expect_continue()` formats the protocol-owned `Expect` singleton
as `Expect: 100-continue`. It is metadata only: the client does not delay
the request body or wait for an interim response. Raw
`header(("Expect", value))` remains available for extension values outside
the typed helper. `Response::informational_responses()` continues to expose a
received `100 Continue` alongside other bounded informational responses.

On the server, `Request::expectations()` and `HttpRequest::expectations()`
delegate to the shared protocol type and return bounded `HttpExpectations`
aliases. `expects_continue()` identifies the standardized expectation, while
`unsupported()` preserves extension names for handler policy. Absent fields
return `Ok(None)`; malformed, duplicate, oversized, or excessive values
return `HttpExpectParseError` without changing the raw request. The server
does not automatically send `100 Continue` or reject unsupported
expectations.

### Bounded Authorization request metadata

`HttpClient::authorization(scheme, credentials)` emits one validated
`Authorization` field from an HTTP-token scheme and a non-empty credential
value bounded to 64 KiB. The client uses the shared `rttp-protocol`
authorization primitive, including CR, LF, NUL, and other control-byte
injection checks. `header(("Authorization", value))` remains available as the
raw escape hatch for application-specific schemes and syntaxes.

On the server, `Request::authorization()` and `HttpRequest::authorization()`
parse exactly one field into `HttpAuthorization`, exposing `scheme()` and
`credentials()`. `Request::proxy_authorization()` and
`HttpRequest::proxy_authorization()` parse `Proxy-Authorization` into
`HttpProxyAuthorization` with the same bounds. Absent metadata returns
`Ok(None)`; invalid, oversized, control-byte-injected, or duplicate fields
return an error so handlers do not receive ambiguous credentials. Typed debug
output redacts credential values.

Credential interpretation remains application-owned. These helpers do not
store credentials, validate individual schemes, refresh tokens, process
challenges, retry requests, or forward credentials across redirects.

### Bounded TE request metadata

`HttpClient::te()` appends a validated transfer coding, `te_with_q()` accepts
an HTTP q-value from `0` through `1` with at most three fractional digits, and
`te_trailers()` declares support for trailers. The helpers validate each
member and the combined field through the shared protocol-owned `rttp-protocol`
`Te` type, which owns coding tokens, the `chunked` rejection, the
`trailers`-without-q-value rule, q-value thousandths, case-insensitive
duplicates, the 32-coding member bound, and the 64 KiB value bound. They emit
one comma-separated `TE` field and reject invalid tokens, q-values, duplicates,
oversized values, and more than 32 codings before a connection is opened.
`trailers` cannot carry a q-value.

On the server, `Request::te()` and `HttpRequest::te()` parse received fields in
wire order through the same protocol type into `HttpRequestTe`; each `HttpTe`
exposes `coding()`, optional thousandths `quality()`, and `is_trailers()`. This
is metadata parsing only: it does not implement transfer coding, trailer
negotiation, compression, or proxy behavior. Bounded h2c remains conservative:
it emits only an exact `TE: trailers` field and strips every other `TE` value
with HTTP/1.x connection-specific request metadata.

### Bounded WWW-Authenticate response metadata

`Response::www_authenticate()` parses all received `WWW-Authenticate` fields
in wire order into bounded `WwwAuthenticate` challenge metadata. Challenges
expose their authentication scheme, optional token68 value, and ordered
auth-parameters with quoted-string unescaping. Absent metadata returns
`Ok(None)`; malformed syntax, duplicate parameter names, invalid tokens,
oversized values, and excessive challenges or parameters return an error while
the raw response headers remain available.

On the server, `HttpWwwAuthenticate::parse()` validates the same syntax and
`HttpResponse::with_www_authenticate()` replaces raw `WWW-Authenticate`
fields with one validated value. `HttpResponse::www_authenticate()` parses raw
response fields on demand without changing them.

These helpers expose authentication challenges as metadata only. RTTP does not
store credentials, select an authentication policy, retry requests, generate
`Authorization`, implement Basic or Bearer authentication, or change redirect
behavior.

### Bounded Proxy-Authenticate response metadata

`Response::proxy_authenticate()` parses all received `Proxy-Authenticate`
fields in wire order into bounded `ProxyAuthenticate` challenge metadata.
Challenges expose their proxy authentication scheme, optional token68 value,
and ordered auth-parameters with quoted-string unescaping. Absent metadata
returns `Ok(None)`; malformed syntax, duplicate parameter names, invalid
tokens, oversized values, and excessive challenges or parameters return an
error while the raw response headers remain available.

`ProxyAuthenticate::parse()` validates a single field value, and
`ProxyAuthenticate::parse_values()` preserves challenges across multiple field
values. These helpers expose proxy authentication challenges as metadata only.
RTTP does not store credentials, select a proxy authentication policy, retry
requests, generate `Proxy-Authorization`, implement Basic or Bearer
authentication, or change redirect behavior.

### Bounded Proxy-Status response metadata

`Response::proxy_status()` parses all received RFC 9209 `Proxy-Status` fields
in wire order into bounded Token or String proxy identifiers with opaque
parameters. Absent metadata returns `Ok(None)`; empty lists, inner-lists,
malformed syntax, control bytes, oversized values, and duplicate parameters
return an error while the raw response headers remain available.

On the server, `HttpProxyStatus::parse()` validates the same syntax and
`HttpResponse::with_proxy_status()` replaces raw `Proxy-Status` fields with
one validated value. `HttpResponse::proxy_status()` parses raw response fields
on demand without changing them.

These helpers expose Proxy-Status as metadata only. RTTP does not interpret
proxy health, retry requests, promote trailers, or generate origin
`Proxy-Status` values.

### Bounded Server-Timing response metadata

`Response::server_timing()` parses all received `Server-Timing` fields in wire
order into bounded `ServerTiming` metrics. Metrics expose their name, optional
`dur` duration, optional `desc` text, and ordered extension parameters;
duplicate metric names remain distinct. Malformed values, duplicate parameter
names, oversized values, and excessive metrics or parameters return an error
while the raw response headers remain available.

On the server, `HttpServerTiming::parse()` validates the same syntax and
`HttpResponse::with_server_timing()` replaces raw `Server-Timing` fields with
one validated value. `HttpResponse::server_timing()` parses raw response fields
on demand without changing them.

These helpers only parse and format timing metadata. They do not collect
metrics, record measurements, export telemetry, or add a metrics backend.

### Bounded Alt-Used response metadata

`Response::alt_used()` and server `HttpResponse::with_alt_used()` /
`HttpResponse::alt_used()` parse or declare one bounded `Alt-Used` authority
through the shared protocol `AltUsed` type. Valid metadata preserves host
spelling, optional port, and bracketed IPv6 literal form. Malformed
authorities, duplicate fields, and values larger than 64 KiB are rejected
while raw headers remain available on parse failures; typed server declaration
replaces existing raw `Alt-Used` fields.

These helpers are metadata-only. RTTP does not select alternative services,
rewrite origins, migrate sockets, retry, or change connection policy from
`Alt-Used`.

### Bounded Origin-Trial response metadata

`Response::origin_trials()` and server `HttpResponse::with_origin_trials()` /
`HttpResponse::origin_trials()` parse or declare bounded opaque `Origin-Trial`
tokens through the shared protocol `OriginTrials` type. Valid metadata
preserves multiple tokens and duplicates in wire order. Each token is limited
to 8 KiB, the collection is limited to 64 tokens, and the combined token
bytes are limited to 64 KiB. Injected controls, obs-text, empty values, and
oversized collections are rejected while raw headers remain available on
parse failures; typed server declaration replaces existing raw `Origin-Trial`
fields and emits one header per token. Token material is redacted from debug
output.

These helpers are metadata-only. RTTP does not validate token signatures,
expiration, origin applicability, or activate browser trials.

### Bounded Warning response metadata

`Response::warning()` parses all received `Warning` fields in wire order into
bounded `Warning` warning-value metadata. Each item exposes its 3-digit
warn-code, opaque warn-agent, unescaped warn-text, and optional HTTP-date.
Absent metadata returns `Ok(None)`; malformed quoting, invalid codes, empty
members, oversized values, and excessive items return an error while the raw
response headers remain available.

These helpers expose Warning as metadata only. RTTP does not use warn-codes as
cache policy, calculate freshness, treat responses as stale, or change
response acceptance.

### Bounded Access-Control-Allow-Credentials response metadata

`Response::access_control_allow_credentials()` parses a singleton
`Access-Control-Allow-Credentials` response field into
`AccessControlAllowCredentials` metadata. Absent metadata returns `Ok(None)`.
The field value must be exactly the standards-defined `true` token, matched
case-sensitively per the Fetch `%s"true"` grammar and exposed in canonical
lowercase form; surrounding SP and HTAB are trimmed. Unknown tokens, empty
values, duplicate fields, oversized values, and control bytes return an error
while the raw response headers remain available through
`Response::header_value()` and `Response::header_values()`.

On the server, `HttpAccessControlAllowCredentials::parse()` validates the same
syntax and `HttpResponse::with_access_control_allow_credentials()` replaces raw
`Access-Control-Allow-Credentials` fields with one validated value.
`HttpResponse::access_control_allow_credentials()` parses raw response fields
on demand without changing them.

These helpers expose credentials metadata only. RTTP does not evaluate CORS
requests, attach credentials to requests, or grant credentials automatically.

### Bounded NEL response metadata

`Response::nel()` parses the `NEL` response field as bounded W3C Network Error
Logging policy metadata. The policy exposes its required non-negative
`max_age` as `u64`, optional `report_to` name, `include_subdomains` flag, and
`success_fraction`/`failure_fraction` values as checked members; unknown JSON
members are preserved verbatim without policy semantics. Absent metadata
returns `Ok(None)`; malformed JSON, invalid member types, duplicate singleton
members, non-finite or out-of-range fractions, and oversized input return an
error while the raw response headers remain available.

On the server, `HttpNel::parse()` validates the same syntax and
`HttpResponse::with_nel()` replaces raw `NEL` fields with one validated value.
`HttpResponse::nel()` parses raw response fields on demand without changing
them.

These helpers expose NEL as metadata only. RTTP does not send network error
reports, persist policy, configure Reporting endpoint groups, or change
redirect behavior.

### Bounded Reporting-Endpoints response metadata

`Response::reporting_endpoints()` parses retained `Reporting-Endpoints`
dictionary fields through the shared protocol type. Present values combine
all fields in wire order into at most 32 endpoint-name to quoted-URL
members. Each field value is bounded to 64 KiB, and the combined raw
field-value bytes are bounded to 64 KiB. Endpoint names are lowercase tokens
that may start with `*`; URLs must be quoted and unescape only `\\` and
`\"`. Absent metadata returns `Ok(None)`; invalid names, unquoted URLs,
malformed quoted strings, duplicate names, oversized input, and too many
members return an error while the raw response headers remain available.

On the server, `HttpReportingEndpoints::parse()` and
`HttpReportingEndpoints::from_endpoints()` validate the same dictionary and
`HttpResponse::with_reporting_endpoints()` replaces raw
`Reporting-Endpoints` fields with one validated value.
`HttpResponse::reporting_endpoints()` parses raw response fields on demand
without changing them.

These helpers expose Reporting-Endpoints as metadata only. RTTP does not
schedule, send, persist, retry, or route reports.

### Bounded Cross-Origin-Opener-Policy-Report-Only response metadata

`Response::cross_origin_opener_policy_report_only()` parses retained
`Cross-Origin-Opener-Policy-Report-Only` fields through the shared protocol
type. Present values must be a singleton structured-field item using the
canonical COOP directives `unsafe-none`, `same-origin-allow-popups`,
`same-origin`, or `noopener-allow-popups`. Well-formed parameters are retained;
`report-to` is exposed as a reporting-endpoint name when present. Each field
value is bounded to 64 KiB. Absent metadata returns `Ok(None)`; duplicate
fields, duplicate parameter names, unknown directives, malformed structured
fields, and oversized values return an error while the raw response headers
remain available.

On the server, `HttpCrossOriginOpenerPolicyReportOnly::parse()` validates the
same syntax and `HttpResponse::with_cross_origin_opener_policy_report_only()`
replaces raw `Cross-Origin-Opener-Policy-Report-Only` fields with one
validated value. `HttpResponse::cross_origin_opener_policy_report_only()`
parses raw response fields on demand without changing them.

These helpers expose COOP Report-Only as metadata only. RTTP does not isolate
browsing contexts, validate `Reporting-Endpoints` members, deliver reports, or
schedule report delivery.

### Bounded Keep-Alive response metadata

Client `Response::keep_alive()` and server `HttpResponse::keep_alive()` parse
all received `Keep-Alive` fields in wire order into bounded RFC 2068
`HttpKeepAlive` metadata; `HttpResponse::with_keep_alive` validates and
replaces the `Keep-Alive` response field. The optional `timeout` delta-seconds
and optional `max` `1*DIGIT` values are parsed as checked unsigned integers;
unrecognized `name=token` parameters are preserved as bounded extension
metadata. Duplicate recognized parameters, malformed values, overflow,
oversized values, and excessive elements return an error while the raw response
headers remain available.

These helpers expose Keep-Alive as metadata only. RTTP does not change
connection lifetime, connection pooling, keep-alive timers, or HTTP/2 behavior.

### Bounded HTTP/1.1 request control metadata

`Request::max_forwards()` and `HttpRequest::max_forwards()` reuse the shared
protocol `Max-Forwards` type. They return `Ok(None)` when the field is absent
and otherwise expose one singleton `1*DIGIT` hop count that fits in `u32`
(`0` through `4294967295`). Duplicate, empty, signed, non-decimal, overflowing,
oversized (over 64 KiB), or control-byte values return a parse error while
`header("Max-Forwards")` continues to expose the raw field. The client helper
`HttpClient::max_forwards()` validates and emits the same type. RTTP does not
decrement the value, route the request, select TRACE or OPTIONS, or infer
forwarding behavior.

`HttpClient::depth()` validates and emits one WebDAV `Depth` request field
through the shared protocol `Depth` type, replacing any existing same-name
field before a socket is opened. `Request::depth()` and
`HttpRequest::depth()` parse received fields into the same `HttpDepth`
representation, returning `Ok(None)` when absent. Recognized values are the
singleton depth values `0`, `1`, and `infinity`, bounded to 64 KiB with
optional surrounding SP or HTAB; malformed, duplicate, oversized, and
control-byte values are rejected while raw request headers remain available
when the typed parser reports an error. RTTP does not traverse resources,
select WebDAV methods, or enforce method policy.

`HttpClient::destination()` validates and emits one WebDAV `Destination`
request field through the shared protocol `Destination` type, replacing any
existing same-name field before a socket is opened. `Request::destination()`
and `HttpRequest::destination()` parse received fields into the same
`HttpDestination` representation, returning `Ok(None)` when absent. A
recognized value is one absolute URI, bounded to 64 KiB with optional
surrounding SP or HTAB; the trimmed URI string is preserved without
resolution or normalization. Malformed, relative, duplicate, oversized, and
control-byte values are rejected while raw request headers remain available
when the typed parser reports an error. RTTP does not resolve the
destination, authorize the target URI, select WebDAV methods, or copy, move,
or delete application resources.

`HttpClient::timeout()` validates and emits one WebDAV `Timeout` request
field through the shared protocol `Timeout` type, replacing any existing
same-name field before a socket is opened. `Request::timeout()` and
`HttpRequest::timeout()` parse received fields into the same `HttpTimeout`
representation, returning `Ok(None)` when absent. Recognized values are
ordered `Second-n` and `Infinite` alternatives, bounded to 64 KiB per field
and 64 KiB aggregate with at most 32 members; malformed, overflowing,
duplicate, oversized, too-many-member, and control-byte values are rejected
while raw request headers remain available when the typed parser reports an
error. RTTP does not create locks, refresh locks, or select an application
timeout.

`HttpClient::idempotency_key()` validates and emits one opaque `Idempotency-Key`
request field through the shared protocol `IdempotencyKey` type, replacing any
existing same-name field before a socket is opened. `Request::idempotency_key()`
and `HttpRequest::idempotency_key()` parse received fields into the same
representation, returning `Ok(None)` when absent. A recognized value is a
singleton key of one or more visible ASCII characters bounded to 64 KiB with
optional surrounding SP or HTAB; empty, space-containing, control-byte
(including CR/LF/NUL and obs-text), duplicate, and oversized values are
rejected. The key is redacted from typed `Debug`, and raw request headers
remain available when the typed parser reports an error. These helpers declare
and observe request metadata only: RTTP does not retry requests, store or
compare keys across requests, deduplicate requests, or apply application
idempotency policy.

`HttpClient::sec_websocket_key()` validates and emits one `Sec-WebSocket-Key`
request field through the shared protocol `SecWebSocketKey` type, replacing
any existing same-name field before a socket is opened. `Request::sec_websocket_key()`
and `HttpRequest::sec_websocket_key()` parse received fields into the same
representation, returning `Ok(None)` when absent. A recognized value is a
singleton RFC 4648 section 4 base64 encoding of exactly 16 nonce bytes bounded
to 64 KiB with optional surrounding SP or HTAB; empty, interior-whitespace,
non-base64, URL-safe or unpadded, wrong-decoded-length, control-byte
(including CR/LF/NUL and obs-text), duplicate, and oversized values are
rejected. Server responses can derive bounded singleton `Sec-WebSocket-Accept`
metadata from a validated key using the RFC 6455 GUID plus SHA-1 and base64
transform; clients can parse `Response::sec_websocket_accept()` and verify it
with `verify_sec_websocket_accept(&key)`. The RFC example key
`dGhlIHNhbXBsZSBub25jZQ==` maps to `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.
Key and accept values are redacted from typed `Debug`, and raw request headers
remain available when the typed parser reports an error. These helpers declare
and observe handshake metadata only: RTTP does not perform an HTTP upgrade,
generate a random nonce, or implement WebSocket frames.

`HttpClient::sec_websocket_version()` validates and emits
`Sec-WebSocket-Version` request metadata through the shared protocol
`SecWebSocketVersion` type, replacing any existing same-name field before a
socket is opened. `Request::sec_websocket_version()` and
`HttpRequest::sec_websocket_version()` parse received fields into the same
representation, returning `Ok(None)` when absent.
`HttpResponse::with_sec_websocket_version(versions)` declares validated
rejection-response metadata without adding `Connection` or `Upgrade`, and
`HttpResponse::sec_websocket_version()` plus client
`Response::sec_websocket_version()` parse attached or received fields.
Recognized values are RFC 6455 version tokens (`0` through `299` without
leading zeros) in numeric descending order, such as `13` or `13, 8, 7`. Empty
members, non-decimal tokens, leading-zero multi-digit tokens, duplicates,
unordered lists, control-byte, over-limit, and oversized values are rejected
while raw headers remain available. These helpers declare and observe
metadata only: RTTP does not perform a WebSocket handshake, emit
`Connection: Upgrade`, compute `Sec-WebSocket-Accept`, negotiate versions, or
switch protocols.

`HttpClient::sec_websocket_protocol()` validates and emits
`Sec-WebSocket-Protocol` request metadata as offers in preference order
through the shared protocol `SecWebSocketProtocol` type, replacing any
existing same-name field before a socket is opened. Recognized members are
RFC 6455 section 11.3.4 `token` values such as `chat`, `superchat`, or
`graphql-transport-ws`, compared case-sensitively. Empty members, malformed
tokens, parameters, slashes, case-sensitive duplicates, control-byte,
over-limit, and oversized values are rejected while raw headers remain
available. Client `Response::sec_websocket_protocol()` parses received
fields as a selection singleton; a multi-token value returns a parse error.
These helpers declare and observe metadata only: RTTP does not perform a
WebSocket handshake, emit `Connection: Upgrade`, choose an application
subprotocol, or switch protocols. Applications own the selection decision.

`HttpClient::traceparent()` and `HttpClient::tracestate()` validate and emit
bounded W3C Trace Context request metadata through shared protocol types,
replacing existing same-name fields before a socket is opened.
`Request::traceparent()` / `HttpRequest::traceparent()` and
`Request::tracestate()` / `HttpRequest::tracestate()` parse received fields,
returning `Ok(None)` when absent and preserving raw headers on parse errors.
Traceparent validation checks version `00`, rejects version `ff` and
unsupported versions, malformed or uppercase identifiers, malformed flags,
duplicates, and all-zero trace or parent identifiers. Tracestate validation
preserves member order while bounding total size, member count, key/value size,
member grammar, and duplicate keys. Typed `Debug` redacts propagation values.
These helpers declare and observe request metadata only: RTTP does not create
trace identifiers, decide sampling, select a tracing backend, or automatically
propagate context.

`HttpClient::baggage()` validates and emits bounded W3C Baggage request
metadata through the shared protocol `Baggage` type, replacing any existing
`baggage` field before a socket is opened. `Request::baggage()` and
`HttpRequest::baggage()` parse received fields, returning `Ok(None)` when
absent and preserving raw headers on parse errors. Validation checks HTTP
token keys, baggage-octet values, optional properties, duplicate member keys,
member order, at most 180 members, 4096-byte per-member limits, and an 8192-byte
combined size. Typed `Debug` and builder or parse errors redact member and
property values. These helpers declare and observe request metadata only:
RTTP does not interpret application baggage data, store request context,
select a tracing backend, or automatically propagate baggage.

`HttpClient::te()`, `te_with_q()`, and `te_trailers()` build a single bounded
`TE` field validated through the shared protocol-owned `rttp-protocol` `Te`
type. `HttpClient::prefer()` and `prefer_with_value()` build a single
bounded `Prefer` field. `Prefer` values are limited to 8 KiB and `wait` accepts
only unsigned decimal integers. Both client helpers reject malformed tokens,
invalid q-values, duplicates, oversized field values, and more than 32 members
before opening a connection; `TE: chunked` is rejected because request framing
remains owned by the existing HTTP/1 implementation.

On the server, `Request::te()`/`HttpRequest::te()` parse ordered `TE` codings
and their q-values, while `Request::prefer()`/`HttpRequest::prefer()` parse
ordered token-only `Prefer` items, including validated `wait` values. Absent
fields return `Ok(None)` and invalid, duplicate, oversized, or excessive values
return a parse error without changing the request's raw headers.

These APIs only declare or parse HTTP/1.1 metadata. They do not add transfer
coding engines, trailer scheduling, proxy routing, automatic retries,
automatic preference handling, cache policy, or forwarding behavior.

### Bounded X-Forwarded request metadata

`HttpClient::x_forwarded_for()`, `x_forwarded_host()`, and
`x_forwarded_proto()` validate and emit bounded compatibility request metadata
through shared protocol-owned `XForwardedFor`, `XForwardedHost`, and
`XForwardedProto` types. Repeated helper calls combine existing same-name
fields in wire order before a socket is opened. Server-side `Request` and
`HttpRequest` helpers parse the same representations, return `Ok(None)` when
absent, and preserve raw headers on parse errors.

`X-Forwarded-For` accepts ordered IP node values and `unknown`,
`X-Forwarded-Host` accepts ordered host authorities, and `X-Forwarded-Proto`
accepts ordered URI scheme tokens. Each field family is bounded to 64 KiB per
field value, 64 KiB for the combined raw field set including `", "` separator
overhead, 64 KiB for serialized output, and 256 members. Empty members,
malformed values, control-byte injection, and bound violations are rejected.

These helpers are compatibility metadata only. RTTP does not trust, rewrite,
or enforce forwarded identity, select a client address, change routing,
redirect, upgrade, or choose a trusted proxy set. Applications that use these
fields must choose and enforce their own trusted proxies.

### Bounded Connection metadata

`Response::connection()` parses retained HTTP/1 `Connection` fields into
`Connection` header metadata. It returns `Ok(None)` when the header is absent.
Present values combine case-insensitive fields in wire order and preserve
token spelling, including duplicates. `Connection::parse(value)` is available
when callers want to validate one raw field value directly.

On the server, `Request::connection()`, `HttpRequest::connection()`, and
`HttpResponse::connection()` parse the same bounded token list from already
retained HTTP/1 headers. Absent fields return `Ok(None)`.

Each field value is limited to 64 KiB. Parsing accepts at most 256 tokens and
rejects empty members, malformed tokens, parameters, oversized values, and too
many tokens. Parse errors do not reject the raw message: original headers
remain available. HTTP/2 continues to reject inbound `Connection` at decode
time. These helpers do not change keep-alive, hop-by-hop stripping,
upgrade/h2c, or HTTP/2 rejection.

### Bounded Upgrade metadata

`HttpClient::upgrade_protocols()` validates and replaces request `Upgrade`
metadata without changing request method, socket handoff, or `Connection`
handling. `Response::upgrade()` parses retained HTTP/1 `Upgrade` response
fields into `Upgrade` metadata.

On the server, `Request::upgrade()`, `HttpRequest::upgrade()`, and
`HttpResponse::upgrade()` parse retained HTTP/1 `Upgrade` fields into
`HttpUpgrade` metadata. `HttpResponse::with_upgrade()` validates and replaces
attached response `Upgrade` metadata. Absent fields return `Ok(None)`, and
present values combine fields in wire order while preserving protocol
spelling.

Each field value is limited to 64 KiB. Parsing accepts at most 32 protocols.
Each protocol must be an HTTP token, optionally followed by `/` and a token
protocol version. Empty members, malformed protocols, control bytes,
oversized values, and too many protocols return a parser error while raw
headers remain available.

These helpers expose HTTP/1 header metadata only. They do not add
`Connection: Upgrade`, select h2c, perform client `upgrade()` handoff, change
server `HttpHandoff::upgrade` socket handoff, or implement the upgraded
protocol.

### Bounded Transfer-Encoding framing metadata

`Response::transfer_encoding()` parses retained HTTP/1 `Transfer-Encoding`
fields into `TransferEncoding` metadata. It returns `Ok(None)` when the header
is absent. Present values combine case-insensitive fields in wire order and
must yield a sole `chunked` coding, matching existing HTTP/1 framing.
`TransferEncoding::parse(value)` is available when callers want to validate
one raw field value directly.

On the server, `Request::transfer_encoding()` and
`HttpRequest::transfer_encoding()` parse the same sole-`chunked` metadata from
already-validated HTTP/1 request headers. Absent fields return `Ok(None)`.

Each field value is limited to 64 KiB. Parsing accepts at most 256 tokens and
rejects empty members, malformed tokens, stacked or non-final `chunked`
codings, combined duplicate fields that are no longer sole `chunked`,
oversized values, and too many tokens. Parse errors do not reject the raw
message: original headers remain available. HTTP/2 continues to reject
`Transfer-Encoding` at decode time. These helpers do not change HTTP/1
framing decoders, `TE`, Content-Length, or HTTP/2 decode.

### Bounded preflight request metadata

`HttpClient::origin(value)` emits one validated `Origin` field, accepting
`null` or an `http`/`https` tuple origin without a path, query, fragment, or
userinfo. `HttpClient::access_control_request_method(value)` emits one
`Access-Control-Request-Method` field from a single HTTP method token.
`HttpClient::access_control_request_headers(field_names)` emits one
`Access-Control-Request-Headers` field from a bounded field-name list,
normalized to lowercase with duplicates rejected.
`HttpClient::access_control_request_private_network()` emits
`Access-Control-Request-Private-Network: true`.

These helpers reject invalid input before a socket is opened: origins with a
path, query, fragment, userinfo, or non-`http(s)` scheme; methods that are
`*`, comma-separated, or not HTTP tokens; and field names that are malformed,
duplicated, or excessive. Values are bounded to 64 KiB and the field-name list
to 256 entries. Callers that need values outside the helper validation can
retain raw-header control with `header(("Origin", "..."))` and the other
`header` forms.

These helpers only declare preflight request metadata. RTTP does not decide
whether a preflight is needed, read `Access-Control-Allow-*` response fields,
apply CORS policy, or apply Private Network Access policy.

### Bounded Save-Data request metadata

`HttpClient::save_data()` emits `Save-Data: on`. On the server,
`Request::save_data()` and `HttpRequest::save_data()` parse the same bounded
singleton `on` token, returning `Ok(None)` when the field is absent and a
parser error for malformed, oversized, duplicate, or control-byte values
while leaving the raw `Save-Data` field available.

These helpers only declare or parse request metadata. RTTP does not select a
representation, compress a body, advertise Client Hints, or apply browser
data-saver policy.

### Bounded Sec-GPC request metadata

`HttpClient::sec_gpc()` emits `Sec-GPC: 1`. On the server,
`Request::sec_gpc()` and `HttpRequest::sec_gpc()` parse the same bounded
singleton `1` signal through the shared protocol representation, returning
`Ok(None)` when the field is absent and a parser error for malformed,
oversized, duplicate, or control-byte values while leaving the raw `Sec-GPC`
field available.

These helpers only declare or parse request metadata. RTTP does not infer or
enforce consent, tracking, legal, or serving policy.

### Bounded Pragma metadata

`rttp-protocol` owns the shared `Pragma` primitive. Client helpers format
through that type, and server `Request` / `HttpRequest` plus client and server
response helpers parse with the same rules.

`HttpClient::pragma(value)` and `HttpClient::pragma_no_cache()` emit bounded
RFC 9111 `Pragma` request metadata, combining and replacing already-attached
same-name fields. `Response::pragma()` parses the same representation on the
client. On the server, `Request::pragma()` and `HttpRequest::pragma()` parse
received fields into `HttpPragma`, and `HttpResponse::with_pragma(value)`
declares validated response metadata that replaces attached same-name fields.
`HttpResponse::pragma()` parses attached response fields. Absent fields return
`Ok(None)`. Multiple fields are combined in wire order, directive names are
matched case-insensitively, duplicate names are rejected, each field value is
bounded to 64 KiB, combined field values are bounded to 64 KiB including
`", "` separator overhead, each directive value is bounded to 64 KiB, and the
combined directive count is bounded to 256. Malformed tokens or
quoted-strings, valued `no-cache` forms, empty members, and bound violations
return a parser error while raw headers remain available.

These helpers only declare or parse metadata. RTTP does not translate `Pragma`
into `Cache-Control`, store cache entries, or apply cache, freshness,
revalidation, intermediary, or HTTP/1.0 compatibility policy.

### Bounded Upgrade-Insecure-Requests request metadata

`HttpClient::upgrade_insecure_requests()` emits `Upgrade-Insecure-Requests: 1`.
On the server, `Request::upgrade_insecure_requests()` and
`HttpRequest::upgrade_insecure_requests()` parse the same bounded singleton
`1` token, returning `Ok(None)` when the field is absent and a parser error for
malformed, oversized, duplicate, or control-byte values while leaving the raw
`Upgrade-Insecure-Requests` field available.

These helpers only declare or parse request metadata. RTTP does not rewrite
`http://` URLs to `https://`, redirect requests, or enforce
Content-Security-Policy.

### Bounded HTTP/1.1 Vary behavior

`Response::vary()` parses one or more response `Vary` header fields into
`Vary` metadata. A parsed value is either the wildcard form, exposed by
`Vary::is_any()`, or a bounded list of normalized field names exposed by
`Vary::field_names()` and checked with `Vary::contains_field_name(name)`.
Field-name comparison is case-insensitive, and parsed field names are
deduplicated in lowercase form.

`Vary: *` is treated as a distinct wildcard result: it means the response
cannot be selected by comparing a bounded set of request header fields. The
wildcard form cannot be combined with named fields. Each `Vary` field value is
limited to 64 KiB, the parsed field-name list is limited to 256 entries, and
each named member must be a valid HTTP token. Empty members, malformed field
names, mixed wildcard/named values, oversized values, and too many field names
make `Response::vary()` return an error while leaving the original response
headers and body available through the ordinary response APIs.

The helper is metadata-only. RTTP does not store cache entries, match stored
responses, persist cache keys, replay requests, enforce shared-cache policy, or
issue automatic conditional requests based on `Vary`.

### Bounded No-Vary-Search metadata

`Response::no_vary_search()` parses one or more `No-Vary-Search` response
fields as bounded Structured Fields dictionary metadata. The typed value
exposes recognized `key-order`, `params`, and `except` members and keeps
extension dictionary members as metadata. Parse errors are returned by the
typed helper while raw headers remain available.

This helper does not create cache storage, match cache keys, normalize URLs,
replay requests, apply browser navigation behavior, or enforce shared-cache
policy.

### Bounded Permissions-Policy metadata

`Response::permissions_policy()` parses one or more `Permissions-Policy`
response fields through the shared protocol parser as bounded W3C Permissions
Policy dictionary metadata. The typed value exposes ordered feature directives
with their allowlists: the `*` token as the whole allowlist, the `self` token,
quoted serialized HTTP(S) origins, and inner lists including the empty `()`
form. Parse errors are returned by the typed helper while raw headers remain
available.

The helper is metadata-only. RTTP does not grant or deny browser permissions,
compare origins, resolve `self`, enable or disable APIs, or enforce origin
policy, and it does not send reports.

### Bounded Document-Policy metadata

`Response::document_policy()` parses one or more `Document-Policy` response
fields through the shared protocol parser as bounded WICG Document Policy
dictionary metadata. The typed value exposes ordered configuration-point
directives with their typed values: boolean (including a bare `?1`), integer,
decimal, or token. Directive names are opaque lowercase tokens or `*` and are
not looked up against a browser configuration-point list. A well-formed
`report-to` parameter is accepted as a token or a quoted string and retained
on the directive. Parse errors are returned by the typed helper while raw
headers remain available.

The helper is metadata-only. RTTP does not execute configuration points, block
document loads, compare required policies, echo `Sec-Required-Document-Policy`,
enable or disable browser features, or send reports.

`Response::document_policy_report_only()` parses
`Document-Policy-Report-Only` response fields through the same shared
protocol parser, formatter, directive model, and bounds while returning the
distinct `DocumentPolicyReportOnly` metadata type. It preserves raw response
headers on parse errors and does not enforce policy or deliver reports.

### Bounded Supports-Loading-Mode metadata

`Response::supports_loading_mode()` parses one or more `Supports-Loading-Mode`
response fields through the shared protocol parser as bounded Structured
Fields token-list metadata, combining fields in wire order. The typed value
exposes the ordered tokens with `tokens()`, membership checks with
`contains(token)`, and exact predicates for the defined `fenced-frame`,
`credentialed-prerender`, and `prerender-cross-origin-frames` tokens;
well-formed unknown tokens such as `uncredentialed-prerender` are retained.
Each field value is limited to 64 KiB, the combined raw bytes across fields
are limited to 64 KiB, and the token count is limited to 256 per header set.
Duplicate tokens are rejected with ASCII case-insensitive comparison. Parse
errors are returned by the typed helper while raw headers remain available.

The helper is metadata-only. RTTP does not prerender documents, admit fenced
frames, change navigation, or alter resource loading based on this field.

### Bounded Speculation-Rules metadata

`Response::speculation_rules()` parses one `Speculation-Rules` response field
through the shared protocol type as bounded opaque metadata. The field value is
limited to 64 KiB, duplicate fields fail closed, and control bytes that could
inject response fields are rejected. Typed `Debug` and typed parse errors do
not dump the field value. RTTP does not fetch, parse, validate, or execute
speculation rule resources.

### Bounded trailer behavior

Trailer support is explicit and bounded by protocol path. On the client,
`HttpClient::trailer` configures request trailer fields. Those fields are sent
for HTTP/1.1 only by `emit_streaming_chunked`; fixed-length HTTP/1.1 requests
and buffered `emit` requests do not have an HTTP/1.1 trailer section. With the
`http2` feature enabled, the same configured request trailers are sent as
HTTP/2 trailing HEADERS by both `emit_http2_prior_knowledge` and the explicit
`emit_http2_upgrade` h2c path after request DATA for buffered POST, PUT, and
PATCH requests. The bounded h2c client rejects request trailers for
`http2_extended_connect`, and the bodyless GET, HEAD, DELETE, OPTIONS, and
TRACE paths cannot carry request DATA before trailers.

Response trailers are read through the existing `Response` trailer accessors.
For HTTP/1.1, RTTP exposes only trailers that arrive in a chunked response
after the terminating zero-size chunk. For bounded h2c, peer response trailers
arrive as trailing HEADERS on the active stream and are exposed through the
same accessors. In both request and response directions, trailer names must be
ordinary field names: HTTP/2 pseudo-headers and fields reserved for connection
state, routing, authentication/cookies, transfer framing, or payload framing
are rejected instead of passed to application code.

HTTP/2 trailer support does not make the generic HTTP/1.1 `upgrade()` or
`CONNECT` handoff paths parse trailers. The h2c Upgrade client path is opt-in
through `emit_http2_upgrade` and replaces the initial HTTP/1.1 exchange with
the bounded HTTP/2 stream model after `101 Switching Protocols`; non-h2c
Upgrade handoffs remain caller-owned bytes.

### Bounded HTTP/2 CONTINUATION behavior

With the `http2` feature enabled, RTTP supports large HTTP/2 header blocks by
splitting outbound HEADERS or trailing HEADERS into an initial HEADERS frame
followed by CONTINUATION frames when the encoded HPACK block is larger than the
active peer `SETTINGS_MAX_FRAME_SIZE`. The same bounded decoder reassembles
inbound HEADERS plus CONTINUATION fragments before normal HPACK decoding and
header-list validation. This applies to request headers, response headers, and
trailing HEADERS on the bounded h2c paths.

Frame-size settings remain frame limits, not metadata-size limits. A legal
peer `SETTINGS_MAX_FRAME_SIZE` value from 16,384 through 16,777,215 bytes
controls how RTTP fragments outbound header blocks, DATA, and trailing
HEADERS. Inbound frames larger than the active local frame-size limit are
rejected even if their decoded metadata would otherwise be acceptable. Decoded
metadata is still bounded separately by `SETTINGS_MAX_HEADER_LIST_SIZE` and by
the HPACK dynamic table limits documented for the client and server paths.

CONTINUATION ordering is strict. Once a HEADERS frame starts a header block
without `END_HEADERS`, only CONTINUATION frames for that same stream may appear
until `END_HEADERS` closes the block. RTTP rejects orphan CONTINUATION frames,
CONTINUATION on stream 0, CONTINUATION on the wrong stream, interleaved DATA or
control frames before `END_HEADERS`, and EOF before a pending header block is
closed. Rejected header-block ordering failures happen before handler dispatch
or before a client response is returned.

The behavior is the same h2c stream model after both entry points:
`emit_http2_prior_knowledge` and explicit `emit_http2_upgrade` on the client,
and HTTP/2 prior-knowledge preface detection or valid `Upgrade: h2c` on the
server. Generic HTTP/1.1 `Upgrade`, `CONNECT`, proxy, TLS ALPN, server push,
extension callback, persistent session, and unbounded multiplexing paths do not
gain additional HTTP/2 header-block handling.

### Tested client protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 response parsing | `Content-Length`, chunked transfer coding, chunk extensions, informational responses, `Response::is_informational`, `is_redirection`, `is_error`, bodyless `204`/`304`, duplicate `Set-Cookie`, and framing ambiguity rejection | Not a complete RFC conformance suite |
| HTTP/1.1 request emission | Origin-form requests, absolute-form proxy requests, `CONNECT`, `HEAD`, fixed bodies, streaming chunked uploads, and explicit `Expect: 100-continue` metadata through the shared protocol type | Expect metadata does not gate body transmission; raw `header(("Expect", value))` remains an escape hatch; SOCKS handshakes are delegated to the `socks` crate |
| Fetch Metadata | Client `sec_fetch_site`, `sec_fetch_mode`, `sec_fetch_dest`, `sec_fetch_user`, and `sec_purpose` emit bounded `Sec-Fetch-*`/`Sec-Purpose` fields; server `Request` helpers parse typed received values while preserving raw headers on errors | No browser security policy, request blocking, origin validation, navigation policy, automatic header generation, prefetch execution, or cache behavior |
| Save-Data | Client `save_data` emits bounded `Save-Data: on` request metadata; server `Request::save_data()` and `HttpRequest::save_data()` parse typed received values while preserving raw headers on errors | No reduced-data serving, content adaptation, compression, Client Hints advertisement, retries, or browser data-saver policy |
| Accept-Charset | Client `accept_charset` and `accept_charset_with_q` format bounded `Accept-Charset` request metadata through the shared `rttp-protocol` type; server `Request::accept_charset()` and `HttpRequest::accept_charset()` parse received fields into `HttpRequestAcceptCharsets` | No content negotiation, charset transcoding, body decoding, MIME sniffing, or response selection |
| Sec-GPC | Client `sec_gpc` emits bounded `Sec-GPC: 1` request metadata; server `Request::sec_gpc()` and `HttpRequest::sec_gpc()` parse typed received values while preserving raw headers on errors | No consent inference, tracking-policy enforcement, legal policy, serving policy, retries, or browser state |
| Upgrade-Insecure-Requests | Client `upgrade_insecure_requests` emits bounded singleton `Upgrade-Insecure-Requests: 1` request metadata; server `Request::upgrade_insecure_requests()` and `HttpRequest::upgrade_insecure_requests()` parse typed received values while preserving raw headers on errors | No URL rewriting, redirecting, Content-Security-Policy enforcement, HSTS, or automatic scheme selection |
| Depth | Client `depth` emits bounded singleton WebDAV `Depth` request metadata through the shared protocol type, replacing an existing same-name field; server `Request::depth()` and `HttpRequest::depth()` parse typed received values while preserving raw headers on errors | No resource traversal, WebDAV method selection, method-policy enforcement, retry, or forwarding policy |
| Destination | Client `destination` emits bounded singleton WebDAV `Destination` request metadata through the shared protocol type, replacing an existing same-name field; server `Request::destination()` and `HttpRequest::destination()` parse typed received values while preserving raw headers on errors | No destination resolution, URI normalization, authorization, COPY/MOVE execution, or application resource policy |
| Timeout | Client `timeout` emits bounded ordered WebDAV `Timeout` request metadata through the shared protocol type, replacing an existing same-name field; server `Request::timeout()` and `HttpRequest::timeout()` parse typed received values while preserving raw headers on errors | No lock creation, lock refresh, application-timeout selection, retry, or forwarding policy |
| Idempotency-Key | Client `idempotency_key` emits bounded singleton opaque `Idempotency-Key` request metadata through the shared protocol type, replacing an existing same-name field; server `Request::idempotency_key()` and `HttpRequest::idempotency_key()` parse typed received values while preserving raw headers on errors, and the key is redacted from typed debug output | No retry, replay, key storage or comparison, deduplication store, or application idempotency policy |
| WebSocket handshake metadata | Client `sec_websocket_key` emits bounded singleton `Sec-WebSocket-Key` request metadata through the shared protocol type, replacing an existing same-name field; server `Request::sec_websocket_key()` and `HttpRequest::sec_websocket_key()` parse typed received values while preserving raw headers on errors; server responses can derive `Sec-WebSocket-Accept` with the RFC GUID plus SHA-1/base64 transform; client responses can parse and verify it against a validated key; key and accept material is redacted from typed debug output | No HTTP upgrade, random nonce generation, WebSocket frames, or handshake policy |
| Sec-WebSocket-Version | Client `sec_websocket_version` emits bounded `Sec-WebSocket-Version` request metadata through the shared protocol type; server `Request`/`HttpRequest` helpers parse received fields; `HttpResponse::with_sec_websocket_version`/`sec_websocket_version` and client `Response::sec_websocket_version` declare or parse rejection-response version lists while preserving raw headers on errors | No WebSocket handshake, `Connection: Upgrade` emission, `Sec-WebSocket-Accept` computation, version negotiation, protocol switch, or frames |
| Sec-WebSocket-Protocol | Client `sec_websocket_protocol` emits bounded `Sec-WebSocket-Protocol` offer metadata in preference order through the shared protocol type; server `Request`/`HttpRequest` helpers parse received offers; `HttpResponse::with_sec_websocket_protocol`/`sec_websocket_protocol` and client `Response::sec_websocket_protocol` declare or parse selection singletons while preserving raw headers on errors | No WebSocket handshake, `Connection: Upgrade` emission, automatic subprotocol choice, protocol switch, or frames |
| Pragma | Client `pragma`/`pragma_no_cache` and `Response::pragma` share the bounded protocol `Pragma` representation with server `Request::pragma`, `HttpRequest::pragma`, `HttpResponse::with_pragma`, and `HttpResponse::pragma` across client construction, server request access, server response construction, and client response access, combining fields in wire order and preserving raw headers on errors | No translation into `Cache-Control`, cache storage, freshness checks, revalidation, or cache/intermediary policy |
| W3C Trace Context | Client `traceparent`/`tracestate` validate and emit bounded W3C Trace Context request metadata through shared protocol types; server `Request`/`HttpRequest` helpers parse received fields, preserve raw headers on errors, preserve tracestate ordering, and redact propagation values from typed debug output | No trace-id creation, sampling decision, tracing backend, span model, or automatic propagation |
| W3C Baggage | Client `baggage` validates and emits bounded W3C Baggage request metadata through the shared protocol type; server `Request`/`HttpRequest` helpers parse received fields, preserve raw headers on errors, preserve member order, and redact member and property values from typed debug output | No application-data interpretation, request-context storage, tracing backend, span model, or automatic propagation |
| X-Forwarded compatibility metadata | Client `x_forwarded_for`, `x_forwarded_host`, and `x_forwarded_proto` emit bounded compatibility request metadata through shared protocol types; server `Request`/`HttpRequest` helpers parse ordered node, authority, and scheme values while preserving raw headers on errors | No forwarded identity trust, client address selection, routing rewrite, scheme rewrite, redirect, upgrade, enforcement, or trusted-proxy selection; applications must choose trusted proxies |
| Accept-Language | Client `accept_language` emits bounded `Accept-Language` request metadata through the protocol `AcceptLanguage` type; server `Request::accept_language()` and `HttpRequest::accept_language()` parse typed received values as `HttpAcceptLanguages` while preserving raw headers on errors | No locale matching, fallback selection, translation lookup, routing, or automatic response choice |
| Preflight request metadata | Client `origin`, `access_control_request_method`, `access_control_request_headers`, and `access_control_request_private_network` emit bounded `Origin`, `Access-Control-Request-Method`, `Access-Control-Request-Headers`, and `Access-Control-Request-Private-Network` request metadata and reject invalid input before connecting | No automatic preflight decision, `Access-Control-Allow-*` response parsing, CORS policy, or Private Network Access policy |
| Access-Control-Allow-Credentials | Client `Response::access_control_allow_credentials` and server `HttpAccessControlAllowCredentials`, `HttpResponse::with_access_control_allow_credentials`, and `HttpResponse::access_control_allow_credentials` parse or declare bounded singleton `Access-Control-Allow-Credentials` `true`-token metadata while preserving raw headers on parse failures | No CORS request evaluation, automatic credential attachment, or automatic credentials granting |
| Digest preferences | `want_content_digest`, `want_content_digest_with_q`, `want_repr_digest`, and `want_repr_digest_with_q` emit bounded `Want-Content-Digest` and `Want-Repr-Digest` request metadata; server `Request::want_content_digest()`, `HttpRequest::want_content_digest()`, `Request::want_repr_digest()`, and `HttpRequest::want_repr_digest()` parse received preference fields | No algorithm selection, digest computation, response body hash validation, retries, or signing |
| Accept-Encoding | Client `accept_encoding`, `accept_encoding_with_q`, and gzip/deflate/br/identity helpers format bounded `Accept-Encoding` request metadata through the shared `rttp-protocol` type; server `Request::accept_encoding()` and `HttpRequest::accept_encoding()` parse received fields into `HttpRequestAcceptEncodings` | No compression, decompression, content negotiation, retries, or transport changes |
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Byte ranges | `range`, `range_from`, `range_suffix`, `if_range_etag`, and `if_range_date` emit bounded HTTP/1.1 range request metadata; `Response::content_range`, `accept_ranges`, `is_partial_content`, and `is_range_not_satisfiable` expose `Content-Range`, `Accept-Ranges`, `206`, and `416` metadata while preserving raw headers | No Range request generation from `Accept-Ranges`, client-side `If-Range` evaluation, partial response engine, byte serving, content slicing, download resume, automatic retry/replay, cache storage, redirect handling, status-policy behavior, multipart range generation, or automatic cache validation policy |
| Conditional requests | `if_none_match`, `if_match`, `if_modified_since`, and `if_unmodified_since` emit bounded HTTP/1.1 validators; the date helpers validate and emit through the shared protocol `IfModifiedSince` and `IfUnmodifiedSince` types; `Response::is_not_modified`, `is_precondition_failed`, typed bounded `etag`, and `last_modified` expose `304`/`412` metadata while preserving raw headers | One ETag validator per helper call, `If-Range` is range-scoped, no cache storage, no automatic revalidation, and no cache-control engine |
| Informational responses and Early Hints | `Response::informational_responses` exposes skipped bounded HTTP/1.1 `1xx` heads, including `103 Early Hints`, with preserved raw headers; server `HttpResponse::early_hints`/`early_hints_with_headers` construct validated bodyless `103` metadata | `101 Switching Protocols` remains terminal for upgrade handoff; no automatic preload execution, cache policy, redirect/retry/replay, route generation, streaming early-write API, TLS/ALPN behavior, or status-policy behavior |
| Cache-Control, CDN-Cache-Control, Cache-Status, Date, Age, Expires, Retry-After, and Allow | `Response::cache_control` parses bounded response directives, numeric freshness fields, quoted field-name lists, and extension directives; `Response::cdn_cache_control` parses bounded `CDN-Cache-Control` directives and CDN extension metadata while preserving raw responses on parse errors; `Response::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw responses on parse errors; `Response::date` parses singleton HTTP-date metadata; `Response::age` parses bounded singleton `Age` metadata through the protocol `Age` type, rejecting duplicate fields, values larger than 64 KiB, and overflowing `u64` delta-seconds; `Response::expires` parses bounded HTTP-date metadata; `Response::retry_after` parses bounded delta-seconds or HTTP-date metadata; `Response::allow` parses bounded ordered method metadata | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, clock-skew correction, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, automatic conditional requests, automatic sleep, retry, replay, redirect, backoff, scheduler integration, fallback method selection, or status-code policy engine |
| Memento-Datetime | `Response::memento_datetime` parses bounded singleton `Memento-Datetime` IMF-fixdate metadata through the protocol `MementoDatetime` type while preserving raw headers on parse errors | No archival selection, `Accept-Datetime` negotiation, TimeGate behavior, retry, or transport changes |
| Content-Security-Policy-Report-Only | `Response::content_security_policy_report_only`, `ContentSecurityPolicyReportOnly`, `HttpContentSecurityPolicyReportOnly`, and `HttpResponse::with_content_security_policy_report_only` share bounded opaque CSP field validation while keeping the report-only header identity distinct, preserving repeated fields in wire order and raw headers on parse failures | No CSP enforcement, directive evaluation, report delivery, browser policy state, retry, redirect, cache behavior, or status-policy behavior |
| Content-Language | `Response::content_language` parses bounded response `Content-Language` fields into ordered language metadata while preserving raw headers | No automatic language negotiation, locale fallback, variant matching, cache policy, retry, replay, redirect, or status-policy behavior |
| Content-Location | `Response::content_location` and `ContentLocation::parse` parse bounded singleton response `Content-Location` metadata while preserving raw headers | No redirect behavior, cache variant selection, representation replacement, retry/replay, route generation, or status-policy behavior |
| Service-Worker-Allowed | `Response::service_worker_allowed` and `ServiceWorkerAllowed::parse` parse bounded singleton response `Service-Worker-Allowed` path metadata while preserving raw headers | No service-worker registration, scope evaluation, script-URL resolution, or application routing policy |
| Content-DPR | `Response::content_dpr` and `ContentDpr::parse` parse bounded singleton response `Content-DPR` decimal-ratio metadata while preserving raw headers | No image rescaling, request DPR emission, Client Hints policy, retry, or transport changes |
| Content-Type and Content-Encoding | `Response::content_type`/`ContentType::parse` parse bounded singleton `Content-Type` metadata, and `Response::content_encoding`/`ContentEncoding::parse` parse bounded ordered `Content-Encoding` codings while preserving raw headers on parse failures | No MIME sniffing, body decoding, charset transcoding, compression/decompression policy, negotiation, cache policy, redirects, retry/replay, or filesystem serving |
| Connection | `Response::connection`/`Connection::parse` parse bounded HTTP/1 `Connection` tokens, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to keep-alive, `auto_add_connection`, hop-by-hop stripping, or HTTP/2 rejection |
| Transfer-Encoding | `Response::transfer_encoding`/`TransferEncoding::parse` parse bounded HTTP/1 `Transfer-Encoding` fields that must be sole `chunked`, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to HTTP/1 framing decoders, `TE`, Content-Length, chunked body decoding policy, or HTTP/2 decode rejection |
| Content-Disposition | Client `Response::content_disposition` and protocol-backed server `HttpContentDisposition`, `HttpResponse::with_content_disposition`, `with_attachment_filename`, and `content_disposition` parse or declare bounded singleton response `Content-Disposition` metadata, preserve raw headers on parse failures, and preserve parsed `filename` plus `filename*` parameter values as metadata | No automatic download, filesystem path handling, MIME sniffing, cache behavior, redirect behavior, retry/replay, negotiation, or status-policy behavior |
| WWW-Authenticate | Client `Response::www_authenticate` and server `HttpWwwAuthenticate`, `HttpResponse::with_www_authenticate`, and `HttpResponse::www_authenticate` parse or declare bounded response authentication challenges while preserving raw headers on parse failures | No credential storage, authentication policy, retry, automatic `Authorization` generation, Basic/Bearer implementation, redirect behavior, or status-policy behavior |
| Authorization and Proxy-Authorization | Protocol `Authorization`/`ProxyAuthorization`, client `HttpClient::authorization`, and server `Request::authorization`/`proxy_authorization` validate bounded request authorization metadata, reject duplicate parsed inbound fields, and redact credentials from typed debug output | No credential storage, authentication policy, challenge processing, retry, Basic/Bearer implementation, redirect policy changes, or automatic credential forwarding |
| Proxy-Authenticate | Client `Response::proxy_authenticate` and protocol `ProxyAuthenticate::parse`/`parse_values` parse bounded proxy authentication challenges across one or more response fields while preserving raw headers on parse failures | No credential storage, proxy authentication policy, retry, automatic `Proxy-Authorization` generation, Basic/Bearer implementation, redirect behavior, or status-policy behavior |
| Proxy-Status | Client `Response::proxy_status` and server `HttpProxyStatus`, `HttpResponse::with_proxy_status`, and `HttpResponse::proxy_status` parse or declare bounded RFC 9209 Token/String proxy identifiers with opaque parameters while preserving raw headers on parse failures | No proxy health checks, retries, trailer promotion, or origin-generation policy |
| Server-Timing | Client `Response::server_timing` and server `HttpServerTiming`, `HttpResponse::with_server_timing`, and `HttpResponse::server_timing` parse or declare bounded response timing metadata while preserving raw headers on parse failures | No metric collection, measurement, telemetry export, metrics backend integration, retry, redirect behavior, or status-policy behavior |
| Alt-Used | Client `Response::alt_used` and server `HttpAltUsed`, `HttpResponse::with_alt_used`, and `HttpResponse::alt_used` parse or declare bounded singleton response authority metadata while preserving raw headers on parse failures and replacing raw response duplicates on typed declaration | No alternative service selection, origin rewriting, socket migration, retry, or connection-policy behavior |
| Origin-Trial | Client `Response::origin_trials` and server `HttpOriginTrials`, `HttpResponse::with_origin_trials`, and `HttpResponse::origin_trials` parse or declare bounded opaque `Origin-Trial` tokens in wire order, preserve duplicates, redact token material from debug output, and replace raw response fields on typed declaration | No token signature validation, expiration checks, origin applicability, feature activation, or browser trial policy |
| Speculation-Rules | Client `Response::speculation_rules` and server `HttpSpeculationRules`, `HttpResponse::with_speculation_rules`, and `HttpResponse::speculation_rules` preserve one bounded opaque `Speculation-Rules` response field, reject duplicates and response-field injection bytes, redact debug output, and replace raw response fields on typed declaration | No speculation rule fetching, parsing, validation, prefetching, prerendering, execution, navigation changes, cache behavior, retry, or redirect behavior |
| Warning | Client `Response::warning` parses bounded RFC 7234 `Warning` warning-value lists while preserving raw headers on parse failures | No cache storage, freshness calculation, stale-response handling, warn-code policy, retry, redirect behavior, or response-acceptance changes |
| NEL | Client `Response::nel` and server `HttpNel`, `HttpResponse::with_nel`, and `HttpResponse::nel` parse or declare bounded W3C Network Error Logging policy JSON while preserving raw headers on parse failures | No network error report sending, policy persistence, Reporting endpoint group configuration, retry, redirect behavior, or status-policy behavior |
| Reporting-Endpoints | Client `Response::reporting_endpoints` and server `HttpReportingEndpoints`, `HttpResponse::with_reporting_endpoints`, and `HttpResponse::reporting_endpoints` parse or declare bounded endpoint-name to quoted-URL dictionaries through the shared protocol type while preserving raw headers on parse failures | No report scheduling, sending, persistence, retry, routing, or endpoint policy behavior |
| Cross-Origin-Opener-Policy-Report-Only | Client `Response::cross_origin_opener_policy_report_only` and server `HttpCrossOriginOpenerPolicyReportOnly`, `HttpResponse::with_cross_origin_opener_policy_report_only`, and `HttpResponse::cross_origin_opener_policy_report_only` parse or declare bounded singleton COOP Report-Only metadata, reuse the canonical COOP directives, retain reporting parameters including `report-to`, and preserve raw headers on parse failures | No browsing-context isolation, report scheduling, sending, persistence, retry, routing, or `Reporting-Endpoints` validation |
| Keep-Alive | Client `Response::keep_alive` and server `HttpKeepAlive`, `HttpResponse::with_keep_alive`, and `HttpResponse::keep_alive` parse or declare bounded RFC 2068 `Keep-Alive` `timeout` and `max` parameters as checked unsigned integers while preserving raw headers on parse failures | No connection lifetime management, connection pooling, keep-alive timers, or HTTP/2 behavior changes |
| Vary | `Response::vary` parses bounded response `Vary` fields into wildcard or normalized case-insensitive field-name metadata | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| No-Vary-Search | `Response::no_vary_search` parses bounded Structured Fields response metadata for query-parameter variance declarations | No cache storage, cache-key matching, URL normalization, navigation behavior, request replay, or shared-cache policy enforcement |
| Permissions-Policy | `Response::permissions_policy` parses bounded W3C Permissions Policy dictionary metadata through the shared protocol type, combining fields in wire order and preserving raw headers on parse failures | No browser permission grants or denials, origin comparison, `self` resolution, API enablement, origin-policy enforcement, or report sending |
| Document-Policy | `Response::document_policy` parses bounded WICG Document Policy dictionary metadata through the shared protocol type, combining fields in wire order, retaining `*` and `report-to`, and preserving raw headers on parse failures | No configuration-point execution, document-load blocking, required-policy comparison, `Sec-Required-Document-Policy` echoing, feature enablement, or report sending |
| Document-Policy-Report-Only | `Response::document_policy_report_only` parses bounded WICG Document Policy Report-Only dictionary metadata through the same shared protocol parser and formatter, retaining report-only type identity, `*`, and `report-to`, and preserving raw headers on parse failures | No policy enforcement, document-load blocking, required-policy comparison, `Sec-Required-Document-Policy` echoing, feature enablement, report delivery, scheduling, retry, or endpoint validation |
| Supports-Loading-Mode | `Response::supports_loading_mode` parses bounded Structured Fields token-list response metadata through the shared protocol type, combining fields in wire order, retaining unknown tokens, and preserving raw headers on parse failures | No prerendering, fenced-frame admission, navigation changes, redirects, retries, or resource-loading behavior |
| Trailers | Chunked response trailers are exposed for blocking and async APIs; streaming chunked uploads can send declared request trailers | Application metadata trailers such as `X-Trace` are allowed; pseudo-header, connection-specific, routing, authentication/cookie, and framing trailer fields are rejected |
| Bounded h2c client | With `http2`, direct `socket2` h2c sends GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, buffered POST, PUT, or PATCH requests, and opt-in RFC 8441 extended CONNECT request HEADERS via `http2_extended_connect`, opens at most one request stream, supports prior-knowledge with `emit_http2_prior_knowledge`, supports explicit HTTP/1.1 `Upgrade: h2c` negotiation with `emit_http2_upgrade`, advertises `SETTINGS_ENABLE_PUSH = 0`, advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` only for the explicit extended CONNECT path, validates received `SETTINGS_ENABLE_PUSH` values as only `0` or `1`, honors initial peer `SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the peer allows zero streams, honors peer-advertised `SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits, accepts only legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, rejects oversized inbound frames when a configured local frame-size limit is exceeded, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, strips HTTP/1.x connection-specific request fields before emission, rejects connection-specific peer response fields, suppresses HEAD response bodies, treats `RST_STREAM` on the active stream as a bounded reset/cancellation signal, acknowledges inbound PING without ACK on stream 0 and exactly 8 octets with matching opaque data, ignores inbound PING ACK, rejects malformed PING frames, DATA bodies, trailers, HPACK static Huffman strings, bounded large header blocks, padded incoming frames, `GOAWAY` shutdown boundaries, PRIORITY metadata validation without scheduling, HTTP/2-allowed unknown/extension frame ignoring inside this bounded path, reserved stream-id high-bit normalization, and conservative DATA flow control | Ordinary `CONNECT`, header-configured `:protocol` metadata, non-h2c HTTP/1.1 `Upgrade` handoff requests, and proxies are rejected deterministically, and `PUSH_PROMISE`/server push is rejected instead of managed; bounded direct h2c only, with no keepalive timers, no automatic client/server initiated PING policy, no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, proxy tunneling to h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry/replay, server push, full session manager, full stream state machine, full multiplex scheduler, unbounded multiplex scheduling, general multiplexing, priority scheduling, request bodies or trailers for extended CONNECT, or request bodies for GET, HEAD, DELETE, OPTIONS, or TRACE |

With the `http2` feature enabled, `emit_http2_prior_knowledge` sends a bounded
prior-knowledge h2c request over a direct socket2 TCP connection. It opens at
most one request stream and honors the peer's initial
`SETTINGS_MAX_CONCURRENT_STREAMS`: a value of zero rejects the request before
HEADERS are sent. It also honors the peer's advertised
`SETTINGS_MAX_HEADER_LIST_SIZE` for request metadata: encoded request HEADERS
and trailing HEADERS must stay within that peer boundary before emission, while
peers that do not advertise the setting keep the bounded direct-client default.
It supports GET, HEAD, bodyless DELETE, OPTIONS, or TRACE,
buffered POST, PUT, or PATCH requests, and the explicit
`HttpClient::http2_extended_connect(protocol)` mode for bounded RFC 8441
extended CONNECT request HEADERS. Non-empty buffered request bodies are sent as
DATA frames for the write methods. GET, HEAD, DELETE, OPTIONS, TRACE, and
extended CONNECT requests with bodies are rejected; HEAD, bodyless DELETE,
OPTIONS, TRACE, and extended CONNECT requests do not send request DATA frames,
and any HEAD response DATA frames are consumed without being exposed as a
response body. The client advertises `SETTINGS_ENABLE_PUSH = 0` in its initial
SETTINGS frame so peers see server push disabled, and it advertises
`SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` only when
`http2_extended_connect(protocol)` is used. It validates received
`SETTINGS_ENABLE_PUSH` values as only `0` or `1`; any other value rejects the
bounded h2c handshake.
`emit_http2_upgrade` is the explicit HTTP/1.1 h2c Upgrade variant of the same
bounded single-request client path. It is opt-in and separate from
`emit_http2_prior_knowledge`: the client first sends an HTTP/1.1 request with
`Connection: Upgrade, HTTP2-Settings`, `Upgrade: h2c`, and the local SETTINGS
payload in `HTTP2-Settings`, requires a `101 Switching Protocols` response
that negotiates `h2c`, then sends the HTTP/2 connection preface and uses the
same bounded single-stream h2c request/response flow on the upgraded socket.
The Upgrade variant supports the same request methods and body limits as the
prior-knowledge h2c path, rejects proxies before opening a socket, rewrites
any preconfigured HTTP/1.x upgrade/framing fields into the required h2c
upgrade fields, and fails deterministically for invalid h2c upgrade responses.
Ordinary `upgrade()` continues to return the socket to the caller for
WebSocket-style protocols, and non-h2c HTTP/1.1 Upgrade handoff remains
outside the bounded h2c client path.
The client validates `SETTINGS_MAX_FRAME_SIZE` boundaries on both sides of the
bounded h2c handshake. A configured local
`http2_max_frame_size` is advertised only when set, must be in the legal
HTTP/2 range of 16,384 through 16,777,215 bytes, and is used to reject inbound
frame payloads larger than that active local limit. Peer-advertised
`SETTINGS_MAX_FRAME_SIZE` values outside that same legal range reject the
handshake. Legal peer values become the outbound frame boundary, so request
HEADERS, DATA, and trailing HEADERS are split into frames no larger than the
active peer limit while the client remains a single-stream prior-knowledge
path. Before encoding request HEADERS, this bounded h2c client path
strips HTTP/1.x connection-specific fields: `Connection`, `Keep-Alive`,
`Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, `TE`, `Trailer`, `Host`,
and any field named by a `Connection` token. Peer response HEADERS are rejected
when they contain `Connection`, `Keep-Alive`, `Proxy-Connection`, `TE`,
`Transfer-Encoding`, or `Upgrade`. Application request trailers such as
`X-Trace`, `X-Upload-Status`, or `X-Upload-Checksum` are valid in this bounded
h2c path and are encoded as trailing HEADERS after request DATA. Configured
request trailers are rejected before emission when their field name is invalid
or reserved for connection/framing/routing behavior: `Connection`,
`Keep-Alive`, `Proxy-Connection`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, `Content-Length`, `Host`, `Proxy-Authenticate`, or
`Proxy-Authorization`. Peer response trailers use the existing
forbidden-trailer validation for invalid pseudo-header-like names,
connection-specific, routing, authentication/cookie, and framing fields such
as `Authorization`, `Connection`, `Content-Length`, `Cookie`, `Host`,
`Keep-Alive`, `Proxy-Authenticate`, `Proxy-Authorization`,
`Proxy-Connection`, `Set-Cookie`, `TE`, `Trailer`, `Transfer-Encoding`,
`Upgrade`, and `WWW-Authenticate`. The client
supports HPACK static Huffman
strings and bounded large header blocks via CONTINUATION frames. It uses HPACK
dynamic entries for repeated request header and trailer fields within the
peer's advertised `SETTINGS_HEADER_TABLE_SIZE`. The default peer limit is
4,096 bytes when the peer does not advertise the setting; a peer-advertised
zero disables request dynamic indexing, so request HEADERS and trailers remain
literal encoded. Peer values above 4,096 bytes are valid, but RTTP caps request
dynamic indexing at its 4,096-byte bounded encoder size. Response decoding is
bounded by the local advertised
`SETTINGS_HEADER_TABLE_SIZE`: the client uses the default 4,096-byte decoder
limit unless `ConfigBuilder::http2_header_table_size` configures and advertises
another `u32`-sized value. Incoming HPACK dynamic table size updates from
response HEADERS or trailers may shrink that decoder table, including to zero;
updates above the local advertised limit are rejected. Dynamic table size
updates are HPACK compression state only and do not change
`SETTINGS_MAX_HEADER_LIST_SIZE`, trailer validation, body framing, or the
single-stream h2c policy. Valid response PRIORITY frames and HEADERS
priority fields are validated and ignored as metadata; malformed priority
metadata is rejected, and no priority scheduling is performed. Inbound PING
without ACK is acknowledged only when it arrives on stream 0 with exactly
8 octets of opaque data; the PING ACK carries that same opaque data. Inbound
PING ACK is ignored for this bounded path. PING with a non-zero stream id or
payload length other than 8 is malformed and rejected. RTTP does not add
keepalive timers, automatic client- or server-initiated PING policy, replay
behavior, a full session manager, or a full multiplex scheduler around this
acknowledgement path. Unknown frame types, including extension frames, are
ignored only after
the prior-knowledge h2c handshake in this bounded direct-client path where
HTTP/2 permits that behavior; RTTP does not expose extension callbacks or
perform full extension negotiation. Reserved stream identifier high bits are
masked when frames are parsed or written, which normalizes wire framing but
does not add broader multiplex scheduling or persistent session management.
Server push is outside this bounded client path even when a peer advertises
`SETTINGS_ENABLE_PUSH = 1`: incoming `PUSH_PROMISE` frames are rejected
deterministically instead of creating or tracking push state. HTTP/1.1
`CONNECT` tunnel handoff remains a separate
client path;
prior-knowledge h2c `GOAWAY` is treated as a bounded shutdown signal: a
response already completed before `GOAWAY` remains usable, an active stream
continues only when the peer's `last-stream-id` includes it, and a lower
boundary rejects the response deterministically. A `GOAWAY` received before
stream 1 is opened is treated as request refusal and no request HEADERS are
sent. RTTP returns that refusal to the caller instead of retrying on a new
connection; callers that know a request is safe or idempotent must choose any
retry policy themselves. This protocol shutdown boundary is distinct from a
transport-level disconnect, read timeout, write timeout, or TCP reset, which
is reported through the normal socket/error path without an HTTP/2
`last-stream-id` boundary. `RST_STREAM` is likewise bounded to this
prior-knowledge h2c client path: a reset for the active stream is reported as
response cancellation, while malformed reset frames are rejected
deterministically. RTTP does not expose a public cancellation callback API or
retry the request automatically. Ordinary `CONNECT`, header-configured RFC
8441 `:protocol` metadata, HTTP/1.1 `Upgrade` handoff requests, and proxy
tunneling are rejected before a client socket is opened. The explicit
`http2_extended_connect(protocol)` mode emits `:method CONNECT` with
`:protocol`, `:scheme`, `:authority`, and `:path`, then returns the peer's
HTTP/2 response through the normal `Response` API; it does not hand an upgraded
socket to the caller and does not send request bodies or request trailers.
HTTP/1.1 `CONNECT` tunnel handoff and `Upgrade` remain separate client handoff
paths; this h2c path does not provide full WebSocket-over-h2, proxy h2, TLS
ALPN, tunnel handoff, persistent multiplex sessions, general tunnel
scheduling, or full RFC 8441 support. Extension callback APIs, full extension
negotiation, external h2 integration, connection pooling, automatic retry,
server push, full stream state machines, and full HTTP/2 features such as
unbounded multiplex scheduling, general multiplexing, and priority scheduling
are not part of that bounded prior-knowledge client path; RTTP also does not
provide a dynamic policy API for changing h2c frame-size or metadata limits at
runtime.

```rust,no_run
# #[cfg(feature = "http2")]
# fn example() -> Result<(), Box<dyn std::error::Error>> {
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/chat")
  .http2_extended_connect("websocket")
  .emit_http2_prior_knowledge()?;
# Ok(())
# }
```

```rust,no_run
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Server

The `rttp` crate exposes `rttp::Http::server`, which creates a blocking
`HttpServer` listener.

```toml
[dependencies]
rttp = "0.2"
```

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
      .header("Trailer", "X-Trace")
      .trailer("X-Trace", "abc")
  })
}
```

Use `HttpServer::bind` directly when you already want the server type,
`HttpServer::local_addr` to read the bound address, `accept_one` for one
connection, and `serve_requests` for a fixed number of sequential connections.
Use `with_read_timeout` and `with_write_timeout` to apply socket-level
timeouts to each accepted connection; pass `None` to leave the corresponding
socket timeout unset. Add `Transfer-Encoding: chunked` to an `HttpResponse` to
write the complete response body with HTTP/1.x chunked transfer framing instead
of an automatic `Content-Length`; response trailers added with
`HttpResponse::trailer` are written after the terminating zero-size chunk. Add a
`Trailer` response header when advertising which trailer fields will follow.
The listener path uses `socket2`.

### Bounded HTTP/1.1 byte ranges

The server provides range parsing and response constructors for applications
that choose to support partial content. It does not automatically serve files
or decide static-file policy. Application code should inspect `Range`, choose
the representation and entity length, and pass the header to
`HttpByteRange::parse(range_header, entity_length)`.

Supported request forms are a single `bytes=start-end`, `bytes=start-`, or
`bytes=-suffix` range. Open-ended ranges are clipped to the representation
length, suffix ranges require a nonzero suffix, unsupported units return
`UnsupportedUnit`, comma-separated ranges return `MultipleRanges`, malformed
or inverted ranges return `InvalidRange`, and ranges beyond the entity return
`UnsatisfiedRange`.

For a satisfiable range, `HttpResponse::partial_content(body, range)` returns
`206 Partial Content`, adds `Content-Range: bytes start-end/length`, and sends
only the selected body bytes. For an unsatisfied range,
`HttpResponse::range_not_satisfiable(entity_length)` returns
`416 Range Not Satisfiable` with `Content-Range: bytes */length` and an empty
body.

For conditional range requests, `Request::evaluate_if_range(&metadata,
entity_length)` composes caller-provided `HttpConditionalMetadata` with the
existing single-range parser. Matching strong ETags or exact HTTP-date
`Last-Modified` validators return `PartialContent(HttpByteRange)`;
non-matching, weak, invalid, or metadata-missing validators return
`FullResponse`; guarded unsatisfied ranges return `RangeNotSatisfiable`.
Application code still chooses the final `200`, `206`, or `416` response.

Multipart ranges are intentionally not generated: RTTP does not serialize
`multipart/byteranges` or pick a multipart response for multiple requested
ranges. Filesystem path normalization, MIME detection, ETag or Last-Modified
generation, authorization, directory indexes, dotfile visibility, cache
storage, automatic cache validation, and any automatic retry policy remain
caller-owned policy before choosing `200`, `206`, or `416`.

`HttpResponse::with_accept_ranges(units)` declares supported range units with
one bounded comma-separated `Accept-Ranges` response header, while
`HttpResponse::with_accept_ranges_none()` declares the exclusive
`Accept-Ranges: none` sentinel. `HttpResponse::accept_ranges()` parses attached
fields into `HttpAcceptRanges`, the shared protocol parser used by both the
client and server facades, bounded to 64 KiB per field and 256 range units.
Malformed or empty values, duplicate units across parsed fields, combining
`none` with any unit, and passing `none` through the unit declaration helper
are rejected. Manual raw `Accept-Ranges` headers remain preserved until a
typed declaration helper replaces them or the typed parser is requested.

The server `Accept-Ranges` helpers are response metadata helpers that compose
with `HttpResponse::cache_control()`, `HttpResponse::vary()`,
`HttpResponse::allow()`, `HttpResponse::content_language()`, and the other raw
header APIs. They do not parse request `Range` fields, generate `Range`
requests, create a partial response engine, serve bytes, slice content, resume
downloads, choose redirect or status-policy behavior, implement cache policy,
retry or replay requests, or automatically select `200`, `206`, or `416`.

### Bounded HTTP/1.1 conditional requests

The server exposes conditional request primitives for applications that already
know the selected representation. Use `HttpConditionalMetadata` with
`HttpEntityTag::strong("tag")` or `HttpEntityTag::weak("tag")` and optional
`last_modified(SystemTime)`, then call
`Request::evaluate_conditional(&metadata)` or
`evaluate_conditional_request(&request, &metadata)`.

The evaluator returns `Proceed`, `NotModified`, or `PreconditionFailed`.
`If-Match` uses strong ETag comparison and takes precedence over
`If-Unmodified-Since`; `If-None-Match` uses weak ETag comparison and takes
precedence over `If-Modified-Since`. Matching `If-None-Match` validators return
`304 Not Modified` behavior for `GET` and `HEAD`, and `412 Precondition Failed`
behavior for other methods. HTTP-date validators are used only when they parse
as HTTP-date values, and `Last-Modified` comparisons are made at second
precision.

Handlers can also observe the raw validators without evaluation:
`Request::if_modified_since()`, `HttpRequest::if_modified_since()`, and the
matching `if_unmodified_since()` accessors parse one HTTP-date instant through
the shared protocol `HttpIfModifiedSince` and `HttpIfUnmodifiedSince` types.
Absent fields return `Ok(None)`; malformed, oversized, duplicate, or
control-byte values return a parse error while the raw field stays available
through `header()`. These accessors only parse metadata and do not change
evaluator precedence or comparison behavior.

Use `HttpResponse::not_modified(&metadata)` to serialize a bodyless `304` with
available `ETag` and `Last-Modified` validators. Use
`HttpResponse::with_etag(HttpEntityTag::strong("tag"))` or
`HttpResponse::with_etag(HttpEntityTag::weak("tag"))` to declare one bounded
response `ETag`; `HttpResponse::etag()` parses attached singleton `ETag`
metadata using the same protocol entity-tag authority and leaves raw headers
intact when typed parsing fails. Use
`HttpResponse::precondition_failed()` for `412`; application code can still add
its own headers or body when desired. A `Proceed` outcome means the handler
should send its normal representation response.

The helper scope is intentionally bounded. RTTP does not choose ETags, read or
serve files, implement static-file serving policy, store cached responses,
automatically revalidate stale entries, or provide a full cache-control engine.
Those remain application policy around the validator helpers.

### Bounded HTTP/1.1 informational responses and Early Hints

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

### Bounded HTTP/1.1 Link response metadata

`HttpResponse::links()` parses final-response `Link` fields into ordered
`HttpLinkValues` and `HttpLinkValue` metadata. URI/reference targets and both
standard and unknown parameters are retained in order. The parser applies 64
KiB per-field and parameter-value limits, plus limits of 256 link-values and
256 parameters per value; parsing errors leave raw response headers intact.

Link metadata uses the same bounded model as Early Hints, but does not execute
preloads or enable fetch scheduling, redirects, cache policy, or route
generation.

### Bounded HTTP/1.1 Cache-Control behavior

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

CDN cache metadata remains metadata-only. The server does not create or manage
a CDN cache, compute freshness, evaluate surrogate keys, revalidate
automatically, enforce shared-cache policy, retry, replay, redirect, or choose
status behavior from `CDN-Cache-Control`.

Server `Age` and `Expires` helpers expose adjacent response metadata without
adding cache policy. `HttpResponse::with_age(delta_seconds)` adds an `Age`
header from a non-negative `u64` delta-seconds value, and
`HttpResponse::age()` parses an attached single `Age` header back into `u64`.
The accepted bound is exactly the `u64` delta-seconds range: `0` through
`u64::MAX`. Empty, signed, fractional, non-numeric, comma-list, duplicated, and
overflowing values return `HttpAgeParseError`.

`HttpResponse::with_expires(time)` serializes an HTTP-date `Expires` header from
`SystemTime`, and `HttpResponse::expires()` parses an attached single `Expires`
header as an HTTP-date. Valid HTTP-date values parse to `SystemTime`; malformed,
duplicated, or non-date values return `HttpExpiresParseError`.

`HttpResponse::with_retry_after_delta(delta_seconds)` serializes a
delta-seconds `Retry-After` header from a non-negative `u64`, while
`HttpResponse::with_retry_after_date(time)` serializes an HTTP-date
`Retry-After` header from `SystemTime`. `HttpResponse::retry_after()` parses an
attached single `Retry-After` header into `HttpRetryAfter::DeltaSeconds` or
`HttpRetryAfter::HttpDate`. Empty, signed, fractional, non-numeric non-date,
comma-list, duplicated, overflowing, oversized, or malformed HTTP-date values
return `HttpRetryAfterParseError`.

Malformed typed helper reads return validation errors, while raw
`HttpResponse::header("Age", ...)` and `HttpResponse::header("Expires", ...)`
and `HttpResponse::header("Retry-After", ...)` values remain preserved exactly
as response headers. These helpers do not calculate freshness, validate cache
state against wall-clock time, store responses, match stored responses,
revalidate responses, enforce shared-cache policy, attach behavior to status
codes, sleep, retry, replay requests, apply backoff, integrate with a scheduler,
or issue automatic conditional requests.

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

### Bounded HTTP/1.1 Allow behavior

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

### Bounded HTTP/1.1 Content-Language behavior

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

### Bounded Accept-Charset request metadata

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

### Bounded Pragma metadata

Server-side `Pragma` helpers expose request and response metadata through the
shared `rttp-protocol` primitive. `Request::pragma()` and `HttpRequest::pragma()`
parse received `Pragma` fields in wire order into `HttpPragma`, and
`HttpResponse::with_pragma(value)` declares validated response metadata that
replaces attached same-name fields. `HttpResponse::pragma()` parses attached
response fields. Absent fields return `Ok(None)`. HTTP/1.1 and HTTP/2 share
the same `Request` helpers. The shared protocol type is the authority for
directive-token, optional-value, duplicate, member-count, and size validation.

Parsing is bounded and validation-oriented. Each `Pragma` field value is
limited to 64 KiB, combined field values are limited to 64 KiB including
`", "` separator overhead, each directive value is limited to 64 KiB, and the
combined directive count is limited to 256. Empty members, malformed tokens or
quoted-strings, valued `no-cache` forms, duplicate names, and bound violations
return `HttpPragmaParseError` from the helper. Raw `Request::header("Pragma",
...)` values remain preserved exactly as ordinary headers; helper parse errors
do not remove existing headers.

These helpers declare and parse metadata only. They do not translate `Pragma`
into `Cache-Control`, store cache entries, or apply cache, freshness,
revalidation, intermediary, or HTTP/1.0 compatibility policy.

### Bounded Accept-Encoding request metadata

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

### Bounded HTTP/1.1 Content-Location behavior

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

### Bounded HTTP/1.1 Service-Worker-Allowed behavior

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

These helpers interoperate with adjacent response metadata helpers by
preserving raw headers and parsing only when requested. They are
metadata-only: RTTP does not register service workers, evaluate
service-worker scope, resolve the value against a script URL, or apply
application routing policy from `Service-Worker-Allowed`.

### Bounded HTTP/1.1 Content-DPR behavior

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

### Bounded HTTP/1.1 Content-Disposition behavior

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

### Bounded HTTP/1.1 representation metadata behavior

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

### Bounded HTTP/1.1 Vary behavior

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

### Bounded No-Vary-Search metadata

Server-side `No-Vary-Search` helpers expose response declaration metadata
without implementing a cache. `HttpResponse::with_no_vary_search(value)`
parses and normalizes a declaration before replacing any existing
`No-Vary-Search` response fields, while `HttpResponse::no_vary_search()`
parses attached raw fields into `HttpNoVarySearch` metadata.

The helpers expose recognized `key-order`, `params`, and `except` members and
keep extension dictionary members as metadata. They do not create cache
storage, match cache keys, normalize URLs, replay requests, apply browser
navigation behavior, or enforce shared-cache policy.

### Bounded Permissions-Policy metadata

Server-side `Permissions-Policy` helpers expose response declaration metadata
without enforcing browser permissions or origin policy.
`HttpResponse::with_permissions_policy(value)` parses a W3C Permissions Policy
Structured Fields dictionary through the shared protocol parser and replaces
any existing `Permissions-Policy` response fields with one canonical value,
while `HttpResponse::permissions_policy()` parses attached raw fields into
`HttpPermissionsPolicy` metadata. The typed value exposes ordered feature
directives with their allowlists: the `*` token as the whole allowlist, the
`self` token, quoted serialized HTTP(S) origins, and inner lists including the
empty `()` form.

The helpers are bounded and metadata-only. Field values are limited to 64 KiB,
directives to 256 per header set, and allowlist members to 256 per directive.
They do not grant or deny browser permissions, compare origins, resolve
`self`, enable or disable APIs, enforce origin policy, or send reports.

### Bounded Document-Policy metadata

Server-side `Document-Policy` helpers expose response declaration metadata
without enforcing document policy in the HTTP layer.
`HttpResponse::with_document_policy(value)` parses a WICG Document Policy
Structured Fields dictionary through the shared protocol parser and replaces
any existing `Document-Policy` response fields with one canonical value, while
`HttpResponse::document_policy()` parses attached raw fields into
`HttpDocumentPolicy` metadata. The typed value exposes ordered
configuration-point directives with their typed values: boolean (including a
bare `?1`), integer, decimal, or token. Directive names are opaque lowercase
tokens or `*` and are not looked up against a browser configuration-point
list. A well-formed `report-to` parameter is accepted as a token or a quoted
string and retained on the directive.

The helpers are bounded and metadata-only. Field values are limited to 64 KiB,
the cumulative raw bytes across all supplied fields to 64 KiB, and directives
to 256 per header set. They do not execute configuration points, block
document loads, compare required policies, echo
`Sec-Required-Document-Policy`, enable or disable browser features, or send
reports.

`HttpResponse::with_document_policy_report_only(value)` and
`HttpResponse::document_policy_report_only()` expose
`Document-Policy-Report-Only` metadata through the same shared protocol
parser, formatter, directive model, and bounds while returning distinct
`HttpDocumentPolicyReportOnly` metadata. Declaration replaces raw duplicate
report-only fields with one canonical value. These helpers do not enforce
policy or deliver reports.

### Bounded Supports-Loading-Mode metadata

Server-side `Supports-Loading-Mode` helpers expose response declaration
metadata without applying prerender or fenced-frame loading policy.
`HttpResponse::with_supports_loading_mode(tokens)` validates a declared token
list through the shared protocol parser and replaces any existing
`Supports-Loading-Mode` response fields with one canonical comma-separated
value, while `HttpResponse::supports_loading_mode()` parses attached raw
fields into `HttpSupportsLoadingMode` metadata. The typed value exposes the
ordered tokens with `tokens()`, membership checks with `contains(token)`, and
exact predicates for the defined `fenced-frame`,
`credentialed-prerender`, and `prerender-cross-origin-frames` tokens;
well-formed unknown tokens such as `uncredentialed-prerender` are retained.

The helpers are bounded and metadata-only. Field values are limited to 64 KiB,
the combined raw bytes across fields to 64 KiB, and tokens to 256 per header
set. They do not prerender documents, admit fenced frames, change navigation,
or alter resource loading.

### Bounded Sec-WebSocket-Version metadata

Server-side `Sec-WebSocket-Version` helpers expose request access and
response declaration metadata without negotiating versions or switching
protocols. `Request::sec_websocket_version()` and
`HttpRequest::sec_websocket_version()` parse received fields into
`HttpSecWebSocketVersion`. `HttpResponse::with_sec_websocket_version(versions)`
validates a declared version list through the shared protocol parser and
replaces any existing `Sec-WebSocket-Version` response fields with one
canonical comma-separated value, while `HttpResponse::sec_websocket_version()`
parses attached raw fields. Recognized values are RFC 6455 version tokens
(`0` through `299` without leading zeros) in numeric descending order, such as
`13` or `13, 8, 7`.

The helpers are bounded and metadata-only. Field values are limited to 64 KiB,
the combined raw or canonical serialized field set to 64 KiB, and members to
32 per header set. They do not perform a WebSocket handshake, emit
`Connection: Upgrade` or `Upgrade: websocket`, compute `Sec-WebSocket-Accept`,
negotiate versions, or switch protocols.

### Bounded Sec-WebSocket-Protocol metadata

Server-side `Sec-WebSocket-Protocol` helpers expose request offers and
response selection metadata without choosing an application subprotocol or
switching protocols. `Request::sec_websocket_protocol()` and
`HttpRequest::sec_websocket_protocol()` parse received fields into
`HttpSecWebSocketProtocol` as offers in preference order.
`HttpResponse::with_sec_websocket_protocol(token)` validates one selected
token through the shared protocol parser and replaces any existing
`Sec-WebSocket-Protocol` response fields with one canonical value, while
`HttpResponse::sec_websocket_protocol()` parses attached raw fields as a
selection singleton.

The helpers are bounded and metadata-only. Field values are limited to 64 KiB,
the combined raw or canonical serialized field set to 64 KiB, and members to
32 per header set. They do not perform a WebSocket handshake, emit
`Connection: Upgrade` or `Upgrade: websocket`, choose an application
subprotocol, or switch protocols; applications own the selection decision.

### Bounded Speculation-Rules response metadata

`HttpResponse::with_speculation_rules(value)` validates and replaces any raw
`Speculation-Rules` response fields with one bounded opaque value, while
`HttpResponse::speculation_rules()` parses attached raw fields into
`HttpSpeculationRules`. Values are limited to 64 KiB, duplicate fields fail
closed, and response-field injection bytes are rejected. The helpers do not
fetch, parse, validate, or execute speculation rule resources.

The server is intentionally small: it handles blocking HTTP/1.x request parsing
for local tests and simple embedded use. It accepts fixed `Content-Length` and
chunked request bodies, exposes chunked request trailers, applies bounded
request head/body validation, handles `HEAD` without writing a response body,
honors `Connection` close/keep-alive semantics across a bounded
`serve_requests` loop, writes response body framing and response trailers
consistently, and exposes `Expect: 100-continue` metadata without automatically
sending an interim response or rejecting extensions. On the same socket2 listener,
the accept path detects either the HTTP/2 client preface or an HTTP/1.1
`Upgrade: h2c` request and dispatches the resulting h2c connection to the same
minimal bounded handler, including bodyless DELETE, OPTIONS, and TRACE
requests. HTTP/1.1 h2c Upgrade is opt-in on both sides: the request must be
`HTTP/1.1`, include `Connection: Upgrade, HTTP2-Settings`, `Upgrade: h2c`,
exactly one `HTTP2-Settings` field with a valid unpadded base64url SETTINGS
payload, and no request body; malformed h2c upgrade attempts receive
`400 Bad Request` before handler dispatch. When the upgrade is valid, the
server writes `101 Switching Protocols`, consumes the client's HTTP/2 preface
on the same socket, applies the advertised SETTINGS as the initial peer
SETTINGS, and uses the HTTP/2 stream id sequence reserved for an HTTP/1.1
upgrade. The server advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the
active request allowance for that bounded accept path and rejects new h2c
streams once the open-stream count plus completed requests reaches that
allowance. It also advertises and enforces a conservative
`SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata; request HEADERS
and trailing HEADERS can span CONTINUATION frames, but the decoded metadata
remains bounded before handler dispatch. The server validates peer
`SETTINGS_ENABLE_PUSH` values as only `0` or `1`; any other value rejects the
bounded h2c handshake. It also validates `SETTINGS_ENABLE_CONNECT_PROTOCOL`
values as only `0` or `1`; a received value of `1`, whether in the initial peer
SETTINGS or a later SETTINGS update, enables bounded RFC 8441 extended CONNECT
request dispatch for subsequent HEADERS on that connection. Without that
negotiated setting, any `:protocol` pseudo-header is rejected before handler
dispatch. The server advertises the default 16,384-byte
`SETTINGS_MAX_FRAME_SIZE`, rejects peer SETTINGS values outside the legal
HTTP/2 range of 16,384 through 16,777,215 bytes, rejects inbound frames larger
than the active local limit, and splits outbound response HEADERS, DATA, and
trailing HEADERS to the active peer frame-size limit. It preserves the same
HEAD body suppression for prior-knowledge h2c responses.
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
CONTINUATION frames. The server accepts peer `SETTINGS_HEADER_TABLE_SIZE`
values as the outbound response compression allowance and applies later peer
updates before encoding response trailers. If the peer advertises zero, the
server evicts response dynamic entries and writes response HEADERS and trailers
without dynamic indexing. Inbound request and request-trailer decoding stays
bounded to the server's fixed 4,096-byte HPACK dynamic table limit; incoming
dynamic table size updates may shrink that decoder table, including to zero,
but updates above 4,096 bytes are rejected. These table-size boundaries affect
only HPACK compression state, not decoded metadata limits, trailer validation,
handler dispatch, DATA flow control, or multiplex scheduling.
Prior-knowledge h2c request headers reject HTTP/1.x connection-specific fields
before handler dispatch: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`Transfer-Encoding`, and `Upgrade`; `TE` is accepted only as `te: trailers`
and other `TE` values are rejected. When serializing h2c responses, the server
strips HTTP/1.x connection-specific response fields and generated HTTP/2
framing fields from HEADERS: `Connection`, `Keep-Alive`, `Proxy-Connection`,
`TE`, `Trailer`, `Transfer-Encoding`, `Upgrade`, and `Content-Length`. H2c
response trailers skip the existing forbidden trailer set, including invalid
pseudo-header-like names, connection-specific, transfer/framing, routing,
authentication, and cookie fields.
Valid standalone PRIORITY frames and HEADERS priority fields are validated and
ignored as metadata; malformed priority metadata is rejected, and request or
response ordering does not use priority scheduling. Inbound PING without ACK
is acknowledged only when it arrives on stream 0 with exactly 8 octets of
opaque data; the PING ACK carries that same opaque data. Inbound PING ACK is
ignored for this bounded path. PING with a non-zero stream id or payload length
other than 8 is malformed and rejected. RTTP does not add keepalive timers,
automatic client- or server-initiated PING policy, replay behavior, a full
session manager, or a full multiplex scheduler around this acknowledgement
path.
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
When the bounded prior-knowledge h2c server loop finishes, it sends `GOAWAY`
with the last completed stream id so clients have a deterministic shutdown
boundary for already processed streams. If the bounded request allowance is
exhausted while additional streams are already open, the server first sends a
graceful `GOAWAY` boundary and lets streams within that boundary finish; new
streams outside the boundary are refused with `REFUSED_STREAM` and are not
dispatched to the handler. If the peer closes the TCP connection, a read/write
timeout fires, or the socket is reset before `GOAWAY` can be written, that is
transport termination rather than an HTTP/2 graceful shutdown signal and no
additional stream boundary is implied.
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
detection preserves those existing handoffs when `Upgrade` is not `h2c`. TLS
ALPN, extension callback APIs, full
extension negotiation, external h2 integration, full WebSocket-over-h2, proxy
h2, tunnel handoff, connection pooling, persistent multiplex sessions,
persistent HTTP/2 session management, full RFC 8441 support, and full HTTP/2
features such as unbounded multiplexing, unbounded multiplex scheduling,
general multiplexing, general tunnel scheduling, server push, and priority
scheduling remain outside this bounded prior-knowledge server path. RTTP does
not expose a dynamic policy API for changing the h2c frame-size or metadata
limit at runtime.

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

It is not a full RFC-covering web server and still does not implement server
TLS or async accept loops.

### Tested server protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, protocol-owned `Expect` metadata including `100-continue`, and obsolete line folding rejection | Expect metadata does not send `100 Continue` or reject unsupported extensions; intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Byte ranges | `HttpByteRange` parses one `bytes` range, `Request::evaluate_if_range` gates it with caller-provided strong ETag or exact HTTP-date metadata, `HttpResponse::partial_content`/`range_not_satisfiable` serialize `206`/`416` with `Content-Range`, and `HttpAcceptRanges` plus `HttpResponse::with_accept_ranges`/`with_accept_ranges_none`/`accept_ranges` declare and parse bounded `Accept-Ranges` metadata while preserving raw headers | No Range request generation, multipart range serialization, partial response engine, automatic retry/replay, redirect behavior, cache storage or policy, filesystem serving, MIME detection, automatic cache validation, automatic static-file policy, automatic byte serving, content slicing, download resume, or status-policy behavior |
| Conditional requests | `Request::evaluate_conditional`, `evaluate_conditional_request`, `HttpConditionalMetadata`, and `HttpEntityTag` evaluate bounded HTTP/1.1 validators; `Request::if_modified_since`/`if_unmodified_since` and the `HttpRequest` equivalents parse HTTP-date validators through the shared protocol types; `HttpResponse::not_modified`, `precondition_failed`, `with_etag`, and typed bounded `etag` serialize or expose `304`/`412` metadata while preserving raw headers | No cache storage, static-file serving policy, automatic revalidation, or cache-control engine |
| Informational responses and Early Hints | `HttpResponse::early_hints` and `early_hints_with_headers` construct validated bodyless `103 Early Hints` response metadata with bounded `Link` and safe metadata headers | `101 Switching Protocols` remains a separate terminal handoff response; no automatic preload execution, cache policy, redirect/retry/replay, route generation, streaming early-write API, TLS/ALPN behavior, or status-policy behavior |
| Cache-Control, CDN-Cache-Control, and Cache-Status | `Request::cache_control`, `HttpRequest::cache_control`, and `HttpResponse::cache_control` parse bounded request/response directives, numeric freshness fields, quoted field-name lists, and extension directives; `HttpResponse::cdn_cache_control` parses bounded response `CDN-Cache-Control` directives and CDN extension metadata while preserving raw response headers on parse errors; `HttpResponse::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw response headers on parse errors; `HttpResponse::with_age`/`age`, `with_expires`/`expires`, and `with_retry_after_delta`/`with_retry_after_date`/`retry_after` declare and parse response `Age`, `Expires`, and `Retry-After` metadata | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, automatic conditional requests, directive-based validator evaluation, automatic sleep, retry, replay, backoff, scheduler integration, or status-code policy engine |
| Memento-Datetime | `HttpResponse::with_memento_datetime`/`memento_datetime` declare and parse bounded singleton `Memento-Datetime` IMF-fixdate metadata while preserving raw headers on parse errors | No archival selection, `Accept-Datetime` negotiation, TimeGate behavior, retry, or transport changes |
| Fetch Metadata | `Request::sec_fetch_site`, `sec_fetch_mode`, `sec_fetch_dest`, `sec_fetch_user`, and `sec_purpose` parse bounded typed `Sec-Fetch-*`/`Sec-Purpose` request fields and preserve raw values on errors | No browser security policy, request blocking, origin validation, navigation policy, automatic header generation, prefetch execution, or cache behavior |
| Save-Data | `Request::save_data` and `HttpRequest::save_data` parse bounded singleton `Save-Data` `on`-token metadata and preserve raw values on errors | No reduced-data serving, content adaptation, compression, Client Hints advertisement, retries, or browser data-saver policy |
| Accept-Charset | `HttpRequestAcceptCharsets`, `Request::accept_charset`, and `HttpRequest::accept_charset` parse bounded `Accept-Charset` request metadata through the shared `rttp-protocol` type | No content negotiation, charset transcoding, body decoding, MIME sniffing, or response selection |
| Sec-GPC | `Request::sec_gpc` and `HttpRequest::sec_gpc` parse bounded singleton `Sec-GPC` `1`-signal metadata and preserve raw values on errors | No consent inference, tracking-policy enforcement, legal policy, serving policy, retries, or browser state |
| Upgrade-Insecure-Requests | `Request::upgrade_insecure_requests` and `HttpRequest::upgrade_insecure_requests` parse bounded singleton `Upgrade-Insecure-Requests` `1`-token metadata and preserve raw values on errors | No URL rewriting, redirecting, Content-Security-Policy enforcement, HSTS, or automatic scheme selection |
| Depth | `Request::depth` and `HttpRequest::depth` parse bounded singleton WebDAV `Depth` request metadata through the shared protocol type and preserve raw values on errors | No resource traversal, WebDAV method selection, method-policy enforcement, retry, or forwarding policy |
| Destination | `Request::destination` and `HttpRequest::destination` parse bounded singleton WebDAV `Destination` request metadata through the shared protocol type and preserve raw values on errors | No destination resolution, URI normalization, authorization, COPY/MOVE execution, or application resource policy |
| Timeout | `Request::timeout` and `HttpRequest::timeout` parse bounded ordered WebDAV `Timeout` request metadata through the shared protocol type and preserve raw values on errors | No lock creation, lock refresh, application-timeout selection, retry, or forwarding policy |
| DAV | `Dav`, `HttpDav`, `HttpResponse::with_dav`/`dav`, and client `Response::dav` parse or declare bounded ordered WebDAV `DAV` response metadata through the shared protocol type, accepting `1`, `2`, `3`, extension tokens, and `<absolute-URI>` Coded-URLs while preserving raw headers on parse failures | No WebDAV feature inference, feature negotiation, method support enforcement, route dispatch, lock behavior, or application resource policy |
| Idempotency-Key | `Request::idempotency_key` and `HttpRequest::idempotency_key` parse bounded singleton opaque `Idempotency-Key` request metadata through the shared protocol type, preserve raw values on errors, and redact the key from typed debug output | No retry, replay, key storage or comparison, deduplication store, or application idempotency policy |
| WebSocket handshake metadata | `Request::sec_websocket_key` and `HttpRequest::sec_websocket_key` parse bounded singleton `Sec-WebSocket-Key` request metadata through the shared protocol type and preserve raw values on errors; `HttpResponse` can derive and parse bounded singleton `Sec-WebSocket-Accept` metadata from a validated key using the RFC GUID plus SHA-1/base64 transform; typed debug output redacts key and accept material | No HTTP upgrade, random nonce generation, WebSocket frames, or handshake policy |
| Sec-WebSocket-Version | `HttpSecWebSocketVersion`, `Request::sec_websocket_version`, `HttpRequest::sec_websocket_version`, `HttpResponse::with_sec_websocket_version`, and `HttpResponse::sec_websocket_version` parse and declare bounded version-list metadata through the shared protocol type, requiring canonical descending order, replacing raw duplicates on declaration, and preserving raw headers on parse failures | No WebSocket handshake, `Connection: Upgrade` emission, `Sec-WebSocket-Accept` computation, version negotiation, protocol switch, or frames |
| Sec-WebSocket-Protocol | `HttpSecWebSocketProtocol`, `Request::sec_websocket_protocol`, `HttpRequest::sec_websocket_protocol`, `HttpResponse::with_sec_websocket_protocol`, and `HttpResponse::sec_websocket_protocol` parse and declare bounded protocol-token metadata through the shared protocol type: request offers preserve preference order while response values are selection singletons, with case-sensitive duplicates rejected and raw headers preserved on parse failures | No WebSocket handshake, `Connection: Upgrade` emission, automatic subprotocol choice, protocol switch, or frames |
| Pragma | `HttpPragma`, `Request::pragma`, `HttpRequest::pragma`, `HttpResponse::with_pragma`, and `HttpResponse::pragma` share the bounded protocol `Pragma` representation for server request access and server response construction, combining fields in wire order and preserving raw headers on errors | No translation into `Cache-Control`, cache storage, freshness checks, revalidation, or cache/intermediary policy |
| W3C Trace Context | `Request::traceparent`/`tracestate` and `HttpRequest` helpers parse bounded W3C Trace Context request metadata, preserve raw values on errors, preserve tracestate ordering, and redact propagation values from typed debug output | No trace-id creation, sampling decision, tracing backend, span model, or automatic propagation |
| W3C Baggage | `Request::baggage` and `HttpRequest::baggage` parse bounded W3C Baggage request metadata, preserve raw values on errors, preserve member order, and redact member and property values from typed debug output | No application-data interpretation, request-context storage, tracing backend, span model, or automatic propagation |
| X-Forwarded compatibility metadata | `Request::x_forwarded_for`, `x_forwarded_host`, and `x_forwarded_proto` plus `HttpRequest` helpers parse bounded ordered node, authority, and scheme metadata while preserving raw headers on errors | No forwarded identity trust, client address selection, routing rewrite, scheme rewrite, redirect, upgrade, enforcement, or trusted-proxy selection; applications must choose trusted proxies |
| Accept-Language | `HttpAcceptLanguages`, `Request::accept_language`, and `HttpRequest::accept_language` parse bounded ordered `Accept-Language` ranges and q-values through the protocol `AcceptLanguage` type and preserve raw values on errors | No locale matching, fallback selection, translation lookup, routing, or automatic response choice |
| Vary | `HttpVary`, `HttpResponse::with_vary`, `HttpResponse::vary`, `Request::vary_selection`, and `HttpRequest::vary_selection` parse, declare, and select bounded `Vary` metadata with case-insensitive field-name handling | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| No-Vary-Search | `HttpNoVarySearch`, `HttpResponse::with_no_vary_search`, and `HttpResponse::no_vary_search` parse and declare bounded Structured Fields response metadata for query-parameter variance declarations | No cache storage, cache-key matching, URL normalization, navigation behavior, request replay, or shared-cache policy enforcement |
| Permissions-Policy | `HttpPermissionsPolicy`, `HttpResponse::with_permissions_policy`, and `HttpResponse::permissions_policy` parse and declare bounded W3C Permissions Policy dictionary response metadata through the shared protocol type, replacing raw duplicates on declaration and preserving raw headers on parse failures | No browser permission grants or denials, origin comparison, `self` resolution, API enablement, origin-policy enforcement, or report sending |
| Document-Policy | `HttpDocumentPolicy`, `HttpResponse::with_document_policy`, and `HttpResponse::document_policy` parse and declare bounded WICG Document Policy dictionary response metadata through the shared protocol type, replacing raw duplicates on declaration, retaining `*` and `report-to`, and preserving raw headers on parse failures | No configuration-point execution, document-load blocking, required-policy comparison, `Sec-Required-Document-Policy` echoing, feature enablement, or report sending |
| Document-Policy-Report-Only | `HttpDocumentPolicyReportOnly`, `HttpResponse::with_document_policy_report_only`, and `HttpResponse::document_policy_report_only` parse and declare bounded WICG Document Policy Report-Only dictionary metadata through the same shared protocol parser and formatter, replacing raw duplicates on declaration, retaining report-only type identity, `*`, and `report-to`, and preserving raw headers on parse failures | No policy enforcement, document-load blocking, required-policy comparison, `Sec-Required-Document-Policy` echoing, feature enablement, report delivery, scheduling, retry, or endpoint validation |
| Supports-Loading-Mode | `HttpSupportsLoadingMode`, `HttpResponse::with_supports_loading_mode`, and `HttpResponse::supports_loading_mode` parse and declare bounded Structured Fields token-list response metadata through the shared protocol type, replacing raw duplicates on declaration, retaining unknown tokens, and preserving raw headers on parse failures | No prerendering, fenced-frame admission, navigation changes, redirects, retries, or resource-loading behavior |
| Allow | `HttpAllowedMethods`, `HttpResponse::with_allow`, and `HttpResponse::allow` declare and parse bounded `Allow` method-list metadata | No route dispatch, automatic `405` generation, `OPTIONS` policy, fallback method selection, retry/replay, or status-code policy engine |
| Content-Security-Policy-Report-Only | `HttpContentSecurityPolicyReportOnly`, `HttpResponse::with_content_security_policy_report_only`, `content_security_policy_report_only`, and client `Response::content_security_policy_report_only` parse or declare bounded opaque `Content-Security-Policy-Report-Only` response metadata while preserving repeated fields in wire order and raw headers on parse failures | No CSP enforcement, directive evaluation, report delivery, browser policy state, retry, redirect, cache behavior, or status-policy behavior |
| Content-Language | `HttpContentLanguages`, `Request::content_language`, `HttpRequest::content_language`, `HttpResponse::with_content_language`, and `HttpResponse::content_language` parse or declare bounded `Content-Language` metadata | No automatic language negotiation, route selection, locale fallback, variant matching, cache policy, retry, replay, redirect, or status-policy behavior |
| Accept-Encoding | `HttpRequestAcceptEncodings`, `Request::accept_encoding`, and `HttpRequest::accept_encoding` parse bounded `Accept-Encoding` request metadata through the shared `rttp-protocol` type | No compression, decompression, content negotiation, retries, or transport changes |
| Content-Location | `HttpResponse::with_content_location` declares one bounded singleton `Content-Location` header, and `HttpResponse::content_location` parses attached singleton response metadata while preserving raw headers | No redirect behavior, cache variant selection, representation replacement, retry/replay, route generation, or status-policy behavior |
| Service-Worker-Allowed | `HttpResponse::with_service_worker_allowed` declares one bounded singleton `Service-Worker-Allowed` header, and `HttpResponse::service_worker_allowed` parses attached singleton path metadata while preserving raw headers | No service-worker registration, scope evaluation, script-URL resolution, or application routing policy |
| Content-DPR | `HttpResponse::with_content_dpr` declares one bounded singleton `Content-DPR` header, and `HttpResponse::content_dpr` plus client `Response::content_dpr` parse attached singleton decimal-ratio metadata while preserving raw headers | No image rescaling, request DPR emission, Client Hints policy, retry, or transport changes |
| Content-Type and Content-Encoding | `HttpContentType`, `Request::content_type`, `HttpRequest::content_type`, `HttpResponse::with_content_type`, `content_type`, `HttpResponseContentEncodings`, `Request::content_encoding`, `HttpRequest::content_encoding`, `HttpResponse::with_content_encoding`, and `content_encoding` parse or declare bounded representation metadata while preserving raw headers on parse failures and replacing raw response duplicates on typed declaration | No MIME sniffing, body decoding, charset transcoding, compression/decompression, negotiation, cache policy, redirects, retry/replay, or filesystem serving |
| Connection | `HttpConnection`, `Request::connection`, `HttpRequest::connection`, and `HttpResponse::connection` parse bounded HTTP/1 `Connection` tokens, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to hop-by-hop stripping, keep-alive/close, upgrade/h2c, or HTTP/2 rejection |
| Transfer-Encoding | `HttpTransferEncoding`, `Request::transfer_encoding`, and `HttpRequest::transfer_encoding` parse bounded HTTP/1 `Transfer-Encoding` fields that must be sole `chunked`, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to `request_body_kind`, `TE`, Content-Length, or HTTP/2 decode rejection |
| Upgrade metadata | `Upgrade`, `HttpUpgrade`, `HttpClient::upgrade_protocols`, `Response::upgrade`, `Request::upgrade`, `HttpRequest::upgrade`, `HttpResponse::with_upgrade`, and `HttpResponse::upgrade` validate, declare, or parse bounded HTTP/1 `Upgrade` protocol metadata while preserving raw headers on parse failures | No automatic `Connection: Upgrade`, h2c selection, client/server socket handoff, ALPN negotiation, or upgraded protocol implementation |
| Content-Disposition | Protocol-backed `HttpContentDisposition`, `HttpResponse::with_content_disposition`, `with_attachment_filename`, and `content_disposition` declare and parse bounded singleton `Content-Disposition` response metadata, preserve parsed `filename` and `filename*` parameter values, preserve raw headers on parse failures, and replace raw duplicates on typed declaration | No automatic download, filesystem path handling, MIME sniffing, cache behavior, redirect behavior, retry/replay, negotiation, or status-policy behavior |
| WWW-Authenticate | `HttpWwwAuthenticate`, `HttpResponse::with_www_authenticate`, and `HttpResponse::www_authenticate` declare or parse bounded response authentication challenge metadata while preserving raw headers on parse failures | No credential storage, authentication policy, retry, automatic `Authorization` generation, Basic/Bearer implementation, redirect behavior, or status-policy behavior |
| Authorization and Proxy-Authorization | `HttpAuthorization`, `HttpProxyAuthorization`, `Request::authorization`, and `Request::proxy_authorization` expose shared bounded request authorization metadata, reject duplicate parsed inbound fields, and redact credentials from typed debug output | No credential storage, authentication policy, challenge processing, retry, Basic/Bearer implementation, redirect policy changes, or automatic credential forwarding |
| Proxy-Status | `HttpProxyStatus`, `HttpResponse::with_proxy_status`, and `HttpResponse::proxy_status` declare or parse bounded RFC 9209 Token/String proxy identifiers with opaque parameters while preserving raw headers on parse failures | No proxy health checks, retries, trailer promotion, or origin-generation policy |
| Cross-Origin-Opener-Policy-Report-Only | `HttpCrossOriginOpenerPolicyReportOnly`, `HttpResponse::with_cross_origin_opener_policy_report_only`, and `HttpResponse::cross_origin_opener_policy_report_only` declare or parse bounded singleton COOP Report-Only metadata, reuse the canonical COOP directives, retain reporting parameters including `report-to`, and preserve raw headers on parse failures | No browsing-context isolation, report scheduling, sending, persistence, retry, routing, or `Reporting-Endpoints` validation |
| Server-Timing | `HttpServerTiming`, `HttpResponse::with_server_timing`, and `HttpResponse::server_timing` declare or parse bounded response timing metadata while preserving raw headers on parse failures | No metric collection, measurement, telemetry export, metrics backend integration, retry, redirect behavior, or status-policy behavior |
| Alt-Used | `HttpAltUsed`, `HttpResponse::with_alt_used`, and `HttpResponse::alt_used` declare or parse bounded singleton response authority metadata while preserving raw headers on parse failures and replacing raw response duplicates on typed declaration | No alternative service selection, origin rewriting, socket migration, retry, or connection-policy behavior |
| Origin-Trial | `HttpOriginTrials`, `HttpResponse::with_origin_trials`, and `HttpResponse::origin_trials` declare or parse bounded opaque `Origin-Trial` tokens in wire order, preserve duplicates, redact token material from debug output, and replace raw response fields on typed declaration | No token signature validation, expiration checks, origin applicability, feature activation, or browser trial policy |
| Speculation-Rules | `HttpSpeculationRules`, `HttpResponse::with_speculation_rules`, and `HttpResponse::speculation_rules` preserve one bounded opaque `Speculation-Rules` response field, reject duplicates and response-field injection bytes, redact debug output, and replace raw response fields on typed declaration | No speculation rule fetching, parsing, validation, prefetching, prerendering, execution, navigation changes, cache behavior, retry, or redirect behavior |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpHandoff::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Application metadata trailers are allowed; trailer names that affect connection state, routing, authentication/cookies, framing, or payload processing are rejected |
| Bounded h2c server | The same `socket2` listener detects the HTTP/2 prior-knowledge preface or a valid HTTP/1.1 `Upgrade: h2c` request with `HTTP2-Settings`, validates SETTINGS including legal `SETTINGS_ENABLE_PUSH` and `SETTINGS_ENABLE_CONNECT_PROTOCOL` values of only `0` or `1` and legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, dispatches RFC 8441 extended CONNECT only after `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` has been negotiated, exposes negotiated extended CONNECT as a normal `Request` with method `CONNECT`, version `HTTP/2`, target from `:path`, `host` from `:authority`, and `Request::extended_connect_protocol()` from `:protocol`, advertises the default 16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects inbound frames above the active local limit, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the bounded active stream allowance, enforces that allowance before dispatching new streams, advertises and enforces a conservative `SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, serves bounded streams including bodyless DELETE, OPTIONS, TRACE, and negotiated extended CONNECT, handles HEAD without response DATA, rejects connection-specific request fields before handler dispatch, strips connection-specific response fields during h2c serialization, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges inbound PING without ACK on stream 0 and exactly 8 octets with matching opaque data, ignores inbound PING ACK, rejects malformed PING frames, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman fields and bounded CONTINUATION header blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | Ordinary `CONNECT`, missing-negotiation `:protocol`, non-CONNECT `:protocol`, malformed h2c Upgrade, request bodies on h2c Upgrade, and `PUSH_PROMISE` are rejected deterministically before handler dispatch; HTTP/1.1 `CONNECT` and non-h2c `Upgrade` remain separate handoff paths; bounded h2c only, with no keepalive timers, no automatic client/server initiated PING policy, no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, full WebSocket-over-h2, proxy h2, tunnel handoff, connection pooling, persistent multiplex sessions, persistent HTTP/2 session management, automatic retry/replay, server push, full RFC 8441 support, full session manager, full stream state machine, full multiplex scheduler, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, general tunnel scheduling, priority scheduling, or full HTTP/2 server feature set |

## Acceptance

This repository participates in automated end-to-end acceptance runs driven by
the CodeOn flow (review, revision, landing preflight, merge).


## Acceptance Testing

### Environment

The full CodeOn acceptance flow expects the following environment variables when
running against this repository:

| Name | Purpose | Example |
| --- | --- | --- |
| `CODEON_DATABASE_URL` | Postgres URL of the CodeOn database | `postgres://codeon:codeon@localhost/codeon` |
| `CODEON_MCP_BEARER_TOKEN` | Bearer token for the CodeOn MCP HTTP API | `secret-token` |
| `GITHUB_TOKEN_FEWENSA` | GitHub credential used by the flow | `ghp_...` |

### Full Flow

The complete acceptance loop reviews the pull request, requests or applies a
revision when the reviewed candidate misses a sealed requirement, runs the
landing preflight checks, merges the clean pull request, and closes the linked
issue after landing is confirmed.

### Examples

```sh
CODEON_DATABASE_URL=postgres://codeon:codeon@localhost/codeon \
CODEON_MCP_BEARER_TOKEN=secret-token \
GITHUB_TOKEN_FEWENSA=ghp_... \
codeon run --repo fewensa/rttp --pr 455 --flow full
```
