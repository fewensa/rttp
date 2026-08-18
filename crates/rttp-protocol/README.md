# rttp-protocol

Shared HTTP wire syntax and framing primitives for the rttp client and server crates.

This crate owns shared, bounded HTTP wire syntax and framing primitives. It
does not own application-level HTTP policy; that policy belongs to the client
and server crates.

This crate supports rttp's implementation; its public API is not a standalone
application-level HTTP interface.

Protocol helpers define and bound wire metadata for the client and server
crates. They do not add higher-level runtime policy such as caching,
authentication, retries, representation selection, or body transformation.

## Authentication-Info

`authentication_info` parses `#auth-param` lists from `Authentication-Info`
fields. Each field value is bounded to 64 KiB, the combined parameter count is
bounded to 256, and each parameter value is bounded to 64 KiB. Parameter names
are matched case-insensitively and must be unique across the combined field
set. Empty input, empty members, malformed syntax, and duplicate names are
rejected. This parser does not implement authentication policy.

## Cross-Origin-Embedder-Policy

`cross_origin_embedder_policy` parses a singleton `Cross-Origin-Embedder-Policy`
structured-field item. Each field value is bounded to 64 KiB. A second field is
rejected after every supplied field is bound-checked. The bare item must be
exactly one of the tokens `unsafe-none`, `require-corp`, or `credentialless`.
Well-formed parameters, including `report-to`, are accepted as syntax and
discarded; this parser does not retain reporting metadata or enforce embedder
policy. Case variants, lists, quoted values, unknown tokens, empty values, and
other unparsable input are errors. The parser never fails open to `unsafe-none`.

## Referer

`referer` parses a singleton HTTP `Referer` request field as one RFC 9110 URI
reference (`absolute-URI` / `partial-URI`). Each field value is bounded to
64 KiB. A second field is rejected after every supplied field is bound-checked.
Surrounding SP and HTAB are trimmed as optional whitespace. The parser
preserves the trimmed reference text and does not canonicalize scheme, host,
port, path, query, or userinfo. Fragments, ASCII controls, interior
whitespace, non-URI bytes, broken percent-encoding, empty values, and values
the structural URL parser cannot accept are errors. This is syntax validation
only: callers own trust, logging, CSRF, and `Referrer-Policy` decisions. This
module is distinct from `referrer_policy`, which parses response policy tokens
rather than URI references.

## Referrer-Policy

`referrer_policy` parses one or more `Referrer-Policy` field values into
recognized policy tokens. Each field value is bounded to 64 KiB, and the
cumulative member count across all supplied fields is bounded to 256 members,
counting recognized and unknown members alike. Members are split on commas with
SP and HTAB accepted only as optional whitespace around each member; empty
members and members containing forbidden ASCII control bytes are rejected.
Recognized tokens are parsed case-insensitively and retained in wire order,
including repeated tokens, while valid unknown tokens are ignored so future
policy names remain forward-compatible within the same validation and count
bounds. A present header set that yields no recognized token still fails as
invalid.
