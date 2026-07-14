# rttp-server

`rttp-server` provides the blocking HTTP server implementation re-exported by
the `rttp` compatibility facade.

## Request Cache-Control metadata

Handlers can call `Request::cache_control()` to obtain typed request cache
directives. The helper combines all case-insensitive `Cache-Control` header
fields, preserves the request for handler-defined error policy, and returns an
error for malformed values, values larger than 64 KiB, or more than 256
directives. It only parses metadata; it does not apply caching behavior.

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
