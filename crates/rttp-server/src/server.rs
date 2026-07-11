//! Blocking HTTP server implementation used by the `rttp` compatibility facade.
//!
//! The public server API is re-exported here while protocol and model concerns live in
//! focused internal modules.

use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use socket2::{Domain, Protocol, Socket, Type};

mod connection;
mod handoff;
mod http1;
mod http2;
mod request;
mod response;

pub use connection::HttpServer;
pub use handoff::*;
pub use http2::Http2ServerPolicy;
pub use request::*;
pub use response::*;

// Protocol modules share implementation details without exposing them from the crate.
pub(crate) use http1::*;
pub(crate) use http2::*;

#[cfg(test)]
include!("server/server_tests.rs");
