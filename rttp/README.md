rttp
====

`rttp` wraps `rttp_client` behind optional client features and provides a small
blocking HTTP server for local tests and simple embedded use.

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
`multipart/byteranges` responses or choose a response for multiple requested
ranges. There is no built-in filesystem serving, path normalization, MIME
selection, ETag or Last-Modified generation, cache storage, automatic cache
validation, automatic retry, authorization, directory-index, or dotfile policy.
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

Use `HttpResponse::not_modified(&metadata)` for `304 Not Modified`; it adds
available `ETag` and `Last-Modified` validators and serializes without a
message body. Use `HttpResponse::precondition_failed()` for
`412 Precondition Failed`; it returns an empty response unless application code
adds its own headers or body. Applications remain responsible for the normal
successful `200` response when evaluation returns `Proceed`.

ETag comparison is deliberately scoped to the supplied representation metadata.
The helpers do not pick entity tags, read files, check authorization, choose a
static-file policy, store cached responses, perform automatic revalidation, or
implement a full cache-control engine. Invalid conditional headers that cannot
be parsed as the bounded helper syntax are ignored by the evaluation helper
rather than rejected before handler code.

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
`HttpResponse::header("Age", ...)`, `HttpResponse::header("Expires", ...)`,
and `HttpResponse::header("Retry-After", ...)` values remain preserved exactly
as response headers. These helpers do not calculate freshness, validate cache
state against wall-clock time, store responses, match stored responses,
revalidate responses, enforce shared-cache policy, attach behavior to status
codes, throttle requests, sleep, retry, replay requests, apply backoff,
integrate with a scheduler, or issue automatic conditional requests.

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

## Bounded HTTP/1.1 Allow behavior

Server-side `Allow` helpers expose response declaration metadata without
implementing method negotiation or automatic 405/OPTIONS behavior.
`HttpResponse::with_allow(methods)` validates an explicit method list and adds
one comma-separated `Allow` header, while `HttpResponse::allow()` parses any
`Allow` headers already attached to a response into `HttpAllowedMethods`.
`HttpAllowedMethods::parse(value)` accepts a comma-separated list of HTTP
method tokens and preserves the declared token spelling and order.

Parsing is bounded and validation-oriented. Each `Allow` field value is
limited to 64 KiB, the parsed method list is limited to 32 entries, and each
member must be a valid HTTP token. Empty members, malformed methods,
duplicates, oversized values, and too many methods return `HttpAllowParseError`
from the helper. Raw `HttpResponse::header("Allow", ...)` values remain
preserved exactly as ordinary response headers; helper parse errors do not
remove existing headers or attach routing behavior.

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
non-h2c `HttpResponse::upgrade` handoffs remain separate caller-owned protocol
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
apply to ordinary HTTP/1.1 `CONNECT`, non-h2c `HttpResponse::upgrade`
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
Valid PING frames on stream 0 are acknowledged with PING ACK frames that carry
the same opaque 8-byte data.
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
authority-form requests and `HttpResponse::upgrade` for non-h2c protocols
remain separate handoff paths for caller-owned protocols, and the h2c Upgrade
detection preserves those existing handoffs when `Upgrade` is not `h2c`.

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
| HTTP/1.1 request parsing | Required `Host` validation, origin-form, absolute-form, asterisk-form `OPTIONS`, authority-form `CONNECT`, fixed and chunked bodies, chunk extensions, `Expect: 100-continue`, and obsolete line folding rejection | Intended for local tests and simple embedded use, not full RFC coverage |
| HTTP/1.1 connection handling | Bounded sequential `serve_requests`, keep-alive and close behavior for HTTP/1.1 and HTTP/1.0, pipelined request boundaries, malformed request rejection before handler dispatch | Blocking listener only; no async accept loop |
| HTTP/1.1 response framing | Automatic `Content-Length`, explicit chunked responses, bodyless `HEAD`, `101`, `204`, and `304`, response trailers after the terminating chunk | No server TLS |
| Byte ranges | `HttpByteRange` parses one `bytes` range, `Request::evaluate_if_range` and `HttpRequest::evaluate_if_range` gate it with caller-provided strong ETag or exact HTTP-date metadata, and `HttpResponse::partial_content`/`range_not_satisfiable` serialize `206`/`416` with `Content-Range` | No multipart range serialization, automatic retry, cache storage, filesystem serving, automatic cache validation, or static-file policy |
| Conditional requests | `Request::evaluate_conditional`, `evaluate_conditional_request`, `HttpConditionalMetadata`, and `HttpEntityTag` evaluate bounded HTTP/1.1 validators; `HttpResponse::not_modified` and `precondition_failed` serialize `304` and `412` outcomes | No cache storage, static-file serving policy, automatic revalidation, or cache-control engine |
| Cache-Control | `Request::cache_control`, `HttpRequest::cache_control`, and `HttpResponse::cache_control` parse bounded request/response directives, numeric freshness fields, quoted field-name lists, and extension directives; `HttpResponse::with_age`/`age`, `with_expires`/`expires`, and `with_retry_after_delta`/`with_retry_after_date`/`retry_after` declare and parse response `Age`, `Expires`, and `Retry-After` metadata | No cache storage, automatic revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache policy enforcement, automatic conditional requests, directive-based validator evaluation, automatic sleep, retry, replay, backoff, scheduler integration, or status-code policy engine |
| Vary | `HttpVary`, `HttpResponse::with_vary`, `HttpResponse::vary`, `Request::vary_selection`, and `HttpRequest::vary_selection` parse, declare, and select bounded `Vary` metadata with case-insensitive field-name handling | No cache storage, stored-response matching engine, cache key persistence, automatic request replay, shared-cache policy enforcement, or automatic conditional requests |
| Allow | `HttpAllowedMethods`, `HttpResponse::with_allow`, and `HttpResponse::allow` declare and parse bounded `Allow` method-list metadata | No request routing, automatic 405 generation, OPTIONS handling, method negotiation, retry, or replay behavior |
| Upgrade and tunnel targets | `CONNECT` authority-form requests are accepted as HTTP requests; `HttpResponse::upgrade` can hand an upgraded socket to caller code after a matching request | The server does not implement the upgraded protocol after handoff |
| Trailers | Chunked request trailers are preserved on `Request`; malformed, oversized, forbidden, and pseudo-header trailers are rejected; response trailers can be serialized for chunked responses | Application metadata trailers are allowed; trailer names that affect connection state, routing, authentication/cookies, framing, or payload processing are rejected |
| Bounded h2c server | The same `socket2` listener detects the HTTP/2 prior-knowledge preface or a valid HTTP/1.1 `Upgrade: h2c` request with `HTTP2-Settings`, validates SETTINGS including legal `SETTINGS_ENABLE_PUSH` and `SETTINGS_ENABLE_CONNECT_PROTOCOL` values of only `0` or `1` and legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, dispatches RFC 8441 extended CONNECT only after `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` has been negotiated, exposes negotiated extended CONNECT as a normal `Request` with method `CONNECT`, version `HTTP/2`, target from `:path`, `host` from `:authority`, and `Request::extended_connect_protocol()` from `:protocol`, advertises the default 16,384-byte `SETTINGS_MAX_FRAME_SIZE`, rejects inbound frames above the active local limit, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, advertises `SETTINGS_MAX_CONCURRENT_STREAMS` from the bounded active stream allowance, enforces that allowance before dispatching new streams, advertises and enforces a conservative `SETTINGS_MAX_HEADER_LIST_SIZE` for inbound request metadata, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, serves bounded streams including bodyless DELETE, OPTIONS, TRACE, and negotiated extended CONNECT, handles HEAD without response DATA, rejects connection-specific request fields before handler dispatch, strips connection-specific response fields during h2c serialization, treats `RST_STREAM` as a bounded reset/cancellation signal for the affected stream, acknowledges valid PING frames with matching opaque data, accepts padded HEADERS/DATA/trailers without exposing padding, handles HPACK Huffman fields and bounded CONTINUATION header blocks, emits `GOAWAY` with the last completed stream id at bounded shutdown, validates and ignores valid PRIORITY metadata, ignores HTTP/2-allowed unknown/extension frames inside this bounded path, normalizes reserved stream-id high bits, and applies conservative DATA flow control | Ordinary `CONNECT`, missing-negotiation `:protocol`, non-CONNECT `:protocol`, malformed h2c Upgrade, request bodies on h2c Upgrade, and `PUSH_PROMISE` are rejected deterministically before handler dispatch; HTTP/1.1 `CONNECT` and non-h2c `Upgrade` remain separate handoff paths; bounded h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, full WebSocket-over-h2, proxy h2, tunnel handoff, connection pooling, persistent multiplex sessions, persistent HTTP/2 session management, automatic retry, server push, full RFC 8441 support, full stream state machine, unbounded multiplexing, unbounded multiplex scheduling, general multiplexing, general tunnel scheduling, priority scheduling, or full HTTP/2 server feature set |

## Client feature

Enable the `client` feature to access `rttp::Http::client`, or enable `async`,
`http2`, `tls-native`, `tls-rustls`, or `all` for the corresponding
`rttp_client` capabilities. The client feature includes the bounded HTTP/1.1
Range helpers from `rttp_client`: `range(start, end)`, `range_from(start)`,
and `range_suffix(length)` set single `bytes` ranges; `if_range_etag(etag)`
sets a single strong entity-tag `If-Range` validator; and
`if_range_date(http_date)` sets an HTTP-date `If-Range` validator. `Response`
exposes `is_partial_content()`, `is_range_not_satisfiable()`, and
`content_range()` for `206` and `416` responses. Manual `Range` and
`If-Range` headers remain available through the generic header API. These
client helpers do not evaluate `If-Range`, retry requests, store cache entries,
generate multipart range requests, or apply automatic cache validation. The
client response API also includes `Response::cache_control()` from
`rttp_client`, which parses bounded response `Cache-Control` directives and
extensions as metadata only; the wrapper does not add cache storage,
automatic revalidation, wall-clock freshness calculation, `Vary` matching,
shared-cache policy enforcement, or automatic conditional requests. The
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
performed. Valid PING frames are acknowledged with PING ACK frames that carry
the same opaque 8-byte data. Server push is outside this bounded client path.
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

Direct TCP client connections use `socket2`. SOCKS proxy handshakes remain
delegated to the `socks` crate.
