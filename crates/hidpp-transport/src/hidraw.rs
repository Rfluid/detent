//! `/dev/hidrawN` transport.
//!
//! - opened `O_RDWR | O_NONBLOCK | O_CLOEXEC`; one `read(2)` returns exactly one report
//! - [`HidrawTransport::recv`] waits with `poll(2)`; the data plane adds its wake `eventfd`
//!   to the same poll set through [`HidrawTransport::fd`]
//! - never allocates per frame: reads into a stack `[u8; 64]` and calls `Frame::parse`
//! - the kernel driver also talks HID++ on this device; frames with its swid are `Foreign`
//!   and are filtered by the caller (`classify`), not here

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use detent_hidpp_proto::{Frame, MAX_FRAME_LEN};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::Transport;

#[derive(Debug)]
pub struct HidrawTransport {
    file: File,
}

impl HidrawTransport {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc_flags::O_NONBLOCK)
            .open(path)?;
        Ok(Self { file })
    }

    /// The raw fd, for callers that poll it together with other fds.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }

    /// Read one report without blocking. `Ok(None)`: nothing pending, or the
    /// report wasn't HID++ (normal mouse input also arrives on hidraw).
    #[inline]
    pub fn try_read(&mut self) -> io::Result<Option<Frame>> {
        let mut buf = [0u8; MAX_FRAME_LEN];
        match self.file.read(&mut buf) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "hidraw node closed (device gone)",
            )),
            Ok(n) => Ok(Frame::parse(&buf[..n]).ok()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Wait until the fd is readable or `timeout` passes. Returns `true` if readable.
    fn wait_readable(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let mut fds = [PollFd::new(&self.file, PollFlags::IN)];
        let ts = timeout.map(|d| Timespec {
            tv_sec: d.as_secs() as _,
            tv_nsec: d.subsec_nanos() as _,
        });
        loop {
            match poll(&mut fds, ts.as_ref()) {
                Ok(0) => return Ok(false),
                Ok(_) => {
                    let ev = fds[0].revents();
                    if ev.intersects(PollFlags::ERR | PollFlags::HUP | PollFlags::NVAL) {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "hidraw node error/hangup (device gone)",
                        ));
                    }
                    return Ok(true);
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// `O_NONBLOCK` without pulling in `libc` just for one constant.
mod libc_flags {
    pub const O_NONBLOCK: i32 = rustix::fs::OFlags::NONBLOCK.bits() as i32;
}

impl Transport for HidrawTransport {
    fn send(&mut self, frame: &Frame) -> io::Result<()> {
        // hidraw writes are atomic per report; a non-blocking write can't partially succeed.
        loop {
            match self.file.write(frame.as_bytes()) {
                Ok(_) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }

    fn recv(&mut self, timeout: Option<Duration>) -> io::Result<Option<Frame>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            if let Some(frame) = self.try_read()? {
                return Ok(Some(frame));
            }
            let remaining = match deadline {
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return Ok(None);
                    }
                    Some(d - now)
                }
                None => None,
            };
            if !self.wait_readable(remaining)? {
                return Ok(None);
            }
        }
    }
}
