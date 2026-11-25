//! Stand-in for `std::os::unix::net::UnixStream` where there are no Unix sockets (Windows).
//! Uninhabited: `connect` always fails with `ErrorKind::Unsupported`, which the client maps
//! to `ClientError::Unsupported`, so no value of it ever exists. ADR 0010 explains why not
//! named pipes yet: there is no daemon on Windows to talk to.

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

pub enum UnixStream {}

impl UnixStream {
    pub fn connect(_path: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no Unix sockets on this OS",
        ))
    }

    pub fn set_read_timeout(&self, _t: Option<Duration>) -> io::Result<()> {
        match *self {}
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        match *self {}
    }
}

impl Read for UnixStream {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        match *self {}
    }
}

impl Write for UnixStream {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        match *self {}
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {}
    }
}
