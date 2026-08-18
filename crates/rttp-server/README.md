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
