rttp_client
===========

`rttp_client` is a small HTTP client crate. Plain HTTP is available by default;
optional features add async request APIs and TLS implementations.

Typed response and request helpers in this crate are metadata-only unless a
section explicitly says otherwise. They validate, emit, or expose bounded HTTP
metadata and do not implement cache, retry, preload, authorization, or policy
engines, nor do they create browser policy, authentication policy,
representation selection, or body transformations.

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

```toml
[dependencies]
rttp_client = { version = "0.2", features = ["async", "tls-rustls"] }
```

Direct TCP connections use `socket2`. SOCKS proxy handshakes remain delegated to
the `socks` crate.
HTTP/1.x chunked responses are decoded, and response trailers are exposed
through `Response::trailers`, `Response::trailer`, and
`Response::trailer_value` for both blocking and async request APIs.

## Bounded Max-Forwards diagnostics

`HttpClient::max_forwards(value)` sets a `Max-Forwards` request header for
application-selected `TRACE` or `OPTIONS` diagnostics. The helper accepts only
up to ten ASCII decimal digits that fit in the `u32` range (`0` through
`4294967295`) and rejects negative, fractional, empty, oversized, and
overflowing values before a socket is opened. It only validates and emits the
header: RTTP does not select a diagnostic policy, route through proxies,
decrement the value, or retry the request. Callers needing an unusual value can
retain full raw-header control with `header(("Max-Forwards", "..."))`.

## Bounded HTTP/1.1 byte ranges

`HttpClient` includes helpers for the single-range `bytes` forms RTTP keeps
bounded: `range(start, end)` emits `Range: bytes=start-end`,
`range_from(start)` emits `Range: bytes=start-`, and `range_suffix(length)`
emits `Range: bytes=-length`. The helpers reject inverted closed ranges and a
zero suffix length before a socket is opened. They are request-header helpers;
manual `Range` headers remain available through `header(("Range", "..."))`
when callers need behavior outside the helper validation.

```rust
client
  .get()
  .url("http://example.test/archive")
  .range(1_024, 2_047)?
  .if_range_etag(r#""revision-42""#)?
  .emit()?;
```

Partial-content responses are exposed through the normal `Response` API.
`Response::is_partial_content()` identifies `206 Partial Content`, and
`Response::content_range()` parses a single `Content-Range` field such as
`bytes 10-19/200` into the shared checked protocol `ContentRange` with `unit`,
`start`, `end`, and `complete_length` accessors. Invalid or duplicate
`Content-Range` metadata returns a response error from the typed helper while
raw headers remain preserved. `Response::is_range_not_satisfiable()` identifies
`416 Range Not Satisfiable`; an unsatisfied `Content-Range` such as
`bytes */200` is exposed with no `start` or `end` and
`ContentRange::is_unsatisfied() == true`. Response bodies and headers are still
preserved normally for both `206` and `416`.

`If-Range` is available as a bounded request helper for the two validator forms
that compose with these range helpers: `if_range_etag(etag)` emits a single
strong entity tag validator, and `if_range_date(http_date)` emits an HTTP-date
validator. The ETag helper rejects weak tags, `*`, lists, and malformed tag
syntax before opening a socket; the date helper requires a parseable HTTP-date.
Manual `If-Range` headers remain available through `header(("If-Range", "..."))`
when callers need values outside the helper validation.

`Response::accept_ranges()` parses one or more response `Accept-Ranges` header
fields into `AcceptRanges` metadata. It returns `Ok(None)` when the header is
absent. Present values expose `units()`, `is_none()`, and `accepts_bytes()`;
the `none` sentinel is represented as an empty unit list, while range units
preserve their spelling and wire order. The parser is shared with the server
facade through `rttp-protocol`.

The helper is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed header set is limited to 256 range units,
malformed or empty values are rejected, duplicate units are rejected
case-insensitively across all parsed header fields, and `none` combined with
any unit is rejected. The original response remains usable: raw
`Accept-Ranges` fields are still available through `Response::header_value()`,
`Response::header_values()`, and the other response metadata helpers.

RTTP does not synthesize multipart range requests, generate `Range` requests
from `Accept-Ranges`, evaluate `If-Range`, retry range requests, store cached
responses, apply automatic cache validation policy, resume downloads, slice
content, or choose status handling on the client side. Multiple ranges can only
be sent by manually setting the header, and any server response is then parsed
as an ordinary HTTP response.

## Bounded HTTP/1.1 conditional requests

`HttpClient` includes bounded helpers for the common conditional request
validators: `if_none_match(etag)`, `if_match(etag)`,
`if_modified_since(http_date)`, and `if_unmodified_since(http_date)`. The ETag
helpers accept one validator at a time: `*`, a strong tag such as `"abc"`, or a
weak tag such as `W/"abc"`. They reject comma-separated lists and obviously
malformed tag syntax before opening a socket; callers that need validator
lists or non-helper behavior can still use the generic `header` API.

The date helpers validate that the supplied value parses as an HTTP-date before
emission and then write the value as provided. They do not normalize date text
or choose a validator policy for the request. `If-Range` uses its own helpers
because it is intended to compose with range requests and permits only strong
entity tags or HTTP-date validators.

```rust
client
  .get()
  .url("http://example.test/manifest")
  .if_none_match(r#""revision-42""#)?
  .if_modified_since("Sun, 06 Nov 1994 08:49:37 GMT")?
  .emit()?;
```

Conditional responses are exposed through response metadata helpers.
`Response::is_not_modified()` identifies `304 Not Modified`,
`Response::is_precondition_failed()` identifies `412 Precondition Failed`,
`Response::etag()` parses one bounded response `ETag` field into the protocol
`EntityTag` type, `Response::etag_value()` returns the raw response `ETag` field
when present, and `Response::last_modified()` returns the response
`Last-Modified` field when present. Malformed, oversized, or duplicate `ETag`
fields make the typed helper return an error while raw values remain available
through `Response::etag_value()`, `Response::header_value()`, and
`Response::header_values()`. `Response::last_modified_date()` parses
`Last-Modified` as an HTTP-date
using the same parser used by the client date helpers: it returns `Ok(None)`
when the header is absent, returns `SystemTime` for a valid HTTP-date
singleton, and returns an error for malformed or duplicate values. The raw
field stays available through `last_modified()` and the ordinary header
accessors. A `304` response is treated as bodyless even if misleading framing
headers are present, so the connection remains framed for the next response.
`412` is surfaced as a normal response status and body/framing rules remain the
server's responsibility.

RTTP does not provide cache storage, automatic revalidation, or a
cache-control engine. Client conditional helpers only set request headers and
expose response metadata; `last_modified_date()` applies no cache policy,
freshness calculation, or revalidation; applications decide when to persist
validators, when to revalidate, and how to interpret cache directives.

## Bounded HTTP/1.1 informational responses and Early Hints

`rttp_client` skips HTTP/1.1 informational response heads before returning the
terminal response, and exposes the skipped metadata through
`Response::informational_responses()`. Each `InformationalResponse` preserves
the observed status code, reason phrase, and raw headers with accessors such
as `code()`, `reason()`, `headers()`, `headers_of_name()`,
`header_value()`, and `header_values()`. This makes `103 Early Hints` link
metadata observable without changing the final response returned by
`emit`, `rasync`, or the async request APIs.

The parser is bounded and validation-oriented. Each informational head is
limited to the normal response-head bound, must use an HTTP/1.1 `1xx` status
line, must contain valid header field names and values, and must not declare
body framing with `Content-Length` or `Transfer-Encoding`. Malformed,
oversized, or ambiguously framed informational heads return an error before
the final response head is consumed; raw header fields from valid skipped
heads are preserved on the informational metadata.

`101 Switching Protocols` is intentionally separate from skipped
informational history. Upgrade and `CONNECT` handoff paths may skip earlier
interim `1xx` heads, but the `101` or tunnel response remains the terminal
handoff response and upgraded protocol bytes are caller-owned. Early Hints
metadata does not trigger automatic preload execution, cache policy, redirect,
retry, replay, route generation, streaming early-write behavior, TLS/ALPN
behavior, or status-policy behavior.

## Bounded HTTP/1.1 Link response metadata

`Response::links()` parses one or more final-response `Link` fields into
ordered `LinkValues` and `LinkValue` metadata. It returns `Ok(None)` when the
header is absent. Each value retains its target URI/reference and ordered
parameters, including unknown parameters such as extensions alongside `rel`.
Parsing is on demand, so malformed or oversized metadata returns an error
without discarding raw response headers. Fields and parameter values are
limited to 64 KiB, with at most 256 link-values and 256 parameters per value;
the original `Link` fields remain available through `Response::header_value()`
and `Response::header_values()`.

The helper is metadata-only and shares Early Hints' bounded metadata posture:
it does not preload, resolve, schedule fetches, redirect, apply cache policy,
or generate routes from `Link`.

## Bounded Cache-Control request metadata

`HttpClient::cache_control_no_cache()`, `cache_control_no_store()`, and
`cache_control_max_age(seconds)` append common request directives to one
`Cache-Control` field. `cache_control_extension(name)` and
`cache_control_extension_with_value(name, value)` append valueless and
token-valued extension directives. The helpers reject malformed tokens,
duplicate directives, oversized values, and more than 256 directives before a
connection is opened.

The helpers only declare request metadata: they do not create a cache, compute
freshness, or automatically revalidate. Raw `header(("Cache-Control", value))`
remains available for quoted-string extension values or other syntax outside
this bounded API.

## Bounded HTTP/1.1 Cache-Control behavior

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

## Bounded CDN-Cache-Control response metadata

`Response::cdn_cache_control()` parses one or more response
`CDN-Cache-Control` header fields into `CdnCacheControl`. The parser preserves
directive order, including CDN-specific extension directives, and exposes each
directive token name plus its optional parsed value.

The helper uses the same bounds and syntax validation as response
`Cache-Control`: 64 KiB per field value, at most 256 parsed directives, valid
HTTP tokens for directive names and unquoted values, and well-formed quoted
strings. Invalid `CDN-Cache-Control` metadata makes the helper return an error
without discarding the raw response headers or body.

This is response metadata only. `rttp_client` does not create a CDN cache,
compute freshness, revalidate automatically, apply surrogate-key behavior,
enforce shared-cache policy, retry, replay, redirect, or alter response
acceptance from `CDN-Cache-Control`.

## Bounded HTTP/1.1 Date, Age, and Expires behavior

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
metadata only; `rttp_client` does not calculate freshness, correct clock skew,
validate cache state against wall-clock time, store responses, match stored
responses, revalidate responses, apply shared-cache policy, issue automatic
conditional requests, retry, redirect, schedule work, or choose status policy.

## Bounded HTTP/1.1 Retry-After behavior

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

## Bounded HTTP/1.1 Allow behavior

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

## Bounded HTTP/1.1 Content-Language behavior

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
preserving raw headers and parsing only when requested. `rttp_client` does not
perform automatic language negotiation, locale fallback, variant matching,
cache policy, retry, replay, redirect, or status-policy behavior from
`Content-Language`.

## Bounded HTTP/1.1 Content-Location behavior

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
`Response::content_language()`, `Response::vary()`, `Response::age()`,
`Response::expires()`, and `Response::accept_ranges()` by preserving raw
headers and parsing only when requested. It is metadata-only: `rttp_client`
does not treat `Content-Location` as redirect behavior, cache variant
selection, representation replacement, retry/replay behavior, route
generation, or status-policy behavior.

## Bounded Deprecation response metadata

`Response::deprecation()` parses a response `Deprecation` header into the
shared protocol-owned `Deprecation` metadata type. It returns `Ok(None)` when
the header is absent and rejects duplicate header fields because `Deprecation`
is handled as a singleton response metadata field. Present values are a
Structured Fields boolean (`?0` / `?1`) or date (`@` followed by signed UNIX
seconds). `Deprecation::parse(value)` is available when callers want to
validate one raw field value directly.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB. Empty values, item parameters, inner lists, comma-joined items,
integers without `@`, decimals, strings, tokens including historical `true`,
byte sequences, display strings, IMF-fixdate values, forbidden ASCII control
bytes, and dates that cannot be represented as `SystemTime` make
`Response::deprecation()` return an error while leaving the original response
headers and body available through `Response::header_value()`,
`Response::header_values()`, and the other response metadata helpers.

The helper is metadata-only: `rttp_client` does not compare `Sunset`, follow
`Link` `rel=deprecation`, decide whether a resource is already deprecated,
retry requests, or select another endpoint.

## Bounded HTTP/1.1 representation metadata behavior

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

## Bounded Accept-Encoding request metadata

`HttpClient::accept_encoding()` appends a validated request coding, while
`accept_encoding_with_q()` accepts an HTTP q-value from `0` through `1` with
at most three fractional digits. Convenience helpers cover `gzip`, `deflate`,
`br`, and `identity`, including q-value variants. The helpers emit one
comma-separated `Accept-Encoding` field and reject invalid coding tokens,
q-values, duplicates, oversized values, and more than 32 codings before a
connection is opened.

These helpers declare request metadata only. They do not enable automatic
compression, decompression, or content negotiation.

## Bounded digest preference request metadata

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

## Bounded HTTP message signature request metadata

`HttpClient::signature()` and `signature_input()` validate and replace one
RFC 9421 `Signature` or `Signature-Input` request field. Malformed present
input is rejected before a connection is opened. `Response::signature()` and
`signature_input()` parse received fields independently, returning `Ok(None)`
when a field set is absent and leaving raw headers in place on parse errors.

These helpers declare and parse metadata only. They do not sign, verify, look
up keys, canonicalize covered components, or apply cryptographic policy.

## Bounded Accept request metadata

`HttpClient::accept()` appends one validated media range, and
`accept_with_q()` adds a q-value from `0` through `1` with at most three
fractional digits. Convenience helpers cover `*/*`, JSON, HTML, XML, and plain
text, including q-value variants. Media types, parameter names, parameter
values, and q-values are validated; duplicate parameters, oversized values,
and more than 32 media ranges are rejected before a connection is opened.

The helpers emit one comma-separated `Accept` field and do not choose a
response representation. `header(("Accept", value))` remains available for
media ranges or extensions outside this bounded helper API.

## Bounded Authorization request metadata

`HttpClient::authorization(scheme, credentials)` emits one `Authorization`
field after validating its HTTP-token scheme and non-empty credential value,
with a 64 KiB bound. Use `header(("Authorization", value))` as the raw escape
hatch for custom scheme syntax. Credential interpretation remains
application-owned: RTTP does not validate individual schemes, store or refresh
credentials, process challenges, retry, or forward credentials on redirects.

## Bounded HTTP request control metadata

`HttpClient::te()`, `te_with_q()`, and `te_trailers()` build a bounded `TE`
field. `HttpClient::prefer()` and `prefer_with_value()` build a bounded
`Prefer` field with token-only values. `Prefer` values are limited to 8 KiB and
`wait` accepts only unsigned decimal integers. Both helpers reject malformed
tokens, invalid q-values, duplicates, oversized values, and more than 32
members before opening a connection. `TE: chunked` is rejected because framing
remains owned by the existing HTTP/1 implementation, and `trailers` cannot
carry a q-value. Bounded h2c emits only an exact `TE: trailers` field and strips
other `TE` values with HTTP/1.x connection-specific request metadata.

These are declaration helpers only. RTTP does not enable a transfer-coding
engine, change request framing, apply response preferences, schedule async
work, forward requests, retry, or otherwise infer behavior from `TE` or
`Prefer`.

## Bounded Connection metadata

`Response::connection()` parses retained HTTP/1 `Connection` fields into
`Connection` header metadata. It returns `Ok(None)` when the header is absent.
Present values combine case-insensitive fields in wire order and preserve
token spelling, including duplicates. `Connection::parse(value)` is available
when callers want to validate one raw field value directly.

Each field value is limited to 64 KiB. Parsing accepts at most 256 tokens and
rejects empty members, malformed tokens, parameters, oversized values, and too
many tokens. Parse errors do not reject the raw response: original headers
remain available through `Response::header_value()` and
`Response::header_values()`. HTTP/2 continues to reject `Connection` at decode
time.

This helper is HTTP/1 header metadata only. `rttp_client` does not change
keep-alive, `auto_add_connection`, hop-by-hop stripping, or HTTP/2 rejection
from this accessor.

## Bounded Upgrade metadata

`HttpClient::upgrade_protocols()` validates and replaces request `Upgrade`
metadata without opening a socket, changing request method, or adding
`Connection: Upgrade`. `Response::upgrade()` parses retained HTTP/1 `Upgrade`
response fields into `Upgrade` metadata. It returns `Ok(None)` when the header
is absent. Present values combine fields in wire order and preserve protocol
spelling.

Each field value is limited to 64 KiB. Parsing accepts at most 32 protocols.
Each protocol must be an HTTP token, optionally followed by `/` and a token
protocol version. Empty members, malformed protocols, control bytes,
oversized values, and too many protocols are rejected. Parse errors do not
reject the raw response: original headers remain available through
`Response::header_value()` and `Response::header_values()`.

These helpers expose HTTP/1 header metadata only. They do not select h2c,
perform `connect()` or `upgrade()` handoff, alter `Connection` handling,
negotiate ALPN, or implement the upgraded protocol.

## Bounded Keep-Alive metadata

`Response::keep_alive()` parses retained HTTP/1 `Keep-Alive` fields into
`KeepAlive` metadata. It returns `Ok(None)` when the header is absent.
Present values combine all `Keep-Alive` fields in wire order into bounded
RFC 2068 metadata. The optional `timeout` delta-seconds and optional `max`
`1*DIGIT` values are parsed as checked unsigned integers; unrecognized
`name=token` parameters are preserved as bounded `KeepAliveExtension`
metadata. `KeepAlive::parse(value)` is available when callers want to
validate one raw field value directly.

Each field value is limited to 64 KiB. Parsing accepts at most 256 parameters
and rejects duplicate recognized parameters, malformed values, overflow,
oversized values, and too many parameters. Parse errors do not reject the raw
response: original headers remain available through
`Response::header_value()` and `Response::header_values()`.

This helper is HTTP/1 header metadata only. `rttp_client` does not change
connection lifetime, connection pooling, keep-alive timers, or HTTP/2
behavior from this accessor.

## Bounded Transfer-Encoding framing metadata

`Response::transfer_encoding()` parses retained HTTP/1 `Transfer-Encoding`
fields into `TransferEncoding` metadata. It returns `Ok(None)` when the header
is absent. Present values combine case-insensitive fields in wire order and
must yield a sole `chunked` coding, matching existing HTTP/1 framing.
`TransferEncoding::parse(value)` is available when callers want to validate
one raw field value directly.

Each field value is limited to 64 KiB. Parsing accepts at most 256 tokens and
rejects empty members, malformed tokens, stacked or non-final `chunked`
codings, combined duplicate fields that are no longer sole `chunked`,
oversized values, and too many tokens. Parse errors do not reject the raw
response: original headers remain available through
`Response::header_value()` and `Response::header_values()`. HTTP/2 continues
to reject `Transfer-Encoding` at decode time.

This helper is framing metadata only. `rttp_client` does not change
`connection_reader`, decode a chunked body from this accessor, negotiate `TE`,
or alter Content-Length handling.

## Bounded preflight request metadata

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

These are declaration helpers only. RTTP does not decide whether a preflight
is needed, read `Access-Control-Allow-*` response fields, apply CORS policy, or
apply Private Network Access policy.

## Bounded HTTP/1.1 Content-Disposition behavior

`Response::content_disposition()` parses a singleton response
`Content-Disposition` header into `ContentDisposition` metadata. It returns
`Ok(None)` when the header is absent and rejects duplicate header fields.
Present values expose the disposition type, ordered parameters, `filename`,
and `filename*`; `ContentDisposition::parse(value)` is available when callers
want to validate a single raw field value directly.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB, the parameter list is limited to 256 entries, disposition type and
unquoted parameter values must be valid HTTP tokens, quoted strings must be
well formed, and `filename*` must be an extended value with valid percent
encoding. Duplicate parameters, malformed quoted strings, CR/LF injection,
oversized values, and too many parameters make
`Response::content_disposition()` return an error while leaving the original
response headers and body available through the ordinary response APIs.

`filename` and `filename*` are preserved as independent parameter values when
both are present. The helper does not decode RFC 5987 extended values or choose
between `filename` and `filename*`; callers that need a filename policy can
inspect `ContentDisposition::filename()`, `filename_ext()`, `parameter()`, or
the ordered `parameters()` list and apply their own precedence rules. The raw
`Content-Disposition` field remains available through `Response::header_value`
and `Response::header_values`, including when typed parsing fails.

The helper is metadata-only. `rttp_client` does not infer automatic download
policy, filesystem paths, MIME sniffing behavior, redirects, retries, cache
behavior, negotiation behavior, or status handling from `Content-Disposition`.

## Bounded HTTP/1.1 Vary behavior

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

The helper is metadata-only. `rttp_client` does not store cache entries, match
stored responses, persist cache keys, replay requests, enforce shared-cache
policy, or issue automatic conditional requests based on `Vary`.

## Bounded NEL response metadata

`Response::nel()` parses the `NEL` response field as bounded W3C Network
Error Logging policy metadata. The policy exposes its required non-negative
`max_age` as `u64`, optional `report_to` name, `include_subdomains` flag, and
`success_fraction`/`failure_fraction` values as checked members; unknown JSON
members are preserved verbatim without policy semantics. The field is handled
as a singleton: duplicate `NEL` fields are rejected. Each field value is
limited to 64 KiB, member counts to 256 per object, nesting depth to 64, and
each decoded string to 64 KiB. Absent metadata returns `Ok(None)`; malformed
JSON, invalid member types, duplicate singleton members, non-finite or
out-of-range fractions, and oversized input return an error while the raw
response headers remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not send network error
reports, persist policy, configure Reporting endpoint groups, or attach
status-code policy from `NEL`.

## Bounded No-Vary-Search metadata

`Response::no_vary_search()` parses one or more `No-Vary-Search` response
fields as bounded Structured Fields dictionary metadata. The typed value
exposes recognized `key-order`, `params`, and `except` members and leaves raw
headers available when helper parsing fails. The helper is metadata-only: it
does not store responses, change cache keys, normalize URLs, replay requests,
apply browser navigation behavior, or enforce shared-cache policy.

## Bounded trailer behavior

Trailer support is explicit and bounded by protocol path. Use
`HttpClient::trailer` to configure request trailer fields. Those fields are
sent for HTTP/1.1 only by `emit_streaming_chunked`; fixed-length HTTP/1.1
requests and buffered `emit` requests do not have an HTTP/1.1 trailer section.
With the `http2` feature enabled, the same configured request trailers are sent
as HTTP/2 trailing HEADERS by both `emit_http2_prior_knowledge` and the
explicit `emit_http2_upgrade` h2c path after request DATA for buffered POST,
PUT, and PATCH requests. The bounded h2c client rejects request trailers for
`http2_extended_connect`, and the bodyless GET, HEAD, DELETE, OPTIONS, and
TRACE paths cannot carry request DATA before trailers.

Response trailers are read through the existing `Response` trailer accessors.
For HTTP/1.1, `rttp_client` exposes only trailers that arrive in a chunked
response after the terminating zero-size chunk. For bounded h2c, peer response
trailers arrive as trailing HEADERS on the active stream and are exposed
through the same accessors. In both request and response directions, trailer
names must be ordinary field names: HTTP/2 pseudo-headers and fields reserved
for connection state, routing, authentication/cookies, transfer framing, or
payload framing are rejected instead of passed to application code.

HTTP/2 trailer support does not make the generic HTTP/1.1 `upgrade()` or
`connect()` handoff paths parse trailers. The h2c Upgrade client path is opt-in
through `emit_http2_upgrade` and replaces the initial HTTP/1.1 exchange with
the bounded HTTP/2 stream model after `101 Switching Protocols`; non-h2c
Upgrade handoffs remain caller-owned bytes.

## Bounded HTTP/2 CONTINUATION behavior

With the `http2` feature enabled, `rttp_client` supports large HTTP/2 header
blocks by fragmenting outbound request HEADERS and trailing HEADERS into an
initial HEADERS frame plus CONTINUATION frames whenever the encoded HPACK block
exceeds the active peer `SETTINGS_MAX_FRAME_SIZE`. It reassembles inbound
response HEADERS or trailing HEADERS that arrive as HEADERS plus CONTINUATION
fragments before HPACK decoding, header-list-size enforcement, and trailer
validation.

The active frame-size limit controls only frame payload size. Legal peer
`SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes become
the outbound fragmentation boundary for request HEADERS, DATA, and trailing
HEADERS. A configured local `http2_max_frame_size` advertises the client's
inbound limit and causes larger inbound frame payloads to be rejected.
`SETTINGS_MAX_HEADER_LIST_SIZE` and `SETTINGS_HEADER_TABLE_SIZE` remain the
separate decoded-metadata and HPACK compression-state limits.

CONTINUATION sequencing is enforced before a response is returned. After a
response HEADERS frame starts a header block without `END_HEADERS`, the client
requires the following frames for that pending block to be CONTINUATION frames
on the same stream until `END_HEADERS`. Orphan CONTINUATION frames,
wrong-stream CONTINUATION frames, interleaved non-CONTINUATION frames, and EOF
before `END_HEADERS` are rejected deterministically.

The same behavior applies to both bounded h2c client entry points:
`emit_http2_prior_knowledge` and explicit `emit_http2_upgrade` after a valid
`101 Switching Protocols` h2c negotiation. Generic HTTP/1.1 `upgrade()` and
`connect()` handoffs, proxies, TLS ALPN, persistent sessions, server push,
extension callbacks, and general multiplexing stay outside this bounded
header-block model.

## Tested protocol coverage

| area | tested coverage | limits |
|------|-----------------|--------|
| HTTP/1.1 response parsing | `Content-Length`, chunked transfer coding, chunk extensions, informational responses, bodyless `204`/`304`, duplicate `Set-Cookie`, and framing ambiguity rejection | Not a complete RFC conformance suite |
| HTTP/1.1 request emission | Origin-form requests, absolute-form proxy requests, `CONNECT`, `HEAD`, fixed bodies, streaming chunked uploads, and `Expect: 100-continue` | SOCKS handshakes are delegated to the `socks` crate |
| Fetch Metadata | `sec_fetch_site`, `sec_fetch_mode`, `sec_fetch_dest`, and `sec_fetch_user` emit bounded `Sec-Fetch-*` request metadata | No browser security policy, automatic header generation, origin validation, navigation policy, or request blocking |
| Preflight request metadata | `origin`, `access_control_request_method`, `access_control_request_headers`, and `access_control_request_private_network` emit bounded `Origin`, `Access-Control-Request-Method`, `Access-Control-Request-Headers`, and `Access-Control-Request-Private-Network` request metadata and reject invalid input before connecting | No automatic preflight decision, `Access-Control-Allow-*` response parsing, CORS policy, or Private Network Access policy |
| Digest preferences | `want_content_digest`, `want_content_digest_with_q`, `want_repr_digest`, and `want_repr_digest_with_q` emit bounded `Want-Content-Digest` and `Want-Repr-Digest` request metadata; server `Request::want_content_digest()`, `HttpRequest::want_content_digest()`, `Request::want_repr_digest()`, and `HttpRequest::want_repr_digest()` parse received preference fields | No algorithm selection, digest computation, response body hash validation, retries, or signing |
| HTTP message signatures | `signature` and `signature_input` emit bounded RFC 9421 request metadata; `Response::signature()` and `signature_input()` parse received fields | No signing, verification, key lookup, covered-component canonicalization, or cryptographic policy |
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Byte ranges | `range`, `range_from`, `range_suffix`, `if_range_etag`, and `if_range_date` emit bounded HTTP/1.1 range request metadata; checked `Response::content_range`, `accept_ranges`, `is_partial_content`, and `is_range_not_satisfiable` expose `Content-Range`, `Accept-Ranges`, `206`, and `416` metadata while preserving raw headers | No Range request generation from `Accept-Ranges`, client-side `If-Range` evaluation, partial response engine, byte serving, content slicing, download resume, automatic retry/replay, cache storage, redirect handling, status-policy behavior, multipart range generation, or automatic cache validation policy |
| Conditional requests | `if_none_match`, `if_match`, `if_modified_since`, and `if_unmodified_since` emit bounded HTTP/1.1 validators; `Response::is_not_modified`, `is_precondition_failed`, typed bounded `etag`, `last_modified`, and `last_modified_date` expose `304`/`412` metadata while preserving raw headers | One ETag validator per helper call, `If-Range` is range-scoped, no cache storage, no automatic revalidation, and no cache-control engine |
| Informational responses and Early Hints | `Response::informational_responses` exposes skipped bounded HTTP/1.1 `1xx` heads, including `103 Early Hints`, with preserved raw headers | `101 Switching Protocols` remains terminal for upgrade handoff; no automatic preload execution, cache policy, redirect/retry/replay, route generation, streaming early-write API, TLS/ALPN behavior, or status-policy behavior |
| Cache-Control, CDN-Cache-Control, Date, Age, and Expires | `Response::cache_control` parses bounded response directives, numeric freshness fields, quoted field-name lists, and extension directives; `Response::cdn_cache_control` parses bounded `CDN-Cache-Control` directives and CDN extension metadata while preserving raw responses on parse errors; `Response::date` parses singleton HTTP-date metadata; `Response::age` parses bounded singleton `Age` metadata through the protocol `Age` type, rejecting duplicate fields, values larger than 64 KiB, and overflowing `u64` delta-seconds; `Response::expires` parses bounded HTTP-date metadata | No cache storage, CDN cache, automatic revalidation, wall-clock freshness calculation, clock-skew correction, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, automatic conditional requests, retry, redirect, scheduling, or status policy |
| Allow | `Response::allow` parses bounded response `Allow` fields into an ordered HTTP method-token list | No fallback method selection, automatic retry/replay, or status-code policy behavior for `405` or `OPTIONS` |
| Client Hints | `Response::accept_ch` and `Response::critical_ch` parse bounded, ordered Client Hints opt-in metadata while preserving raw headers on parse failures | No browser opt-in state, request-header generation, retry, persistence, or Client Hints policy |
| Content-Language | `Response::content_language` parses bounded response `Content-Language` fields into ordered language metadata while preserving raw headers | No automatic language negotiation, locale fallback, variant matching, cache policy, retry, replay, redirect, or status-policy behavior |
| Content-Location | `Response::content_location` and `ContentLocation::parse` parse bounded singleton response `Content-Location` metadata while preserving raw headers | No redirect behavior, cache variant selection, representation replacement, retry/replay, route generation, or status-policy behavior |
| Deprecation | `Response::deprecation` and `Deprecation::parse` parse bounded singleton Structured Fields boolean or date `Deprecation` metadata while preserving raw headers | No Sunset comparison, Link follow, already-deprecated clocks, retries, endpoint selection, or browser/cache policy |
| Content-Type and Content-Encoding | `Response::content_type`/`ContentType::parse` parse bounded singleton `Content-Type` metadata, and `Response::content_encoding`/`ContentEncoding::parse` parse bounded ordered `Content-Encoding` codings while preserving raw headers on parse failures | No MIME sniffing, body decoding, charset transcoding, compression/decompression policy, negotiation, cache policy, redirects, retry/replay, or filesystem serving |
| Connection | `Response::connection`/`Connection::parse` parse bounded HTTP/1 `Connection` tokens, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to keep-alive, `auto_add_connection`, hop-by-hop stripping, or HTTP/2 rejection |
| Keep-Alive | `Response::keep_alive` parses bounded RFC 2068 `Keep-Alive` fields in wire order with `timeout` delta-seconds and `max` `1*DIGIT` values as checked unsigned integers, preserving unrecognized `name=token` parameters as bounded extension metadata and raw headers on parse failures | No connection lifetime management, connection pooling, keep-alive timers, or HTTP/2 behavior changes |
| Transfer-Encoding | `Response::transfer_encoding`/`TransferEncoding::parse` parse bounded HTTP/1 `Transfer-Encoding` fields that must be sole `chunked`, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to HTTP/1 framing decoders, `TE`, Content-Length, chunked body decoding policy, or HTTP/2 decode rejection |
| Content-Disposition | `Response::content_disposition` and `ContentDisposition::parse` parse bounded singleton response `Content-Disposition` metadata into disposition type plus ordered parameters, including preserved `filename` and `filename*` values, while preserving raw headers on parse failures | No automatic download, filesystem path handling, MIME sniffing, redirect behavior, retry/replay, cache behavior, negotiation behavior, or status-policy behavior |
| Vary | `Response::vary` parses bounded response `Vary` fields into wildcard or normalized case-insensitive field-name metadata | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| NEL | `Response::nel` parses the bounded singleton `NEL` field as W3C Network Error Logging policy metadata while preserving raw headers | No network error report sending, policy persistence, Reporting endpoint group configuration, or status-policy behavior |
| No-Vary-Search | `Response::no_vary_search` parses bounded Structured Fields response metadata for query-parameter variance declarations | No cache storage, cache-key matching, URL normalization, navigation behavior, request replay, or shared-cache policy enforcement |
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
`HttpClient::http2_extended_connect(protocol)` request mode for bounded RFC
8441 extended CONNECT request HEADERS. Non-empty buffered request bodies are
sent as DATA frames for the write methods; GET, HEAD, DELETE, OPTIONS, TRACE,
and extended CONNECT requests with bodies are rejected. HEAD, bodyless DELETE,
OPTIONS, TRACE, and extended CONNECT requests do not send request DATA frames,
and any HEAD response DATA frames are consumed without being exposed as a
response body. The client advertises
`SETTINGS_ENABLE_PUSH = 0` in its initial SETTINGS frame so peers see server
push disabled, and it advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` only
when `http2_extended_connect` is used. It validates received
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
bounded h2c handshake. A configured local `http2_max_frame_size` is advertised
only when set, must be in the legal HTTP/2 range of 16,384 through 16,777,215
bytes, and is used to reject inbound frame payloads larger than that active
local limit. Peer-advertised `SETTINGS_MAX_FRAME_SIZE` values outside that
same legal range reject the handshake. Legal peer values become the outbound
frame boundary, so request HEADERS, DATA, and trailing HEADERS are split into
frames no larger than the active peer limit while the client remains a
single-stream prior-knowledge path. Before encoding
request HEADERS, this bounded h2c path strips
HTTP/1.x connection-specific fields: `Connection`, `Keep-Alive`,
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
`Upgrade`, and `WWW-Authenticate`.
HPACK static Huffman strings and bounded large header
blocks are supported, repeated request header and trailer fields can use HPACK
dynamic entries within the peer's advertised `SETTINGS_HEADER_TABLE_SIZE`.
When the peer omits the setting, the request encoder uses the 4,096-byte HPACK
default. When the peer advertises zero, request dynamic indexing is disabled
and request HEADERS and trailers are literal encoded. Peer values above 4,096
bytes are valid, but RTTP caps request dynamic indexing at its 4,096-byte
bounded encoder size. Incoming response
HEADERS and trailers are decoded with the locally advertised
`SETTINGS_HEADER_TABLE_SIZE`: the default is 4,096 bytes unless
`ConfigBuilder::http2_header_table_size` configures another value that fits in
`u32`. Incoming dynamic table size updates may shrink that decoder table,
including to zero, but updates above the local advertised limit are rejected.
These HPACK table limits do not change request metadata list-size enforcement,
response trailer validation, body framing, DATA flow control, or the
single-stream h2c policy. Valid response PRIORITY frames and
HEADERS priority fields are validated and ignored as metadata; malformed
priority metadata is rejected, and no priority scheduling is performed. Inbound
PING without ACK is acknowledged only when it arrives on stream 0 with exactly
8 octets of opaque data; the PING ACK carries that same opaque data. Inbound
PING ACK is ignored for this bounded path. PING with a non-zero stream id or
payload length other than 8 is malformed and rejected. This acknowledgement
path does not add keepalive timers, automatic client- or server-initiated PING
policy, retry/replay, a full session manager, or a full multiplex scheduler.
Unknown frame types, including extension frames, are ignored only after the
h2c handshake in this bounded direct-client path where HTTP/2 permits that
behavior; RTTP does not expose extension callbacks or perform full extension
negotiation. Reserved stream identifier high bits are masked when frames are
parsed or written, which normalizes wire framing but does not add broader
multiplex scheduling or persistent session management.
Server push is outside this bounded client path even when a peer advertises
`SETTINGS_ENABLE_PUSH = 1`: incoming `PUSH_PROMISE` frames are rejected
deterministically instead of creating or tracking push state.
HTTP/1.1 `CONNECT` tunnel handoff remains a separate client path;
prior-knowledge h2c `GOAWAY` is treated as a bounded shutdown signal:
completed responses remain usable, active responses continue only when the
peer's `last-stream-id` includes the stream, and lower boundaries reject the
response deterministically. A `GOAWAY` received before stream 1 is opened is
treated as request refusal and no request HEADERS are sent. RTTP returns that
refusal to the caller instead of retrying on a new connection; callers that
know a request is safe or idempotent must choose any retry policy themselves.
This protocol shutdown boundary is distinct from a transport-level
disconnect, read timeout, write timeout, or TCP reset, which is reported
through the normal socket/error path without an HTTP/2 `last-stream-id`
boundary. `RST_STREAM` is likewise bounded to this
prior-knowledge h2c client path: a reset for the active stream is reported as
response cancellation, while malformed reset frames are rejected
deterministically. RTTP does not expose a public cancellation callback API or
retry the request automatically. Ordinary `CONNECT`, header-configured RFC
8441 `:protocol` metadata, HTTP/1.1 `Upgrade` handoff requests, and proxy
tunneling are rejected before a client socket is opened. The explicit
`http2_extended_connect(protocol)` mode emits `:method CONNECT` with
`:protocol`, `:scheme`, `:authority`, and `:path`, then returns the peer's
HTTP/2 response through the normal `Response` API. It remains a bounded
single-stream request/response path without request bodies, request trailers,
or upgraded socket handoff. HTTP/1.1 `CONNECT` tunnel handoff and `Upgrade`
remain separate client handoff paths; this h2c path does not provide full
WebSocket-over-h2, proxy h2, TLS ALPN, tunnel handoff, persistent multiplex
sessions, general tunnel scheduling, or full RFC 8441 support. Extension
callback APIs, full extension negotiation, external h2 integration, connection
pooling, automatic retry, server push, full stream state machines, and full
HTTP/2 features such as unbounded multiplex scheduling, general multiplexing,
and priority scheduling are not part of that bounded prior-knowledge client
path. RTTP does not expose a dynamic policy API for changing h2c frame-size or
metadata limits at runtime.

### Bounded RFC 8441 extended CONNECT

`HttpClient::http2_extended_connect(protocol)` is the supported client entry
point for RFC 8441 metadata on RTTP's bounded HTTP/2 path. It is prior-knowledge
h2c only, runs over the direct `socket2` TCP transport used by
`emit_http2_prior_knowledge`, and opens at most one request stream. The client
advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` only for this explicit mode,
then emits `:method CONNECT` with required `:protocol`, `:scheme`,
`:authority`, and `:path` pseudo-header metadata. The peer's result is returned
as a normal `Response`.

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

This is not a full WebSocket-over-h2 implementation, arbitrary tunnel
scheduler, upgraded socket handoff, or general multiplexing guarantee beyond
the bounded single-stream request/response path. It does not alter HTTP/1.1
`CONNECT` tunnel handoff or `Upgrade` semantics. Ordinary `CONNECT`,
header-configured `:protocol` metadata, HTTP/1.1 `Upgrade` handoff requests,
proxies, request bodies, and request trailers are rejected for this path.

## Client Hints response metadata

`Response::accept_ch()` and `Response::critical_ch()` parse `Accept-CH` and
`Critical-CH` response fields into `AcceptCh` and `CriticalCh` metadata. Both
helpers combine case-insensitive repeated fields in wire order and return
`Ok(None)` when the respective field is absent. Parsed client-hint tokens are
available through `client_hints()`; malformed, empty, oversized, or
overlong-field-set values return an error while the raw response headers remain
available through `Response::header_value()` and `Response::header_values()`.

These helpers are observation-only. `rttp_client` does not select or send
client hints, persist an `Accept-CH` opt-in, retry after `Critical-CH`, or add
any automatic client-hint negotiation behavior.

## Examples

```rust,no_run
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

```rust,no_run
use rttp_client::HttpClient;
use rttp_client::types::Proxy;

let response = HttpClient::new()
  .post()
  .url("http://127.0.0.1:8080/messages")
  .content_type("application/json")
  .raw(r#"{"from":"rttp"}"#)
  .proxy(Proxy::http("127.0.0.1", 1081))
  .emit()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

```rust,no_run
# #[cfg(feature = "async")]
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use rttp_client::HttpClient;

let response = HttpClient::new()
  .get()
  .url("http://127.0.0.1:8080/health")
  .rasync()
  .await?;
# Ok(())
# }
```
