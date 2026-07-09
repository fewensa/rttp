rttp_client
===========

`rttp_client` is a small HTTP client crate. Plain HTTP is available by default;
optional features add async request APIs and TLS implementations.

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

## Bounded HTTP/1.1 byte ranges

`HttpClient` includes helpers for the single-range `bytes` forms RTTP keeps
bounded: `range(start, end)` emits `Range: bytes=start-end`,
`range_from(start)` emits `Range: bytes=start-`, and `range_suffix(length)`
emits `Range: bytes=-length`. The helpers reject inverted closed ranges and a
zero suffix length before a socket is opened. They are request-header helpers;
manual `Range` headers remain available through `header(("Range", "..."))`
when callers need behavior outside the helper validation.

Partial-content responses are exposed through the normal `Response` API.
`Response::is_partial_content()` identifies `206 Partial Content`, and
`Response::content_range()` parses a `Content-Range` field such as
`bytes 10-19/200` into a `ContentRange` with `unit`, `start`, `end`, and
`complete_length` accessors. `Response::is_range_not_satisfiable()` identifies
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

RTTP does not synthesize multipart range requests, evaluate `If-Range`, retry
range requests, store cached responses, or apply automatic cache validation
policy on the client side. Multiple ranges can only be sent by manually setting
the header, and any server response is then parsed as an ordinary HTTP
response.

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

Conditional responses are exposed through response metadata helpers.
`Response::is_not_modified()` identifies `304 Not Modified`,
`Response::is_precondition_failed()` identifies `412 Precondition Failed`,
`Response::etag()` returns the response `ETag` field when present, and
`Response::last_modified()` returns the response `Last-Modified` field when
present. A `304` response is treated as bodyless even if misleading framing
headers are present, so the connection remains framed for the next response.
`412` is surfaced as a normal response status and body/framing rules remain the
server's responsibility.

RTTP does not provide cache storage, automatic revalidation, or a
cache-control engine. Client conditional helpers only set request headers and
expose response metadata; applications decide when to persist validators, when
to revalidate, and how to interpret cache directives.

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
| Upgrade and tunnel handoff | `CONNECT` returns the tunnel socket after a successful `200`; `upgrade()` returns the socket after `101 Switching Protocols` and skips interim `1xx` responses | Upgraded protocols are handed to the caller and are not parsed by `rttp_client` |
| Redirects | Auto-redirect covers 301, 302, 303, 307, and 308 method/body behavior, relative and absolute `Location` resolution, same- and cross-authority header handling, loop detection, and redirect bounds | Redirects are HTTP client behavior, not a browser policy implementation |
| Byte ranges | `range`, `range_from`, `range_suffix`, `if_range_etag`, and `if_range_date` emit bounded HTTP/1.1 range request metadata; `Response::content_range`, `is_partial_content`, and `is_range_not_satisfiable` expose `206` and `416` metadata | No client-side `If-Range` evaluation, automatic retry, cache storage, multipart range generation, or automatic cache validation policy |
| Conditional requests | `if_none_match`, `if_match`, `if_modified_since`, and `if_unmodified_since` emit bounded HTTP/1.1 validators; `Response::is_not_modified`, `is_precondition_failed`, `etag`, and `last_modified` expose `304`/`412` metadata | One ETag validator per helper call, `If-Range` is range-scoped, no cache storage, no automatic revalidation, and no cache-control engine |
| Cache-Control | `Response::cache_control` parses bounded response directives, numeric freshness fields, quoted field-name lists, and extension directives into metadata helpers | No cache storage, automatic revalidation, wall-clock freshness calculation, `Vary` matching, shared-cache policy enforcement, or automatic conditional requests |
| Trailers | Chunked response trailers are exposed for blocking and async APIs; streaming chunked uploads can send declared request trailers | Application metadata trailers such as `X-Trace` are allowed; pseudo-header, connection-specific, routing, authentication/cookie, and framing trailer fields are rejected |
| Bounded h2c client | With `http2`, direct `socket2` h2c sends GET, HEAD, bodyless DELETE, OPTIONS, or TRACE, buffered POST, PUT, or PATCH requests, and opt-in RFC 8441 extended CONNECT request HEADERS via `http2_extended_connect`, opens at most one request stream, supports prior-knowledge with `emit_http2_prior_knowledge`, supports explicit HTTP/1.1 `Upgrade: h2c` negotiation with `emit_http2_upgrade`, advertises `SETTINGS_ENABLE_PUSH = 0`, advertises `SETTINGS_ENABLE_CONNECT_PROTOCOL = 1` only for the explicit extended CONNECT path, validates received `SETTINGS_ENABLE_PUSH` values as only `0` or `1`, honors initial peer `SETTINGS_MAX_CONCURRENT_STREAMS` by failing before request HEADERS when the peer allows zero streams, honors peer-advertised `SETTINGS_MAX_HEADER_LIST_SIZE` request metadata limits, accepts only legal `SETTINGS_MAX_FRAME_SIZE` values from 16,384 through 16,777,215 bytes, splits outbound HEADERS, DATA, and trailers to the active peer frame-size limit, rejects oversized inbound frames when a configured local frame-size limit is exceeded, bounds HPACK dynamic table use with `SETTINGS_HEADER_TABLE_SIZE`, strips HTTP/1.x connection-specific request fields before emission, rejects connection-specific peer response fields, suppresses HEAD response bodies, treats `RST_STREAM` on the active stream as a bounded reset/cancellation signal, acknowledges valid PING frames with matching opaque data, DATA bodies, trailers, HPACK static Huffman strings, bounded large header blocks, padded incoming frames, `GOAWAY` shutdown boundaries, PRIORITY metadata validation without scheduling, HTTP/2-allowed unknown/extension frame ignoring inside this bounded path, reserved stream-id high-bit normalization, and conservative DATA flow control | Ordinary `CONNECT`, header-configured `:protocol` metadata, non-h2c HTTP/1.1 `Upgrade` handoff requests, and proxies are rejected deterministically, and `PUSH_PROMISE`/server push is rejected instead of managed; bounded direct h2c only, with no public cancellation callback API, no dynamic policy API, no extension callback API, no full extension negotiation, TLS ALPN, external h2 integration, proxy tunneling to h2, proxy h2, tunnel handoff, connection pooling, persistent HTTP/2 session management, automatic retry, server push, full stream state machine, unbounded multiplex scheduling, general multiplexing, priority scheduling, request bodies or trailers for extended CONNECT, or request bodies for GET, HEAD, DELETE, OPTIONS, or TRACE |

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
priority metadata is rejected, and no priority scheduling is performed. Valid
PING frames are acknowledged with PING ACK frames that carry the same opaque
8-byte data.
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
