//! The byte pipe under a [`crate::Link`]: TCP or USB HID.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};

/// What the worker needs from a connection.
pub(crate) trait Transport: Send {
    /// Send one complete frame.
    fn send(&mut self, frame: &[u8]) -> io::Result<()>;
    /// Wait a few milliseconds for incoming bytes. `Ok(0)` means the
    /// connection is gone; `WouldBlock`/`TimedOut` means nothing arrived.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn close(&mut self);
}

/// The socket has a short read timeout (`Link::connect`).
impl Transport for TcpStream {
    fn send(&mut self, frame: &[u8]) -> io::Result<()> {
        self.write_all(frame)
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read(buf)
    }

    fn close(&mut self) {
        let _ = self.shutdown(Shutdown::Both);
    }
}
