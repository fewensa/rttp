#![allow(clippy::module_inception)]

#[cfg(feature = "async")]
pub(crate) use self::async_connection::AsyncStreamingRequestBody;
#[cfg(feature = "async")]
pub use self::async_connection::*;
pub use self::block_connection::*;
pub(crate) use self::connection::StreamingRequestBody;
pub use self::connection_reader::{ConnectionReader, ResponseBodyReader, StreamingResponse};

#[cfg(feature = "async")]
mod async_connection;
mod block_connection;
mod connection;
mod connection_reader;
