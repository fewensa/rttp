# rttp-protocol

Shared bounded HTTP wire syntax and framing primitives for the rttp client and
server crates.

This crate owns protocol wire syntax and framing only, not application-level
HTTP policy. It supports rttp's implementation; its public API is not a
standalone application-level HTTP interface.

Protocol helpers define and bound wire metadata for the client and server
crates. They do not add higher-level runtime policy such as caching,
authentication, retries, representation selection, or body transformation.
