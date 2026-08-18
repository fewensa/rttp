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
`sec_fetch_dest()`, and `sec_fetch_user()` to observe bounded typed
`Sec-Fetch-*` request metadata. Malformed values return a parser error while
`Request::header()` continues to expose the original raw field. RTTP does not
enforce browser security policy, block requests, validate origins, or infer
navigation policy from these fields.

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
