//! In-memory transport for unit tests: script inbound frames, inspect outbound ones.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use detent_hidpp_proto::Frame;

use crate::Transport;

#[derive(Debug, Default)]
pub struct MockTransport {
    pub inbound: VecDeque<Frame>,
    pub sent: Vec<Frame>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_inbound(&mut self, frame: Frame) {
        self.inbound.push_back(frame);
    }
}

impl Transport for MockTransport {
    fn send(&mut self, frame: &Frame) -> io::Result<()> {
        self.sent.push(*frame);
        Ok(())
    }

    fn recv(&mut self, _timeout: Option<Duration>) -> io::Result<Option<Frame>> {
        Ok(self.inbound.pop_front())
    }
}
