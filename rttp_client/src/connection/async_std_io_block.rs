//use std::io::{Error, ErrorKind};
use async_std::prelude::*;
use std::borrow::BorrowMut;


#[derive(Debug)]
pub struct AsyncToBlockStream {
  async_stream: async_std::net::TcpStream,
}

impl AsyncToBlockStream {
  pub fn new(async_stream: async_std::net::TcpStream) -> Self {
    Self {
      async_stream
    }
  }
}

impl std::io::Read for AsyncToBlockStream {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
    async_std::task::block_on(async {
      self.async_stream.read(buf).await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
  }
}

impl std::io::Write for AsyncToBlockStream {
  fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
    async_std::task::block_on(async {
      self.async_stream.write(buf).await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
  }

  fn flush(&mut self) -> Result<(), std::io::Error> {
    async_std::task::block_on(async {
      self.async_stream.flush().await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    })
  }
}

// todo: impl blcok to async stream
//pub struct BlockToAsyncStream<'a> {
//  block_stream: Box<&'a mut dyn io::Read + io::Write>,
//}
//
//impl<'a> BlockToAsyncStream<'a> {
//  pub fn new(block_stream: &'a mut dyn io::Read + io::Write) -> BlockToAsyncStream<'a> {
//    Self {
//      block_stream: Box::new(block_stream)
//    }
//  }
//}
//
//impl<'a> async_std::io::Stream for BlockToAsyncStream<'a> {
//  fn poll_read(
//    self: async_std::pin::Pin<&mut Self>,
//    cx: &mut async_std::task::Context,
//    buf: &mut [u8],
//  ) -> async_std::task::Poll<async_std::io::Result<usize>> {
//
//  }
//
//  fn poll_read_vectored(
//    self: async_std::pin::Pin<&mut Self>,
//    cx: &mut async_std::task::Context<'_>,
//    bufs: &mut [async_std::io::IoSliceMut<'_>],
//  ) -> async_std::task::Poll<io::Result<usize>> {
//    async_std::task::Poll::new(&mut &*self).poll_read_vectored(cx, bufs)
//  }
//}
//
//impl<'a> async_std::io::Write for BlockToAsyncStream<'a> {
//  fn poll_write(
//    self: async_std::pin::Pin<&mut Self>,
//    cx: &mut async_std::task::Context,
//    buf: &[u8],
//  ) -> async_std::task::Poll<async_std::io::Result<usize>> {
//
//  }
//  fn poll_flush(self: async_std::pin::Pin<&mut Self>, cx: &mut async_std::task::Context)
//                -> async_std::task::Poll<async_std::io::Result<()>> {
//
//  }
//  fn poll_close(self: async_std::pin::Pin<&mut Self>, cx: &mut async_std::task::Context)
//                -> async_std::task::Poll<async_std::io::Result<()>> {
//
//  }
//}
