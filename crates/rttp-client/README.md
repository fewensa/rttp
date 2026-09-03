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

Buffered responses automatically decode a fully supported `Content-Encoding`
stack of `gzip` and zlib-wrapped `deflate`, including mixed and repeated
layers, in reverse header order. Successful decoding removes stale
`Content-Encoding` and `Content-Length` from the parsed header view and
from `Response::content_encoding()` / `Response::content_length()`;
`Response::binary()` still retains the original capture. Unknown, mixed
unsupported, `identity`, or parse-invalid stacks leave the original headers
and body unchanged. Decoding is atomic: a malformed layer fails the
response without exposing partial plaintext. Empty bodies are not decoded.
`max_buffered_response_body_bytes` bounds each decoded layer. Raw
non-zlib `deflate` is not supported. Streaming bodies and async HTTP/2
stay out of this buffered path.

## Bounded Max-Forwards diagnostics

`HttpClient::max_forwards(value)` sets a `Max-Forwards` request header for
application-selected `TRACE` or `OPTIONS` diagnostics through the shared
protocol `MaxForwards` type. The helper accepts a singleton `1*DIGIT` hop
count that fits in the `u32` range (`0` through `4294967295`), trims HTTP OWS,
and rejects negative, fractional, empty, oversized (over 64 KiB), overflowing,
duplicate, and control-byte values before a socket is opened. It only
validates and emits the canonical integer form: RTTP does not select a
diagnostic policy, route through proxies, decrement the value, or retry the
request. Callers needing an unusual value can retain full raw-header control
with `header(("Max-Forwards", "..."))`.

## Bounded WebDAV Depth metadata

`HttpClient::depth(value)` sets a WebDAV `Depth` request header through the
shared protocol `Depth` type. The helper accepts the singleton values `0`,
`1`, and `infinity`, trims HTTP OWS, normalizes `infinity` to lowercase, and
rejects empty, unsupported, comma-list, oversized (over 64 KiB), duplicate,
and control-byte values before a socket is opened. It only validates and
emits the canonical metadata value: RTTP does not traverse resources, select
WebDAV methods, or enforce method policy. Callers needing an unusual value
can retain full raw-header control with `header(("Depth", "..."))`.

## Bounded WebDAV Lock-Token metadata

`HttpClient::lock_token(value)` sets a WebDAV `Lock-Token` request header
through the shared protocol `LockToken` type. The helper accepts exactly one
angle-bracketed absolute URI, trims HTTP OWS, and rejects empty, unbracketed,
relative, comma-list, extra-bracket, oversized (over 64 KiB), duplicate, and
control-byte values before a socket is opened. It only validates and emits
the trimmed coded URL, including `<` and `>`: RTTP does not create, refresh,
release, persist, or enforce locks. The token is redacted from typed `Debug`
and builder error text. Callers needing an unusual value can retain full
raw-header control with `header(("Lock-Token", "..."))`.

Responses expose `Response::lock_token()` to parse one bounded `Lock-Token`
response field. Absent fields return `Ok(None)`. Malformed, duplicate, or
oversized values return a response error while raw headers remain available.
Parse errors do not include the token value.

## Bounded WebDAV Destination metadata

`HttpClient::destination(value)` sets a WebDAV `Destination` request header
through the shared protocol `Destination` type. The helper accepts one
absolute URI, trims HTTP OWS, preserves the trimmed URI string, and rejects
empty, relative, scheme-relative, malformed, oversized (over 64 KiB),
duplicate, injection, and control-byte values before a socket is opened. It
only validates and emits the preserved metadata value: RTTP does not resolve
the destination, normalize URI components, authorize access, select WebDAV
methods, or copy or move resources. Callers needing an unusual value can
retain full raw-header control with `header(("Destination", "..."))`.

## Bounded WebDAV Timeout metadata

`HttpClient::timeout(value)` sets a WebDAV `Timeout` request header through
the shared protocol `Timeout` type. The helper accepts ordered `Second-n` and
`Infinite` alternatives, trims HTTP OWS, normalizes members to lowercase, and
rejects malformed, overflowing, duplicate, oversized, too-many-member, and
control-byte values before a socket is opened. It only validates and emits the
canonical metadata value: RTTP does not create locks, refresh locks, or select
an application timeout. Callers needing an unusual value can retain full
raw-header control with `header(("Timeout", "..."))`.

## Bounded If-Schedule-Tag-Match metadata

`HttpClient::if_schedule_tag_match(value)` sets an `If-Schedule-Tag-Match`
request header through the shared protocol `IfScheduleTagMatch` type, which
reuses the shared `EntityTag` representation. The helper accepts one
entity-tag-shaped schedule validator such as `"sched-17"` or `W/"sched-17"`,
trims HTTP OWS, and rejects empty, malformed, wildcard, comma-list, duplicate,
injection, control-byte, and oversized (over 64 KiB) values before a socket is
opened. It only validates and emits the canonical entity tag: RTTP does not
compare the validator to stored calendar state, inspect calendars, or apply
scheduling policy. Callers needing an unusual value can retain full raw-header
control with `header(("If-Schedule-Tag-Match", "..."))`.

## Bounded Schedule-Tag response metadata

`Response::schedule_tag()` parses one bounded `Schedule-Tag` response field
through the shared protocol `ScheduleTag` type, which reuses the shared
`EntityTag` representation. `Response::schedule_tag_value()` returns the raw
field when present. The helper accepts one entity-tag-shaped schedule validator
such as `"sched-17"` or `W/"sched-17"`, trims HTTP OWS, and rejects empty,
malformed, wildcard, comma-list, duplicate, injection, control-byte, and
oversized (over 64 KiB) values while leaving raw headers available through the
ordinary response header accessors. RTTP does not generate calendar versions,
compare validators, inspect calendars, or apply scheduling policy.

## Bounded WebDAV Overwrite metadata

`HttpClient::overwrite(value)` sets a WebDAV `Overwrite` request header
through the shared protocol `Overwrite` type. The helper accepts the
singleton tokens `T` and `F`, trims HTTP OWS, and rejects empty, lowercase,
comma-list, oversized (over 64 KiB), duplicate, and control-byte values
before a socket is opened. It only validates and emits the canonical metadata
value: RTTP does not overwrite destination resources, apply the RFC 4918
default `T` when the header is absent, or enforce WebDAV policy. Callers
needing an unusual value can retain full raw-header control with
`header(("Overwrite", "..."))`.

## Bounded WebDAV If metadata

`HttpClient::if_header(value)` sets an RFC 4918 section 10.4 WebDAV `If`
request header through the shared protocol `If` type. The helper accepts
condition lists that are entirely untagged like
`(<opaquelocktoken:...>) (Not <DAV:no-lock>)` or entirely tagged like
`<http://example.test/src> (<opaquelocktoken:...>)`, trims HTTP OWS, and
preserves list order, resource tags, `Not`, state tokens, and entity tags.
It rejects empty or unterminated lists, mixed tagged and untagged
productions, relative or fragment-bearing URIs, malformed state tokens,
resource tags, and entity tags, duplicate fields, control-byte and
obs-text values, and values over 64 KiB, 32 lists, or 256 conditions before
a socket is opened. It only validates and emits the canonical field text:
RTTP does not evaluate locks, entity tags, or other resource state, and it
does not generate precondition outcomes such as 412 Precondition Failed.
State tokens are redacted from typed `Debug`. Callers needing an unusual
value can retain full raw-header control with `header(("If", "..."))`.

## Bounded WebDAV DAV response metadata

`Response::dav()` parses WebDAV `DAV` response fields through the shared
protocol `Dav` type. The helper preserves wire order across repeated fields,
accepts standard classes `1`, `2`, and `3`, extension tokens, and
`<absolute-URI>` Coded-URLs, and rejects malformed, duplicate, oversized,
aggregate-oversized, or over-32-member values while raw response headers remain
available through the ordinary header accessors. It exposes response metadata
only: RTTP does not infer, negotiate, or enforce WebDAV feature support from
the header.

Workspace HTTP/1.1 and h2c integration tests cover a metadata-only WebDAV
matrix for `Depth`, `Destination`, `Overwrite`, `Timeout`, `Lock-Token`,
`If`, `DAV`, `If-Schedule-Tag-Match`, and `Schedule-Tag`, including valid
roundtrips, malformed and duplicate rejection, bounds, raw-header
observability, and `Lock-Token`/`If` redaction. These helpers still do not
store resources, create locks, or enforce WebDAV method policy.

## Bounded Idempotency-Key metadata

`HttpClient::idempotency_key(value)` sets an `Idempotency-Key` request header
for application-generated idempotency keys through the shared protocol
`IdempotencyKey` type. The helper accepts a singleton opaque visible-ASCII key
up to 64 KiB, trims HTTP OWS, and rejects empty, space-containing,
control-byte (including CR/LF/NUL and obs-text), duplicate, and oversized
values before a socket is opened. It validates and emits the trimmed key
unchanged: RTTP does not retry requests, store or compare keys across
requests, or apply application idempotency policy. The key is redacted from
typed `Debug` and builder error text. Callers needing an unusual value can
retain full raw-header control with `header(("Idempotency-Key", "..."))`.

## Bounded Sec-WebSocket-Key metadata

`HttpClient::sec_websocket_key(value)` sets a `Sec-WebSocket-Key` request
header for application-generated handshake nonces through the shared protocol
`SecWebSocketKey` type. The helper accepts a singleton RFC 4648 section 4
base64 encoding of exactly 16 nonce bytes, trims HTTP OWS, and rejects empty,
interior-whitespace, non-base64, URL-safe or unpadded, wrong-decoded-length,
control-byte (including CR/LF/NUL and obs-text), duplicate, and oversized
values before a socket is opened. It validates and emits the trimmed encoded
nonce unchanged: RTTP does not perform an HTTP upgrade, generate a random
nonce, or implement WebSocket frames. The nonce is redacted from typed
`Debug` and builder error text.
Callers needing an unusual value can retain full raw-header control with
`header(("Sec-WebSocket-Key", "..."))`.

Responses expose `Response::sec_websocket_accept()` to parse one bounded
`Sec-WebSocket-Accept` response field and
`Response::verify_sec_websocket_accept(&key)` to compare it against the RFC
6455 GUID plus SHA-1 and base64 derivation from a validated
`SecWebSocketKey`. The RFC example key `dGhlIHNhbXBsZSBub25jZQ==` verifies
against `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`. Accept values are redacted from typed
`Debug`, and parse errors do not include key or accept material.

## Bounded Sec-WebSocket-Version metadata

`HttpClient::sec_websocket_version(value)` sets a `Sec-WebSocket-Version`
request header through the shared protocol `SecWebSocketVersion` type. The
helper accepts one or more RFC 6455 version tokens (`0` through `299`
without leading zeros), trims HTTP OWS, requires numeric descending order for
multi-member lists such as `13, 8, 7`, and rejects empty members, non-decimal
tokens, duplicates, over-limit member counts, control-byte (including
CR/LF/NUL and obs-text), and oversized values before a socket is opened. It
replaces any existing same-name field with the canonical comma-separated
value. `Response::sec_websocket_version()` parses the same representation on
received responses, including application-owned rejection responses that
declare supported versions. These helpers only declare or parse metadata:
RTTP does not perform a WebSocket handshake, emit `Connection: Upgrade`,
compute `Sec-WebSocket-Accept`, negotiate versions, or switch protocols.
Callers needing an unusual value can retain full raw-header control with
`header(("Sec-WebSocket-Version", "..."))`.

## Bounded Sec-WebSocket-Protocol metadata

`HttpClient::sec_websocket_protocol(value)` sets a `Sec-WebSocket-Protocol`
request header through the shared protocol `SecWebSocketProtocol` type. The
helper accepts one or more RFC 6455 section 11.3.4 `token` offers such as
`chat, superchat, graphql-ws` in client preference order, trims HTTP OWS,
preserves token spelling, and rejects empty members, malformed tokens,
parameters, slashes, case-sensitive duplicates, over-limit member counts,
control-byte (including CR/LF/NUL and obs-text), and oversized values before
a socket is opened. It replaces any existing same-name field with the
canonical comma-separated value. `Response::sec_websocket_protocol()` parses
the same representation on received responses as a selection singleton: a
successful handshake carries exactly one token, and a multi-token value
returns a parse error while raw headers remain available. These helpers only
declare or parse metadata: RTTP does not perform a WebSocket handshake, emit
`Connection: Upgrade`, choose an application subprotocol, or switch
protocols. Applications own the selection decision. Callers needing an
unusual value can retain full raw-header control with
`header(("Sec-WebSocket-Protocol", "..."))`.

## Bounded Sec-WebSocket-Extensions metadata

`HttpClient::sec_websocket_extensions(value)` sets a
`Sec-WebSocket-Extensions` request header through the shared protocol
`SecWebSocketExtensions` type. The helper accepts ordered RFC 6455 extension
offers such as `permessage-deflate; client_max_window_bits, x-test`, preserves
ordered parameters, supports token and quoted-string parameter values, rejects
duplicate extension tokens and duplicate parameter names, and enforces the
64 KiB and 32-member bounds before a socket is opened. It replaces any
existing same-name field with the canonical value.
`Response::sec_websocket_extensions()` parses received responses as a
selection singleton: a successful response carries exactly one extension
member, and a multi-extension value returns a parse error while raw headers
remain available. These helpers only declare or parse metadata: RTTP does not
activate compression, negotiate extensions, emit `Connection: Upgrade`, or
switch protocols. Applications own all extension behavior. Callers needing an
unusual value can retain full raw-header control with
`header(("Sec-WebSocket-Extensions", "..."))`.

## Bounded W3C Trace Context metadata

`HttpClient::traceparent(value)` and `HttpClient::tracestate(value)` validate
and emit W3C Trace Context request metadata through the shared protocol
`TraceParent` and `TraceState` types, replacing existing same-name fields
before a socket is opened. `traceparent` accepts version `00` fixed-width
lowercase identifiers and flags, rejects unsupported versions, version `ff`,
malformed fields, duplicate fields, and all-zero trace or parent identifiers.
`tracestate` preserves member order while rejecting malformed members,
duplicate keys, more than 32 members, values over 512 bytes, and oversized
keys or member values.

Trace context fields are redacted from typed `Debug` and builder error text.
These helpers only declare request metadata: RTTP does not create trace
identifiers, decide sampling, select a tracing backend, or automatically
propagate context between requests.

## Bounded W3C Baggage metadata

`HttpClient::baggage(value)` validates and emits W3C Baggage request metadata
through the shared protocol `Baggage` type, replacing any existing `baggage`
field before a socket is opened. It preserves member order while rejecting
malformed keys, values, or properties, duplicate member keys, more than 180
members, members over 4096 bytes, and combined values over 8192 bytes.
Application keys and values are not decoded or interpreted.

Baggage fields are redacted from typed `Debug` and builder error text. These
helpers only declare request metadata: RTTP does not store request context,
select a tracing backend, or automatically propagate baggage between
requests.

## Bounded CDN-Loop forwarding metadata

`HttpClient::cdn_loop(value)` validates and emits RFC 8586 `CDN-Loop` request
metadata through the shared protocol `CdnLoop` type, combining any existing
`CDN-Loop` field with the new member in wire order before a socket is opened.
Each field value, the combined raw field set including `", "` separator
overhead, and the combined serialized value are bounded to 64 KiB, the
combined member count is bounded to 256, and each member is bounded to 32
parameters. Malformed identifiers, valueless or duplicate parameters, empty
members, and bound violations are rejected before connecting.

The helper only declares forwarding metadata: RTTP does not insert a local CDN
identifier, append the field on every outbound request, reject requests
because an identifier is already present, or treat `CDN-Loop` as hop-by-hop.

## Bounded Via forwarding metadata

`HttpClient::via(value)` validates and emits HTTP `Via` request metadata
through the shared protocol `Via` type, combining any existing `Via` field
with the new hops in wire order before a socket is opened. Each field value,
the combined raw field set including `", "` separator overhead, and the
combined serialized value are bounded to 64 KiB, and the combined member
count is bounded to 256. Malformed received-protocol, received-by, or
comment syntax, empty members, and bound violations are rejected before
connecting.

The helper only declares caller-supplied hop metadata: RTTP does not append
a local hop, remove existing hops, or change proxy or tunnel policy.

## Bounded X-Forwarded compatibility metadata

`HttpClient::x_forwarded_for(value)`, `x_forwarded_host(value)`, and
`x_forwarded_proto(value)` validate and emit bounded `X-Forwarded-For`,
`X-Forwarded-Host`, and `X-Forwarded-Proto` request metadata through the
shared protocol types. Repeated helper calls combine existing same-name fields
with the new values in wire order before a socket is opened.

`X-Forwarded-For` accepts ordered IP node values and `unknown`,
`X-Forwarded-Host` accepts ordered host authorities, and `X-Forwarded-Proto`
accepts ordered URI scheme tokens. Each field family is bounded to 64 KiB per
field value, 64 KiB for the combined raw field set including `", "` separator
overhead, 64 KiB for serialized output, and 256 members. Malformed values,
empty members, control-byte injection, and bound violations are rejected
before connecting.

These helpers only emit caller-supplied compatibility metadata. RTTP does not
trust, rewrite, or enforce forwarded identity, select a client address, change
routing, redirect, upgrade, or choose a trusted proxy set. Applications that
use these fields must choose and enforce their own trusted proxies.

## Bounded HTTP/1.1 byte ranges

`HttpClient` includes helpers for the single-range `bytes` forms RTTP keeps
bounded: `range(start, end)` emits `Range: bytes=start-end`,
`range_from(start)` emits `Range: bytes=start-`, and `range_suffix(length)`
emits `Range: bytes=-length`. `ranges` emits one canonical
`Range: bytes=...` header from closed (`ByteRangeSpec::FromTo { end: Some(...) }`),
open-ended (`ByteRangeSpec::FromTo { end: None }`), and suffix
(`ByteRangeSpec::Suffix`) members, replacing any prior `Range` field. The
helpers reject inverted closed ranges, a zero suffix length, an empty set, and
more than 32 members before a socket is opened. They are request-header
helpers; manual `Range` headers remain available through
`header(("Range", "..."))` when callers need behavior outside the helper
validation.

```rust
use rttp_client::{ByteRangeSpec, HttpClient};

HttpClient::new()
  .get()
  .url("http://example.test/archive")
  .ranges([
    ByteRangeSpec::FromTo {
      start: 0,
      end: Some(2),
    },
    ByteRangeSpec::FromTo {
      start: 10,
      end: None,
    },
    ByteRangeSpec::Suffix { length: 4 },
  ])?
  .emit()?;
```

Partial-content responses are exposed through the normal `Response` API.
`Response::is_partial_content()` identifies `206 Partial Content`. A
single-range `206` includes a top-level `Content-Range` field such as
`bytes 10-19/200`; `Response::content_range()` parses that field into the
shared checked protocol `ContentRange` with `unit`, `start`, `end`, and
`complete_length` accessors. A multi-range `206` is `multipart/byteranges`:
there is no top-level `Content-Range`, `Content-Type` carries a boundary,
`Content-Length` matches the framed body, and the body contains per-part
`Content-Range` headers plus a closing delimiter in request-member order.
Unsatisfiable members are omitted by the server; if every member is
unsatisfiable the response is `416`. Invalid or duplicate `Content-Range`
metadata returns a response error from the typed helper while raw headers
remain preserved. `Response::is_range_not_satisfiable()` identifies
`416 Range Not Satisfiable`; an unsatisfied `Content-Range` such as
`bytes */200` is exposed with no `start` or `end` and
`ContentRange::is_unsatisfied() == true`. Response bodies and headers are still
preserved normally for both `206` and `416`. The client does not decode
multipart parts into structured ranges.

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

RTTP does not synthesize multipart request bodies, generate `Range` requests
from `Accept-Ranges`, evaluate `If-Range`, retry range requests, store cached
responses, apply automatic cache validation policy, resume downloads, slice
content, or choose status handling on the client side. Multiple ranges are
emitted by `HttpClient::ranges`; any server response is then parsed as an
ordinary HTTP response.

## Bounded HTTP/1.1 conditional requests

`HttpClient` includes bounded helpers for the common conditional request
validators: `if_none_match(etag)`, `if_match(etag)`,
`if_modified_since(http_date)`, and `if_unmodified_since(http_date)`. The ETag
helpers accept one validator at a time: `*`, a strong tag such as `"abc"`, or a
weak tag such as `W/"abc"`. They reject comma-separated lists and obviously
malformed tag syntax before opening a socket; callers that need validator
lists or non-helper behavior can still use the generic `header` API.

The date helpers validate the supplied value as one HTTP-date through the
shared protocol `IfModifiedSince` and `IfUnmodifiedSince` types and emit the
canonical IMF-fixdate form. They reject empty, malformed, oversized (over
64 KiB), and control-byte values before a socket is opened. They do not choose
a validator policy for the request. `If-Range` uses its own helpers because it
is intended to compose with range requests and permits only strong entity tags
or HTTP-date validators.

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
when present, `Response::schedule_tag()` parses one bounded `Schedule-Tag`
response field into the protocol `ScheduleTag` type, and
`Response::last_modified()` returns the response
`Last-Modified` field when present. Malformed, oversized, or duplicate `ETag`
or `Schedule-Tag` fields make the typed helper return an error while raw
values remain available through `Response::etag_value()`,
`Response::schedule_tag_value()`, `Response::header_value()`, and
`Response::header_values()`. `Response::last_modified_date()` parses
`Last-Modified` through the shared response HTTP-date primitive: it returns
`Ok(None)` when the header is absent, returns `SystemTime` for a valid
singleton, and returns an error for malformed, duplicate, control-byte, or
oversize values. The raw field stays available through `last_modified()` and
the ordinary header accessors. A `304` response is treated as bodyless even if misleading framing
headers are present, so the connection remains framed for the next response.
`412` is surfaced as a normal response status and body/framing rules remain the
server's responsibility.

`Response::delta_base()` parses one bounded `Delta-Base` response field into
the shared protocol `DeltaBase` type, which reuses `EntityTag` for the base
validator. `Response::delta_base_value()` returns the raw field when present.
Malformed, duplicate, comma-list, or oversized values return a parse error
while raw headers remain available. `rttp_client` does not locate cached
entities, compare base validators, or apply deltas automatically.

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

## Bounded Cache-Status response metadata

`Response::cache_status()` parses one or more response `Cache-Status` header
fields into `CacheStatus`. The parser combines repeated fields in wire order
as an RFC 9211 / RFC 8941 list of cache identifiers and parameters, including
typed `hit`, `fwd`, `fwd-status`, `ttl`, `stored`, `collapsed`, `key`, and
`detail` values plus well-formed extension parameters.

The helper uses these bounds: 64 KiB per field value, at most 256 members, at
most 256 parameters per member, and 64 KiB per parameter value. Invalid
`Cache-Status` metadata makes the helper return an error without discarding
the raw response headers or body. An absent header returns `Ok(None)`.

This is response metadata only. `rttp_client` does not store cache entries,
compute freshness, revalidate, select endpoints, retry, or alter response
acceptance from `Cache-Status`.

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

## Bounded Surrogate-Control response metadata

`Response::surrogate_control()` parses one or more response
`Surrogate-Control` header fields into `SurrogateControl`. The parser preserves
directive order, including extension directives, and exposes each directive
token name plus its optional parsed value.

Validation is bounded and metadata-only: 64 KiB per field value, 64 KiB across
the parsed header set, at most 256 parsed directives, valid HTTP tokens for
directive names and unquoted values, well-formed quoted strings, and no
duplicate directive names case-insensitively across fields. Invalid
`Surrogate-Control` metadata makes the helper return an error without
discarding the raw response headers or body.

`rttp_client` does not create a CDN cache, compute freshness, evaluate
surrogate keys, translate directives into `Cache-Control`, enforce
shared-cache policy, retry, replay, redirect, or alter response acceptance from
`Surrogate-Control`.

## Bounded HTTP/1.1 Date, Age, Expires, and Last-Modified behavior

`Response::date()` parses the response `Date` header through the shared
protocol `ResponseDate` singleton HTTP-date primitive. The helper returns
`Ok(None)` when the header is absent, returns `SystemTime` when the value is
present and valid, and returns an error for malformed, duplicate, control-byte,
or oversize values.

`Response::age()` parses the response `Age` header through the protocol `Age`
type as HTTP/1.1 delta-seconds metadata. The helper returns `Ok(None)` when the
header is absent, returns the non-negative decimal value as `u64` when it is
present and valid, and returns an error for empty, signed, fractional,
non-numeric, comma-list, overflowing, duplicate, or oversize values.
Surrounding SP and HTAB are trimmed as optional whitespace. Each field value
is bounded to 64 KiB, and the accepted numeric bound is the `u64`
delta-seconds range: `0` through `u64::MAX`.

`Response::expires()` parses the response `Expires` header through the shared
protocol `ResponseExpires` primitive. `Response::last_modified_date()` parses
`Last-Modified` through `ResponseLastModified` while
`Response::last_modified()` keeps exposing the raw field. These HTTP-date
helpers return `Ok(None)` when the header is absent, return `SystemTime` for
valid values, and reject malformed, duplicate, control-byte, and oversize
values. Supported HTTP-date forms are IMF-fixdate, obsolete RFC 850 dates, and
asctime dates; typed formatting emits canonical IMF-fixdate.

Malformed helper values do not reject the raw response. The original `Date`,
`Age`, `Expires`, and `Last-Modified` fields remain available through
`header_value`, `header_values`, and the other raw header accessors. These helpers expose
metadata only; `rttp_client` does not calculate freshness, correct clock skew,
validate cache state against wall-clock time, store responses, match stored
responses, revalidate responses, apply shared-cache policy, issue automatic
conditional requests, retry, redirect, schedule work, or choose status policy.

## Bounded Memento-Datetime behavior

`Response::memento_datetime()` parses the response `Memento-Datetime` header
through the protocol `MementoDatetime` type as one singleton IMF-fixdate.
The helper returns `Ok(None)` when the header is absent, returns
`MementoDatetime` when the value is present and valid, and returns an error
for empty, malformed, control-byte, duplicate, or oversize values. Each field
value is bounded to 64 KiB. Surrounding SP and HTAB are trimmed as optional
whitespace.

Malformed helper values do not reject the raw response. The original
`Memento-Datetime` field remains available through `header_value` and
`header_values`. This helper exposes metadata only; `rttp_client` does not
select an archival representation, negotiate `Accept-Datetime`, implement
TimeGate behavior, retry, or change transport handling. The matching request
metadata helper is `accept_datetime`, which parses the same HTTP-date
instants; the two helpers still do not negotiate with each other.

## Bounded Accept-Datetime request metadata

`HttpClient::accept_datetime(http_date)` sets an `Accept-Datetime` request
header through the shared protocol `AcceptDatetime` type. The helper accepts
one HTTP-date in IMF-fixdate, obsolete RFC 850, or asctime form, trims HTTP
OWS, and emits the canonical IMF-fixdate form. It rejects empty, malformed,
oversized (over 64 KiB), control-byte, and comma-joined values before a socket
is opened, and a second `accept_datetime` call replaces the existing field.
The parsed instant matches `Response::memento_datetime()` for the same
HTTP-date.

This helper declares metadata only; `rttp_client` does not select an archived
representation, implement TimeGate behavior, add `Vary`, alter cache policy,
or change conditional-request handling. Callers needing an unusual value can
retain full raw-header control with `header(("Accept-Datetime", "..."))`.

## Bounded HTTP/1.1 Retry-After behavior

`Response::retry_after()` parses a single response `Retry-After` header through
the protocol `RetryAfter` type as either HTTP-date metadata or non-negative
delta-seconds. It returns `Ok(None)` when the header is absent. Present values
are exposed as `RetryAfter`, with `delta_seconds()` returning `u64` for the
delta form and `http_date()` returning `SystemTime` for the date form.

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

## Bounded HTTP/1.1 Service-Worker-Allowed behavior

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
raw headers and parsing only when requested. It is metadata-only: `rttp_client`
does not register service workers, evaluate service-worker scope, resolve the
value against a script URL, or apply application routing policy.

## Bounded HTTP/1.1 Content-DPR behavior

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

The helper is observation-only. `rttp_client` does not rescale images, send
request DPR, apply Client Hints policy, retry, replay, redirect, or change
transport from `Content-DPR`.

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

## Bounded Accept-Charset request metadata

`rttp-protocol` owns the shared `Accept-Charset` primitive. Client helpers
format through that type.

`HttpClient::accept_charset()` appends a validated request charset range,
while `accept_charset_with_q()` accepts an HTTP q-value from `0` through `1`
with at most three fractional digits. The helpers emit one comma-separated
`Accept-Charset` field and reject invalid charset tokens, q-values,
duplicates, oversized values, and more than 32 ranges before a connection is
opened.

These helpers declare request metadata only. They do not negotiate, transcode,
decode bodies, sniff MIME types, or select a response charset.

## Bounded A-IM request metadata

`rttp-protocol` owns the shared `A-IM` primitive. Client helpers format
through that type.

`HttpClient::a_im()` appends a validated instance-manipulation token, while
`a_im_with_q()` accepts an HTTP q-value from `0` through `1` with at most
three fractional digits. `a_im_value()` appends a validated field value that
may include q-values and extension parameters. The helpers emit one
comma-separated `A-IM` field and reject invalid tokens, q-values, parameters,
duplicates, oversized values, more than 16 parameters per member, and more
than 32 members before a connection is opened.

These helpers declare request metadata only. They do not select a preferred
instance manipulation or apply delta encodings.

## Bounded IM response metadata

`Response::im()` parses one or more response `IM` header fields into `Im`
metadata, the shared protocol parser also used by the server facade. It
returns `Ok(None)` when the header is absent. Present values expose ordered
`members()` with `token()` and `parameters()` per member and `header_value()`
for serialization.

The helper is bounded and validation-oriented. Each header field value is
limited to 64 KiB, the parsed header set is limited to 32 members, and each
member is limited to 16 parameters. Empty members, invalid tokens or
parameters, duplicates across all parsed
header fields, and oversized or over-limit values return an error while the
original response remains usable: raw `IM` fields stay available through
`Response::header_value()`, `Response::header_values()`, and the other
response metadata helpers.

RTTP does not decode, invert, or apply instance manipulations, and it does
not require or synthesize the `226 IM Used` status.

## Bounded Negotiate request metadata

`rttp-protocol` owns the shared RFC 2295 `Negotiate` primitive. Client
helpers format through that type.

`HttpClient::negotiate(value)` validates and emits one `Negotiate` field,
replacing any existing same-name field before a socket is opened. The value
must be an ordered comma-separated list of `trans`, `vlist`, `guess-small`,
`*`, `major.minor` remote variant selection algorithm versions, or
`token[=token]` extension directives. Flags are normalized to lowercase,
versions are normalized to `major.minor`, and duplicate directives are
rejected: at most one of each flag and `*`, one occurrence of each version
pair, and one extension per case-insensitive name. The helper rejects invalid
tokens, valued flags or versions, empty or trailing comma members,
duplicates, oversized values, and more than 32 members before a connection is
opened.

These helpers declare request metadata only. They do not select a variant,
run transparent content negotiation, or change cache selection.

## Bounded Accept-Encoding request metadata

`rttp-protocol` owns the shared `Accept-Encoding` primitive. Client helpers
format through that type.

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
values, q-values, duplicate parameters, duplicate q-values, bounds, and header
formatting are validated by the shared `rttp-protocol` Accept primitive; the
client keeps the existing 32-helper-range limit and rejects oversized values,
excessive media ranges, or invalid existing raw `Accept` fields before a helper
append opens a connection. Helper q-value arguments preserve the existing
facade boundary by accepting legacy empty fractional forms such as `0.` and
`1.` while rejecting surrounding whitespace.

The helpers emit one comma-separated `Accept` field and do not choose a
response representation. `header(("Accept", value))` remains available for
media ranges or extensions outside this bounded helper API.

## Bounded Expect request metadata

`rttp-protocol` owns the shared `Expect` primitive. `HttpClient::expect_continue()`
formats that type's standardized singleton as `Expect: 100-continue`. It is
metadata only: the client does not delay the request body or wait for an
interim response. Raw `header(("Expect", value))` remains available for
extension values outside the typed helper.

## Bounded Authorization request metadata

`HttpClient::authorization(scheme, credentials)` emits one `Authorization`
field after validating its HTTP-token scheme and non-empty credential value,
with the shared `rttp-protocol` request authorization primitive and a 64 KiB
bound. Credentials reject CR, LF, NUL, and other control-byte injection. Use
`header(("Authorization", value))` as the raw escape hatch for custom scheme
syntax. Credential interpretation remains application-owned: RTTP does not
validate individual schemes, store or refresh credentials, process challenges,
retry, or forward credentials on redirects.

## Bounded HTTP request control metadata

`HttpClient::te()`, `te_with_q()`, and `te_trailers()` build a bounded `TE`
field, validating each member and the combined field through the shared
protocol-owned `rttp-protocol` `Te` type. `HttpClient::prefer()` and
`prefer_with_value()` build a bounded
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

## Bounded Save-Data request metadata

`HttpClient::save_data()` emits `Save-Data: on`.

This helper only declares request metadata. RTTP does not select a
representation, compress a body, advertise Client Hints, or apply browser
data-saver policy. Callers that need values outside the helper can retain
raw-header control with `header(("Save-Data", "..."))`.

## Bounded DNT request metadata

`HttpClient::dnt(value)` emits one `DNT` field from the user's declared
tracking preference. The value must be the W3C Tracking Preference Expression
token `0` (allow tracking) or `1` (do not track), validated through the shared
protocol `Dnt` type; malformed, oversized, duplicate, or control-byte input is
rejected before a socket is opened.

This helper only declares request metadata. RTTP does not disable cookies,
strip `Referer`, change analytics or advertising behavior, or enforce tracking
policy. Callers that need values outside the helper can retain raw-header
control with `header(("DNT", "..."))`.

## Bounded Sec-GPC request metadata

`HttpClient::sec_gpc()` emits `Sec-GPC: 1` through the shared protocol
`SecGpc` representation.

This helper only declares request metadata. RTTP does not infer or enforce
consent, tracking, legal, or serving policy. Callers that need values outside
the helper can retain raw-header control with `header(("Sec-GPC", "..."))`.

## Bounded Upgrade-Insecure-Requests request metadata

`HttpClient::upgrade_insecure_requests()` emits `Upgrade-Insecure-Requests: 1`.

This helper only declares request metadata. RTTP does not rewrite `http://`
URLs to `https://`, redirect requests, or enforce Content-Security-Policy.
Callers that need values outside the helper can retain raw-header control with
`header(("Upgrade-Insecure-Requests", "..."))`.

## Bounded Pragma request metadata

`HttpClient::pragma(value)` validates RFC 9111 `pragma-directive` metadata
through the shared protocol `Pragma` type and emits one normalized `Pragma`
field. Already-attached `Pragma` fields are combined in wire order and
replaced by that single field, so duplicate directive names, empty members,
malformed tokens, and per-field or combined-size bound violations fail before
a socket opens.
`HttpClient::pragma_no_cache()` is a convenience for the defined valueless
`no-cache` directive.

This helper only declares request metadata. RTTP does not translate `Pragma`
into `Cache-Control`, store cache entries, or apply cache, intermediary, or
HTTP/1.0 compatibility policy. Callers that need unusual values can retain
raw-header control with `header(("Pragma", "..."))`.

## Bounded HTTP/1.1 Content-Disposition behavior

`Response::content_disposition()` parses a singleton response
`Content-Disposition` header into the shared protocol-owned
`ContentDisposition` metadata type. It returns `Ok(None)` when the header is
absent and rejects duplicate header fields. Present values expose the
disposition type, ordered parameters, `filename`, and `filename*`;
`ContentDisposition::parse(value)` is available when callers want to validate
a single raw field value directly.

The helper is bounded and validation-oriented. The field value is limited to
64 KiB, the parameter list is limited to 256 entries, each parameter value is
limited to 64 KiB, disposition type and unquoted parameter values must be
valid HTTP tokens, quoted strings must be well formed, and `filename*` must be
an extended value with valid percent encoding. Duplicate parameters, malformed quoted strings, CR/LF injection,
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

## Bounded Permissions-Policy response metadata

`Response::permissions_policy()` parses one or more response
`Permissions-Policy` header fields into the shared protocol-owned
`PermissionsPolicy` metadata type, returning `Ok(None)` when the header is
absent. Fields are combined in wire order after each field is validated.
Present values expose ordered feature directives with their allowlists: the
`*` token as the whole allowlist, the `self` token, quoted serialized HTTP(S)
origins, and inner lists including the empty `()` form.

The helper is bounded and validation-oriented. Each field value is limited to
64 KiB, the directive count is limited to 256 per header set, and each
allowlist is limited to 256 members. Feature names are opaque tokens; the
HTML-attribute tokens `src` and `'none'` are rejected, duplicate feature keys
and duplicate allowlist members are errors, and a well-formed `report-to`
parameter is accepted and dropped. Unparsable input makes
`Response::permissions_policy()` return an error while leaving the original
response headers and body available through the ordinary response APIs.

The helper is metadata-only. `rttp_client` does not grant or deny browser
permissions, compare origins, resolve `self`, enable or disable APIs, or
enforce origin policy, and it does not send reports.

## Bounded Document-Policy response metadata

`Response::document_policy()` parses one or more response `Document-Policy`
header fields into the shared protocol-owned `DocumentPolicy` metadata type,
returning `Ok(None)` when the header is absent. Fields are combined in wire
order after each field is validated. Present values expose ordered
configuration-point directives with their typed values: boolean (including a
bare `?1`), integer, decimal, or token. Directive names are opaque lowercase
tokens or `*` and are not looked up against a browser configuration-point
list. A well-formed `report-to` parameter is accepted as a token or a quoted
string and retained on the directive.

The helper is bounded and validation-oriented. Each field value is limited to
64 KiB, the cumulative raw bytes across all supplied fields are limited to
64 KiB, and the combined directive count is limited to 256. Duplicate
directive names, duplicate parameters, empty dictionaries, and unparsable
input make `Response::document_policy()` return an error while leaving the
original response headers and body available through the ordinary response
APIs.

The helper is metadata-only. `rttp_client` does not execute configuration
points, block document loads, compare required policies, echo
`Sec-Required-Document-Policy`, enable or disable browser features, or send
reports.

`Response::document_policy_report_only()` parses
`Document-Policy-Report-Only` response fields through the same shared
protocol parser, formatter, directive model, and bounds while returning the
distinct `DocumentPolicyReportOnly` metadata type. It preserves raw response
headers on parse errors and does not enforce policy or deliver reports.

## Bounded Supports-Loading-Mode response metadata

`Response::supports_loading_mode()` parses one or more response
`Supports-Loading-Mode` header fields into the shared protocol-owned
`SupportsLoadingMode` metadata type, returning `Ok(None)` when the header is
absent. Fields are combined in wire order after each field is validated.
Present values expose the ordered tokens with `tokens()`, membership checks
with `contains(token)`, and exact predicates for the defined
`fenced-frame`, `credentialed-prerender`, and
`prerender-cross-origin-frames` tokens; well-formed unknown tokens such as
`uncredentialed-prerender` are retained.

The helper is bounded and validation-oriented. Each field value is limited to
64 KiB, the combined raw bytes across fields are limited to 64 KiB, and the
token count is limited to 256 per header set. Duplicate tokens, including
across fields, are rejected with ASCII case-insensitive comparison. Empty
members, strings, integers, inner lists, parameterized items, non-token
members, and oversized values make `Response::supports_loading_mode()` return
an error while leaving the original response headers and body available
through the ordinary response APIs.

The helper is metadata-only. `rttp_client` does not prerender documents,
admit fenced frames, change navigation, or alter resource loading based on
this field.

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

## Bounded Alt-Used response metadata

`Response::alt_used()` parses the `Alt-Used` response field as one bounded
authority value using the shared protocol `AltUsed` type. It returns
`Ok(None)` when the header is absent. Valid metadata preserves host spelling,
optional port, and bracketed IPv6 literal form. Malformed authorities,
duplicate fields, and values larger than 64 KiB return an error while the raw
response headers remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not select an alternative
service, rewrite origins, migrate sockets, retry, or change connection policy
based on `Alt-Used`.

## Bounded Alternates response metadata

`Response::alternates()` parses retained `Alternates` fields as bounded
variant metadata through the shared protocol `Alternates` type. It returns
`Ok(None)` when the header is absent. Valid metadata preserves variant
URI-references, the original accepted source-quality text, and attributes
such as `type`, `language`, `encoding`, and `length`. Each field value is
limited to 64 KiB, the combined field bytes are limited to 64 KiB, the
variant count is limited to 256, and each variant holds at most 256
attributes. Malformed members, invalid URIs, invalid qvalues, duplicate
attributes or variants, and oversized values return an error while the raw
response headers remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not select a variant, fetch
a variant URI, replay requests, resolve URIs against the response URL, apply
`Vary` matching, or change representation policy from `Alternates`.

## Bounded TCN response metadata

`Response::tcn()` parses retained `TCN` fields as bounded RFC 2295
transparent-negotiation result metadata through the shared protocol `Tcn`
type. It returns `Ok(None)` when the header is absent. Valid metadata is a
singleton response field containing `list`, `choice`, `adhoc`, `re-choose`,
or `keep` tokens, normalized to lowercase in wire order. Duplicate fields,
duplicate tokens, malformed or unknown tokens, empty members, oversized
values, and control-byte injection return an error while the raw response
headers remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not select a variant, request
alternates, apply `Vary` matching, or change cache behavior from `TCN`.

## Bounded Variant-Vary response metadata

`Response::variant_vary()` parses retained `Variant-Vary` fields as bounded
RFC 2295 variant-list metadata through the shared protocol `VariantVary`
type. It returns `Ok(None)` when the header is absent. Valid metadata is
either the exclusive `*` wildcard or an ordered list of HTTP field-name
tokens, normalized to lowercase in first-seen order. Duplicate names,
duplicate or mixed wildcards, malformed or empty members, oversized values,
and control-byte injection return an error while the raw response headers
remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not construct a cache key,
select a variant, request alternates, apply `Vary` matching, or change cache
behavior from `Variant-Vary`.

## Bounded Origin-Trial response metadata

`Response::origin_trials()` parses retained `Origin-Trial` fields as an
ordered collection of opaque tokens through the shared protocol `OriginTrials`
type. It returns `Ok(None)` when the header is absent. Valid metadata
preserves multiple tokens and duplicates in wire order. Each token is limited
to 8 KiB, the collection is limited to 64 tokens, and the combined token
bytes are limited to 64 KiB. Injected controls, obs-text, empty values,
oversized tokens, and oversized collections return an error while the raw
response headers remain available through `Response::header_value()` and
`Response::header_values()`. Token material is redacted from typed `Debug`
output and generic `Header` debug output.

The helper is metadata-only. `rttp_client` does not validate token
signatures, expiration, origin applicability, or activate browser trials.

## Bounded Speculation-Rules response metadata

`Response::speculation_rules()` parses one `Speculation-Rules` response field
as bounded opaque metadata through the shared protocol `SpeculationRules`
type. It returns `Ok(None)` when the header is absent. Values are limited to
64 KiB, duplicate fields fail closed, and control bytes that could inject
response fields are rejected. Typed `Debug` and typed parse errors do not dump
the field value.

The helper is metadata-only. `rttp_client` does not fetch, parse, validate, or
execute speculation rule resources, and it does not prefetch, prerender,
change navigation, or alter cache behavior based on this field.

## Bounded Reporting-Endpoints response metadata

`Response::reporting_endpoints()` parses retained `Reporting-Endpoints`
dictionary fields through the shared protocol type. It returns `Ok(None)`
when the header is absent. Present values combine all fields in wire order
into at most 32 endpoint-name to quoted-URL members. Each field value is
limited to 64 KiB, and the combined raw field-value bytes are limited to
64 KiB. Invalid names, unquoted URLs, malformed quoted strings, duplicate
names, oversized input, and too many members return an error while the raw
response headers remain available through `Response::header_value()` and
`Response::header_values()`.

The helper is metadata-only. `rttp_client` does not schedule, send, persist,
retry, or route reports.

## Bounded Cross-Origin-Opener-Policy-Report-Only response metadata

`Response::cross_origin_opener_policy_report_only()` parses retained
`Cross-Origin-Opener-Policy-Report-Only` fields through the shared protocol
type. It returns `Ok(None)` when the header is absent. Present values must be
a singleton structured-field item using the canonical COOP directives
`unsafe-none`, `same-origin-allow-popups`, `same-origin`, or
`noopener-allow-popups`. Well-formed parameters are retained as metadata;
`report-to` is exposed as a reporting-endpoint name when present. Each field
value is limited to 64 KiB; parameter count is bounded to 256, and each
parameter value is bounded to 64 KiB. Duplicate fields, duplicate parameter
names, unknown directives, malformed structured fields, and oversized values
return an error while the raw response headers remain available through
`Response::header_value()` and `Response::header_values()`.

The helper is metadata-only. `rttp_client` does not isolate browsing
contexts, validate `Reporting-Endpoints` members, or send reports.

## Bounded Proxy-Status response metadata

`Response::proxy_status()` parses retained RFC 9209 `Proxy-Status` fields into
`ProxyStatus` metadata. It returns `Ok(None)` when the header is absent.
Present values combine all `Proxy-Status` fields in wire order into a bounded
Structured Fields list of Token or String proxy identifiers with opaque
parameters. `ProxyStatus::parse(value)` is available when callers want to
validate one raw field value directly.

Each field value is limited to 64 KiB. Parsing accepts at most 256 members
and 256 parameters per member. Empty lists, inner-lists, malformed syntax,
control bytes, oversized values, duplicate parameters, and too many members
return an error while the original headers remain available through
`Response::header_value()` and `Response::header_values()`.

The helper is metadata-only. `rttp_client` does not interpret proxy health,
retry requests, promote trailers, or generate origin `Proxy-Status` values.

## Bounded Via response metadata

`Response::via()` parses retained HTTP `Via` fields into `Via` metadata. It
returns `Ok(None)` when the header is absent. Present values combine all
`Via` fields in wire order into a bounded hop chain that preserves protocol
name, protocol version, received-by, comments, duplicates, and ordering.
`Via::parse(value)` is available when callers want to validate one raw field
value directly.

Each field value is limited to 64 KiB. Parsing accepts at most 256 members.
Empty members, malformed syntax, control bytes, oversized values, and too
many members return an error while the original headers remain available
through `Response::header_value()` and `Response::header_values()`.

The helper is metadata-only. `rttp_client` does not append or remove hops or
apply proxy policy.

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
| Buffered content decoding | Automatic gzip and zlib-wrapped deflate stacks in reverse header order on buffered HTTP/1.1 and supported h2c paths; successful decoding drops stale `Content-Encoding`/`Content-Length`; unsupported or invalid stacks preserve headers and body; malformed layers fail atomically; size bounds apply per decoded layer; `Response::binary()` retains the original capture | No extra compression formats, raw deflate, streaming decode, or async HTTP/2 |
| HTTP/1.1 request emission | Origin-form requests, absolute-form proxy requests, `CONNECT`, `HEAD`, fixed bodies, streaming chunked uploads, and explicit `Expect: 100-continue` metadata through the shared protocol type | Expect metadata does not gate body transmission; raw `header(("Expect", value))` remains an escape hatch; SOCKS handshakes are delegated to the `socks` crate |
| Fetch Metadata | `sec_fetch_site`, `sec_fetch_mode`, `sec_fetch_dest`, `sec_fetch_user`, and `sec_purpose` emit bounded `Sec-Fetch-*`/`Sec-Purpose` request metadata | No browser security policy, automatic header generation, origin validation, navigation policy, request blocking, prefetch execution, or cache behavior |
| Save-Data | `save_data` emits bounded `Save-Data: on` request metadata | No reduced-data serving, content adaptation, compression, Client Hints advertisement, retries, or browser data-saver policy |
| DNT | `dnt` emits bounded `DNT: 0`/`DNT: 1` request metadata through the shared protocol `Dnt` type and rejects malformed or oversized input before connecting | No tracking enforcement, cookie changes, `Referer` stripping, analytics or advertising behavior, `Tk` emission, retries, or privacy-preference policy |
| Sec-GPC | `sec_gpc` emits bounded `Sec-GPC: 1` request metadata through the shared protocol type | No consent inference, tracking-policy enforcement, legal policy, serving policy, retries, or browser state |
| Upgrade-Insecure-Requests | `upgrade_insecure_requests` emits bounded singleton `Upgrade-Insecure-Requests: 1` request metadata | No URL rewriting, redirecting, Content-Security-Policy enforcement, HSTS, or automatic scheme selection |
| Max-Forwards | `max_forwards` emits bounded singleton `Max-Forwards` request metadata through the shared protocol type | No hop decrement, proxy routing, TRACE/OPTIONS selection, retry, or forwarding policy |
| Depth | `depth` emits bounded singleton WebDAV `Depth` request metadata through the shared protocol type, normalizing `infinity` to lowercase and replacing an existing same-name field | No resource traversal, WebDAV method selection, method-policy enforcement, retry, or forwarding policy |
| Lock-Token | `lock_token` emits bounded singleton WebDAV `Lock-Token` request metadata through the shared protocol type, replacing an existing same-name field and redacting the token from typed debug output; `Response::lock_token` parses bounded singleton response metadata | No lock creation, refresh, release, persistence, ownership comparison, or WebDAV lock policy |
| Destination | `destination` emits bounded singleton WebDAV `Destination` request metadata through the shared protocol type, preserving one absolute URI and replacing an existing same-name field | No destination resolution, URI normalization, authorization, COPY/MOVE execution, or application resource policy |
| Timeout | `timeout` emits bounded ordered WebDAV `Timeout` request metadata through the shared protocol type, normalizing `Second-n`/`Infinite` alternatives to lowercase and replacing an existing same-name field | No lock creation, lock refresh, application-timeout selection, retry, or forwarding policy |
| If-Schedule-Tag-Match and Schedule-Tag | `if_schedule_tag_match` emits bounded singleton `If-Schedule-Tag-Match` request metadata, and `Response::schedule_tag` parses bounded singleton `Schedule-Tag` response metadata through shared protocol types backed by `EntityTag` | No calendar-version generation, schedule-tag comparison, calendar inspection, scheduling policy, retry, or status-policy behavior |
| Overwrite | `overwrite` emits bounded singleton WebDAV `Overwrite` request metadata through the shared protocol type, accepting only the tokens `T` and `F` and replacing an existing same-name field | No destination overwrite, RFC 4918 default-`T` synthesis, resource policy, COPY/MOVE execution, retry, or forwarding policy |
| If | `if_header` emits bounded RFC 4918 WebDAV `If` request metadata through the shared protocol type, validating untagged or tagged condition lists, `Not`, state tokens, and bracketed entity tags, preserving order and replacing an existing same-name field without touching `If-Match` | No lock, entity-tag, or resource-state evaluation; no 412 or other precondition outcome; no lock creation, refresh, or release; no COPY/MOVE/UNLOCK execution; no retry, or forwarding policy |
| WebDAV metadata matrix | Workspace integration tests cover live HTTP/1.1 and h2c client/server/facade roundtrips for `Depth`, `Destination`, `Overwrite`, `Timeout`, `Lock-Token`, `If`, `DAV`, `If-Schedule-Tag-Match`, and `Schedule-Tag`, including valid values, malformed/duplicate/bounds rejection, raw-header observability, and `Lock-Token`/`If` redaction | No resource storage, locking, COPY/MOVE/LOCK/UNLOCK execution, default `Overwrite: T` synthesis, or WebDAV method policy |
| Transparent negotiation metadata matrix | Workspace integration tests cover live HTTP/1.1 and h2c client/server/facade roundtrips for `A-IM`, `IM`, `Delta-Base`, `Alternates`, `Negotiate`, `TCN`, and `Variant-Vary`, including valid values, malformed/duplicate/bounds rejection, raw-header observability, and declared-order q-value recording | No variant selection, `Alternates` URI fetching, delta application, `226 IM Used` synthesis, or cache policy |
| Idempotency-Key | `idempotency_key` emits bounded singleton opaque `Idempotency-Key` request metadata through the shared protocol type, replacing an existing same-name field | No retry, replay, key storage or comparison, deduplication store, or application idempotency policy |
| WebSocket handshake metadata | `sec_websocket_key` emits bounded singleton `Sec-WebSocket-Key` request metadata through the shared protocol type, replacing an existing same-name field and redacting the nonce from typed debug output; `Response::sec_websocket_accept` parses bounded singleton response metadata and `verify_sec_websocket_accept` checks the RFC GUID plus SHA-1/base64 derivation against a validated key | No HTTP upgrade, random nonce generation, WebSocket frames, or handshake policy |
| Sec-WebSocket-Version | `sec_websocket_version` emits bounded `Sec-WebSocket-Version` request metadata through the shared protocol type, replacing an existing same-name field, and `Response::sec_websocket_version` parses received fields including rejection-response version lists | No WebSocket handshake, `Connection: Upgrade` emission, `Sec-WebSocket-Accept` computation, version negotiation, protocol switch, or frames |
| Sec-WebSocket-Protocol | `sec_websocket_protocol` emits bounded `Sec-WebSocket-Protocol` offer metadata in preference order through the shared protocol type, replacing an existing same-name field, and `Response::sec_websocket_protocol` parses received fields as a selection singleton | No WebSocket handshake, `Connection: Upgrade` emission, automatic subprotocol choice, protocol switch, or frames |
| Sec-WebSocket-Extensions | `sec_websocket_extensions` emits bounded ordered `Sec-WebSocket-Extensions` offer metadata through the shared protocol type, replacing an existing same-name field, and `Response::sec_websocket_extensions` parses received fields as a one-extension selection singleton while preserving raw headers on errors | No WebSocket handshake, `Connection: Upgrade` emission, compression activation, extension negotiation, protocol switch, or frames |
| Pragma | `pragma` and `pragma_no_cache` emit bounded RFC 9111 `Pragma` request metadata through the shared protocol type, combining and replacing existing same-name fields | No translation into `Cache-Control`, cache storage, freshness checks, revalidation, or cache/intermediary policy |
| W3C Trace Context | `traceparent` and `tracestate` validate and emit bounded W3C Trace Context request metadata through shared protocol types, replacing existing same-name fields and redacting propagation values from typed debug output | No trace-id creation, sampling decision, tracing backend, span model, or automatic propagation |
| W3C Baggage | `baggage` validates and emits bounded W3C Baggage request metadata through the shared protocol type, replacing an existing same-name field and redacting member and property values from typed debug output | No application-data interpretation, request-context storage, tracing backend, span model, or automatic propagation |
| CDN-Loop | `cdn_loop` validates and emits bounded RFC 8586 `CDN-Loop` request metadata through the shared protocol type, combining an existing same-name field with the new member in wire order and rejecting malformed or oversized values before connecting | No CDN identifier insertion, loop detection or rejection, automatic forwarding, or hop-by-hop handling |
| Via | `via` validates and emits bounded HTTP `Via` request metadata through the shared protocol type, combining an existing same-name field with the new hops in wire order and rejecting malformed or oversized values before connecting; `Response::via` parses received hop chains while preserving raw headers on parse failures | No automatic hop insertion or removal, trusted-proxy inference, identity rewrite, or HTTP/1.1 or HTTP/2 proxy-policy changes |
| Preflight request metadata | `origin`, `access_control_request_method`, `access_control_request_headers`, and `access_control_request_private_network` emit bounded `Origin`, `Access-Control-Request-Method`, `Access-Control-Request-Headers`, and `Access-Control-Request-Private-Network` request metadata and reject invalid input before connecting | No automatic preflight decision, `Access-Control-Allow-*` response parsing, CORS policy, or Private Network Access policy |
| Digest preferences | `want_content_digest`, `want_content_digest_with_q`, `want_repr_digest`, and `want_repr_digest_with_q` emit bounded `Want-Content-Digest` and `Want-Repr-Digest` request metadata; server `Request::want_content_digest()`, `HttpRequest::want_content_digest()`, `Request::want_repr_digest()`, and `HttpRequest::want_repr_digest()` parse received preference fields | No algorithm selection, digest computation, response body hash validation, retries, or signing |
| Accept | `accept` and `accept_with_q` format bounded `Accept` request metadata through the shared `rttp-protocol` type, replacing existing same-name fields after validating helper-built and existing raw values | No content negotiation, representation selection, MIME sniffing, body decoding, cache `Vary` synthesis, or response choice |
| Accept-Charset | `accept_charset` and `accept_charset_with_q` format bounded `Accept-Charset` request metadata through the shared `rttp-protocol` type | No content negotiation, charset transcoding, body decoding, MIME sniffing, or response selection |
| A-IM | `a_im`, `a_im_with_q`, and `a_im_value` format bounded `A-IM` request metadata through the shared `rttp-protocol` type | No automatic delta-encoding selection, application, compression, or response transformation |
| IM | `Response::im` parses bounded ordered `IM` response metadata through the shared `rttp-protocol` type while preserving raw headers on parse errors | No instance-manipulation decoding, inversion, or application, and no `226 IM Used` status policy |
| Delta-Base | `Response::delta_base` parses bounded singleton `Delta-Base` response metadata through the shared entity-tag primitive while preserving raw headers on parse errors | No cached-entity lookup, validator comparison, delta application, or cache policy |
| Negotiate | `negotiate` emits bounded RFC 2295 `Negotiate` request metadata through the shared `rttp-protocol` type, replacing an existing same-name field | No variant selection, transparent content negotiation, `Alternates`/`TCN` synthesis, or automatic cache selection |
| TCN | `Response::tcn()` parses bounded RFC 2295 `TCN` response metadata through the shared `rttp-protocol` type while preserving raw headers on parse errors | No variant selection, `Alternates`/`Vary` synthesis, transparent content negotiation, or cache behavior |
| Set-Cookie | `Response::set_cookies()` parses bounded protocol `Set-Cookie` response metadata, preserves multiple field lines and raw headers, and redacts cookie values from typed debug and errors; the typed accessor rejects duplicate attributes, valued flag attributes such as `Secure=true`, non-standard `SameSite` values, signed, empty, or overflowing `Max-Age`, malformed quoted values such as backslash escapes, field-count and size bounds, and other invalid protocol metadata while raw header access remains available; `Response::cookies()`/`cookie()` remain a legacy compatibility view that only exposes fields accepted by the protocol parser, strips surrounding cookie-value quotes, maps legacy `hostOnly`/`host_only` extension attributes to `host_only`, silently omits invalid `Set-Cookie` fields, and redacts `Cookie` `Display`; legacy callers that need a wire header value should call `Cookie::string()` explicitly | No cookie jar, persistence, domain/path matching, expiry enforcement, SameSite or partitioning policy, or automatic request `Cookie` emission |
| Variant-Vary | `Response::variant_vary()` parses bounded RFC 2295 `Variant-Vary` response metadata through the shared `rttp-protocol` type while preserving raw headers on parse errors | No cache-key construction, variant selection, `Alternates`/`TCN`/`Vary` synthesis, transparent content negotiation, or cache behavior |
| Accept-Encoding | `accept_encoding`, `accept_encoding_with_q`, and gzip/deflate/br/identity helpers format bounded `Accept-Encoding` request metadata through the shared `rttp-protocol` type | No compression, decompression, content negotiation, retries, or transport changes |
| HTTP message signatures | `signature` and `signature_input` emit bounded RFC 9421 request metadata; `Response::signature()` and `signature_input()` parse received fields | No signing, verification, key lookup, covered-component canonicalization, or cryptographic policy |
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Byte ranges | `range`, `range_from`, `range_suffix`, `ranges`, `if_range_etag`, and `if_range_date` emit bounded HTTP/1.1 single- and multi-range request metadata; checked `Response::content_range`, `accept_ranges`, `is_partial_content`, and `is_range_not_satisfiable` expose `Content-Range`, `Accept-Ranges`, `206`, and `416` metadata while preserving raw headers, including multipart/byteranges bodies | No Range request generation from `Accept-Ranges`, client-side `If-Range` evaluation, partial response engine, byte serving, content slicing, download resume, automatic retry/replay, cache storage, redirect handling, status-policy behavior, client multipart/byteranges part decoding into structured ranges, or automatic cache validation policy |
| Conditional requests | `if_none_match`, `if_match`, `if_modified_since`, and `if_unmodified_since` emit bounded HTTP/1.1 validators; the date helpers validate and emit through the shared protocol `IfModifiedSince` and `IfUnmodifiedSince` types; `Response::is_not_modified`, `is_precondition_failed`, typed bounded `etag`, `delta_base`, `last_modified`, and `last_modified_date` expose `304`/`412` and delta-base metadata while preserving raw headers | One ETag validator per helper call, `If-Range` is range-scoped, no cache storage, no cached-entity lookup, no automatic revalidation, no delta application, and no cache-control engine |
| Informational responses and Early Hints | `Response::informational_responses` exposes skipped bounded HTTP/1.1 `1xx` heads, including `103 Early Hints`, with preserved raw headers | `101 Switching Protocols` remains terminal for upgrade handoff; no automatic preload execution, cache policy, redirect/retry/replay, route generation, streaming early-write API, TLS/ALPN behavior, or status-policy behavior |
| Cache-Control, CDN-Cache-Control, Cache-Status, Date, Age, Expires, and Retry-After | `Response::cache_control` parses bounded response directives, numeric freshness fields, quoted field-name lists, and extension directives; `Response::cdn_cache_control` parses bounded `CDN-Cache-Control` directives and CDN extension metadata while preserving raw responses on parse errors; `Response::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw responses on parse errors; `Response::date` parses singleton HTTP-date metadata; `Response::age` parses bounded singleton `Age` metadata through the protocol `Age` type, rejecting duplicate fields, values larger than 64 KiB, and overflowing `u64` delta-seconds; `Response::expires` parses bounded HTTP-date metadata; `Response::retry_after` parses bounded singleton delta-seconds or HTTP-date metadata through the protocol `RetryAfter` type while preserving raw headers on parse errors | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, clock-skew correction, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, automatic conditional requests, automatic sleep, retry, replay, redirect, backoff, scheduler integration, or status policy |
| Cache-Control, CDN-Cache-Control, Surrogate-Control, Cache-Status, Date, Age, Expires, and Last-Modified | `Response::cache_control` parses bounded response directives, numeric freshness fields, quoted field-name lists, and extension directives; `Response::cdn_cache_control` parses bounded `CDN-Cache-Control` directives and CDN extension metadata while preserving raw responses on parse errors; `Response::surrogate_control` parses bounded `Surrogate-Control` directives with duplicate rejection and aggregate-size validation while preserving raw responses on parse errors; `Response::cache_status` parses bounded RFC 9211 `Cache-Status` list members and parameters while preserving raw responses on parse errors; `Response::date`, `Response::expires`, and `Response::last_modified_date` parse bounded singleton HTTP-date metadata through shared protocol primitives; `Response::age` parses bounded singleton `Age` metadata through the protocol `Age` type, rejecting duplicate fields, values larger than 64 KiB, and overflowing `u64` delta-seconds | No cache storage, CDN cache, Cache-Status forwarding or freshness policy, automatic revalidation, wall-clock freshness calculation, clock-skew correction, `Vary` matching, shared-cache policy enforcement, surrogate-key behavior, `Surrogate-Control` to `Cache-Control` translation, automatic conditional requests, retry, redirect, scheduling, or status policy |
| Alt-Used | `Response::alt_used` parses bounded singleton response authority metadata through the shared protocol `AltUsed` type while preserving raw headers on parse failures | No alternative service selection, origin rewriting, socket migration, retry, or connection-policy behavior |
| Alternates | `Response::alternates` parses bounded RFC 2295-style variant metadata through the shared protocol `Alternates` type, validating URIs, qvalues, attributes, duplicates, member counts, and size bounds while preserving raw headers on parse failures | No transparent content negotiation, variant selection, automatic fetch, request replay, URI resolution, cache storage, `Vary` matching, or quality ranking |
| Origin-Trial | `Response::origin_trials` parses bounded opaque `Origin-Trial` tokens in wire order through the shared protocol `OriginTrials` type, preserves duplicates, redacts token material from debug output, and preserves raw headers on parse failures | No token signature validation, expiration checks, origin applicability, feature activation, or browser trial policy |
| Speculation-Rules | `Response::speculation_rules` preserves one bounded opaque `Speculation-Rules` response field through the shared protocol `SpeculationRules` type, rejects duplicates and response-field injection bytes, redacts debug output, and preserves raw headers on parse failures | No speculation rule fetching, parsing, validation, prefetching, prerendering, execution, navigation changes, cache behavior, retry, or redirect behavior |
| Memento-Datetime | `Response::memento_datetime` parses bounded singleton `Memento-Datetime` IMF-fixdate metadata through the protocol `MementoDatetime` type while preserving raw headers on parse errors | No archival selection, `Accept-Datetime` negotiation, TimeGate behavior, retry, or transport changes |
| Accept-Datetime | `accept_datetime` validates and emits bounded singleton `Accept-Datetime` request metadata through the protocol `AcceptDatetime` type, accepting obsolete HTTP-date forms and replacing an existing same-name field | No archival selection, TimeGate behavior, `Vary` injection, cache-policy changes, or conditional-request handling |
| Allow | `Response::allow` parses bounded response `Allow` fields into an ordered HTTP method-token list | No fallback method selection, automatic retry/replay, or status-code policy behavior for `405` or `OPTIONS` |
| Client Hints | `Response::accept_ch` and `Response::critical_ch` parse bounded, ordered Client Hints opt-in metadata while preserving raw headers on parse failures | No browser opt-in state, request-header generation, retry, persistence, or Client Hints policy |
| Content-Security-Policy-Report-Only | `Response::content_security_policy_report_only` parses bounded `Content-Security-Policy-Report-Only` response metadata through the protocol type, preserving repeated fields in wire order and leaving raw headers observable on parse failures | No CSP enforcement, directive evaluation, report delivery, browser policy state, retry, redirect, cache behavior, or status-policy behavior |
| Content-Language | `Response::content_language` parses bounded response `Content-Language` fields into ordered language metadata while preserving raw headers | No automatic language negotiation, locale fallback, variant matching, cache policy, retry, replay, redirect, or status-policy behavior |
| Content-Location | `Response::content_location` and `ContentLocation::parse` parse bounded singleton response `Content-Location` metadata while preserving raw headers | No redirect behavior, cache variant selection, representation replacement, retry/replay, route generation, or status-policy behavior |
| Service-Worker-Allowed | `Response::service_worker_allowed` and `ServiceWorkerAllowed::parse` parse bounded singleton response `Service-Worker-Allowed` path metadata while preserving raw headers | No service-worker registration, scope evaluation, script-URL resolution, or application routing policy |
| Content-DPR | `Response::content_dpr` and `ContentDpr::parse` parse bounded singleton response `Content-DPR` decimal-ratio metadata while preserving raw headers | No image rescaling, request DPR emission, Client Hints policy, retry, or transport changes |
| Deprecation | `Response::deprecation` and `Deprecation::parse` parse bounded singleton Structured Fields boolean or date `Deprecation` metadata while preserving raw headers | No Sunset comparison, Link follow, already-deprecated clocks, retries, endpoint selection, or browser/cache policy |
| Content-Type and Content-Encoding | `Response::content_type`/`ContentType::parse` parse bounded singleton `Content-Type` metadata, and `Response::content_encoding`/`ContentEncoding::parse` parse bounded ordered `Content-Encoding` codings while preserving raw headers on parse failures | No MIME sniffing, body decoding, charset transcoding, compression/decompression policy, negotiation, cache policy, redirects, retry/replay, or filesystem serving |
| Connection | `Response::connection`/`Connection::parse` parse bounded HTTP/1 `Connection` tokens, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to keep-alive, `auto_add_connection`, hop-by-hop stripping, or HTTP/2 rejection |
| Keep-Alive | `Response::keep_alive` parses bounded RFC 2068 `Keep-Alive` fields in wire order with `timeout` delta-seconds and `max` `1*DIGIT` values as checked unsigned integers, preserving unrecognized `name=token` parameters as bounded extension metadata and raw headers on parse failures | No connection lifetime management, connection pooling, keep-alive timers, or HTTP/2 behavior changes |
| Transfer-Encoding | `Response::transfer_encoding`/`TransferEncoding::parse` parse bounded HTTP/1 `Transfer-Encoding` fields that must be sole `chunked`, combining duplicate fields in wire order while preserving raw headers on parse failures | No change to HTTP/1 framing decoders, `TE`, Content-Length, chunked body decoding policy, or HTTP/2 decode rejection |
| Content-Disposition | `Response::content_disposition` and the protocol-owned `ContentDisposition::parse` parse bounded singleton response `Content-Disposition` metadata into disposition type plus ordered parameters, including preserved `filename` and `filename*` values, while preserving raw headers on parse failures | No automatic download, filesystem path handling, MIME sniffing, redirect behavior, retry/replay, cache behavior, negotiation behavior, or status-policy behavior |
| Vary | `Response::vary` parses bounded response `Vary` fields into wildcard or normalized case-insensitive field-name metadata | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| NEL | `Response::nel` parses the bounded singleton `NEL` field as W3C Network Error Logging policy metadata while preserving raw headers | No network error report sending, policy persistence, Reporting endpoint group configuration, or status-policy behavior |
| Reporting-Endpoints | `Response::reporting_endpoints` parses bounded endpoint-name to quoted-URL dictionaries through the shared protocol type while preserving raw headers | No report scheduling, sending, persistence, retry, routing, or endpoint policy behavior |
| Cross-Origin-Opener-Policy-Report-Only | `Response::cross_origin_opener_policy_report_only` parses bounded singleton COOP Report-Only metadata through the shared protocol type, reuses the canonical COOP directives, retains reporting parameters including `report-to`, and preserves raw headers | No browsing-context isolation, report scheduling, sending, persistence, retry, routing, or `Reporting-Endpoints` validation |
| Proxy-Status | `Response::proxy_status` parses bounded RFC 9209 Token/String proxy identifiers with opaque parameters while preserving raw headers on parse failures | No proxy health checks, retries, trailer promotion, or origin-generation policy |
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
